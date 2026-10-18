// wakuwaku-dsh-plugin
// Wakuwaku's plugin for DeepSeek Harness (DSH): what DSH's sessions do, told
// to her on her port. Written by Wakuwaku (Connect → DeepSeek Harness) into
// DSH's own folder, <DSH home>/wakuwaku/, beside a package.json of its own,
// and inserted by its path into DSH's home-level patch layer
// (<DSH home>/cordis.patch.yml), which every profile applies: the desktop
// app's and the command line's. A path, not a package name: nothing goes into
// a profile's node_modules, so no pnpm run of DSH's ever takes it away (a
// bridge package that was not there is what zhoupengjie/Wakuwaku#61 found).
// Remove there takes it all away again. It needs nothing but Node's own
// modules.
//
// It runs inside DSH's own process (its Host), as any of DSH's plugins: no
// program of hers is started, nothing waits on her. Each event is handed to
// her port over HTTP, one at a time and in order, after those before it;
// DSH's own work never waits for one (every listener returns at once, the
// waterfalls' next() handed back as it is), a post gives up after a moment,
// and while she is not there (no answer) nothing is sent for a few seconds.
// Every error is swallowed: DSH never sees one of ours.
//
// What it tells, in the words of Claude Code's hooks as DSH's own Claude Code
// hook bridge said them (so her reading of them, events_dsh.rs, stays as it
// was): SessionStart as a session's agent is made, UserPromptSubmit as the
// person's words enter a turn, PreToolUse and PostToolUse around each tool,
// Stop as a turn ends. What only a plugin sees goes with them: the turn's last
// words (last_assistant_message), the session's name as DSH gave it
// (session_title), whose a session is (origin "subagent", the session it
// works for: parent_session, top_session), and a turn that ended without
// finishing (stopped by the person, failed: stop_reason, StopFailure).
//
// As it is loaded it says so twice: to her port (WakuwakuLoaded), and in a
// small file beside itself (loaded-<profile>.json: this process, when, which
// version), which her Connect page reads to tell whether DSH really loaded
// it, she running or not; the file goes again as the plugin is let go.
//
// The row that loads it gives its config: { port }, her port.

import http from 'node:http'
import { unlinkSync } from 'node:fs'
import { writeFile } from 'node:fs/promises'
import { join } from 'node:path'
import { fileURLToPath } from 'node:url'

// This file's version: her Connect page compares it with the one loaded.
export const VERSION = 1
export const name = 'wakuwaku'

const DEFAULT_PORT = 47213
// How long one event may take to reach her.
const SEND_MS = 2000
// Not there: tried again after this long.
const AWAY_MS = 5000
// Events waiting at most; past that, new ones are dropped (she is slow or gone).
const QUEUE_MAX = 200
// Sessions remembered (their names, their last words) at most.
const KEPT = 200
// A tool's result as far as she reads it (whether it failed), and any text
// of its input: the start of it.
const TEXT_MAX = 2000

// A map kept small: the oldest go first.
function remember(map, key, value) {
  map.delete(key)
  map.set(key, value)
  while (map.size > KEPT) map.delete(map.keys().next().value)
}

// Content blocks' text, joined (what a hook payload carries).
function textOf(content, sep = '') {
  if (!Array.isArray(content)) return ''
  return content.filter(b => b && b.type === 'text' && typeof b.text === 'string').map(b => b.text).join(sep)
}

const clip = (text, max = TEXT_MAX) => (text.length > max ? text.slice(0, max) : text)

// A tool's input with any long text cut short (a file written whole is no
// business of hers), a few levels down.
function slim(value, depth = 0) {
  if (typeof value === 'string') return clip(value)
  if (value === null || typeof value !== 'object' || depth > 4) return value
  if (Array.isArray(value)) return value.slice(0, 100).map(v => slim(v, depth + 1))
  const out = {}
  for (const [k, v] of Object.entries(value)) out[k] = slim(v, depth + 1)
  return out
}

