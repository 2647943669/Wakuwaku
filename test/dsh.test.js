// Her plugin for DeepSeek Harness (integrations/dsh/wakuwaku.js): which of
// DSH's points it listens on and how (first in line, back at once), what it
// tells her and in which words (Claude Code's hooks', as DSH's own bridge
// said them), what only it sees (the last words, the name, whose a session
// is, a turn that ended unfinished), its word on loading, and that nothing of
// hers ever reaches DSH: no wait, no error. Against a stand-in for her port
// and a stand-in for DSH's context (never DSH itself).
const assert = require('node:assert/strict')
const fs = require('node:fs')
const http = require('node:http')
const os = require('node:os')
const path = require('node:path')
const { test } = require('node:test')

const SOURCE = fs.readFileSync(path.join(__dirname, '..', 'integrations', 'dsh', 'wakuwaku.js'), 'utf8')
const load = () => import('data:text/javascript,' + encodeURIComponent(SOURCE))

// Her port: what came, in order.
async function fakeHer() {
  const got = []
  const server = http.createServer((req, res) => {
    let body = ''
    req.on('data', chunk => (body += chunk))
    req.on('end', () => {
      got.push({ url: req.url, event: JSON.parse(body) })
      res.setHeader('content-type', 'application/json')
      res.end('{}')
    })
  })
  await new Promise(done => server.listen(0, '127.0.0.1', done))
  return { got, port: server.address().port, close: () => new Promise(done => server.close(done)) }
}

// DSH's side: the listeners a plugin registers (and whether first in line),
// its effects, its services.
function fakeDsh(services = {}) {
  const on = {}
  const effects = []
  return {
    on,
    effects,
    ctx: {
      on: (name, fn, prepend) => (on[name] = { fn, prepend: !!prepend }),
      effect: (fn, label) => effects.push({ dispose: fn(), label }),
      get: key => services[key],
    },
  }
}

const WORKSPACE = 'C:\\Users\\waku\\Documents\\deepseek-harness\\default-workspace'
const session = (id, more = {}) => ({ id, header: { type: 'session', version: 4, id, cwd: WORKSPACE, createdAt: 1, isSeeded: false, delegationDepth: 0, ...more } })
const agentOf = s => ({ session: s })
const ev = (type, data) => ({ type, seq: 0, time: 0, data })
// What a waterfall's next() gives back, to see it handed back as it is.
const NEXT = Object.freeze({ kind: 'enter', marker: true })

