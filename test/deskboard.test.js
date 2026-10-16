const assert = require('node:assert/strict')
const { test } = require('node:test')

const G = require('../src/pet/deskgrid')

// Boards (工作台, the widget design's §7 and its note of 2026-10-16): a pane
// 688 px across, its title bar 46 px, 14 px round a grid of 8 columns of the
// desktop's 72 px cells, 2 to 4 rows; its widgets flat tiles on that grid.
const WORK = [0, 0, 2048, 1104]
const here = { key: 'd1', primary: true, keys: ['d1'] }
const w = (id, size, extra = {}) => ({ id, type: 'widget', of: id.slice(1), size, ...extra })
const tile = (of, size, at, extra = {}) => ({ type: 'widget', of, size, at, ...extra })
const board = (id, items = [], extra = {}) => ({ id, type: 'board', items, ...extra })
let n = 0
const newId = () => `wn${++n}`

test('a board: 688 px across, its rows as its tiles need, 2 to 4; folded, its title bar alone', () => {
  assert.equal(G.BOARD.width, 688)
  assert.deepEqual(G.boardBox(board('wb')), { w: 688, h: 46 + 156 + 14 })
  assert.equal(G.boardRows(board('wb', [tile('a', 's', [0, 0])])), 2)
  assert.equal(G.boardRows(board('wb', [tile('a', 'l', [4, 0])])), 4)
  assert.equal(G.boardRows(board('wb', [tile('a', 's', [0, 1])])), 3)
  assert.deepEqual(G.boardBox(board('wb', [tile('a', 'l', [0, 0])])), { w: 688, h: 46 + 324 + 14 })
  assert.deepEqual(G.boardBox(board('wb', [tile('a', 'l', [0, 0])], { folded: true })), { w: 688, h: 46 })
  // On the desktop's grid it spans 9 columns (688 > 8 cells' 660) and 3 rows (216 px).
  assert.deepEqual(G.spanOf(G.boardBox(board('wb'))), [9, 3])
  assert.deepEqual(G.spanOf({ w: 688, h: 46 }), [9, 1])
  // Its tiles where the board's grid has them: 14 px in, under its title bar.
  assert.deepEqual(G.tileRect([2, 2], 'm'), { x: 14 + 168, y: 46 + 168, w: 324, h: 156 })
})

test('on the desktop a board takes its box; grown, it keeps its place and the others make way', () => {
  const g = G.gridOf(WORK)
  const b = board('wb', [tile('a', 'm', [0, 0])], { d: 'd1', at: { d1: [2, 1] } })
  const single = w('wx', 's', { d: 'd1', at: { d1: [2, 4] } })
  // Two rows: the board reaches down to 24 + 84 + 216; the one below it is clear.
  let s = G.settle([single, b], here, g)
  assert.deepEqual(s.at.get('wb'), [2, 1])
  assert.deepEqual(s.at.get('wx'), [2, 4])
  assert.deepEqual(s.moved, [])
  // Grown to four rows (a large tile): the board stays, the one under it moves.
  const grown = { ...b, items: [...b.items, tile('c', 'l', [4, 0])] }
  s = G.settle([single, grown], here, g)
  assert.deepEqual(s.at.get('wb'), [2, 1])
  assert.notDeepEqual(s.at.get('wx'), [2, 4])
  assert.deepEqual(s.moved, ['wx'])
  // Folded, it is its title bar alone: what stood under it may.
  assert.deepEqual(G.settle([single, { ...grown, folded: true }], here, g).moved, [])
  // A new board goes top right, whole on the grid.
  const fresh = G.settle([board('wn')], here, g).at.get('wn')
  assert.deepEqual(fresh, [g.cols - 9, 0])
  assert.ok(G.fits(g, fresh, G.boardBox(board('wn'))))
})

