// The desktop's widgets on one display (the widget design of 2026-10-16, §3,
// §7, §8, §9, §10; desk.rs makes a window for each display that has some):
// each alone or several stacked, where it stands on the display's grid, its
// glass the wallpaper under it blurred (mica.js glassPaper, desk.rs reads
// which picture), the glass colour over it.
//
// Alone: dragged anywhere (it snaps to the grid, clear of the icons and the
// others; dropped on icons, the nearest free spot); two buttons on the
// pointer's (size, more); a double-click opens its settings. Stacked: one
// dragged onto another of its size and held there half a second; the wheel,
// a drag up or down, or its dots turn it (0.45 s); a drag out of it takes the
// one on top out; held still half a second first, the whole stack moves.
// Right-click: its menu, as the taskbar's and the capsule's.
//
// The window lets clicks through but where a widget is (desk.rs polls the
// pointer against the shapes this page sends, desk_shapes); while a press is
// held it keeps the mouse (desk_hold). Nothing moves by itself, nothing is
// drawn while nothing changes.
;(function () {
  const { invoke } = window.__TAURI__.core
  const { listen } = window.__TAURI__.event
  const { I18n, Widgets, WCards, WDesk, DeskGrid: G, Mica } = window
  const LABEL = window.__TAURI_INTERNALS__?.metadata?.currentWebview?.label || ''
  const target = { kind: 'WebviewWindow', label: LABEL }
  const desk = document.getElementById('desk')

  // What desk.rs says: this display (key, primary, keys of those there,
  // work area, size), her language, Windows' mode, the specifics setting,
  // her home, her sheet; the layout (deskLayout, every display's).
  let here = null
  let layout = []
  let all = []
  // The wallpaper blurred: { url, size: [w, h] with its margin, margin }.
  let paper = null
  let paperTurn = 0
  // The icons on this display (desk_icons), as rects.
  let icons = []
  const nodes = new Map()
  const tops = new Map()
  let spots = new Map()
  const drawn = new Set()
  const firstSeen = new Map()
  let menu = null
  let drag = null
  let turnedAt = 0

  const lang = () => here?.lang || 'zh'
  const t = (key, vars) => I18n.t(lang(), key, vars)
  const esc = WCards.esc
  const ctx = () => ({ lang: lang(), light: !!here?.light, detailed: here?.detailed !== false, now: Date.now(), all })
  const newId = () => `w${Date.now().toString(36).slice(-6)}${Math.random().toString(36).slice(2, 6)}`
  const reduced = () => matchMedia('(prefers-reduced-motion: reduce)').matches
  const grid = () => G.gridOf(here.work)
  const shown = () => (here ? G.shownHere(layout, here) : [])
  const visiting = e => !!e.d && e.d !== here.key
  const find = id => layout.find(e => e.id === id)

  // --- What is drawn ---------------------------------------------------------------------
  function draw() {
    if (!here) return
    const entries = shown()
    const g = grid()
    const fresh = new Set(entries.filter(e => !drawn.has(e.id)).map(e => e.id))
    const { at, moved } = G.settle(entries, here, g, icons, fresh)
    spots = at
    for (const e of entries) {
      drawn.add(e.id)
      if (!firstSeen.has(e.id)) firstSeen.set(e.id, Date.now())
    }
    if (moved.length) keep(entries.filter(e => moved.includes(e.id)).map(e => G.placed(e, here.key, at.get(e.id), visiting(e))))
    const ids = new Set(entries.map(e => e.id))
    for (const [id, n] of nodes)
      if (!ids.has(id)) {
        n.el.remove()
        nodes.delete(id)
        tops.delete(id)
        // Back here later (from another display): new here again, clear of the icons.
        drawn.delete(id)
      }
    for (const e of entries) drawEntry(e, at.get(e.id))
    if (menu && !nodes.has(menu.id)) closeMenu()
    shapes()
  }

  function drawEntry(e, cell) {
    let n = nodes.get(e.id)
    if (!n) {
      n = { el: document.createElement('div'), key: '', x: 0, y: 0 }
      n.el.dataset.id = e.id
      desk.append(n.el)
      nodes.set(e.id, n)
      wire(n)
    }
    n.entry = e
    n.rect = G.rectAt(grid(), cell, e.size)
    const key = contentKey(e)
    if (key !== n.key) {
      n.key = key
      fill(n)
    }
    if (!(drag && drag.n === n && drag.mode !== 'press')) place(n, n.rect.x, n.rect.y)
  }

  // What a node shows changes with: its entry, the one on top, the words of
  // its widgets, the mode, the language, the specifics, and a member just
  // put there (loading) or not.
  function contentKey(e) {
    const ws = G.membersOf(e).map(m => Widgets.sourcesOf(m, all).map(w => [w.id, w.label, w.value, w.detail, w.color, w.private]))
    const loading = G.membersOf(e).map(m => isLoading(e, m))
    return JSON.stringify([e.type, e.size, G.membersOf(e), topOf(e), ws, here.light, lang(), here.detailed, here.sprite, loading])
  }
  const topOf = e => (G.isStack(e) ? Math.min(tops.get(e.id) || 0, e.items.length - 1) : 0)
  const isLoading = (e, m) => !Widgets.sourcesOf(m, all).length && Date.now() - (firstSeen.get(e.id) || 0) < 12000

  // A member's face at the entry's size: its widget's (a plugin's first,
  // which shows the others), or, with nothing to say, its name greyed.
  function face(e, m) {
    const ws = Widgets.sourcesOf(m, all)
    const c = ctx()
    if (!ws.length) {
      const name = m.name || Widgets.nameOf(lang(), { type: m.type, of: m.of, ...(m.name ? { name: m.name } : {}) }, all, c.detailed)
      return { html: WDesk.emptyHTML(m, name, isLoading(e, m) ? 'loading' : 'none', c), color: '', w: null }
    }
    return { html: WDesk.faceHTML(ws[0], e.size, c), color: WCards.colorOf(ws[0], c), w: ws[0] }
  }

  // A card: its own picture of the wallpaper under it (a canvas: desk.rs's
  // picture comes from another origin, which a page may draw but not read),
  // the glass colour and its glow over that, then its words.
  const TOOLS = `<div class="tools"><button type="button" data-act="size" title="">&#xE740;</button><button type="button" data-act="more" title="">&#xE712;</button></div>`
  const CARD = size => `<div class="wd ${size}"><canvas class="wpaper"></canvas><div class="wtint"></div><div class="wbody"></div></div>`
  // Each member's place under the one on top: 0 on top, then the two
  // layers that show, then the rest unseen behind them.
  const LAYERS = ['top', 'b1', 'b2']
  const layerOf = (i, top, n) => LAYERS[(i - top + n) % n] || 'rest'

  // Built anew only when its shape changes (alone or stacked, its size, its
  // members); else only the words that changed, in place (the monitor's
  // every two seconds: its words alone, not its glass).
  function fill(n) {
    const e = n.entry
    const shape = JSON.stringify([e.type, e.size, G.membersOf(e).map(G.keyOf)])
    const top = topOf(e)
    n.el.className = `dw ${e.size}${G.isStack(e) ? ' stack' : ''}${n.el.classList.contains('hover') ? ' hover' : ''}${menu?.id === e.id ? ' menu' : ''}`
    if (shape !== n.shape) {
      n.shape = shape
      if (!G.isStack(e)) n.el.innerHTML = CARD(e.size)
      else {
        const layers = e.items.map((m, i) => `<div class="layer ${layerOf(i, top, e.items.length)}" data-i="${i}">${CARD(e.size)}</div>`).join('')
        const dots = e.items.map((_, i) => `<button type="button" data-p="${i}" aria-label="${i + 1}"></button>`).join('')
        n.el.innerHTML = `${layers}<div class="sdots">${dots}</div>`
      }
      const cards = n.el.querySelectorAll('.wd')
      cards[top].insertAdjacentHTML('beforeend', TOOLS)
      for (const b of n.el.querySelectorAll('.tools button')) b.title = t(b.dataset.act === 'size' ? 'menu.size' : 'desk.more')
      n.painted = ''
    }
    G.membersOf(e).forEach((m, i) => {
      const wd = n.el.querySelectorAll('.wd')[i]
      const body = wd?.querySelector('.wbody')
      if (!body) return
      const f = face(e, m)
      if (body._html !== f.html) {
        body.innerHTML = f.html
        body._html = f.html
        fillHer(body)
      }
      wd.classList.toggle('empty-w', !f.w)
      if (f.color) wd.style.setProperty('--c', f.color)
      else wd.style.removeProperty('--c')
    })
    if (G.isStack(e)) {
      n.el.querySelectorAll('.layer').forEach((l, i) => {
        for (const k of ['top', 'b1', 'b2', 'rest', 'gone']) l.classList.toggle(k, k === layerOf(i, top, e.items.length))
      })
      for (const b of n.el.querySelectorAll('.sdots button')) b.classList.toggle('on', Number(b.dataset.p) === top)
      const tools = n.el.querySelector('.tools')
      const onTop = n.el.querySelectorAll('.wd')[top]
      if (tools && onTop && tools.parentElement !== onTop) onTop.append(tools)
    }
    paint(n)
    // Loading (just put there, its plugin starting): drawn again once that
    // is over, no data then greyed; one timer, not a loop.
    if (G.membersOf(e).some(m => isLoading(e, m))) {
      clearTimeout(n.loadingTimer)
      n.loadingTimer = setTimeout(draw, Math.max(0, 12050 - (Date.now() - (firstSeen.get(e.id) || 0))))
    }
  }

  // Her face where today's glyph goes: the still first frame of her sheet.
  function fillHer(root) {
    for (const slot of root.querySelectorAll('.wher')) {
      if (!here.sprite) continue
      const px = slot.closest('.wmh') ? 16 : 14
      const s = px * 0.00875
      slot.style.backgroundImage = `url("${here.sprite}")`
      slot.style.backgroundSize = `${1536 * s}px ${2288 * s}px`
      slot.style.backgroundPosition = `${-(192 * s - px) / 2}px -2px`
    }
  }

  // --- Its glass: the wallpaper under it, blurred ---------------------------------------------
  // A card at (x, y), w by h, drawn scaled by s from its middle and moved
  // down by dy (a stack's layers): its canvas given the part of the blurred
  // picture that lies under it on the screen, so the two line up.
  const LAYER_FORM = { top: [1, 0], b1: [0.94, 13], b2: [0.88, 25], rest: [0.88, 25], gone: [0.96, -30] }
  function paperFor(canvas, x, y, w, h, s = 1, dy = 0) {
    const dpr = devicePixelRatio || 1
    const cw = Math.round(w * dpr)
    const ch = Math.round(h * dpr)
    if (canvas.width !== cw) canvas.width = cw
    if (canvas.height !== ch) canvas.height = ch
    const c = canvas.getContext('2d')
    if (!paper) return c.clearRect(0, 0, cw, ch)
    const k = paper.scale
    const sx = (x + (w / 2) * (1 - s) + paper.margin) * k
    const sy = (y + (h / 2) * (1 - s) + dy + paper.margin) * k
    c.imageSmoothingEnabled = true
    c.imageSmoothingQuality = 'high'
    c.drawImage(paper.canvas, sx, sy, w * s * k, h * s * k, 0, 0, cw, ch)
  }
  // Each of its cards painted for where it is now; not again while that is
  // so (the words changing leave its glass as it is).
  function paint(n, force = false) {
    const { w, h } = G.sizePx(n.entry.size)
    const cards = [...n.el.querySelectorAll('.wd')]
    const forms = cards.map(wd => {
      const layer = wd.parentElement?.classList.contains('layer') ? wd.parentElement : null
      const kind = layer ? ['top', 'b1', 'b2', 'rest', 'gone'].find(k => layer.classList.contains(k)) || 'top' : 'top'
      return [kind, layer?._pull?.[0] || 0, layer?._pull?.[1] || 0]
    })
    const now = JSON.stringify([n.x, n.y, w, h, forms, paperTurn, devicePixelRatio])
    if (!force && now === n.painted) return
    n.painted = now
    cards.forEach((wd, i) => {
      const canvas = wd.querySelector('.wpaper')
      if (!canvas) return
      const [kind, px, py] = forms[i]
      const [s, dy] = LAYER_FORM[kind]
      paperFor(canvas, n.x + px, n.y + py, w, h, s, dy)
    })
  }
  function place(n, x, y) {
    n.x = Math.round(x)
    n.y = Math.round(y)
    n.el.style.transform = `translate(${n.x}px, ${n.y}px)`
    paint(n)
  }

  // A new picture (desk:paper): blurred once, for all of them.
  async function setPaper(p) {
    const mine = ++paperTurn
    if (!p) {
      paper = null
      for (const n of nodes.values()) paint(n, true)
      return
    }
    let img = null
    if (p.url) {
      img = new Image()
      img.src = p.url
      try {
        await img.decode()
      } catch {
        img = null
      }
    }
    if (mine !== paperTurn) return
    const { canvas, margin, scale } = Mica.glassPaper(p, img, { blur: 30, saturate: 1.6 })
    if (img) img.src = ''
    paper = { canvas, margin, scale }
    for (const n of nodes.values()) paint(n, true)
  }

  // --- Where the window takes the mouse -------------------------------------------------------
  // Each widget (a stack with its layers and its dots), and an open menu.
  let sentShapes = ''
  function shapes() {
    const list = []
    for (const n of nodes.values()) {
      const { w, h } = G.sizePx(n.entry.size)
      if (G.isStack(n.entry)) list.push([n.x, n.y, w + 18, h + 28, 22])
      else list.push([n.x, n.y, w, h, 22])
    }
    const m = menuEl && !menuEl.hidden ? menuEl.getBoundingClientRect() : null
    if (m) list.push([m.left, m.top, m.width, m.height, 12])
    const now = JSON.stringify(list.map(r => r.map(v => Math.round(v))))
    if (now === sentShapes) return
    sentShapes = now
    invoke('desk_shapes', { shapes: JSON.parse(now) }).catch(() => {})
  }
  const hold = on => invoke('desk_hold', { on }).catch(() => {})

  // --- Kept: what changed, merged with the others' (desk.rs desk_keep) ------------------------
  function keep(entries, gone = []) {
    if (!entries.length && !gone.length) return
    const given = new Map(entries.map(e => [e.id, e]))
    const out = []
    for (const e of layout) {
      if (gone.includes(e.id)) continue
      out.push(given.get(e.id) || e)
      given.delete(e.id)
    }
    layout = [...out, ...given.values()]
    invoke('desk_keep', { entries, gone }).catch(err => console.log('desk: not kept', err))
  }
  // A whole new layout kept as what changed in it.
  function keepLayout(next) {
    const was = new Map(layout.map(e => [e.id, JSON.stringify(e)]))
    const ids = new Set(next.map(e => e.id))
    keep(
      next.filter(e => was.get(e.id) !== JSON.stringify(e)),
      layout.filter(e => !ids.has(e.id)).map(e => e.id),
    )
    draw()
  }

  // --- The pointer: hover, press, drag, drop -----------------------------------------------------
  function wire(n) {
    const el = n.el
    el.addEventListener('mouseenter', () => el.classList.add('hover'))
    el.addEventListener('mouseleave', () => {
      if (!drag) el.classList.remove('hover')
    })
    el.addEventListener('pointerdown', ev => press(ev, n))
    el.addEventListener('pointermove', ev => move(ev))
    el.addEventListener('pointerup', ev => release(ev))
    el.addEventListener('pointercancel', () => cancel())
    el.addEventListener('dblclick', ev => {
      if (ev.target.closest('.tools, .sdots')) return
      const w = shownWidget(n.entry)
      if (w) ask('settings', { widget: w.id })
    })
    el.addEventListener('contextmenu', ev => {
      ev.preventDefault()
      openMenu(n.entry.id, 'full', { x: ev.clientX, y: ev.clientY })
    })
    el.addEventListener('click', ev => {
      const b = ev.target.closest('.tools button')
      if (b) {
        ev.stopPropagation()
        const r = b.getBoundingClientRect()
        return menu?.id === n.entry.id && menu.kind === b.dataset.act ? closeMenu() : openMenu(n.entry.id, b.dataset.act, { x: r.left, y: r.bottom + 6, button: r })
      }
      const dot = ev.target.closest('.sdots button')
      if (dot && G.isStack(n.entry)) turnTo(n, Number(dot.dataset.p))
    })
    el.addEventListener(
      'wheel',
      ev => {
        if (!G.isStack(n.entry)) return
        ev.preventDefault()
        if (performance.now() - turnedAt < (reduced() ? 250 : 450)) return
        turn(n, ev.deltaY > 0 ? 1 : -1)
      },
      { passive: false },
    )
  }

  // The widget a stack shows (its top), or the one alone.
  function shownWidget(e) {
    const m = G.isStack(e) ? e.items[topOf(e)] : G.memberOf(e)
    return Widgets.sourcesOf(m, all)[0] || null
  }

  function press(ev, n) {
    if (ev.button !== 0 || ev.target.closest('.tools')) return
    closeMenu()
    // A drag along the dots turns the stack through them.
    if (ev.target.closest('.sdots')) {
      drag = { n, mode: 'dots', id: ev.pointerId }
      n.el.setPointerCapture(ev.pointerId)
      hold(true)
      return
    }
    drag = { n, mode: 'press', id: ev.pointerId, x0: ev.clientX, y0: ev.clientY, dx: ev.clientX - n.x, dy: ev.clientY - n.y, armed: null, over: null, overTimer: 0, lift: 0 }
    n.el.setPointerCapture(ev.pointerId)
    hold(true)
    // The icons as they are now, for the drop.
    loadIcons()
    // Held still half a second on a stack: the whole stack picked up.
    if (G.isStack(n.entry))
      drag.lift = setTimeout(() => {
        if (drag?.n === n && drag.mode === 'press') {
          drag.mode = 'lift'
          n.el.classList.add('lifted')
        }
      }, 500)
  }

  function move(ev) {
    const d = drag
    if (!d || ev.pointerId !== d.id) return
    const n = d.n
    if (d.mode === 'dots') {
      const dots = [...n.el.querySelectorAll('.sdots button')]
      const near = dots.reduce((best, b, i) => {
        const r = b.getBoundingClientRect()
        const dist = Math.abs(ev.clientY - (r.top + r.height / 2))
        return dist < best.dist ? { i, dist } : best
      }, { i: 0, dist: Infinity })
      if (near.i !== topOf(n.entry) && performance.now() - turnedAt > 160) turnTo(n, near.i)
      return
    }
    const mx = ev.clientX - d.x0
    const my = ev.clientY - d.y0
    if (d.mode === 'press' || d.mode === 'lift') {
      if (Math.hypot(mx, my) < 5) return
      clearTimeout(d.lift)
      d.mode = G.isStack(n.entry) && d.mode === 'press' ? 'card' : 'move'
      if (d.mode === 'move') n.el.classList.add('dragging')
    }
    if (d.mode === 'move') {
      place(n, ev.clientX - d.dx, ev.clientY - d.dy)
      return seek(ev, n.entry)
    }
    // The one on top pulled: up or down within the stack turns it; out of it, it comes out.
    const top = n.el.querySelector('.layer.top')
    if (!top) return
    top.classList.add('pulled')
    const r = n.rect
    const inside = ev.clientX > n.x - 24 && ev.clientX < n.x + r.w + 24 && ev.clientY > n.y - 24 && ev.clientY < n.y + r.h + 24
    if (d.mode === 'card' && !inside) {
      d.mode = 'out'
      n.el.classList.add('dragging')
    }
    const pull = d.mode === 'card' ? [0, Math.max(-60, Math.min(60, my * 0.6))] : [mx, my]
    top._pull = pull
    top.style.transform = `translate(${pull[0]}px, ${pull[1]}px)`
    paint(n)
    if (d.mode === 'out') seek(ev, { ...G.memberOf(n.entry.items[topOf(n.entry)]), id: '', size: n.entry.size })
  }

  // Over one it may stack onto, held half a second: that one ringed, armed.
  function seek(ev, moving) {
    const d = drag
    const over = [...nodes.values()].find(o => o !== d.n && ev.clientX >= o.x && ev.clientX <= o.x + o.rect.w && ev.clientY >= o.y && ev.clientY <= o.y + o.rect.h)
    const ok = over && G.canStack(moving, over.entry) ? over : null
    if (ok === d.over) return
    clearTimeout(d.overTimer)
    d.over?.el.classList.remove('target')
    d.over = ok
    d.armed = null
    if (ok)
      d.overTimer = setTimeout(() => {
        if (drag === d && d.over === ok) {
          d.armed = ok.entry.id
          ok.el.classList.add('target')
        }
      }, 500)
  }

  function release(ev) {
    const d = drag
    if (!d || ev.pointerId !== d.id) return
    drag = null
    clearTimeout(d.lift)
    clearTimeout(d.overTimer)
    hold(false)
    const n = d.n
    n.el.classList.remove('dragging', 'lifted')
    d.over?.el.classList.remove('target')
    if (d.mode === 'dots' || d.mode === 'press' || d.mode === 'lift') return
    const top = n.el.querySelector('.layer.top')
    if (d.mode === 'card') {
      unpull(n, top)
      const my = ev.clientY - d.y0
      if (my < -24) turn(n, 1)
      else if (my > 24) turn(n, -1)
      return
    }
    const x = ev.clientX - d.dx
    const y = ev.clientY - d.dy
    const off = ev.clientX < 0 || ev.clientY < 0 || ev.clientX > innerWidth || ev.clientY > innerHeight
    if (d.mode === 'move') {
      const e = n.entry
      if (d.armed) {
        // The one dropped on top.
        tops.set(d.armed, 0)
        return keepLayout(G.stackOnto(layout, e.id, d.armed, newId))
      }
      if (off) return elsewhere(ev, x - ev.clientX, y - ev.clientY, e.size, cell => keepLayout(layout.map(o => (o.id === e.id ? G.placed(o, cell.key, cell.at) : o))), () => place(n, n.rect.x, n.rect.y))
      const cell = G.dropAt(grid(), x, y, e.size, others(e.id), icons)
      n.rect = G.rectAt(grid(), cell, e.size)
      place(n, n.rect.x, n.rect.y)
      return keepLayout(layout.map(o => (o.id === e.id ? G.placed(o, here.key, cell) : o)))
    }
    // Out of a stack: the one on top alone where it was let go of, or onto another.
    const s = n.entry
    const index = topOf(s)
    unpull(n, top)
    const cellNear = () => G.dropAt(grid(), n.x + (ev.clientX - d.x0), n.y + (ev.clientY - d.y0), s.size, others(s.id), icons)
    if (d.armed) {
      const out = G.takeOut(layout, s.id, index, newId, here.key, cellNear())
      const alone = out.find(e => !layout.some(o => o.id === e.id) && G.keyOf(e) === G.keyOf(s.items[index]))
      tops.set(s.id, 0)
      tops.set(d.armed, 0)
      return keepLayout(alone ? G.stackOnto(out, alone.id, d.armed, newId) : out)
    }
    if (off) return elsewhere(ev, n.x - d.x0, n.y - d.y0, s.size, cell => (tops.set(s.id, 0), keepLayout(G.takeOut(layout, s.id, index, newId, cell.key, cell.at))), () => {})
    tops.set(s.id, 0)
    keepLayout(G.takeOut(layout, s.id, index, newId, here.key, cellNear()))
  }
  function cancel() {
    const d = drag
    if (!d) return
    drag = null
    clearTimeout(d.lift)
    clearTimeout(d.overTimer)
    hold(false)
    d.n.el.classList.remove('dragging', 'lifted')
    d.over?.el.classList.remove('target')
    const top = d.n.el.querySelector('.layer.top')
    if (top) unpull(d.n, top)
    place(d.n, d.n.rect.x, d.n.rect.y)
  }
  function unpull(n, top) {
    if (!top) return
    top._pull = null
    top.style.transform = ''
    top.classList.remove('pulled')
    paint(n)
  }
  // The others' rects here (not this one).
  const others = id => [...nodes.values()].filter(o => o.entry.id !== id).map(o => o.rect)

  // Let go of on another display: its cell there (desk.rs desk_where says
  // which display, its work area and the point in its window's px); that
  // display's page clears it of icons and others as it comes (settle).
  async function elsewhere(ev, ox, oy, size, then, back) {
    let there = null
    try {
      there = await invoke('desk_where', { x: ev.clientX, y: ev.clientY })
    } catch {}
    if (!there || there.key === here.key) return back()
    const g = G.gridOf(there.work)
    then({ key: there.key, at: G.snap(g, there.x + ox, there.y + oy, size) })
  }

  async function loadIcons() {
    try {
      const list = await invoke('desk_icons')
      icons = (Array.isArray(list) ? list : []).map(([x, y, w, h]) => ({ x, y, w, h }))
    } catch {
      icons = []
    }
  }

  // --- A stack turned (§7, §9): the one on top up and away, the next rising --------------------
  function turn(n, by) {
    const s = n.entry
    turnTo(n, G.flip(topOf(s), s.items.length, by), by)
  }
  function turnTo(n, next, by = 0) {
    const s = n.entry
    const count = s.items.length
    const was = topOf(s)
    if (next === was || !G.isStack(s)) return
    by = by || (next > was ? 1 : -1)
    turnedAt = performance.now()
    tops.set(s.id, next)
    n.key = contentKey(s)
    const layers = [...n.el.querySelectorAll('.layer')]
    const setTo = (layer, cls) => {
      for (const k of ['top', 'b1', 'b2', 'rest', 'gone']) layer.classList.toggle(k, k === cls)
    }
    // The buttons go with the one on top.
    const tools = n.el.querySelector('.tools')
    if (tools) layers[next].querySelector('.wd').append(tools)
    const still = reduced()
    if (by > 0 && !still) {
      layers.forEach((l, i) => i !== was && setTo(l, layerOf(i, next, count)))
      setTo(layers[was], 'gone')
      setTimeout(() => {
        layers[was].classList.add('instant')
        setTo(layers[was], layerOf(was, next, count))
        void layers[was].offsetWidth
        layers[was].classList.remove('instant')
        paint(n)
      }, 450)
    } else if (by < 0 && !still) {
      layers[next].classList.add('instant')
      setTo(layers[next], 'gone')
      void layers[next].offsetWidth
      layers[next].classList.remove('instant')
      layers.forEach((l, i) => setTo(l, layerOf(i, next, count)))
    } else layers.forEach((l, i) => setTo(l, layerOf(i, next, count)))
    for (const b of n.el.querySelectorAll('.sdots button')) b.classList.toggle('on', Number(b.dataset.p) === next)
    paint(n)
    setTimeout(() => paint(n), 460)
  }

  // A letter come into one of a stack's (its unread grew): it comes to the
  // top, once (§7).
  function news(before) {
    for (const n of nodes.values()) {
      if (!G.isStack(n.entry)) continue
      const at = G.newsTop(n.entry, before, all)
      if (at >= 0 && at !== topOf(n.entry)) turnTo(n, at)
    }
  }

  // --- Its menu (§7, §10): size; where; stacked or apart; the board to come; settings; off ---------
  const menuEl = document.createElement('div')
  menuEl.className = 'wmenu'
  menuEl.hidden = true
  document.body.append(menuEl)

  // Why a size is not there, by kind (the spec's §4, as island.js has it).
  function noSize(e, size) {
    if (G.isStack(e)) return t('desk.stackSize')
    const w = shownWidget(e)
    const kind = w ? WCards.kindOf(w) : ''
    if (size === 'l' && kind === 'pomodoro') return t('menu.why.pomodoro')
    if (kind === 'countdown') return t(size === 'm' ? 'menu.why.countdownM' : 'menu.why.countdownL')
    return t(size === 'm' ? 'menu.noMedium' : 'menu.noLarge')
  }

  // The nearest of the same size it may stack with, here.
  function neighbour(e) {
    const n = nodes.get(e.id)
    if (!n) return null
    let best = null
    for (const o of nodes.values()) {
      if (o === n || !G.canStack(e, o.entry)) continue
      const d = Math.hypot(o.x - n.x, o.y - n.y)
      if (!best || d < best.d) best = { o, d }
    }
    return best?.o.entry || null
  }
  const nameOfEntry = e => {
    const w = shownWidget(e)
    const m = G.isStack(e) ? e.items[topOf(e)] : G.memberOf(e)
    return w ? WCards.titleOf(w, ctx()) : m.name || Widgets.nameOf(lang(), m, all, here.detailed !== false)
  }

  function menuHTML(e, kind) {
    const c = ctx()
    const opt = (act, v, label, on, why = '') => `<button type="button" class="wmo${on ? ' on' : ''}${why ? ' off' : ''}" data-act="${act}" data-v="${v}"${why ? ` aria-disabled="true" data-why="${esc(why)}"` : ''}>${esc(label)}</button>`
    const item = (act, label, why = '', v = '') => `<button type="button" class="wmi${why ? ' off' : ''}" data-act="${act}" data-v="${esc(v)}"${why ? ` aria-disabled="true" data-why="${esc(why)}"` : ''}>${esc(label)}</button>`
    const line = (label, html) => `<div class="wmr"><span class="wml">${esc(label)}</span><span class="wms">${html}</span></div>`
    const w = shownWidget(e)
    const stack = G.isStack(e)
    const glyph = w ? WCards.glyphHTML(w, 'wmg') : WCards.memberGlyph(stack ? e.items[0] : G.memberOf(e))
    const title = stack ? `${nameOfEntry(e)} · ${t('desk.inStack', { n: e.items.length })}` : nameOfEntry(e)
    const parts = [`<div class="wmh" style="--c:${w ? WCards.colorOf(w, c) : 'currentColor'}">${glyph}<b>${esc(title)}</b></div>`]
    if (kind === 'size' || kind === 'full') {
      const sizes = G.sizesOfEntry(e, all)
      parts.push(line(t('menu.size'), ['s', 'm', 'l'].map(z => opt('size', z, t(`menu.${z}`), e.size === z, sizes.includes(z) ? '' : noSize(e, z))).join('')))
    }
    if (kind === 'more' || kind === 'full') {
      const notBar = here.home === 'taskbar' ? '' : t('menu.notTaskbar')
      parts.push(line(t('menu.where'), opt('where', 'bar', t('menu.bar'), false, notBar) + opt('where', 'isle', t('menu.isle'), false) + opt('where', 'desk', t('menu.desk'), true)))
      parts.push('<div class="wmsep"></div>')
      if (stack) {
        parts.push(item('takeOut', t('menu.takeOut', { name: nameOfEntry(e) }), '', String(topOf(e))))
        parts.push(item('ungroup', t('menu.ungroup')))
      } else {
        const next = neighbour(e)
        const name = next ? nameOfEntry(next) : ''
        parts.push(next ? item('group', t('menu.stackWith', { name }), '', next.id) : item('group', t('menu.stackWithNone'), t('desk.noNeighbour')))
      }
      parts.push(item('board', t('menu.board'), t('desk.boardLater')))
      if (w) parts.push(item('settings', t('menu.settings')))
      parts.push(item('remove', t('desk.remove')))
    }
    parts.push('<div class="wmwhy" hidden></div>')
    return parts.join('')
  }

  function openMenu(id, kind, at) {
    const e = find(id)
    if (!e || !here) return
    for (const n of nodes.values()) n.el.classList.toggle('menu', n.entry.id === id)
    menu = { id, kind }
    menuEl.innerHTML = menuHTML(e, kind)
    fillHer(menuEl)
    menuEl.hidden = false
    // By the button (under it, its right edge at the button's) or at the
    // pointer; kept inside the display.
    const w = menuEl.offsetWidth
    const h = menuEl.offsetHeight
    let x = at.button ? at.button.right - w : at.x
    let y = at.y
    if (y + h > innerHeight - 8) y = (at.button ? at.button.top - 6 : at.y) - h
    x = Math.max(8, Math.min(x, innerWidth - w - 8))
    y = Math.max(8, Math.min(y, innerHeight - h - 8))
    menuEl.style.left = `${Math.round(x)}px`
    menuEl.style.top = `${Math.round(y)}px`
    shapes()
  }
  function closeMenu() {
    if (!menu) return
    menu = null
    menuEl.hidden = true
    for (const n of nodes.values()) n.el.classList.remove('menu')
    shapes()
  }
  menuEl.addEventListener('mouseover', ev => {
    const why = menuEl.querySelector('.wmwhy')
    const off = ev.target.closest('[data-why]')
    if (!why) return
    why.hidden = !off
    if (off) why.textContent = off.dataset.why
    shapes()
  })
  menuEl.addEventListener('contextmenu', ev => ev.preventDefault())
  menuEl.addEventListener('click', ev => {
    const b = ev.target.closest('[data-act]')
    if (!b || b.classList.contains('off') || !menu) return
    const e = find(menu.id)
    closeMenu()
    if (e) act(e, b.dataset.act, b.dataset.v)
  })

  // Each of its widgets or plugins, as the island's placeWidget takes them.
  const itemsOf = e => G.membersOf(e).map(m => (m.type === 'plugin' ? { plugin: m.of } : { id: m.of }))

  function act(e, what, v) {
    switch (what) {
      case 'size': {
        // The new size where it stands, or the nearest free spot from there.
        const cell = spots.get(e.id) || [0, 0]
        const blocks = others(e.id)
        const at = G.isFree(grid(), cell, v, blocks) ? cell : G.nearestFree(grid(), cell, v, [...blocks, ...icons]) || G.nearestFree(grid(), cell, v, blocks) || cell
        return keepLayout(layout.map(o => (o.id === e.id ? G.placed({ ...o, size: v }, here.key, at, visiting(o)) : o)))
      }
      case 'where':
        if (v === 'desk') return
        // A stack to the taskbar as a stack there; off the desktop by the island's page.
        return ask('place', { items: itemsOf(e), where: v, stack: G.isStack(e) })
      case 'group':
        tops.set(v, 0)
        return keepLayout(G.stackOnto(layout, e.id, v, newId))
      case 'ungroup': {
        const cell = spots.get(e.id)
        const apart = G.ungroup(layout, e.id, newId).map(o => (cell && !layout.some(x => x.id === o.id) ? G.placed(o, here.key, cell) : o))
        tops.delete(e.id)
        return keepLayout(apart)
      }
      case 'takeOut': {
        const cell = spots.get(e.id) || [0, 0]
        const free = G.nearestFree(grid(), cell, e.size, [...others(''), ...icons]) || cell
        tops.set(e.id, 0)
        return keepLayout(G.takeOut(layout, e.id, Number(v) || 0, newId, here.key, free))
      }
      case 'settings': {
        const w = shownWidget(e)
        return w && ask('settings', { widget: w.id })
      }
      case 'remove':
        return ask('place', { items: itemsOf(e), where: 'isle' })
    }
  }

  // What her island's page does for the desktop (desk.rs desk_ask): a
  // widget's settings opened, a widget put elsewhere.
  const ask = (what, args) => invoke('desk_ask', { what, args }).catch(() => {})

  // --- From desk.rs -------------------------------------------------------------------------------
  const on = (name, fn) => listen(name, e => fn(e.payload), { target })
  Promise.all([
    on('desk:state', async s => {
      const first = !here
      here = s
      layout = Array.isArray(s.layout) ? s.layout : []
      document.body.classList.toggle('lw', !!s.light)
      if (first) await loadIcons()
      draw()
    }),
    on('desk:widgets', p => {
      const before = all
      all = Array.isArray(p?.widgets) ? p.widgets : []
      draw()
      news(before)
    }),
    on('desk:paper', p => setPaper(p)),
    // The pointer went off its widgets: the window lets clicks through again.
    on('desk:pointer', () => {
      if (drag) return
      for (const n of nodes.values()) n.el.classList.remove('hover')
    }),
    // A press elsewhere (on the desktop, another window): the menu goes.
    on('desk:outside', () => closeMenu()),
  ])
    .then(() => invoke('desk_ready'))
    .catch(err => console.log('desk: listeners failed', err))


  // The page's console into debug.log (when it is on), and /debug/eval.
  const log = console.log.bind(console)
  console.log = (...args) => {
    log(...args)
    invoke('pet_log', { line: args.map(String).join(' ') }).catch(() => {})
  }
  window.__wkEval = (id, fn) => {
    Promise.resolve()
      .then(fn)
      .catch(err => 'error: ' + err.message)
      .then(value => invoke('debug_result', { id, value: value === undefined ? null : value }))
  }
  window.addEventListener('error', e => console.log('error:', e.message, e.filename, e.lineno))
  // For the tests' eyes: what is drawn where.
  window.Desk = { state: () => ({ here, layout, spots: Object.fromEntries(spots), icons, paper: !!paper, shapes: sentShapes }), draw, turn: (id, by) => nodes.get(id) && turn(nodes.get(id), by), openMenu }
})()
