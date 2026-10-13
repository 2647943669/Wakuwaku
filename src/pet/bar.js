// Her taskbar on another display (bars.rs: taskbarDisplays all): Start, the
// windows on this display (as Windows' own setting for several displays
// says) and the clock, at the end the sliver that shows the desktop. Her
// capsule, the tray and the widgets are on the main display's alone (the
// appearance session's spec, 2026-10-13). What it shows comes as bar:state
// (bars.rs refresh); a press asks what the main taskbar's would (taskbar.rs).
;(function () {
  const { invoke } = window.__TAURI__.core
  const { listen } = window.__TAURI__.event
  // For this window alone: listen() by itself hears what is sent to any (bridge.js).
  const LABEL = window.__TAURI_INTERNALS__?.metadata?.currentWebview?.label || window.__TAURI__.webviewWindow?.getCurrentWebviewWindow?.()?.label || ''
  const target = { kind: 'WebviewWindow', label: LABEL }

  const body = document.body
  const start = document.querySelector('.start')
  const wins = document.getElementById('wins')
  const clock = document.querySelector('.clock')
  let state = { windows: [], look: {}, edge: 'bottom', align: 'center', material: 'clear', lang: 'zh' }

  const el = (tag, cls, text) => {
    const n = document.createElement(tag)
    if (cls) n.className = cls
    if (text != null) n.textContent = text
    return n
  }
  const isSide = () => state.edge === 'left' || state.edge === 'right'
  const isLight = () => state.look?.mode === 'light'

  // The Start mark, drawn on the screen's own pixels (as the main taskbar's, island.js drawStartMark).
  let startScale = 0
  function drawStart() {
    const dpr = devicePixelRatio || 1
    if (dpr === startScale) return
    startScale = dpr
    const box = Math.round(24 * dpr)
    const gap = Math.max(1, Math.round(0.8 * dpr))
    const pane = Math.floor((box - 1 - gap) / 2)
    const at = Math.floor((box - 2 * pane - gap) / 2)
    const far = at + pane + gap
    const end = far + pane
    const r = Math.max(0.5, 0.8 * dpr).toFixed(2)
    const rect = (x, y) => `<rect x="${x}" y="${y}" width="${pane}" height="${pane}" rx="${r}"/>`
    start.innerHTML = `<svg viewBox="0 0 ${box} ${box}" style="width:${box / dpr}px;height:${box / dpr}px"><defs><linearGradient id="start-blue" gradientUnits="userSpaceOnUse" x1="${at}" y1="${at}" x2="${end}" y2="${end}"><stop class="s0" offset="0"/><stop class="s1" offset="1"/></linearGradient></defs><g fill="url(#start-blue)">${rect(at, at)}${rect(far, at)}${rect(at, far)}${rect(far, far)}</g></svg>`
  }

  function tickClock() {
    const d = new Date()
    const zh = state.lang === 'zh' ? 'zh-CN' : undefined
    const time = d.toLocaleTimeString(zh, { hour: '2-digit', minute: '2-digit' })
    const date = d.toLocaleDateString(zh, isSide() ? { month: 'numeric', day: 'numeric' } : { year: 'numeric', month: 'numeric', day: 'numeric' })
    if (clock.dataset.at !== time + date) {
      clock.dataset.at = time + date
      clock.replaceChildren(el('b', '', time), el('i', '', date))
    }
  }

  // A button for each program, as the main taskbar's: its icon, a mark under
  // it while running (longer in the accent for the one in front), its windows' titles in its tip.
  function drawWindows() {
    const list = state.windows || []
    wins.replaceChildren(
      ...list.map(app => {
        const windows = app.windows || []
        const b = el('div', 'win')
        b.dataset.app = app.app
        b.classList.toggle('run', windows.length > 0)
        b.classList.toggle('front', windows.some(w => w.front))
        b.classList.toggle('flash', windows.some(w => w.flash))
        b.title = [app.name, ...windows.map(w => w.title)].filter(Boolean).join('\n')
        const img = el('img')
        img.src = app.png || ''
        img.alt = ''
        b.append(img)
        return b
      }),
    )
  }

  function draw() {
    const edge = state.edge || 'bottom'
    for (const e of ['bottom', 'top', 'left', 'right']) body.classList.toggle(`b-${e}`, e === edge)
    body.classList.toggle('b-side', isSide())
    body.classList.toggle('b-light', isLight())
    for (const m of ['clear', 'black', 'mica']) body.classList.toggle(`b-${m}`, (state.material || 'clear') === m)
    body.classList.toggle('b-start', state.align === 'left')
    const accent = state.look?.accent
    if (accent) body.style.setProperty('--accent', isLight() ? accent.dark || accent.base : accent.light || accent.base)
    document.documentElement.lang = state.lang === 'zh' ? 'zh-CN' : 'en'
    drawStart()
    drawWindows()
    tickClock()
  }

  // Presses: Start and the clock open what Windows' do, the sliver shows the
  // desktop; a program's button as on the main taskbar (island.js): one not
  // running starts, one window to the front or minimized, several in turn.
  document.addEventListener('click', e => {
    const what = e.target.closest('[data-bar]')?.dataset.bar
    if (what) return invoke('taskbar_open', { what })
    const key = e.target.closest('[data-app]')?.dataset.app
    const app = (state.windows || []).find(a => a.app === key)
    if (!app) return
    const windows = app.windows || []
    if (!windows.length) return invoke('taskbar_app', { path: String(app.path), what: 'launch', id: app.id ? String(app.id) : null }).catch(() => {})
    if (windows.length === 1) return invoke('taskbar_window', { id: Number(windows[0].id), what: 'press' }).catch(() => {})
    invoke('taskbar_windows', { ids: windows.map(w => Number(w.id)) }).catch(() => {})
  })
  document.addEventListener('contextmenu', e => e.preventDefault())

  setInterval(tickClock, 5000)
  addEventListener('resize', () => {
    startScale = 0
    drawStart()
  })
  listen('bar:state', e => {
    state = e.payload || state
    draw()
  }, { target }).then(() => invoke('bar_ready').catch(() => {}))
  draw()
})()
