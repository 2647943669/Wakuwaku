// The waku object of a plugin's main.js (docs/PLUGINS.md), in its frame: a
// hidden iframe of her island's page, sandboxed with scripts alone, at
// http://wakuplugin.localhost/<id>/__waku/main.html (addons.rs serve, which
// sends this file as __waku/sdk.js). This runs first: it tells the page
// holding the frame it is ready, is told the plugin's id, version, her
// language, theme and quiet, its settings and its main, sets up `waku`, and
// then loads the main. Everything else goes by messages to that page
// (src/pet/addons.js), which knows the plugin by this frame and asks for it:
// its widgets, the web, its storage, a nudge, the agents and the hardware
// where its manifest has them. Nothing here is trusted on her side: a
// plugin may do what it likes with this file's objects, and what it is
// allowed is decided over there.
;(() => {
  'use strict'
  const parent = window.parent
  if (!parent || parent === window) return

  // What the frame's Content-Security-Policy does not reach: WebRTC's
  // connections. Taken away before the plugin's code runs, for good.
  for (const name of ['RTCPeerConnection', 'webkitRTCPeerConnection', 'RTCDataChannel', 'RTCSessionDescription', 'RTCIceCandidate', 'RTCRtpSender', 'RTCRtpReceiver', 'RTCRtpTransceiver']) {
    try {
      Object.defineProperty(window, name, { value: undefined, writable: false, configurable: false })
    } catch {}
  }

  const EVENTS = ['settings', 'lang', 'theme', 'quiet', 'agents', 'hardware', 'view']
  const tell = (t, args) => parent.postMessage({ waku: 1, t, ...args }, '*')
  // Asked of the page: a promise of its answer (its value, or an Error
  // with the words of why not).
  const pending = new Map()
  let next = 1
  const ask = (t, args = {}) =>
    new Promise((resolve, reject) => {
      const n = next++
      pending.set(n, { resolve, reject })
      tell(t, { n, ...args })
    })
  // A value as JSON has it (what a message carries, checked over there).
  const plain = v => (v === undefined ? null : JSON.parse(JSON.stringify(v)))

  // --- What goes wrong, told ---------------------------------------------------------------
  const base = new URL('../', location.href).href
  const short = file => String(file || '').replace(base, '')
  function report(err, file, line) {
    const message = err && typeof err === 'object' ? `${err.name || 'Error'}: ${err.message}` : String(err)
    const where = file ? ` (${short(file)}${line ? `:${line}` : ''})` : ''
    tell('error', { line: `${message}${where}`.slice(0, 300) })
  }
  window.addEventListener('error', e => report(e.error || e.message, e.filename, e.lineno))
  window.addEventListener('unhandledrejection', e => report(e.reason))
  const call = (fn, ...args) => {
    try {
      const r = fn(...args)
      if (r && typeof r.then === 'function') r.then(undefined, report)
    } catch (err) {
      report(err)
    }
  }

  // --- Its state, as the page tells it ------------------------------------------------------
  let state = null
  const listeners = new Map(EVENTS.map(e => [e, new Set()]))
  const fire = (what, value) => {
    for (const fn of listeners.get(what)) call(fn, value)
  }

  // waku.every's timers: each at its interval, three times as long while
  // nobody is about; started over from now as that changes.
  const timers = new Set()
  function arm(t) {
    clearTimeout(t.handle)
    t.handle = setTimeout(
      () => {
        if (t.stopped) return
        arm(t)
        call(t.fn)
      },
      t.secs * 1000 * (state?.quiet ? 3 : 1),
    )
  }
  function every(seconds, fn) {
    if (typeof fn !== 'function') throw new TypeError('waku.every(seconds, fn)')
    const t = { secs: Math.max(1, Number(seconds) || 1), fn, handle: 0, stopped: false }
    timers.add(t)
    arm(t)
    return () => {
      t.stopped = true
      clearTimeout(t.handle)
      timers.delete(t)
    }
  }

  // An answer of waku.fetch, as a fetch's is read: its status, whether ok,
  // its headers (a few), and its text, read as text or JSON.
  function answerOf(r) {
    const text = String(r?.text ?? '')
    return Object.freeze({
      status: Number(r?.status) || 0,
      ok: r?.ok === true,
      headers: Object.freeze({ ...(r?.headers || {}) }),
      text: () => Promise.resolve(text),
      json: () => Promise.resolve().then(() => JSON.parse(text)),
    })
  }
  function headersOf(h) {
    const out = {}
    if (!h || typeof h !== 'object') return out
    const pairs = Array.isArray(h) ? h : typeof h.entries === 'function' && !(h.constructor === Object) ? [...h.entries()] : Object.entries(h)
    for (const [k, v] of pairs) out[String(k)] = String(v)
    return out
  }

  function words(args) {
    return args
      .map(a => {
        if (typeof a === 'string') return a
        if (a instanceof Error) return `${a.name}: ${a.message}`
        try {
          return JSON.stringify(a)
        } catch {
          return String(a)
        }
      })
      .join(' ')
  }

  const waku = Object.freeze({
    get id() {
      return state.id
    },
    get version() {
      return state.version
    },
    get lang() {
      return state.lang
    },
    get theme() {
      return state.theme
    },
    get quiet() {
      return state.quiet
    },
    get settings() {
      return state.settings
    },
    widget: Object.freeze({
      // set(data), or set(sub, data) for a second widget.
      set(sub, data) {
        if (typeof sub !== 'string') [sub, data] = ['', sub]
        return ask('widget', { sub, data: plain(data) })
      },
      remove(sub = '') {
        return ask('remove', { sub: String(sub) })
      },
    }),
    every,
    fetch(url, opts = {}) {
      const body = opts.body == null ? null : String(opts.body)
      return ask('fetch', { url: String(url), method: String(opts.method || 'GET'), headers: headersOf(opts.headers), body }).then(answerOf)
    },
    storage: Object.freeze({
      get: key => ask('storage', { op: 'get', key: String(key) }),
      set: (key, value) => ask('storage', { op: 'set', key: String(key), value: plain(value) }),
      remove: key => ask('storage', { op: 'remove', key: String(key) }),
      all: () => ask('storage', { op: 'all' }),
    }),
    agents: Object.freeze({ list: () => ask('agents') }),
    hardware: Object.freeze({ read: () => ask('hardware') }),
    notify: text => ask('notify', { text: String(text ?? '') }),
    on(what, fn) {
      if (!listeners.has(what) || typeof fn !== 'function') throw new TypeError(`waku.on(${EVENTS.join(' | ')}, fn)`)
      listeners.get(what).add(fn)
      if (what === 'agents' || what === 'hardware') tell('listen', { what })
      return () => listeners.get(what).delete(fn)
    },
    log: (...args) => tell('log', { line: words(args).slice(0, 500) }),
  })

  const frozen = v => Object.freeze(plain(v) ?? {})

  window.addEventListener('message', e => {
    if (e.source !== parent) return
    const m = e.data
    if (!m || typeof m !== 'object' || m.waku !== 1) return
    if (m.t === 'init' && !state) {
      state = { id: String(m.id), version: String(m.version), lang: m.lang === 'en' ? 'en' : 'zh', theme: m.theme === 'light' ? 'light' : 'dark', quiet: m.quiet === true, settings: frozen(m.settings) }
      Object.defineProperty(window, 'waku', { value: waku, enumerable: true })
      import(new URL(String(m.main), base).href).catch(err => report(err))
      return
    }
    if (m.t === 'reply') {
      const p = pending.get(m.n)
      if (!p) return
      pending.delete(m.n)
      if (m.ok) p.resolve(m.value)
      else p.reject(new Error(String(m.error)))
      return
    }
    if (m.t === 'ping') return tell('pong', { n: m.n })
    if (m.t !== 'event' || !state || !listeners.has(m.what)) return
    const v = m.value
    if (m.what === 'settings') fire('settings', (state.settings = frozen(v)))
    else if (m.what === 'lang') fire('lang', (state.lang = v === 'en' ? 'en' : 'zh'))
    else if (m.what === 'theme') fire('theme', (state.theme = v === 'light' ? 'light' : 'dark'))
    else if (m.what === 'quiet') {
      state.quiet = v === true
      for (const t of timers) arm(t)
      fire('quiet', state.quiet)
    } else fire(m.what, v)
  })

  tell('ready', {})
})()