test('let go of over a board: the nearest empty cell; none, a row more, up to four; full, none', () => {
  // Empty: where it was let go of.
  assert.deepEqual(G.boardSpot(board('wb'), [3, 0], 's'), [3, 0])
  assert.deepEqual(G.boardSpot(board('wb'), [6, 1], 'm'), [4, 0])
  // Its two rows full of mediums: a small goes into a third and fourth row.
  const two = board('wb', [tile('a', 'm', [0, 0]), tile('b', 'm', [4, 0])])
  assert.deepEqual(G.boardSpot(two, [5, 0], 's'), [5, 2])
  // Room in the rows it has: there, not a row more.
  const gap = board('wb', [tile('a', 'm', [0, 0]), tile('b', 's', [4, 0])])
  assert.deepEqual(G.boardSpot(gap, [0, 3], 's'), [6, 0])
  // Four rows full: none (it shakes, the widget goes back).
  const full = board('wb', [tile('a', 'm', [0, 0]), tile('b', 'm', [4, 0]), tile('c', 'm', [0, 2]), tile('d', 'm', [4, 2])])
  assert.equal(G.boardSpot(full, [0, 0], 's'), null)
  // A large needs four rows of its four columns.
  assert.deepEqual(G.boardSpot(two, [0, 0], 'l'), null)
  assert.deepEqual(G.boardSpot(board('wb', [tile('a', 'm', [0, 0])]), [0, 0], 'l'), [4, 0])
  // Moving one of its own: its own cells are free to it.
  assert.deepEqual(G.boardSpot(two, [4, 0], 'm', 1), [4, 0])
  assert.deepEqual(G.boardSpot(two, [2, 1], 'm', 1), [4, 0])
  // Sixteen tiles at most.
  assert.equal(G.boardSpot(board('wb', Array.from({ length: 16 }, (_, i) => tile(`t${i}`, 's', [0, 0]))), [0, 0], 's'), null)
})

test('its Add puts one in the first empty cell, in reading order', () => {
  assert.deepEqual(G.boardFirst(board('wb'), 'm'), [0, 0])
  assert.deepEqual(G.boardFirst(board('wb', [tile('a', 's', [0, 0])]), 'm'), [2, 0])
  assert.deepEqual(G.boardFirst(board('wb', [tile('a', 'm', [2, 0])]), 's'), [0, 0])
  assert.deepEqual(G.boardFirst(board('wb', [tile('a', 'm', [0, 0]), tile('b', 'm', [4, 0])]), 's'), [0, 2])
  assert.equal(G.boardFirst(board('wb', [tile('a', 'l', [0, 0]), tile('b', 'l', [4, 0])]), 's'), null)
})

test('a tile let go of snaps to the board cell under its corner', () => {
  assert.deepEqual(G.tileAt(14, 46, 's'), [0, 0])
  assert.deepEqual(G.tileAt(14 + 84 * 3 + 40, 46 + 84 - 30, 's'), [3, 1])
  assert.deepEqual(G.tileAt(9999, 9999, 'm'), [4, 2])
  assert.deepEqual(G.tileAt(-99, -99, 'l'), [0, 0])
})

test('into a board and out again: off wherever it was; alone where it was let go of', () => {
  const layout = [
    w('wa', 's', { d: 'd1', at: { d1: [0, 0] } }),
    { id: 'ws', type: 'stack', size: 'm', d: 'd1', items: [{ type: 'widget', of: 'b' }, { type: 'plugin', of: 'stock' }] },
    board('wb', [tile('c', 's', [0, 0])], { d: 'd1', at: { d1: [5, 0] } }),
  ]
  // The one alone, dragged in: its entry gone, a tile on the cell.
  let next = G.intoBoard(layout, 'wb', G.memberOf(layout[0]), 's', [2, 0], newId)
  assert.deepEqual(
    next.map(e => e.id),
    ['ws', 'wb'],
  )
  assert.deepEqual(next[1].items[1], { type: 'widget', of: 'a', size: 's', at: [2, 0] })
  // One of a stack, from the library: out of its stack (one left: alone).
  next = G.intoBoard(next, 'wb', { type: 'plugin', of: 'stock' }, 'm', [4, 0], newId)
  assert.deepEqual(next[0], { id: 'wn1', type: 'widget', of: 'b', size: 'm', d: 'd1' })
  assert.deepEqual(next[1].items.map(G.keyOf), ['widget:c', 'widget:a', 'plugin:stock'])
  // Moved in it, then another size there.
  next = G.moveInBoard(next, 'wb', 1, [0, 2])
  assert.deepEqual(next[1].items[1].at, [0, 2])
  next = G.moveInBoard(next, 'wb', 1, [0, 2], 'm')
  assert.equal(next[1].items[1].size, 'm')
  // Taken out: alone on its cell of the display, the board without it.
  next = G.outOfBoard(next, 'wb', 0, newId, 'd1', [3, 6])
  assert.deepEqual(next[1].items.map(G.keyOf), ['widget:a', 'plugin:stock'])
  assert.deepEqual(next[2], { id: 'wn2', type: 'widget', of: 'c', size: 's', d: 'd1', at: { d1: [3, 6] } })
  assert.ok(G.isLayout(next), JSON.stringify(next))
})

