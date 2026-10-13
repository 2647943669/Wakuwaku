// Her taskbar on another display (bars.rs: taskbarDisplays all): Start, the
// windows on this display (as Windows' own setting for several displays
// says) and the clock, at the end the sliver that shows the desktop. Her
// capsule, the tray and the widgets are on the main display's alone (the
// appearance session's spec, 2026-10-13), but for a prompt waiting: then her
// capsule comes at its start, "waiting for your approval · N", a press
// showing the prompt in her island there. What it shows comes as bar:state
// (bars.rs refresh); a press asks what the main taskbar's would (taskbar.rs).
;(function () {
  const { invoke } = window.__TAURI__.core
  const { listen } = window.__TAURI__.event
  const { t } = window.I18n
  const { CELL_W, CELL_H, CLIPS } = window.Sprite
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

  // --- Her capsule, while a prompt waits ---------------------------------------
  // As her island on the main taskbar draws itself (island.js placeHer,
  // drawIslandLook): her round portrait playing her waiting clip, ringed in
  // the waiting colour, and the words in it; its ground her island's, as the
  // settings have it for Windows' mode (islandLook: bg, op, blur, edge,
  // bare), Windows' flyout grey by default; bare, none, its words haloed as
  // the clear strip's are; on the black strip, none either (it is black).
  const her = document.getElementById('her')
  const film = her.querySelector('.film')
  const sheet = her.querySelector('.sheet')
  const words = her.querySelector('.words')
  const WAITING = { dark: '#ffb340', light: '#b25e00' }
  const FACE = 30
  const PER_PX = 0.00875
  let frames = 0

  // A colour's relative lightness (WCAG), 0 to 1 (island.js lightness).
  function lightness(rgb) {
    const [r, g, b] = [(rgb >> 16) & 255, (rgb >> 8) & 255, rgb & 255].map(v => {
      v /= 255
      return v <= 0.03928 ? v / 12.92 : ((v + 0.055) / 1.055) ** 2.4
    })
    return 0.2126 * r + 0.7152 * g + 0.0722 * b
  }

  function drawHer() {
    const asks = state.asks
    const on = !!asks && asks.n > 0
    if (her.hidden === on) her.hidden = !on
    body.classList.toggle('b-her', on)
    if (!on) {
      clearInterval(frames)
      frames = 0
      return
    }
    const mode = isLight() ? 'light' : 'dark'
    const capsule = isLight() || state.material !== 'black'
    const mine = (capsule && state.islandLook?.[mode]) || {}
    const bare = !capsule || (mine.bare ?? mode === 'dark') === true
    const bg = /^#[0-9a-f]{6}$/i.test(mine.bg || '') ? parseInt(mine.bg.slice(1), 16) : null
    const op = Number.isFinite(mine.op) ? Math.min(100, Math.max(20, mine.op)) / 100 : 1
    const blur = Number.isFinite(mine.blur) ? Math.min(60, Math.max(0, mine.blur)) : 0
    const inkLight = bare || bg === null ? isLight() : lightness(bg) > 0.35
    const ring = WAITING[inkLight ? 'light' : 'dark']
    her.classList.toggle('bare', bare)
    her.classList.toggle('ink-light', inkLight)
    her.style.setProperty('--ring', ring)
    her.style.setProperty('--h-bg', bg === null ? (isLight() ? '#f3f3f3' : '#2c2c2c') : `rgba(${(bg >> 16) & 255}, ${(bg >> 8) & 255}, ${bg & 255}, ${op})`)
    her.style.setProperty('--h-blur', blur ? `blur(${blur}px) saturate(1.25)` : 'none')
    const line = inkLight ? 'rgba(0, 0, 0, 0.08)' : 'rgba(255, 255, 255, 0.1)'
    const edge = { ring: `0 0 0 1.5px ${ring}, 0 0 12px color-mix(in srgb, ${ring} 35%, transparent)`, none: '0 0 0 0 transparent' }[mine.edge]
    her.style.setProperty('--h-edge', edge || `0 0 0 1px ${line}`)
    const said = t(state.lang, asks.kind === 'answer' ? 'bar.answer' : 'bar.approve', { n: asks.n })
    // Along a side, upright: her portrait and the number; the words in its tip.
    words.textContent = isSide() ? String(asks.n) : said
    her.title = said
    // Her sheet, scaled to the portrait and centred on her face.
    const scale = FACE * PER_PX
    film.style.backgroundImage = state.sprite ? `url("${state.sprite}")` : ''
    film.style.height = `${state.spriteVersion === 1 ? 1872 : 2288}px`
    sheet.style.transform = `translate(${-(CELL_W * scale - FACE) / 2}px, -2px) scale(${scale})`
    const clip = CLIPS.waiting
    const show = () => {
      if (!document.hidden) film.style.transform = `translate3d(${-(Math.floor(Date.now() / clip.ms) % clip.frames) * CELL_W}px, ${-clip.row * CELL_H}px, 0)`
    }
    show()
    if (!frames) frames = setInterval(show, clip.ms)
    // Room for it before Start and the buttons laid from the start.
    body.style.setProperty('--her-w', `${her.offsetWidth}px`)
    body.style.setProperty('--her-h', `${her.offsetHeight}px`)
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
    drawHer()
  }

  // Presses: Start and the clock open what Windows' do, the sliver shows the
  // desktop; a program's button as on the main taskbar (island.js): one not
  // running starts, one window to the front or minimized, several in turn.
  document.addEventListener('click', e => {
    // Her capsule: the prompt shown in her island on the main display (bars.rs bar_asks).
    closePop()
    if (e.target.closest('#her')) return invoke('bar_asks').catch(() => {})
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

  // Over a program's button, as on the main taskbar (island.js openPop): the
  // pointer resting on a running one, its windows; a right press, its menu.
  // In a window of their own over the strip (bars.rs bar_pop, barpop.js),
  // which goes once the pointer has left both a moment (told it here,
  // bar_pop_over) or at a press elsewhere. { app, kind } while open.
  let popFor = null
  let popTimer
  const call = (cmd, args) => invoke(cmd, args).catch(() => {})
  function openPop(node, kind) {
    const b = node.getBoundingClientRect()
    popFor = { app: node.dataset.app, kind }
    call('bar_pop', { kind, key: node.dataset.app, at: [b.left, b.top, b.width, b.height] })
  }
  function closePop() {
    clearTimeout(popTimer)
    if (!popFor) return
    popFor = null
    call('barpop_hide')
  }
  wins.addEventListener('mouseover', e => {
    const node = e.target.closest('[data-app]')
    clearTimeout(popTimer)
    const app = node && (state.windows || []).find(a => a.app === node.dataset.app)
    if (!app || popFor?.kind === 'menu' || popFor?.app === app.app || !(app.windows || []).length) return
    popTimer = setTimeout(() => openPop(node, 'list'), popFor ? 0 : 500)
  })
  wins.addEventListener('mouseenter', () => call('bar_pop_over', { over: true }))
  wins.addEventListener('mouseleave', () => {
    clearTimeout(popTimer)
    call('bar_pop_over', { over: false })
  })
  wins.addEventListener('contextmenu', e => {
    const node = e.target.closest('[data-app]')
    if (!node) return
    e.preventDefault()
    clearTimeout(popTimer)
    openPop(node, 'menu')
  })
  // Gone by itself (the pointer away, a press elsewhere, its program gone).
  listen('bar:pop-gone', () => (popFor = null), { target })

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