// The folder this file is in (its marker goes beside it).
function hereDir() {
  try {
    return fileURLToPath(new URL('.', import.meta.url))
  } catch {
    return ''
  }
}

// A profile's name as a file name's part.
const fileSafe = text => String(text || 'default').replace(/[^A-Za-z0-9._-]/g, '_').slice(0, 64)

// The plugin for a context and its config. options: { dir } where its marker
// goes (beside this file by default), for the tests.
export function make(ctx, config = {}, options = {}) {
  const port = Number.isInteger(Number(config?.port)) && Number(config.port) > 0 ? Number(config.port) : DEFAULT_PORT
  const path = '/hook?from=wakuwaku&agent=dsh'
  const titles = new Map()
  const replies = new Map()
  // Sessions whose turn was told ended (Stop), until the next one begins.
  const stopped = new Set()
  let queue = Promise.resolve()
  let waiting = 0
  let awayUntil = 0
  let closed = false

  // One event to her port; resolves when she answered, or gave none in time.
  function post(body) {
    return new Promise(done => {
      if (closed || Date.now() < awayUntil) return done()
      let data
      try {
        data = JSON.stringify(body)
      } catch {
        return done()
      }
      let req
      try {
        req = http.request(
          { host: '127.0.0.1', port, path, method: 'POST', agent: false, headers: { 'content-type': 'application/json', 'content-length': Buffer.byteLength(data) } },
          res => {
            res.on('error', () => done())
            res.on('end', () => done())
            res.resume()
          },
        )
      } catch {
        return done()
      }
      // Nothing of hers keeps DSH's process up as it ends.
      req.on('socket', socket => socket.unref?.())
      req.setTimeout(SEND_MS, () => req.destroy(new Error('timeout')))
      req.on('error', err => {
        // No one on her port: away for a while. Slow to answer: only this one lost.
        if (err?.message !== 'timeout') awayUntil = Date.now() + AWAY_MS
        done()
      })
      req.end(data)
    })
  }

  // An event for her, after those before it.
  function send(body) {
    if (closed || waiting >= QUEUE_MAX) return
    waiting++
    queue = queue
      .then(() => post(body))
      .catch(() => {})
      .then(() => {
        waiting--
      })
  }

  const service = key => {
    try {
      return ctx.get?.(key)
    } catch {
      return undefined
    }
  }

  // The session at the top of a subagent's lineage, as far as live sessions say.
  function topOf(header) {
    let top = header.parentSession || ''
    const sessions = service('sessions')
    for (let i = 0; top && sessions && i < 8; i++) {
      let up
      try {
        up = sessions.get(top)?.header
      } catch {
        break
      }
      if (!up || up.origin !== 'subagent' || !up.parentSession || up.parentSession === top) break
      top = up.parentSession
    }
    return top
  }

  // What every event carries: the session, its folder (the one it began in:
  // DSH keeps its log under it), whose it is, its name if DSH gave one (a
  // subagent's, made of its task, is no name for her: none).
  function base(session, event) {
    const header = session?.header || {}
    const id = header.id || session?.id || ''
    const out = { session_id: id, transcript_path: '', cwd: header.cwd || process.cwd(), hook_event_name: event, origin: header.origin || '', parent_session: header.parentSession || '' }
    if (header.origin === 'subagent') out.top_session = topOf(header)
    else if (titles.has(id)) out.session_title = titles.get(id)
    return out
  }

  const idOf = session => session?.header?.id || session?.id || ''

  function sessionStart({ agent, source }) {
    send({ ...base(agent?.session, 'SessionStart'), source: typeof source === 'string' ? source : 'startup' })
  }

  // The person's words entering a turn (or a subagent's task): the messages
  // entering a step; none for a step that only goes on.
  function promptSubmit({ agent, messages }) {
    if (!Array.isArray(messages) || messages.length === 0) return
    const theirs = messages.filter(m => m?.source?.kind === 'user')
    const words = theirs.length ? theirs : messages
    send({ ...base(agent?.session, 'UserPromptSubmit'), prompt: words.map(m => textOf(m?.content)).join(''), source: theirs.length ? 'user' : messages[0]?.source?.kind || '' })
  }

  function preTool(exec) {
    send({ ...base(exec?.agent?.session, 'PreToolUse'), tool_name: exec?.name || '', tool_input: slim(exec?.arguments ?? {}), tool_use_id: exec?.callId || '' })
  }

  function postTool(exec, result) {
    send({
      ...base(exec?.agent?.session, 'PostToolUse'),
      tool_name: exec?.name || '',
      tool_input: slim(exec?.arguments ?? {}),
      tool_use_id: exec?.callId || '',
      tool_response: clip(textOf(result?.content)),
      is_error: result?.isError === true,
    })
  }

  // A turn finishing: its last words with it.
  function stop({ agent }) {
    const session = agent?.session
    const id = idOf(session)
    stopped.add(id)
    send({ ...base(session, 'Stop'), stop_hook_active: false, last_assistant_message: replies.get(id) || '' })
  }

  // DSH's log as it is written: the turn's last words, the session's name,
  // and a turn that ended without finishing (no Stop for it: stopped by the
  // person, refused, failed).
  function logged(session, event) {
    switch (event?.type) {
      case 'turn/start': {
        const id = idOf(session)
        replies.delete(id)
        stopped.delete(id)
        return
      }
      case 'assistant/message': {
        const text = textOf(event.data?.message?.content, '\n').trim()
        if (text) remember(replies, idOf(session), text)
        return
      }
      case 'session/title': {
        const title = event.data?.title
        if (typeof title === 'string' && title.trim()) remember(titles, idOf(session), title.trim())
        return
      }
      case 'turn/end': {
        const id = idOf(session)
        if (stopped.delete(id)) return
        const reason = event.data?.reason || {}
        if (reason.kind === 'error') {
          const failure = reason.error || {}
          send({ ...base(session, 'StopFailure'), error_details: clip(String(failure.message || failure.code || 'error'), 300) })
        } else {
          send({ ...base(session, 'Stop'), stop_hook_active: false, stop_reason: String(reason.kind || ''), last_assistant_message: reason.kind === 'completed' ? replies.get(id) || '' : '' })
        }
        return
      }
    }
  }

  // Each of DSH's points, each listener first in line (it changes nothing)
  // and back at once.
  function listen() {
    const quiet = fn => (...args) => {
      try {
        fn(...args)
      } catch {}
    }
    ctx.on('agent/created', quiet(sessionStart))
    ctx.on('agent/turn-stopping', quiet(stop))
    ctx.on('session/event', quiet(logged))
    ctx.on('agent/pre-step', (arg, next) => (quiet(promptSubmit)(arg), next()), true)
    ctx.on('tools/pre-execute', (exec, next) => (quiet(preTool)(exec), next()), true)
    ctx.on('tools/post-execute', (exec, result, next) => (quiet(postTool)(exec, result), next()), true)
  }

  // Loaded: said to her, and in the marker beside this file; the marker goes
  // as the plugin is let go (the row taken out, DSH ending).
  function hello() {
    const profile = String(service('profileContext')?.name || '')
    const dir = options.dir ?? hereDir()
    const marker = dir ? join(dir, `loaded-${fileSafe(profile)}.json`) : ''
    if (marker) writeFile(marker, JSON.stringify({ pid: process.pid, at: Date.now(), version: VERSION, profile, port })).catch(() => {})
    send({ session_id: '', hook_event_name: 'WakuwakuLoaded', version: VERSION, pid: process.pid, profile })
    return () => {
      closed = true
      try {
        if (marker) unlinkSync(marker)
      } catch {}
    }
  }

  return { listen, hello, sessionStart, promptSubmit, preTool, postTool, stop, logged, idle: () => queue, close: () => (closed = true) }
}

export function apply(ctx, config) {
  try {
    const plugin = make(ctx, config)
    plugin.listen()
    const letGo = plugin.hello()
    ctx.effect?.(() => letGo, 'wakuwaku: let go')
  } catch {}
}