test('a new board from one alone, where it stood; never from a stack; apart again', () => {
  const layout = [w('wa', 'm', { d: 'd1', at: { d1: [4, 2] } }), { id: 'ws', type: 'stack', size: 's', items: [{ type: 'widget', of: 'b' }, { type: 'widget', of: 'c' }] }]
  const made = G.newBoard(layout, 'wa', newId)
  assert.deepEqual(made[0], { id: made[0].id, type: 'board', items: [{ type: 'widget', of: 'a', size: 'm', at: [0, 0] }], d: 'd1', at: { d1: [4, 2] } })
  assert.equal(G.newBoard(layout, 'ws', newId), layout)
  const b = { ...made[0], items: [...made[0].items, tile('x', 's', [4, 0])] }
  const apart = G.boardApart([b, layout[1]], b.id, newId)
  assert.deepEqual(
    apart.map(e => [e.type, e.of, e.size, e.at]),
    [
      ['widget', 'a', 'm', { d1: [4, 2] }],
      ['widget', 'x', 's', undefined],
      ['stack', undefined, 's', undefined],
    ],
  )
})

test('boards in a layout: as settings.rs checks them; what they show; off the desktop', () => {
  const b = board('wb', [tile('a', 'm', [0, 0]), tile('stock', 's', [4, 0], { type: 'plugin' })], { name: '行情', folded: false, d: 'd1', at: { d1: [1, 1] } })
  assert.ok(G.isLayout([b, w('wz', 's')]))
  assert.ok(G.isLayout([board('wb')]))
  const bad = [
    [board('wb', [tile('a', 's', [0, 0]), tile('b', 's', [1, 0])])], // they meet
    [board('wb', [tile('a', 'm', [6, 0])])], // past its eighth column
    [board('wb', [tile('a', 's', [0, 3])])], // past its fourth row
    [board('wb', [tile('a', 'xl', [0, 0])])],
    [board('wb', [{ type: 'widget', of: 'a', size: 's' }])],
    [board('wb', [], { name: '一二三四五六七八九十一二三四五六七' })],
    [board('wb', [], { name: 'a\nb' })],
    [board('wb', [], { folded: 1 })],
    [board('wb', [], { size: 's' })],
    [board('wb', [tile('a', 's', [0, 0]), tile('a', 's', [2, 0])])],
    [board('wb', [tile('a', 's', [0, 0])]), w('wa', 's')], // in a board and alone
    [board('wb', [{ id: 'ws', type: 'stack', size: 's', items: [] }])],
    [board('wb', Array.from({ length: 17 }, (_, i) => tile(`t${i}`, 's', [(i % 4) * 2, 0])))],
  ]
  for (const x of bad) assert.ok(!G.isLayout(x), JSON.stringify(x))
  // What it shows, and off it: the board stays, without that one.
  assert.ok(G.has([b], { id: 'a' }))
  assert.ok(G.has([b], { plugin: 'stock' }))
  const all = [{ id: 'a' }, { id: 'stock-aapl', plugin: 'stock' }, { id: 'q' }]
  assert.deepEqual([...G.shownOn([b], all)].sort(), ['a', 'stock-aapl'])
  const left = G.without([b], { plugin: 'stock' }, newId)
  assert.deepEqual(left[0].items.map(G.keyOf), ['widget:a'])
  assert.deepEqual(G.without(left, { id: 'a' }, newId)[0].items, [])
  // Never stacked, nor stacked onto.
  assert.ok(!G.canStack(b, w('wq', 's')))
  assert.ok(!G.canStack(w('wq', 's'), b))
  assert.ok(G.isBoardName('工作'))
  assert.ok(!G.isBoardName('x'.repeat(17)))
})
