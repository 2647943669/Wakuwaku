// Third-party plugins' frames, in her island's page (addons.rs runs the
// rest; docs/PLUGINS.md is the contract). One hidden <iframe
// sandbox="allow-scripts"> for each plugin she says to run (addons_running,
// then addons:changed), at its own first page (__waku/main.html, whose SDK,
// src/addon/sdk.js, loads its main). A plugin is known here by its frame
// alone: a message counts as one plugin's only when it comes from that
// frame's window (event.source), whatever it says it is, and what it asks
// is asked of her in that plugin's name. Its settings, her language, her
// theme and whether anybody is about are told to it as they change; the
// agents' sessions (from what the page already hears) and the hardware's
// readings only to one whose manifest asks for them, and only once it
// listens. A widget's words go at most once a second, a few requests at
// once, a few log lines a minute. A frame that loads its page again (sent
// elsewhere and kept back by guard_frames, or reloaded) starts over; one
// that keeps doing it is left stopped until it changes.
//
// The host itself (createHost) is plain logic over what it is given, for
// the tests (test/addons.test.js); the page's part is boot().
;(function (root) {
  const ORIGIN = 'http://wakuplugin.localhost'
  // A widget's words sent at most once in so long; requests on the web at
  // once; log lines a minute; the agents told at most once in so long; the
  // hardware read every so often while one listens (three times as long
  // while nobody is about); a message's widget, as JSON, at most; starts
  // over in a minute before a frame is left stopped.
  const WIDGET_EVERY = 1000
  const FETCHES = 4
  const LOGS = 30
  const AGENTS_EVERY = 1000
  const HARDWARE_EVERY = 5000
  const DATA_MAX = 64 * 1024
  const RESTARTS = 3

  const pageOf = id => `${ORIGIN}/${id}/__waku/main.html`
  const json = v => {
    try {
      return JSON.stringify(v) ?? null
    } catch {
      return null
    }
  }
  const str = (v, max = 4096) => String(v ?? '').slice(0, max)

  // The agents' sessions as a plugin sees them (waku.agents): each one's id,
  // agent, project, mood, since, and the line beside it; no more.
  const MOODS = ['idle', 'working', 'waiting', 'done', 'review', 'error']
  function agentsOf(list) {
    return (Array.isArray(list) ? list : [])
      .filter(s => s && typeof s === 'object' && typeof s.id === 'string' && s.id)
      .map(s => ({
        id: str(s.id, 80),
        agent: str(s.agent || 'claude', 20),
        project: str(s.project, 80),
        mood: MOODS.includes(s.mood) ? s.mood : 'idle',
        since: Number.isFinite(s.since) ? s.since : null,
        detail: typeof s.detail === 'string' ? s.detail.slice(0, 80) : '',
      }))
  }

  // deps: invoke(cmd, args) a promise; make(id, src) → { el, win } a frame;
  // drop(rec) takes one away; post(win, message); log(line); now(); and
  // the timers.
  function createHost({ invoke, make, drop, post, log = () => {}, now = () => Date.now(), setTimer = setTimeout, clearTimer = clearTimeout }) {
    const recs = new Map()
    const bySource = new Map()
    // Kept stopped (loaded again too often), by id: its rev then.
    const held = new Map()
    const env = { lang: 'zh', theme: 'dark', quiet: false }
    let known = false
    let pending = null
    let agents = []
    let agentsJson = '[]'
    let agentsAt = -Infinity
    let agentsTimer = 0
    let hardwareTimer = 0

    const tell = (rec, what, value) => post(rec.win, { waku: 1, t: 'event', what, value })
    const reply = (rec, n, ok, value) => post(rec.win, ok ? { waku: 1, t: 'reply', n, ok: true, value } : { waku: 1, t: 'reply', n, ok: false, error: String(value?.message ?? value) })
    const refusal = r => (r && typeof r === 'object' && r.error ? String(r.error) : null)
    // Asked of her in its name: { error } a refusal, else its value (pick).
    function ask(rec, n, cmd, args, pick = r => r ?? null) {
      return Promise.resolve()
        .then(() => invoke(cmd, { id: rec.id, ...args }))
        .then(
          r => (refusal(r) ? reply(rec, n, false, refusal(r)) : reply(rec, n, true, pick(r))),
          e => reply(rec, n, false, e),
        )
    }

    function start(item) {
      const { el, win } = make(item.id, pageOf(item.id))
      const rec = { id: item.id, item, el, win, ready: false, loads: 0, restarts: [], fetches: 0, logs: [], listens: new Set(), widgets: new Map(), lastPong: 0 }
      recs.set(item.id, rec)
      bySource.set(win, rec)
      return rec
    }
    function stop(rec) {
      recs.delete(rec.id)
      bySource.delete(rec.win)
      for (const w of rec.widgets.values()) clearTimer(w.timer)
      drop(rec)
    }

    // The list she sent: those not in it stopped, new ones started, one
    // changed (another version, its manifest edited, other permissions)
    // started over, one whose settings changed told them.
    function apply(list) {
      const items = (Array.isArray(list) ? list : []).filter(i => i && typeof i.id === 'string' && /^[a-z][a-z0-9-]{1,39}$/.test(i.id))
      if (!known) {
        pending = items
        return
      }
      const ids = new Set(items.map(i => i.id))
      for (const rec of [...recs.values()]) if (!ids.has(rec.id)) stop(rec)
      for (const [id, rev] of [...held]) if (!ids.has(id) || items.find(i => i.id === id).rev !== rev) held.delete(id)
      for (const item of items) {
        if (held.has(item.id)) continue
        const rec = recs.get(item.id)
        if (!rec) {
          start(item)
          continue
        }
        if (rec.item.rev !== item.rev || rec.item.main !== item.main || json(rec.item.perms) !== json(item.perms)) {
          stop(rec)
          start(item)
          continue
        }
        const changed = json(rec.item.settings) !== json(item.settings)
        rec.item = item
        if (changed && rec.ready) tell(rec, 'settings', item.settings)
      }
      keepHardware()
    }

    // Her language, theme, quiet as they are now: those changed told.
    function setEnv(patch = {}) {
      const changed = []
      for (const key of ['lang', 'theme', 'quiet']) {
        if (!(key in patch) || patch[key] === undefined) continue
        const v = key === 'quiet' ? patch.quiet === true : key === 'lang' ? (patch.lang === 'en' ? 'en' : 'zh') : patch.theme === 'light' ? 'light' : 'dark'
        if (env[key] !== v) {
          env[key] = v
          changed.push(key)
        }
      }
      for (const rec of recs.values()) if (rec.ready) for (const key of changed) tell(rec, key, env[key])
      if (changed.includes('quiet')) keepHardware(true)
      // Her language known: the frames may start.
      if (!known && 'lang' in patch) {
        known = true
        if (pending) apply(pending)
        pending = null
      }
    }

    // The agents' sessions as the page heard them: told to those that may
    // and listen, when they changed, once a second at most.
    function setAgents(list) {
      const next = agentsOf(list)
      const text = json(next)
      if (text === agentsJson) return
      agents = next
      agentsJson = text
      if (agentsTimer) return
      const wait = agentsAt + AGENTS_EVERY - now()
      const go = () => {
        agentsTimer = 0
        agentsAt = now()
        for (const rec of recs.values()) if (rec.ready && rec.item.perms?.agents && rec.listens.has('agents')) tell(rec, 'agents', agents)
      }
      if (wait <= 0) go()
      else agentsTimer = setTimer(go, wait)
    }

    // The hardware read for those that may and listen, while any does.
    function keepHardware(again = false) {
      const listening = [...recs.values()].filter(r => r.ready && r.item.perms?.hardware && r.listens.has('hardware'))
      if (again || !listening.length) {
        clearTimer(hardwareTimer)
        hardwareTimer = 0
      }
      if (!listening.length || hardwareTimer) return
      hardwareTimer = setTimer(
        () => {
          hardwareTimer = 0
          const now = [...recs.values()].filter(r => r.ready && r.item.perms?.hardware && r.listens.has('hardware'))
          if (!now.length) return
          Promise.resolve()
            .then(() => invoke('hardware', { id: now[0].id }))
            .then(r => {
              if (!refusal(r)) for (const rec of now) if (recs.get(rec.id) === rec) tell(rec, 'hardware', r)
            })
            .catch(() => {})
            .finally(() => keepHardware())
        },
        HARDWARE_EVERY * (env.quiet ? 3 : 1),
      )
    }

    function logLine(rec, line) {
      const t = now()
      rec.logs = rec.logs.filter(at => t - at < 60_000)
      if (rec.logs.length >= LOGS) return false
      rec.logs.push(t)
      log(`plugin ${rec.id}: ${str(line, 500)}`)
      return true
    }

    // A widget's words: sent at once, or once its second is up (the last
    // words then); each asking answered as that send is.
    function widget(rec, m) {
      const sub = typeof m.sub === 'string' ? m.sub : ''
      const text = json(m.data)
      if (text === null || text.length > DATA_MAX) return reply(rec, m.n, false, 'a widget is JSON, up to 64 KB')
      let w = rec.widgets.get(sub)
      if (!w) rec.widgets.set(sub, (w = { at: -Infinity, timer: 0, data: null, waiting: [] }))
      w.data = JSON.parse(text)
      w.waiting.push(m.n)
      if (w.timer) return
      const go = () => {
        w.timer = 0
        w.at = now()
        const [data, waiting] = [w.data, w.waiting]
        w.waiting = []
        Promise.resolve()
          .then(() => invoke('widget', { id: rec.id, sub: sub || null, data }))
          .then(
            r => waiting.forEach(n => (refusal(r) ? reply(rec, n, false, refusal(r)) : reply(rec, n, true, null))),
            e => waiting.forEach(n => reply(rec, n, false, e)),
          )
      }
      const wait = w.at + WIDGET_EVERY - now()
      if (wait <= 0) go()
      else w.timer = setTimer(go, wait)
    }

    function handle(rec, m) {
      const perms = rec.item.perms || {}
      switch (m.t) {
        case 'ready':
          rec.ready = true
          post(rec.win, { waku: 1, t: 'init', id: rec.id, version: rec.item.version, lang: env.lang, theme: env.theme, quiet: env.quiet, settings: rec.item.settings || {}, main: rec.item.main })
          return
        case 'widget':
          return widget(rec, m)
        case 'remove':
          return ask(rec, m.n, 'remove', { sub: typeof m.sub === 'string' && m.sub ? m.sub : null }, () => null)
        case 'fetch': {
          if (rec.fetches >= FETCHES) return reply(rec, m.n, false, `${FETCHES} requests at once at most`)
          rec.fetches++
          const headers = {}
          if (m.headers && typeof m.headers === 'object') for (const [k, v] of Object.entries(m.headers).slice(0, 32)) headers[str(k, 64)] = str(v)
          const body = m.body == null ? null : String(m.body)
          if (body && body.length > 1 << 20) {
            rec.fetches--
            return reply(rec, m.n, false, 'a body over 1 MB')
          }
          return ask(rec, m.n, 'fetch', { url: str(m.url, 8192), method: str(m.method, 16), headers, body }).finally(() => rec.fetches--)
        }
        case 'storage': {
          const op = ['get', 'set', 'remove', 'all'].includes(m.op) ? m.op : 'get'
          const value = op === 'set' ? (json(m.value) === null ? null : m.value) : null
          return ask(rec, m.n, 'storage', { op, key: m.key == null ? null : str(m.key, 200), value }, r => r?.value ?? null)
        }
        case 'agents':
          return perms.agents ? reply(rec, m.n, true, agents) : reply(rec, m.n, false, 'no agents permission')
        case 'hardware':
          return perms.hardware ? ask(rec, m.n, 'hardware', {}) : reply(rec, m.n, false, 'no hardware permission')
        case 'notify':
          return perms.notify ? ask(rec, m.n, 'notify', { text: str(m.text, 200) }, () => null) : reply(rec, m.n, false, 'no notify permission')
        case 'listen':
          if ((m.what === 'agents' && perms.agents) || (m.what === 'hardware' && perms.hardware)) {
            rec.listens.add(m.what)
            if (m.what === 'agents') tell(rec, 'agents', agents)
            keepHardware()
          }
          return
        case 'log':
          logLine(rec, m.line)
          return
        case 'error':
          if (logLine(rec, `error: ${str(m.line, 300)}`)) Promise.resolve(invoke('said', { id: rec.id, line: str(m.line, 300) })).catch(() => {})
          return
        case 'pong':
          rec.lastPong = now()
          return
      }
    }

    // A message from a frame: the plugin whose frame sent it, or nobody's.
    function receive(source, data) {
      const rec = bySource.get(source)
      if (!rec || !data || typeof data !== 'object' || data.waku !== 1 || typeof data.t !== 'string') return false
      handle(rec, data)
      return true
    }

    // A frame's page loaded: the first time, as it should; again (sent
    // elsewhere and kept back, or reloaded), it starts over, and one that
    // keeps doing so is left stopped until she sends it changed.
    function loaded(id, el) {
      const rec = recs.get(id)
      if (!rec || rec.el !== el) return
      rec.loads++
      if (rec.loads === 1) return
      const t = now()
      const restarts = [...rec.restarts.filter(at => t - at < 60_000), t]
      stop(rec)
      if (restarts.length > RESTARTS) {
        held.set(id, rec.item.rev)
        log(`plugin ${id}: loaded its page again ${restarts.length} times in a minute: stopped`)
        Promise.resolve(invoke('said', { id, line: 'loaded its page again and again: stopped until it changes' })).catch(() => {})
        return
      }
      log(`plugin ${id}: its page loaded again: started over`)
      start(rec.item).restarts = restarts
    }

    // Phase 2's watchdog will ping each frame here (the SDK answers pong).
    function ping(id, n = now()) {
      const rec = recs.get(id)
      if (rec?.ready) post(rec.win, { waku: 1, t: 'ping', n })
      return !!rec
    }

    return { apply, setEnv, setAgents, receive, loaded, ping, recs, env, held, get agents() { return agents } }
  }

  // --- In her island's page -------------------------------------------------------------
  function boot() {
    const box = document.createElement('div')
    box.id = 'addon-frames'
    box.hidden = true
    document.body.appendChild(box)
    let payload = null
    let look = null
    // Her theme as her island shows it: Windows' mode, from the taskbar's
    // look there, else from pet:update at the top of the screen.
    const themeNow = () => (payload?.config?.display === 'taskbar' ? (look?.mode === 'light' ? 'light' : 'dark') : payload?.winLight === true ? 'light' : 'dark')
    const host = createHost({
      invoke: (cmd, args) => window.pet.addons.call(cmd, args),
      make: (id, src) => {
        const el = document.createElement('iframe')
        el.setAttribute('sandbox', 'allow-scripts')
        el.setAttribute('referrerpolicy', 'no-referrer')
        el.setAttribute('aria-hidden', 'true')
        el.tabIndex = -1
        el.addEventListener('load', () => host.loaded(id, el))
        el.src = src
        box.appendChild(el)
        return { el, win: el.contentWindow }
      },
      drop: rec => rec.el.remove(),
      post: (win, message) => {
        try {
          win.postMessage(message, '*')
        } catch {}
      },
      log: line => console.log(line),
    })
    window.addEventListener('message', e => host.receive(e.source, e.data))
    window.pet.onAddons(list => host.apply(list))
    window.pet.onUpdate(p => {
      payload = p
      host.setEnv({ lang: p?.lang, quiet: p?.quiet === true, theme: themeNow() })
      host.setAgents(p?.list)
    })
    window.pet.onQuiet(q => host.setEnv({ quiet: q?.on === true }))
    window.pet.onLook(l => {
      look = l
      host.setEnv({ theme: themeNow() })
    })
    window.pet.addons.running().then(list => host.apply(list))
    api.host = host
  }

  const api = { createHost, agentsOf, pageOf, ORIGIN }
  if (typeof module !== 'undefined' && module.exports) module.exports = api
  else {
    root.Addons = api
    if (new URLSearchParams(location.search).get('role') === 'island' && root.pet?.addons) boot()
  }
})(this)