test('a turn as DSH runs it, told in the words of its Claude Code bridge', async () => {
  const her = await fakeHer()
  const { make } = await load()
  const dsh = fakeDsh()
  const plugin = make(dsh.ctx, { port: her.port }, { dir: '' })
  plugin.listen()
  // Every point, the waterfalls first in line; a waterfall's next() back as it is, at once.
  assert.deepEqual(Object.keys(dsh.on).sort(), ['agent/created', 'agent/pre-step', 'agent/turn-stopping', 'session/event', 'tools/post-execute', 'tools/pre-execute'])
  for (const name of ['agent/pre-step', 'tools/pre-execute', 'tools/post-execute']) assert.equal(dsh.on[name].prepend, true, name)
  const s = session('session-6aa5a96d-33fc')
  const agent = agentOf(s)
  const log = e => dsh.on['session/event'].fn(s, e)

  assert.equal(dsh.on['agent/created'].fn({ agent, source: 'startup' }), undefined)
  log(ev('turn/start', { turn: 1 }))
  const user = { role: 'user', content: [{ type: 'text', text: 'Read note.txt, then create copy.txt' }], source: { kind: 'user' } }
  assert.equal(dsh.on['agent/pre-step'].fn({ agent, messages: [user], turn: 1 }, () => NEXT), NEXT)
  // A step that only goes on: nothing told.
  assert.equal(dsh.on['agent/pre-step'].fn({ agent, messages: [], turn: 1 }, () => NEXT), NEXT)
  log(ev('session/title', { title: 'Use the ask_user_question tool to', source: { kind: 'fallback' } }))
  log(ev('session/title', { title: 'Read and copy a note', source: { kind: 'provider' } }))
  const write = { name: 'write', arguments: { file_path: 'C:\\w\\copy.txt', content: 'x'.repeat(100000) }, callId: 'call_00_CKTB', agent }
  assert.equal(dsh.on['tools/pre-execute'].fn(write, () => NEXT), NEXT)
  assert.equal(dsh.on['tools/post-execute'].fn(write, { content: [{ type: 'text', text: '<path>C:\\w\\copy.txt</path>' }], isError: false }, () => NEXT), NEXT)
  log(ev('assistant/message', { turn: 1, step: 1, message: { role: 'assistant', content: [{ type: 'text', text: "I'll read note.txt first." }, { type: 'tool-call', id: 'c' }] } }))
  log(ev('assistant/message', { turn: 1, step: 2, message: { role: 'assistant', content: [{ type: 'text', text: 'Done — copy.txt' }, { type: 'text', text: 'has `hi`.' }] } }))
  assert.equal(dsh.on['agent/turn-stopping'].fn({ agent, turn: 1 }), undefined)
  // Its turn/end after a Stop told: nothing more.
  log(ev('turn/end', { turn: 1, reason: { kind: 'completed' } }))
  await plugin.idle()
  await her.close()

  assert.ok(her.got.every(g => g.url === '/hook?from=wakuwaku&agent=dsh'))
  const told = her.got.map(g => g.event)
  assert.deepEqual(told.map(e => e.hook_event_name), ['SessionStart', 'UserPromptSubmit', 'PreToolUse', 'PostToolUse', 'Stop'])
  const base = { session_id: 'session-6aa5a96d-33fc', transcript_path: '', cwd: WORKSPACE, origin: '', parent_session: '' }
  assert.deepEqual(told[0], { ...base, hook_event_name: 'SessionStart', source: 'startup' })
  assert.deepEqual(told[1], { ...base, hook_event_name: 'UserPromptSubmit', prompt: 'Read note.txt, then create copy.txt', source: 'user' })
  // The name as DSH gave it last; a file written whole cut short.
  assert.equal(told[2].session_title, 'Read and copy a note')
  assert.deepEqual([told[2].tool_name, told[2].tool_use_id, told[2].tool_input.file_path, told[2].tool_input.content.length], ['write', 'call_00_CKTB', 'C:\\w\\copy.txt', 2000])
  assert.deepEqual([told[3].tool_response, told[3].is_error], ['<path>C:\\w\\copy.txt</path>', false])
  // The turn's last words, its text parts joined.
  assert.deepEqual([told[4].stop_hook_active, told[4].last_assistant_message], [false, 'Done — copy.txt\nhas `hi`.'])
})

test('a turn that ends unfinished, and a subagent', async () => {
  const her = await fakeHer()
  const { make } = await load()
  const parent = session('session-1')
  const mid = session('a1-mid', { origin: 'subagent', parentSession: 'session-1', delegationDepth: 1 })
  const child = session('b2-child', { origin: 'subagent', parentSession: 'a1-mid', delegationDepth: 2 })
  const live = new Map([parent, mid, child].map(s => [s.id, s]))
  const dsh = fakeDsh({ sessions: { get: id => live.get(id) } })
  const plugin = make(dsh.ctx, { port: her.port }, { dir: '' })
  plugin.listen()
  const log = (s, e) => dsh.on['session/event'].fn(s, e)
  // Stopped by the person (no turn-stopping): Stop, its reason, no words.
  log(parent, ev('turn/start', { turn: 1 }))
  log(parent, ev('assistant/message', { message: { content: [{ type: 'text', text: 'Half way' }] } }))
  log(parent, ev('turn/end', { turn: 1, reason: { kind: 'aborted', reason: { kind: 'user' } } }))
  // A turn that failed: StopFailure, why.
  log(parent, ev('turn/start', { turn: 2 }))
  log(parent, ev('turn/end', { turn: 2, reason: { kind: 'error', error: { message: '401 Unauthorized', code: 'AUTH' } } }))
  // A subagent's tool: whose, up its lineage as far as live sessions go; its
  // own name (made of its task) not sent.
  log(child, ev('session/title', { title: 'List all files in the' }))
  const exec = { name: 'grep', arguments: { pattern: 'TODO' }, callId: 'c9', agent: agentOf(child) }
  dsh.on['tools/pre-execute'].fn(exec, () => NEXT)
  // Its parent not live: the parent itself.
  live.delete('a1-mid')
  dsh.on['tools/pre-execute'].fn(exec, () => NEXT)
  await plugin.idle()
  await her.close()
  const told = her.got.map(g => g.event)
  assert.deepEqual(
    told.map(e => [e.hook_event_name, e.stop_reason, e.last_assistant_message, e.error_details]),
    [
      ['Stop', 'aborted', '', undefined],
      ['StopFailure', undefined, undefined, '401 Unauthorized'],
      ['PreToolUse', undefined, undefined, undefined],
      ['PreToolUse', undefined, undefined, undefined],
    ],
  )
  assert.deepEqual([told[2].origin, told[2].parent_session, told[2].top_session], ['subagent', 'a1-mid', 'session-1'])
  assert.equal('session_title' in told[2], false)
  assert.equal(told[3].top_session, 'a1-mid')
  assert.equal(told[0].origin, '')
  assert.equal('top_session' in told[0], false)
})

