// A stand-in for Tauri (window.__TAURI__, __TAURI_INTERNALS__) that the real
// pages take as Tauri's: bridge.js (the island's page, her window), desk.js
// and bar.js. Loaded before the page's own scripts (boot.js), so the pages
// run in a plain browser as they run in her windows.
//
// What the Rust side would say comes from a world kept here (Fake.world):
// her home, the strip's edge and material, Windows' mode and accent, the
// sessions in their moods, the prompts waiting, the widgets (their sizes,
// where each is put), the monitor's readings, the desktop's layout, the
// settings. A scene changes it (Fake.set) and sends it to the page
// (Fake.push), or emits any event itself (Fake.emit). What the page asks
// (invoke) is answered from the world, or by a handler a scene sets
// (Fake.handle); every call is kept in Fake.calls.
//
// The clock: Date starts at a fixed moment (Fake.T0, 14:42 on a Saturday)
// and runs with the page's own clock, so the taskbar's time, the sessions'
// clocks and her frames are the same each run. ?clock=real keeps the real one.
;(function () {
  const HERE = new URL('.', document.currentScript.src).href
  const ROOT = new URL('../../', HERE).href
  const Q = new URLSearchParams(location.search)
  const role = Q.get('role') || 'island'
  // Each window's label, as main names them (bridge.js listens for its own).
  const LABELS = { island: 'island', pet: 'pet', desk: 'desk-1', bar: 'bar-1', barpop: 'barpop' }
  const label = LABELS[role] || role

  // --- The clock ------------------------------------------------------------------------
  // 14:42 on 17 October 2026, in this computer's time zone.
  const T0 = new Date(2026, 9, 17, 14, 42, 0).getTime()
  if (Q.get('clock') !== 'real') {
    const Real = Date
    const start = performance.now()
    const nowMs = () => T0 + Math.floor(performance.now() - start)
    class FakeDate extends Real {
      constructor(...args) {
        super(...(args.length ? args : [nowMs()]))
      }
      static now() {
        return nowMs()
      }
    }
    window.Date = FakeDate
  }

  // --- The IPC --------------------------------------------------------------------------
  const listeners = new Map()
  const handlers = new Map()
  const calls = []
  const errors = []
  const report = err => {
    errors.push(String(err?.stack || err))
    console.error(err)
  }

  function listen(channel, fn) {
    if (!listeners.has(channel)) listeners.set(channel, new Set())
    listeners.get(channel).add(fn)
    return Promise.resolve(() => listeners.get(channel)?.delete(fn))
  }

  // An event to the page, as Tauri delivers it ({ event, id, payload }).
  function emit(channel, payload) {
    for (const fn of [...(listeners.get(channel) || [])]) {
      try {
        fn({ event: channel, id: 0, payload })
      } catch (err) {
        report(err)
      }
    }
  }

  function invoke(cmd, args = {}) {
    calls.push({ cmd, args })
    const answer = handlers.get(cmd) || ANSWERS[cmd]
    try {
      return Promise.resolve(answer ? answer(args) : null)
    } catch (err) {
      return Promise.reject(err)
    }
  }

  // Told to the page holding this one (shot.html), if any: Windows' acrylic
  // under the island (island_plate) is drawn there, behind the page.
  const tellOuter = message => {
    if (window.parent !== window) window.parent.postMessage({ wk: true, ...message }, '*')
  }

  window.__TAURI_INTERNALS__ = { metadata: { currentWebview: { label }, currentWindow: { label } } }
  window.__TAURI__ = {
    core: { invoke },
    event: { listen, emit: () => Promise.resolve() },
    webviewWindow: { getCurrentWebviewWindow: () => ({ label }) },
  }

  // --- The world ------------------------------------------------------------------------
  // The settings a new user has (data.rs defaults), but for what would make
  // two runs differ: no OLED pixel shift, the monitor on.
  const DEFAULTS = {
    lang: 'zh',
    pet: 'claude-chan',
    scale: 0.55,
    display: 'taskbar',
    out: false,
    islandWidth: 'normal',
    islandHideUnder: true,
    taskbarMaterial: 'mica',
    taskbarDisplays: 'main',
    taskbarClearWhen: 'mica',
    taskbarButtons: 'icons',
    taskbarAlign: 'center',
    taskbarEdge: 'bottom',
    oledShift: false,
    autoUpdateCheck: true,
    updateFrom: 'auto',
    details: true,
    widgetsOff: [],
    monitor: { cpu: true, mem: true, net: true, battery: false, every: 2, pin: true },
    widgetOrder: [],
    widgetSpin: 8,
    widgetNudge: true,
    plugins: {},
    mail: [],
    walk: true,
    look: true,
    hold: 8,
    endCue: 'ring',
    idleHideMin: 30,
    notify: { waiting: false, done: false, error: false },
    sound: false,
    promptWaitSec: 290,
    dnd: false,
    hideInFullscreen: true,
    onboarded: true,
    lookDefaults: 2,
  }

  const world = {
    lang: 'zh',
    // Windows' mode and accent colour (taskbar.rs look).
    mode: 'dark',
    accent: { base: '#0078d4', light: '#4cc2ff', dark: '#0067c0' },
    // Her home (taskbar, island), the strip's edge and material.
    home: 'taskbar',
    edge: 'bottom',
    material: 'mica',
    // The display, in px, and the strip's thickness.
    screen: [1600, 900],
    strip: 48,
    wallpaper: HERE + 'assets/wall.jpg',
    sprite: ROOT + 'pets/claude-chan/spritesheet.webp',
    // The sessions as state.rs views them; the first is the one shown.
    sessions: [],
    // The prompts waiting (asks.rs views).
    asks: [],
    // Every widget with something to say (widgets.rs), and the plugins she runs (scripts.rs).
    widgets: [],
    plugins: [],
    taskbarLayout: null,
    deskLayout: [],
    // The taskbar's programs, tray icons and keyboard (taskbar.rs).
    windows: [],
    tray: [],
    keys: {},
    // Any other setting, over the defaults.
    config: {},
    connected: true,
    quiet: false,
    version: '0.3.2',
    displayCount: 1,
  }

  // The screen is the world's (headless Edge's is its window, or 800 x 600):
  // what is laid out by it (the open island's most, the settings') as on hers.
  const SCREEN = {
    width: () => world.screen[0],
    height: () => world.screen[1],
    availWidth: () => world.screen[0],
    availHeight: () => world.screen[1] - (world.home === 'taskbar' ? 0 : world.strip),
    availLeft: () => 0,
    availTop: () => 0,
  }
  for (const [key, get] of Object.entries(SCREEN)) Object.defineProperty(window.screen, key, { get, configurable: true })

  const config = () => ({
    ...DEFAULTS,
    lang: world.lang,
    display: world.home,
    taskbarEdge: world.edge,
    taskbarMaterial: world.material,
    ...(world.taskbarLayout ? { taskbarLayout: world.taskbarLayout } : {}),
    deskLayout: world.deskLayout,
    ...world.config,
  })

  const IDLE = { id: '', project: '', agent: 'claude', jump: false, mood: 'idle', detail: null, since: null, took: null, name: '', step: null, stepSince: null, todo: null, files: 0, reply: '', error: null }

  // pet:update, as main.rs payload makes it.
  function payload() {
    const list = world.sessions
    const top = list[0] || IDLE
    const busy = list.filter(s => s.id !== top.id && s.mood !== 'idle')
    const second = (busy.find(s => s.mood === 'waiting' || s.mood === 'error') || busy[0])?.mood || null
    return {
      ...IDLE,
      ...top,
      others: busy.length,
      sessions: list.length,
      list,
      config: config(),
      lang: world.lang,
      sprite: world.sprite,
      spriteVersion: 2,
      second,
      islandTemp: false,
      settingsOpen: false,
      quiet: world.quiet,
      fullScreen: false,
      plate: true,
      ...(world.home !== 'taskbar' ? { winLight: world.mode === 'light' } : {}),
      widgets: world.widgets,
      unconnected: !world.connected,
      ...(world.connected ? {} : { tools: ['claude', 'codex'] }),
    }
  }

  // settings_get, as settings.rs snapshot makes it.
  function snapshot() {
    const on = world.connected
    return {
      settings: config(),
      lang: world.lang,
      pets: [{ id: 'claude-chan', name: 'Claude', author: 'Wakuwaku', url: world.sprite, version: 2, codex: false }],
      now: payload(),
      sessions: world.sessions,
      asks: world.asks,
      isVisible: true,
      hidden: false,
      connection: on ? 'hooks' : 'none',
      pluginCommands: ['/plugin marketplace add zhoupengjie/Wakuwaku', '/plugin install wakuwaku@wakuwaku'],
      hooks: on ? 'ok' : 'missing',
      claudeSeen: on,
      settingsFile: 'C:\\Users\\you\\.claude\\settings.json',
      settingsShown: '~\\.claude\\settings.json',
      settingsBackup: true,
      claudeHere: true,
      codex: { hooks: on ? 'ok' : 'missing', file: 'C:\\Users\\you\\.codex\\hooks.json', fileShown: '~\\.codex\\hooks.json', seen: on, version: '0.150.0', async: true },
      agy: { hooks: 'absent' },
      opencode: { plugin: 'absent' },
      omp: { hooks: 'absent' },
      zcode: { hooks: 'absent' },
      dsh: { hooks: 'absent' },
      agentVersions: { claude: '2.1.0' },
      displayCount: world.displayCount,
      loginAtStart: true,
      winLook: { light: world.mode === 'light', accent: world.accent.base },
      winPaper: { file: 'gpt-cloud-terrace.png', position: 'fill', every: 0, shuffle: false },
      version: world.version,
      update: { current: world.version, may: true, latest: { version: world.version, newer: false, canInstall: true }, checkedAt: T0 - 2 * 3600e3 },
      crash: { can: false },
      fullscreenAvailable: true,
      plate: true,
      widgets: world.widgets,
      lhm: { status: 'none' },
      hw: {},
      monitorSource: 'builtin',
      cpuSensors: { state: 'off' },
      lhmSetup: {},
      plugins: world.plugins,
      mail: [],
      mailAgents: { claude: true, codex: true },
      mailModels: {},
      mailTalks: [],
      mailSystemProxy: null,
      mailRev: 0,
      waku: 'C:\\Users\\you\\AppData\\Local\\wakuwaku\\waku.ps1',
      port: 47213,
    }
  }

  // The desktop's picture as taskbar.rs sends it for the strip's Mica: the
  // display from the strip's top-left.
  function stripPaper() {
    const [w, h] = world.screen
    const s = world.strip
    const at = { bottom: [0, -(h - s)], top: [0, 0], left: [0, 0], right: [-(w - s), 0] }[world.edge] || [0, -(h - s)]
    return { url: world.wallpaper, position: 'fill', color: '#1b2533', mon: [...at, w, h], screen: [...at, w, h], sf: 1 }
  }

  // The desktop's window on the display (desk.rs desk:state): its work
  // area the display less the strip.
  function deskState() {
    const [w, h] = world.screen
    const s = world.strip
    const work = { bottom: [0, 0, w, h - s], top: [0, s, w, h - s], left: [s, 0, w - s, h], right: [0, 0, w - s, h] }[world.edge] || [0, 0, w, h - s]
    return { key: 'd1', primary: true, keys: ['d1'], sf: 1, size: [w, h], work, lang: world.lang, light: world.mode === 'light', detailed: config().details !== false, home: world.home, sprite: world.sprite, layout: world.deskLayout }
  }

  const deskPaper = () => {
    const [w, h] = world.screen
    return { url: world.wallpaper, position: 'fill', color: '#1b2533', mon: [0, 0, w, h], screen: [0, 0, w, h], sf: 1 }
  }

  const look = () => ({ mode: world.mode, accent: world.accent, endTask: false, flashes: 7 })

  // Her taskbar on another display (bars.rs bar:state): Start, the windows,
  // the clock; her capsule at its start while a prompt waits.
  function barState() {
    const c = config()
    return {
      windows: world.windows,
      look: look(),
      edge: world.edge,
      align: c.taskbarAlign,
      material: world.material,
      clearWhen: c.taskbarClearWhen,
      filled: false,
      lang: world.lang,
      paper: stripPaper(),
      sprite: world.sprite,
      spriteVersion: 2,
      islandLook: c.islandLook,
      asks: world.asks.length ? { n: world.asks.length, kind: world.asks[0].kind === 'question' ? 'answer' : 'approve' } : null,
    }
  }

  // The world sent to the page, as main sends it to that window.
  function push() {
    if (role === 'bar') return emit('bar:state', barState())
    if (role === 'desk') {
      emit('desk:state', deskState())
      emit('desk:widgets', { widgets: world.widgets })
      emit('desk:paper', deskPaper())
      return
    }
    if (role === 'island') {
      emit('taskbar:look', look())
      if (world.home === 'taskbar') {
        emit('taskbar:wallpaper', stripPaper())
        emit('taskbar:windows', world.windows)
        emit('taskbar:keys', world.keys)
        emit('taskbar:tray', world.tray)
      }
      emit('pet:update', payload())
      emit('pet:asks', world.asks)
    }
  }

  // Part of the world changed; objects merged one level down (config, keys).
  function set(patch = {}) {
    for (const [k, v] of Object.entries(patch)) {
      if (v && typeof v === 'object' && !Array.isArray(v) && world[k] && typeof world[k] === 'object' && !Array.isArray(world[k]) && k !== 'taskbarLayout') world[k] = { ...world[k], ...v }
      else world[k] = v
    }
    return world
  }

  // --- What the page asks ---------------------------------------------------------------
  const ANSWERS = {
    settings_get: () => snapshot(),
    settings_set: ({ patch }) => {
      Object.assign(world.config, patch || {})
      return snapshot()
    },
    settings_gallery: () => ({ ok: true, items: [], page: 1, totalPages: 1, total: 0 }),
    session_jump: () => true,
    pet_walk: () => false,
    taskbar_open: () => true,
    taskbar_volume: () => true,
    taskbar_clock_tip: () => '2026年10月17日 星期六',
    taskbar_end_task: () => false,
    taskbar_inputs: () => [],
    taskbar_battery_font: () => null,
    taskbar_held: () => ({ shift: false, ctrl: false }),
    taskbar_window: () => true,
    taskbar_windows: () => true,
    taskbar_window_menu: () => true,
    taskbar_app: () => true,
    desk_icons: () => [],
    desk_where: () => null,
    desk_keep: () => true,
    mail_draft: () => null,
    island_plate: ({ rect, fade }) => tellOuter({ plate: rect || null, fade: fade || 0 }),
    // Her own menu is Windows' (tray.rs menu), not the page's: said, not drawn.
    pet_menu: ({ part }) => tellOuter({ nativeMenu: part || 'her' }),
  }

  window.Fake = {
    T0,
    HERE,
    ROOT,
    role,
    label,
    world,
    DEFAULTS,
    calls,
    errors,
    listeners,
    emit,
    set,
    push,
    payload,
    snapshot,
    config,
    deskState,
    stripPaper,
    barState,
    handle: (cmd, fn) => handlers.set(cmd, fn),
    heard: channel => (listeners.get(channel)?.size || 0) > 0,
    called: cmd => calls.filter(c => c.cmd === cmd),
  }
})()
