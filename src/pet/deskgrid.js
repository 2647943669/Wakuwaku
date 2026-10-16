// The desktop's widgets as a layout (the widget design of 2026-10-16, §3,
// §7, §10): where each one stands on a display's grid, which display shows
// it, and several stacked into one. Plain data in, plain data out, for
// desk.js to draw and the tests to try; desk.rs keeps the layout
// (deskLayout) and settings.rs checks it the same way (isEntry).
//
// The grid: cells of 72 px with 12 px gutters (a pitch of 84), from 24 px
// inside the display's work area; small 2 x 2 cells (156 x 156), medium
// 4 x 2 (324 x 156), large 4 x 4 (324 x 324), all in the display's CSS px.
//
// An entry: { id, type: 'widget' | 'plugin', of, size: 's' | 'm' | 'l',
// name?, icon?, d?, at? } alone, or { id, type: 'stack', items, size, d?,
// at? } several in one, 2 to 8, each { type, of, name?, icon? } as the
// taskbar's groups have them (widgets.js isMember). d: the display it is on
// (desk.rs key_of); at: where it stood on each display it has been on,
// { key: [col, row] }, so each keeps its own (the coordinator's "positions
// remembered per display").
;(function (root) {
  const Widgets = root.Widgets || require('./widgets')

  const CELL = 72
  const GAP = 12
  const PITCH = CELL + GAP
  const EDGE = 24
  // Cells across and down, by size.
  const SIZES = { s: [2, 2], m: [4, 2], l: [4, 4] }
  const MAX_ENTRIES = 32
  const MAX_MEMBERS = 8
  const MAX_AT = 8

  const cellsOf = size => SIZES[size] || SIZES.s
  const pxOf = n => n * PITCH - GAP
  const sizePx = size => ({ w: pxOf(cellsOf(size)[0]), h: pxOf(cellsOf(size)[1]) })

  // --- The grid ------------------------------------------------------------------------
  // A display's grid from its work area [x, y, w, h] (the window's CSS px):
  // its first cell 24 px in, as many whole cells as fit with 24 px to spare
  // at the far side too.
  function gridOf(work) {
    const [x, y, w, h] = Array.isArray(work) && work.length === 4 ? work : [0, 0, 0, 0]
    const fit = n => Math.max(0, Math.floor((n - 2 * EDGE + GAP) / PITCH))
    return { x0: x + EDGE, y0: y + EDGE, cols: fit(w), rows: fit(h) }
  }

  // Where a cell is, and a widget of a size standing on it.
  const rectAt = (grid, [c, r], size) => ({ x: grid.x0 + c * PITCH, y: grid.y0 + r * PITCH, ...sizePx(size) })

  // Whether a widget of a size on this cell is on the grid, whole.
  function fits(grid, [c, r], size) {
    const [cw, ch] = cellsOf(size)
    return Number.isInteger(c) && Number.isInteger(r) && c >= 0 && r >= 0 && c + cw <= grid.cols && r + ch <= grid.rows
  }

  // The cell nearest a top-left corner (px), kept on the grid.
  function snap(grid, x, y, size) {
    const [cw, ch] = cellsOf(size)
    const c = Math.round((x - grid.x0) / PITCH)
    const r = Math.round((y - grid.y0) / PITCH)
    return [Math.max(0, Math.min(Math.max(0, grid.cols - cw), c)), Math.max(0, Math.min(Math.max(0, grid.rows - ch), r))]
  }

  // Two rects meet (sharing an edge is not meeting).
  const overlaps = (a, b) => a.x < b.x + b.w && b.x < a.x + a.w && a.y < b.y + b.h && b.y < a.y + a.h

  // Free: on the grid, over none of what is there (other widgets, icons).
  const isFree = (grid, cell, size, blocks) => fits(grid, cell, size) && !blocks.some(b => overlaps(rectAt(grid, cell, size), b))

  // The free cell nearest one (by the distance between the two places'
  // corners; the same distance, the higher, then the more to the left);
  // null when there is none.
  function nearestFree(grid, [c0, r0], size, blocks) {
    const [cw, ch] = cellsOf(size)
    let best = null
    for (let r = 0; r + ch <= grid.rows; r++)
      for (let c = 0; c + cw <= grid.cols; c++) {
        const d = (c - c0) ** 2 + (r - r0) ** 2
        if (best && (d > best.d || (d === best.d && (r > best.r || (r === best.r && c >= best.c))))) continue
        if (isFree(grid, [c, r], size, blocks)) best = { c, r, d }
      }
    return best ? [best.c, best.r] : null
  }

  // Where a new one goes first: the top right, as the desktop's icons start
  // at the top left.
  const firstSpot = (grid, size) => [Math.max(0, grid.cols - cellsOf(size)[0]), 0]

  // Let go of with its top-left here (px): the cell it snaps to, or, that
  // over another or over icons, the nearest free one; with nothing free,
  // where it snapped (over icons sooner than over another widget).
  function dropAt(grid, x, y, size, widgets, icons) {
    const want = snap(grid, x, y, size)
    return nearestFree(grid, want, size, [...widgets, ...icons]) || nearestFree(grid, want, size, widgets) || want
  }

  // --- Which display shows what ---------------------------------------------------------
  // Those this display shows: its own, and on the main display those whose
  // display is not there (gone, or none said yet).
  function shownHere(layout, here) {
    const keys = here.keys || []
    return (Array.isArray(layout) ? layout : []).filter(e => e.d === here.key || (here.primary && (!e.d || !keys.includes(e.d))))
  }

  // Where each of them stands here, in the layout's order: where it stood
  // last on this display, unless another took it first or it is off the
  // grid now (the work area changed), or, new on the display (fresh: those
  // not drawn here before, the page just up or one come from another
  // display), it lies over icons; else, and for one never here, the nearest
  // free cell from there (or the top right), clear of the icons. Gives
  // { at: Map(id -> [c, r]), moved: [ids] }: those moved, to keep.
  function settle(entries, here, grid, icons = [], fresh = null) {
    const at = new Map()
    const moved = []
    const taken = []
    for (const e of entries) {
      const size = e.size
      const was = Array.isArray(e.at?.[here.key]) ? e.at[here.key] : null
      const clear = fresh && fresh.has(e.id) ? [...taken, ...icons] : taken
      let cell = was && isFree(grid, was, size, clear) ? was : null
      if (!cell) {
        const from = was ? snap(grid, rectAt(grid, was, size).x, rectAt(grid, was, size).y, size) : firstSpot(grid, size)
        cell = nearestFree(grid, from, size, [...taken, ...icons]) || nearestFree(grid, from, size, taken) || from
        moved.push(e.id)
      }
      at.set(e.id, cell)
      taken.push(rectAt(grid, cell, size))
    }
    return { at, moved }
  }

  // An entry standing on a cell of a display (and on it now). visiting:
  // shown on the main display while its own is not there, it stays its
  // own display's, to go back there once it is (its place here kept too).
  const placed = (e, key, cell, visiting = false) => ({ ...e, ...(visiting && e.d ? {} : { d: key }), at: keepAt({ ...(e.at || {}), [key]: cell }, key) })
  // Eight displays remembered at most: the one it is on kept, the oldest gone.
  function keepAt(at, key) {
    const keys = Object.keys(at)
    if (keys.length <= MAX_AT) return at
    const drop = keys.filter(k => k !== key).slice(0, keys.length - MAX_AT)
    return Object.fromEntries(keys.filter(k => !drop.includes(k)).map(k => [k, at[k]]))
  }

  // --- Several in one: a stack (§7) ------------------------------------------------------
  const isStack = e => e?.type === 'stack'
  const memberOf = e => ({ type: e.type, of: e.of, ...(e.name ? { name: e.name } : {}), ...(e.icon ? { icon: e.icon } : {}) })
  const membersOf = e => (isStack(e) ? e.items : e && (e.type === 'widget' || e.type === 'plugin') ? [memberOf(e)] : [])
  const keyOf = m => `${m.type}:${m.of}`

  // The sizes a member has: its widgets' (a plugin's, the first's); one
  // with nothing to say yet, any.
  function sizesOfMember(m, all) {
    const ws = Widgets.sourcesOf(m, all)
    return ws.length ? Widgets.sizesOf(ws[0]) : ['s', 'm', 'l']
  }
  // An entry's: those all its members have.
  const sizesOfEntry = (e, all) => membersOf(e).reduce((acc, m) => acc.filter(z => sizesOfMember(m, all).includes(z)), ['s', 'm', 'l'])

  // Whether one may be stacked onto another: the same size, not itself,
  // eight at most in all once none is there twice.
  function canStack(from, onto) {
    if (!from || !onto || from.id === onto.id || from.size !== onto.size) return false
    const keys = new Set(membersOf(onto).map(keyOf))
    const more = membersOf(from).filter(m => !keys.has(keyOf(m)))
    return more.length > 0 && keys.size + more.length <= MAX_MEMBERS
  }

  // One dropped onto another (held there half a second): a stack where the
  // other stood, the one dropped on top (first); one stack onto another,
  // all of its members. The one dropped gone from the layout. Unchanged
  // when they may not stack.
  function stackOnto(layout, fromId, ontoId, newId) {
    const from = layout.find(e => e.id === fromId)
    const onto = layout.find(e => e.id === ontoId)
    if (!canStack(from, onto)) return layout
    const keys = new Set(membersOf(onto).map(keyOf))
    const items = [...membersOf(from).filter(m => !keys.has(keyOf(m))), ...membersOf(onto)]
    const stack = { id: isStack(onto) ? onto.id : newId(), type: 'stack', items, size: onto.size, ...(onto.d ? { d: onto.d } : {}), ...(onto.at ? { at: onto.at } : {}) }
    return layout.filter(e => e.id !== fromId).map(e => (e.id === ontoId ? stack : e))
  }

  // One member of a stack taken out (dragged out of it): alone, where it
  // was let go of; the stack without it (one left, that one alone, where
  // the stack stood).
  function takeOut(layout, stackId, index, newId, key, cell) {
    const s = layout.find(e => e.id === stackId)
    if (!isStack(s) || !s.items[index]) return layout
    const items = s.items.filter((_, i) => i !== index)
    const where = { ...(s.d ? { d: s.d } : {}), ...(s.at ? { at: s.at } : {}) }
    const rest = items.length === 1 ? { id: newId(), ...items[0], size: s.size, ...where } : { ...s, items }
    const out = placed({ id: newId(), ...s.items[index], size: s.size, ...where }, key, cell)
    return layout.flatMap(e => (e.id === stackId ? [rest, out] : [e]))
  }

  // A stack taken apart: each alone, the first where it stood, the others
  // where the display finds room (settle).
  function ungroup(layout, stackId, newId) {
    const s = layout.find(e => e.id === stackId)
    if (!isStack(s)) return layout
    const where = { ...(s.d ? { d: s.d } : {}) }
    const alone = s.items.map((m, i) => ({ id: newId(), ...m, size: s.size, ...where, ...(i === 0 && s.at ? { at: s.at } : {}) }))
    return layout.flatMap(e => (e.id === stackId ? alone : [e]))
  }

  // Turned by the wheel, a drag or a dot: the next, the one before, or one.
  const flip = (top, n, by) => (n ? (((top + by) % n) + n) % n : 0)

  // A member with news comes to the top, once (§7): a letter came (its
  // unread grew, widgets.js newsIn). Its index, or -1.
  function newsTop(stack, before, after) {
    if (!isStack(stack)) return -1
    const was = new Map((before || []).map(w => [w.id, w]))
    return stack.items.findIndex(m => Widgets.sourcesOf(m, after || []).some(w => Widgets.newsIn(was.get(w.id), w)))
  }

  // --- Kept as settings.rs checks it ------------------------------------------------------
  const isKey = k => typeof k === 'string' && /^[a-z0-9]{1,40}$/.test(k)
  const isCell = v => Array.isArray(v) && v.length === 2 && v.every(n => Number.isInteger(n) && n >= 0 && n <= 255)
  function isEntry(e) {
    if (!e || typeof e !== 'object' || Array.isArray(e)) return false
    if (typeof e.id !== 'string' || !/^w[a-z0-9]{1,15}$/.test(e.id)) return false
    if (!['s', 'm', 'l'].includes(e.size)) return false
    if (e.d !== undefined && !isKey(e.d)) return false
    if (e.at !== undefined && !(e.at && typeof e.at === 'object' && !Array.isArray(e.at) && Object.keys(e.at).length <= MAX_AT && Object.entries(e.at).every(([k, v]) => isKey(k) && isCell(v)))) return false
    const known = ['id', 'type', 'of', 'size', 'name', 'icon', 'd', 'at', 'items']
    if (!Object.keys(e).every(k => known.includes(k))) return false
    if (isStack(e)) {
      return e.of === undefined && e.name === undefined && e.icon === undefined && Array.isArray(e.items) && e.items.length >= 2 && e.items.length <= MAX_MEMBERS && e.items.every(Widgets.isMember) && new Set(e.items.map(keyOf)).size === e.items.length
    }
    return (e.type === 'widget' || e.type === 'plugin') && e.items === undefined && Widgets.isMember(memberOf(e))
  }
  // A layout: 32 at most, each id once, each widget or plugin once.
  function isLayout(layout) {
    if (!Array.isArray(layout) || layout.length > MAX_ENTRIES || !layout.every(isEntry)) return false
    const ids = layout.map(e => e.id)
    const keys = layout.flatMap(membersOf).map(keyOf)
    return new Set(ids).size === ids.length && new Set(keys).size === keys.length
  }

  // Whether the desktop shows a widget ({ id }) or a plugin ({ plugin }).
  function has(layout, item) {
    const key = item.plugin ? `plugin:${item.plugin}` : `widget:${item.id}`
    return (Array.isArray(layout) ? layout : []).some(e => membersOf(e).some(m => keyOf(m) === key))
  }
  // The widgets it shows (not to take their turns in the island too).
  function shownOn(layout, all) {
    const ids = new Set()
    for (const e of Array.isArray(layout) ? layout : []) for (const m of membersOf(e)) for (const w of Widgets.sourcesOf(m, all)) ids.add(w.id)
    return ids
  }
  // A widget or plugin off the desktop: its entry gone, or it out of its stack.
  function without(layout, item, newId) {
    const key = item.plugin ? `plugin:${item.plugin}` : `widget:${item.id}`
    return (Array.isArray(layout) ? layout : []).flatMap(e => {
      if (!isStack(e)) return membersOf(e).some(m => keyOf(m) === key) ? [] : [e]
      const items = e.items.filter(m => keyOf(m) !== key)
      if (items.length === e.items.length) return [e]
      if (items.length === 1) return [{ id: newId(), ...items[0], size: e.size, ...(e.d ? { d: e.d } : {}), ...(e.at ? { at: e.at } : {}) }]
      return [{ ...e, items }]
    })
  }

  const api = {
    CELL,
    GAP,
    PITCH,
    EDGE,
    SIZES,
    sizePx,
    gridOf,
    rectAt,
    fits,
    snap,
    overlaps,
    isFree,
    nearestFree,
    firstSpot,
    dropAt,
    shownHere,
    settle,
    placed,
    isStack,
    membersOf,
    memberOf,
    keyOf,
    sizesOfEntry,
    canStack,
    stackOnto,
    takeOut,
    ungroup,
    flip,
    newsTop,
    isEntry,
    isLayout,
    has,
    shownOn,
    without,
  }

  if (typeof module !== 'undefined' && module.exports) {
    module.exports = api
  } else {
    root.DeskGrid = api
  }
})(this)