test('its word as DSH loads it, and as it is let go', async () => {
  const her = await fakeHer()
  const { make, apply, VERSION } = await load()
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'wakuwaku-dsh-'))
  const dsh = fakeDsh({ profileContext: { name: 'desktop' } })
  const plugin = make(dsh.ctx, { port: her.port }, { dir, sha: '0123456789abcdef' })
  const letGo = plugin.hello()
  await plugin.idle()
  const marker = path.join(dir, 'loaded-desktop.json')
  // Written in the background: wait for it a moment.
  for (let i = 0; i < 50 && !fs.existsSync(marker); i++) await new Promise(done => setTimeout(done, 10))
  const word = JSON.parse(fs.readFileSync(marker, 'utf8'))
  assert.deepEqual([word.pid, word.version, word.sha, word.profile, word.port], [process.pid, VERSION, '0123456789abcdef', 'desktop', her.port])
  assert.ok(Math.abs(Date.now() - word.at) < 5000)
  assert.deepEqual(her.got.map(g => g.event), [{ session_id: '', hook_event_name: 'WakuwakuLoaded', version: VERSION, pid: process.pid, profile: 'desktop' }])
  letGo()
  assert.equal(fs.existsSync(marker), false)
  // Let go: nothing more told.
  plugin.sessionStart({ agent: agentOf(session('s')), source: 'startup' })
  await plugin.idle()
  assert.equal(her.got.length, 1)
  // apply, as DSH calls it: its listeners and its letting go, as an effect.
  const real = fakeDsh({ profileContext: { name: 'web' } })
  apply(real.ctx, { port: her.port })
  assert.equal(Object.keys(real.on).length, 6)
  assert.equal(real.effects.length, 1)
  real.effects[0].dispose()
  await her.close()
  fs.rmSync(dir, { recursive: true, force: true })
})

test('nothing of hers reaches DSH: she not there, odd input, errors', async () => {
  const { make, apply } = await load()
  // A port no one listens on.
  const closed = await fakeHer()
  await closed.close()
  const dsh = fakeDsh()
  const plugin = make(dsh.ctx, { port: closed.port }, { dir: '' })
  plugin.listen()
  const started = Date.now()
  for (let i = 0; i < 20; i++) dsh.on['agent/created'].fn({ agent: agentOf(session(`s${i}`)), source: 'startup' })
  // Odd input: no agent, no session, no content; a session whose header throws.
  assert.equal(dsh.on['tools/pre-execute'].fn(undefined, () => NEXT), NEXT)
  assert.equal(dsh.on['tools/post-execute'].fn({ name: 'x' }, null, () => NEXT), NEXT)
  assert.equal(dsh.on['agent/pre-step'].fn({ messages: [{}] }, () => NEXT), NEXT)
  const bad = { get header() { throw new Error('gone') } }
  assert.doesNotThrow(() => dsh.on['session/event'].fn(bad, ev('turn/end', {})))
  assert.doesNotThrow(() => dsh.on['session/event'].fn(null, null))
  assert.doesNotThrow(() => dsh.on['agent/turn-stopping'].fn({}))
  await plugin.idle()
  // Away after the first refusal: the rest not even tried.
  assert.ok(Date.now() - started < 1500, `${Date.now() - started} ms`)
  // A context that throws on everything: apply still returns.
  assert.doesNotThrow(() => apply({ on() { throw new Error('no') }, effect() { throw new Error('no') }, get() { throw new Error('no') } }, {}))
  // Its config's port, else her own default.
  const other = fakeDsh()
  assert.doesNotThrow(() => make(other.ctx, { port: 'x' }, { dir: '' }).listen())
})
