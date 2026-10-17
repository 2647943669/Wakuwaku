const assert = require('node:assert/strict')
const { test } = require('node:test')

const Flow = require('../src/pet/flow')

const near = (a, b, within, what) => assert.ok(Math.abs(a - b) <= within, `${what}: ${a} not within ${within} of ${b}`)

test('only a session waiting on the person opens the island; the rest ring', () => {
  assert.equal(Flow.cueFor('waiting', {}), 'open')
  for (const kind of ['done', 'review', 'error', 'hello', 'plugin']) assert.equal(Flow.cueFor(kind, {}), 'ring', kind)
  // A new version, a crash report: never urgent, the dot alone.
  assert.equal(Flow.cueFor('notice', {}), 'dot')
})

test('the setting decides all that needs no answer: open, a ring, or just the dot', () => {
  for (const kind of ['done', 'review', 'error', 'hello', 'plugin']) {
    assert.equal(Flow.cueFor(kind, { setting: 'open' }), 'open', kind)
    assert.equal(Flow.cueFor(kind, { setting: 'ring' }), 'ring', kind)
    assert.equal(Flow.cueFor(kind, { setting: 'none' }), 'dot', kind)
  }
  // Waiting opens whatever it says.
  for (const setting of ['open', 'ring', 'none']) assert.equal(Flow.cueFor('waiting', { setting }), 'open')
  // Open is all as before: a notice and what happens inside a turn open it too.
  assert.equal(Flow.cueFor('notice', { setting: 'open' }), 'open')
  assert.equal(Flow.cueFor('process', { setting: 'open' }), 'open')
  assert.equal(Flow.cueFor('notice', { setting: 'none' }), 'dot')
})

test('inside a turn (a tool failed, an item done): her clip alone, never a ring or a dot', () => {
  for (const setting of ['ring', 'none']) {
    for (const view of ['compact', 'expanded', 'ask', 'settings']) assert.equal(Flow.cueFor('process', { setting, view }), 'clip', `${setting} ${view}`)
    assert.equal(Flow.cueFor('process', { setting, blocked: true }), 'clip')
  }
})

test('the setting from before it was a choice: openOnEnd off is nothing, else the ring', () => {
  assert.equal(Flow.endCue({}), 'ring')
  assert.equal(Flow.endCue({ openOnEnd: true }), 'ring')
  assert.equal(Flow.endCue({ openOnEnd: false }), 'none')
  assert.equal(Flow.endCue({ endCue: 'open', openOnEnd: false }), 'open')
  assert.equal(Flow.endCue({ endCue: 'spin' }), 'ring')
  assert.equal(Flow.endCue(null), 'ring')
})

test('full screen, nobody about, do not disturb: no ring, the dot alone', () => {
  for (const kind of ['done', 'review', 'error', 'hello', 'plugin']) assert.equal(Flow.cueFor(kind, { blocked: true }), 'dot', kind)
  // Waiting still opens; the setting's open still opens.
  assert.equal(Flow.cueFor('waiting', { blocked: true }), 'open')
  assert.equal(Flow.cueFor('done', { setting: 'open', blocked: true }), 'open')
})

test('the island open or the settings up: neither ring nor dot', () => {
  for (const kind of ['done', 'review', 'error']) {
    assert.equal(Flow.cueFor(kind, { view: 'expanded' }), null, kind)
    assert.equal(Flow.cueFor(kind, { view: 'settings' }), null, kind)
    assert.equal(Flow.cueFor(kind, { view: 'ask' }), null, kind)
  }
  // A hello or a plugin's words go into the open island; behind a prompt or
  // the settings, they wait for the next look.
  for (const kind of ['hello', 'plugin', 'notice']) {
    assert.equal(Flow.cueFor(kind, { view: 'expanded' }), 'show', kind)
    assert.equal(Flow.cueFor(kind, { view: 'settings' }), 'later', kind)
    assert.equal(Flow.cueFor(kind, { view: 'ask', blocked: true }), 'later', kind)
  }
})

test('of her reactions only a hello rings; a task done or a tool failed is inside the turn', () => {
  assert.equal(Flow.kindOfReact('wave'), 'hello')
  assert.equal(Flow.kindOfReact('jump'), 'process')
  assert.equal(Flow.kindOfReact('failed'), 'process')
  assert.equal(Flow.kindOfReact('other'), 'process')
})

test('what the island would have shown stays as long as an ending does; a notice until seen', () => {
  assert.equal(Flow.keepFor('hello', 8), 8000)
  assert.equal(Flow.keepFor('plugin', 120), 120000)
  assert.equal(Flow.keepFor('hello', 'seen'), Infinity)
  assert.equal(Flow.keepFor('notice', 8), Infinity)
  // A custom hold as long as it says (#59).
  assert.equal(Flow.keepFor('hello', 45), 45000)
  assert.equal(Flow.keepFor('plugin', 600), 600000)
})

test("Custom's seconds as typed: whole, within 3 to 600; no number is none (#59)", () => {
  assert.equal(Flow.holdSecs('45'), 45)
  assert.equal(Flow.holdSecs(' 45 '), 45)
  assert.equal(Flow.holdSecs('3'), 3)
  assert.equal(Flow.holdSecs('600'), 600)
  assert.equal(Flow.holdSecs('2'), 3)
  assert.equal(Flow.holdSecs('0'), 3)
  assert.equal(Flow.holdSecs('-8'), 3)
  assert.equal(Flow.holdSecs('601'), 600)
  assert.equal(Flow.holdSecs('99999'), 600)
  assert.equal(Flow.holdSecs('7.5'), 8)
  assert.equal(Flow.holdSecs('7.4'), 7)
  // An input method's full-width digits; a unit after them.
  assert.equal(Flow.holdSecs('４５'), 45)
  assert.equal(Flow.holdSecs('45秒'), 45)
  assert.equal(Flow.holdSecs('45 s'), 45)
  assert.equal(Flow.holdSecs('90 seconds'), 90)
  for (const none of ['', '   ', 'x', '4x5', '秒', 'Infinity', null, undefined]) assert.equal(Flow.holdSecs(none), null, String(none))
})

test('which hold choice is on: until seen, a quick one, or Custom for any other number (#59)', () => {
  assert.equal(Flow.holdChoice('seen'), 'seen')
  for (const secs of Flow.HOLD_QUICK) assert.equal(Flow.holdChoice(secs), secs)
  assert.deepEqual(Flow.HOLD_QUICK, [8, 30, 120])
  assert.equal(Flow.holdChoice(45), 'custom')
  assert.equal(Flow.holdChoice(3), 'custom')
  assert.equal(Flow.holdChoice(600), 'custom')
  // Custom picked, its box open before anything is typed: on, whatever is kept.
  assert.equal(Flow.holdChoice(8, true), 'custom')
  assert.equal(Flow.holdChoice('seen', true), 'custom')
  // No whole number is until seen, as main reads it (main.rs hold).
  for (const odd of [undefined, null, 'x', '8', 7.5, -8]) assert.equal(Flow.holdChoice(odd), 'seen', String(odd))
  assert.equal(Flow.HOLD_MIN, 3)
  assert.equal(Flow.HOLD_MAX, 600)
})

test('a ring: three turns over 2.55 s on its curve, lit 2.8 s, the dot as it fades', () => {
  const { act, ring } = Flow.play(Flow.idle(), 'done', 1000)
  assert.equal(act, 'start')
  assert.equal(ring.turns, 3)
  assert.equal(Flow.angleAt(ring, 1000), 0)
  assert.equal(Flow.angleAt(ring, 1000 + 2550), 1080)
  // Symmetric: half way round at half time; slow at the ends, fast between.
  near(Flow.angleAt(ring, 1000 + 1275), 540, 0.5, 'half way')
  assert.ok(Flow.speedAt(ring, 1100) < Flow.speedAt(ring, 2275) / 3)
  assert.ok(Flow.speedAt(ring, 3500) < Flow.speedAt(ring, 2275) / 3)
  assert.equal(Flow.lightAt(ring, 1000), 0)
  assert.equal(Flow.lightAt(ring, 1000 + 280), 1)
  assert.equal(Flow.lightAt(ring, 1000 + 2240), 1)
  near(Flow.lightAt(ring, 1000 + 2520), 0.5, 1e-9, 'half faded')
  assert.equal(Flow.lightAt(ring, 1000 + 2800), 0)
  assert.equal(ring.dotAt, 1000 + 2240)
  assert.equal(ring.until, 1000 + 2800)
  // As the page animates it: the turning begun, the light as keyframes.
  const f = Flow.frames(ring, 1500)
  assert.deepEqual([f.turn.from, f.turn.to, f.turn.delay, f.turn.duration, f.turn.easing], [0, 1080, -500, 2550, 'cubic-bezier(0.45, 0.05, 0.55, 0.95)'])
  assert.deepEqual(
    f.light.keyframes.map(k => [+k.offset.toFixed(3), k.opacity]),
    [
      [0, 0],
      [0.1, 1],
      [0.8, 1],
      [1, 0],
    ],
  )
  assert.deepEqual([f.light.delay, f.light.duration, f.dotIn, f.end], [-500, 2800, 1740, 2300])
})

test('another while it plays takes its colour and turns on to 1.5 turns of it, never past 4.5', () => {
  const first = Flow.play(Flow.idle(), 'done', 0)
  // Early on, there are turns enough left: only the colour changes.
  const early = Flow.play(first.state, 'error', 600)
  assert.deepEqual([early.act, early.retimed, early.ring.kind, early.ring.turns, early.ring.until], ['blend', false, 'error', 3, 2800])
  // Late, it turns on: 1.5 turns more from where it is, carried on at the
  // speed it had, no jump in the angle.
  const late = Flow.play(first.state, 'review', 2000)
  assert.deepEqual([late.act, late.retimed, late.ring.kind], ['blend', true, 'review'])
  const at = Flow.angleAt(first.ring, 2000)
  near(late.ring.turns, at / 360 + 1.5, 1e-9, 'turns')
  near(Flow.angleAt(late.ring, 2000), at, 1e-6, 'angle carried on')
  near(Flow.speedAt(late.ring, 2000.01), Flow.speedAt(first.ring, 2000), 0.01, 'speed carried on')
  assert.equal(Flow.angleAt(late.ring, late.ring.until), late.ring.turns * 360)
  // Its light held until the new end, the dot later with it.
  assert.equal(Flow.lightAt(late.ring, 2600), 1)
  assert.equal(late.ring.dotAt, late.ring.until - 250 - 310)
  // It never turns backwards.
  let was = -1
  for (let t = 0; t <= late.ring.until; t += 10) {
    const a = Flow.angleAt(late.ring, t)
    assert.ok(a >= was - 1e-6, `backwards at ${t}`)
    was = a
  }
  // Come while it fades: lit again over 0.3 s, then on.
  const fading = Flow.play(first.state, 'error', 2600)
  assert.ok(Flow.lightAt(fading.ring, 2600) < 1)
  assert.equal(Flow.lightAt(fading.ring, 2900), 1)
  assert.equal(fading.ring.turns, 4.5)
  // Once at 4.5, a third only changes the colour; never more than 4.5 turns
  // (about 3.8 s of turning).
  const third = Flow.play(fading.state, 'error', 3200)
  assert.deepEqual([third.act, third.retimed, third.ring.turns], ['blend', false, 4.5])
  const capped = Flow.play(late.state, 'error', 3300)
  assert.ok(capped.ring.turns <= 4.5)
  assert.ok(capped.ring.turnings.at(-1).t1 - capped.ring.t0 <= 4500)
})

test('a lower one does not take the colour of a higher one playing; the dot has it', () => {
  const order = ['error', 'review', 'done', 'hello', 'plugin']
  for (let i = 0; i < order.length; i++) {
    const first = Flow.play(Flow.idle(), order[i], 0)
    for (let j = 0; j < order.length; j++) {
      const next = Flow.play(first.state, order[j], 500)
      assert.equal(next.act, j > i ? 'hold' : 'blend', `${order[j]} on ${order[i]}`)
      assert.equal(next.ring.kind, j > i ? order[i] : order[j])
    }
  }
})

test('within 8 s of a ring the next does not turn: the dot hops; after, a ring again', () => {
  const first = Flow.play(Flow.idle(), 'done', 0)
  const soon = Flow.play(first.state, 'hello', 2800 + 7999)
  assert.equal(soon.act, 'hop')
  // The 8 s are from the last ring's end, not from the hop.
  assert.equal(Flow.play(soon.state, 'error', 2800 + 7999.5).act, 'hop')
  assert.equal(Flow.play(soon.state, 'error', 2800 + 8000).act, 'start')
  // Cut short because the island opened: the next rings anew.
  assert.equal(Flow.play(Flow.stop(first.state), 'hello', 3000).act, 'start')
})

test('reduced motion: no turning, lit still 1.4 s, no glow, no hop', () => {
  const { act, ring } = Flow.play(Flow.idle(), 'done', 0, { still: true })
  assert.equal(act, 'start')
  assert.equal(ring.still, true)
  assert.equal(Flow.angleAt(ring, 700), 0)
  assert.equal(Flow.frames(ring, 0).turn, null)
  assert.deepEqual(
    [0, 200, 1200, 1300, 1400].map(t => Flow.lightAt(ring, t)),
    [0, 1, 1, 0.5, 0],
  )
  assert.deepEqual([ring.dotAt, ring.until], [1200, 1400])
  assert.equal(Flow.glows(false, true), false)
  assert.equal(Flow.glows(false, false), true)
  assert.equal(Flow.glows(true, false), false)
  // Another while lit: the colour only; soon after: the dot's colour only.
  const next = Flow.play({ ring, until: ring.until }, 'error', 600, { still: true })
  assert.deepEqual([next.act, next.retimed, next.ring.until], ['blend', false, 1400])
  assert.equal(Flow.play({ ring, until: ring.until }, 'error', 5000, { still: true }).act, 'swap')
})

test('the hold as a time: its seconds, or until seen for anything else, as main reads it (#65)', () => {
  assert.equal(Flow.holdMs(8), 8000)
  assert.equal(Flow.holdMs(3), 3000)
  assert.equal(Flow.holdMs(600), 600000)
  for (const seen of ['seen', undefined, null, '8', 7.5, 0, -8]) assert.equal(Flow.holdMs(seen), Infinity, String(seen))
})

test('open by itself, what endCue governs stays as long as the hold says; the rest their own time (#65)', () => {
  assert.deepEqual(Flow.CUED, ['done', 'review', 'error', 'hello', 'plugin'])
  for (const kind of Flow.CUED) {
    assert.equal(Flow.openFor(kind, 30, 3600), 30000, kind)
    assert.equal(Flow.openFor(kind, 8, 6500), 8000, kind)
    assert.equal(Flow.openFor(kind, 'seen', 3600), Infinity, kind)
  }
  // A prompt, a new version, a tool failed (set to open): as before.
  assert.equal(Flow.openFor('waiting', 30, 3600), 3600)
  assert.equal(Flow.openFor('notice', 'seen', 6500), 6500)
  assert.equal(Flow.openFor('process', 120, 3600), 3600)
})

// Never backwards, and no jump in its speed (no jerk) anywhere from a to b.
function smooth(ring, a, b, what) {
  let was = Flow.angleAt(ring, a)
  for (let t = a + 5; t <= b; t += 5) {
    const now = Flow.angleAt(ring, t)
    assert.ok(now >= was - 1e-6, `${what}: backwards at ${t}`)
    was = now
  }
  for (let t = a + 1; t < b; t += 7) {
    const [v0, v1] = [Flow.speedAt(ring, t), Flow.speedAt(ring, t + 1)]
    assert.ok(Math.abs(v1 - v0) < 0.01, `${what}: speed jumps at ${t}: ${v0} to ${v1}`)
  }
}

test('held: its three turns into a calm flow (a turn each 2.25 s) until the hold from the event is over, then slowing as it fades, the dot as it starts to (#65)', () => {
  const W = Flow.FLOW_W
  assert.equal(Flow.FLOW_TURN_MS, 2250)
  near(W, 360 / 2250, 1e-12, 'its speed')
  // The same start as a ring's own curve; its end as steep as the flow.
  assert.deepEqual(Flow.EASE_FLOW.slice(0, 3), [0.45, 0.05, 0.55])
  near(Flow.EASE_FLOW[3], 0.83, 1e-9, 'its last control point')
  const { act, ring } = Flow.play(Flow.idle(), 'done', 1000, { hold: 8000 })
  assert.equal(act, 'start')
  assert.equal(ring.turns, 3)
  assert.equal(Flow.angleAt(ring, 1000 + 2550), 1080)
  // Flowing on: steadily, from where its turns end, at their end's speed.
  near(Flow.speedAt(ring, 3549.9), W, 0.002, 'its turns end at the flow')
  assert.equal(Flow.speedAt(ring, 6000), W)
  near(Flow.angleAt(ring, 6000), 1080 + W * 2450, 1e-9, 'flowing')
  // Until the hold from the event is over (8 s), then slowing to a stop as
  // it fades over 0.56 s, half the way it would have flowed.
  assert.deepEqual([ring.fadeAt, ring.dotAt, ring.until], [9000, 9000, 9560])
  near(Flow.speedAt(ring, 9000.5), W, 0.001, 'slowing from the flow')
  near(Flow.speedAt(ring, 9559.5), 0, 0.001, 'to a stop')
  near(Flow.angleAt(ring, 9560) - Flow.angleAt(ring, 9000), W * 280, 1e-6, 'slowing')
  assert.deepEqual(
    [1000, 1280, 6000, 9000, 9280, 9560].map(t => Flow.lightAt(ring, t)),
    [0, 1, 1, 1, 0.5, 0],
  )
  smooth(ring, 1000, 9560, 'held 8 s')
  // As the page animates it: each turning in turn, its light coming up, down at its end.
  const f = Flow.frames(ring, 1500)
  const [turns, flow, slow] = f.turns
  assert.equal(f.turns.length, 3)
  assert.deepEqual([turns.from, turns.to, turns.delay, turns.duration, turns.easing, turns.iterations], [0, 1080, -500, 2550, 'cubic-bezier(0.45, 0.05, 0.55, 0.83)', 1])
  assert.deepEqual([flow.from, flow.delay, flow.duration, flow.easing, flow.iterations], [1080, 2050, 5450, 'linear', 1])
  near(flow.to, 1080 + W * 5450, 1e-6, 'flowing until the hold is over')
  assert.deepEqual([slow.delay, slow.duration, slow.iterations], [7500, 560, 1])
  near(slow.from, flow.to, 1e-6, 'slowing from where it flowed to')
  near(slow.to - slow.from, W * 280, 1e-6, 'slowing')
  assert.equal(f.turn, turns)
  assert.deepEqual(
    f.light.keyframes.map(k => [k.offset, k.opacity]),
    [
      [0, 0],
      [1, 1],
    ],
  )
  assert.deepEqual([f.light.delay, f.light.duration, f.still, f.paused], [-500, 280, null, false])
  assert.deepEqual([f.out.delay, f.out.duration, f.out.keyframes.map(k => k.opacity), f.dotIn, f.end], [7500, 560, [1, 0], 7500, 8060])
  // Drawn again as it flows: from the flow on.
  const g = Flow.frames(ring, 6000)
  assert.deepEqual([g.turns.length, g.turns[0].easing, g.turns[0].delay], [2, 'linear', -2450])
  // The least hold (3 s) flows a moment after its turns.
  const least = Flow.play(Flow.idle(), 'done', 0, { hold: 3000 }).ring
  assert.deepEqual([least.fadeAt, least.until], [3000, 3560])
  smooth(least, 0, 3560, 'held 3 s')
})

test('until seen: it flows with no end of its own, a turn each 2.25 s for ever (#65)', () => {
  const ring = Flow.play(Flow.idle(), 'error', 0, { hold: Infinity }).ring
  assert.deepEqual([ring.fadeAt, ring.until], [Infinity, Infinity])
  assert.equal(Flow.lightAt(ring, 1e7), 1)
  assert.equal(Flow.speedAt(ring, 1e7), Flow.FLOW_W)
  const f = Flow.frames(ring, 5000)
  assert.deepEqual([f.turns.length, f.turns[0].from, f.turns[0].to, f.turns[0].duration, f.turns[0].easing, f.turns[0].iterations], [1, 1080, 1440, 2250, 'linear', Infinity])
  assert.deepEqual([f.out, f.dotIn, f.end], [null, Infinity, Infinity])
})

test('another while it flows: its colour, its hold from it, the turns not begun again; as it fades, back up to the flow (#65)', () => {
  const hold = 30000
  const W = Flow.FLOW_W
  const first = Flow.play(Flow.idle(), 'done', 0, { hold })
  const again = Flow.play(first.state, 'error', 10000, { hold })
  assert.deepEqual([again.act, again.retimed, again.ring.kind], ['blend', true, 'error'])
  // On from where it is, at its speed: nothing begun again.
  near(Flow.angleAt(again.ring, 10000), Flow.angleAt(first.ring, 10000), 1e-6, 'no jump')
  assert.equal(Flow.speedAt(again.ring, 10001), W)
  assert.equal(again.ring.turnings.filter(s => s.kind === 'turns').length, 1)
  assert.deepEqual([again.ring.fadeAt, again.ring.until], [40000, 40560])
  for (const t of [10000, 20000, 39000]) assert.equal(Flow.lightAt(again.ring, t), 1, `lit at ${t}`)
  smooth(again.ring, 9000, 40560, 'taken on')
  // In its first turns: they go on as they are, into the flow; the hold from it.
  const early = Flow.play(first.state, 'review', 1000, { hold })
  assert.equal(Flow.angleAt(early.ring, 2550), 1080)
  assert.equal(early.ring.fadeAt, 31000)
  smooth(early.ring, 0, 31560, 'in its first turns')
  // A lower one keeps its colour (the dot has it), its hold as it was.
  const lower = Flow.play(again.state, 'hello', 20000, { hold })
  assert.deepEqual([lower.act, lower.ring.fadeAt], ['hold', 40000])
  // Come as it fades: lit again over 0.3 s, back up to the flow over 0.6 s.
  const fading = Flow.play(first.state, 'review', 30280, { hold })
  assert.equal(Flow.lightAt(first.ring, 30280), 0.5)
  assert.equal(Flow.lightAt(fading.ring, 30280), 0.5)
  assert.equal(Flow.lightAt(fading.ring, 30580), 1)
  near(Flow.angleAt(fading.ring, 30280), Flow.angleAt(first.ring, 30280), 1e-6, 'no jump')
  near(Flow.speedAt(fading.ring, 30280 + Flow.RAMP_MS + 1), W, 1e-9, 'flowing again')
  assert.equal(fading.ring.fadeAt, 60280)
  smooth(fading.ring, 29000, 60840, 'taken on as it faded')
  // Gone: within 8 s the dot hops, as ever; after, a ring again.
  assert.equal(Flow.play(first.state, 'hello', 30560 + 7999, { hold }).act, 'hop')
  assert.equal(Flow.play(first.state, 'hello', 30560 + 8000, { hold }).act, 'start')
})

test('reduced motion, held: lit still for the whole hold, no 1.4 s, then down over 0.2 s (#65)', () => {
  const { ring } = Flow.play(Flow.idle(), 'done', 0, { still: true, hold: 30000 })
  assert.deepEqual(
    [0, 200, 1400, 15000, 30000, 30100, 30200].map(t => Flow.lightAt(ring, t)),
    [0, 1, 1, 1, 1, 0.5, 0],
  )
  assert.deepEqual([ring.stillAt, ring.fadeAt, ring.dotAt, ring.until], [200, 30000, 30000, 30200])
  const f = Flow.frames(ring, 0)
  assert.deepEqual([f.turn, f.still, f.rest, f.out.delay, f.out.duration], [null, 200, 0, 30000, 200])
  // Another while lit: its colour, the hold from it.
  const next = Flow.play({ ring, until: ring.until }, 'error', 10000, { still: true, hold: 30000 })
  assert.deepEqual([next.act, next.retimed, next.ring.kind, next.ring.fadeAt], ['blend', true, 'error', 40000])
})

test('held, and seen or its session at work again: slowing to a stop from its speed as it fades, now; the dot (#65)', () => {
  const W = Flow.FLOW_W
  const held = Flow.play(Flow.idle(), 'done', 0, { hold: Infinity }).state
  const now = Flow.release(held, 10000)
  assert.deepEqual([now.ring.fadeAt, now.ring.dotAt, now.ring.until, now.until], [10000, 10000, 10560, 10560])
  assert.equal(Flow.lightAt(now.ring, 9000), 1)
  assert.equal(Flow.lightAt(now.ring, 10280), 0.5)
  near(Flow.speedAt(now.ring, 10000.5), W, 0.001, 'from its speed')
  near(Flow.angleAt(now.ring, 10560) - Flow.angleAt(now.ring, 10000), W * 280, 1e-6, 'slowing to a stop')
  smooth(now.ring, 9000, 10560, 'released')
  const f = Flow.frames(now.ring, 10000)
  assert.deepEqual([f.out.delay, f.out.duration, f.turns.length], [0, 560, 1])
  // In its first turns: from the speed it has then, at once.
  const early = Flow.release(held, 1000)
  assert.equal(early.ring.fadeAt, 1000)
  near(Flow.speedAt(early.ring, 1000.5), Flow.speedAt(held.ring, 1000), 0.01, 'from its speed then')
  smooth(early.ring, 0, 1560, 'released in its turns')
  // Reduced motion: down over its 0.2 s.
  const still = Flow.play(Flow.idle(), 'done', 0, { still: true, hold: 30000 }).state
  assert.equal(Flow.release(still, 5000).ring.until, 5200)
  // Still while nobody could see it: gone now, the dot alone.
  const paused = Flow.pause(held, 5000)
  assert.equal(Flow.release(paused, 6000).until, 6000)
  // Held by nothing, gone, going down already, or none: as it is.
  const plain = Flow.play(Flow.idle(), 'done', 0).state
  assert.equal(Flow.release(plain, 1000), plain)
  assert.equal(Flow.release(now, 20000), now)
  assert.equal(Flow.release(now, 10100), now)
  const none = Flow.idle()
  assert.equal(Flow.release(none, 0), none)
})

test('nobody can see it: held still where it is, its hold going on; seen again, back up to the flow, or gone if its hold is over (#65)', () => {
  const W = Flow.FLOW_W
  const hold = 30000
  const flowing = Flow.play(Flow.idle(), 'done', 0, { hold }).state
  const at = Flow.angleAt(flowing.ring, 10000)
  const still = Flow.pause(flowing, 10000)
  // Still where it was, lit, nothing turning; gone at its hold's end, not fading.
  for (const t of [10000, 15000, 29000]) {
    assert.equal(Flow.angleAt(still.ring, t), at)
    assert.equal(Flow.speedAt(still.ring, t), 0)
    assert.equal(Flow.lightAt(still.ring, t), 1)
  }
  assert.deepEqual([still.ring.dotAt, still.ring.until, still.until], [30000, 30000, 30000])
  const f = Flow.frames(still.ring, 12000)
  assert.deepEqual([f.paused, f.turns.length, f.light, f.out, f.lit, f.end], [true, 0, null, null, 1, 18000])
  near(f.rest, at % 360, 1e-9, 'resting where it was')
  // Paused again, or not held: as it is.
  assert.equal(Flow.pause(still, 12000), still)
  const plain = Flow.play(Flow.idle(), 'done', 0).state
  assert.equal(Flow.pause(plain, 1000), plain)
  const reduced = Flow.play(Flow.idle(), 'done', 0, { still: true, hold }).state
  assert.equal(Flow.pause(reduced, 1000), reduced)
  // Seen again with hold left: from where it rests, up to the flow over 0.6 s, its hold as it was.
  const again = Flow.resume(still, 20000)
  assert.equal(again.ring.paused, null)
  assert.equal(Flow.angleAt(again.ring, 20000), at)
  assert.equal(Flow.speedAt(again.ring, 20000), 0)
  near(Flow.speedAt(again.ring, 20000 + Flow.RAMP_MS + 1), W, 1e-9, 'flowing again')
  assert.deepEqual([again.ring.fadeAt, again.ring.until], [30000, 30560])
  smooth(again.ring, 20000, 30560, 'resumed')
  assert.equal(Flow.resume(flowing, 20000), flowing)
  // Its hold over while still: gone, the dot alone.
  assert.equal(Flow.resume(still, 31000), still)
  // Going down already as nobody can see it: gone now.
  assert.equal(Flow.pause(flowing, 30200).until, 30200)
  // Until seen: still with no end; seen again, it flows on with none.
  const seen = Flow.pause(Flow.play(Flow.idle(), 'done', 0, { hold: Infinity }).state, 5000)
  assert.equal(seen.until, Infinity)
  assert.equal(Flow.resume(seen, 9000).ring.until, Infinity)
})

test('each event its own colours, the gradient closed; deeper on a light ground', () => {
  for (const kind of ['done', 'review', 'error', 'hello']) {
    for (const light of [false, true]) {
      const stops = Flow.palette(kind, light)
      assert.equal(stops[0], stops.at(-1), `${kind} ${light}`)
    }
  }
  assert.deepEqual(Flow.palette('error', false), ['#ff453a', '#ff9f0a', '#ff453a', '#ff9f0a', '#ff453a'])
  assert.equal(Flow.palette('done', true)[2], '#c99a00')
  // A plugin's: its colour and 45 % white by turns; deeper by 15 % on a light ground.
  assert.deepEqual(Flow.palette('plugin', false, '#ff0000'), ['#ff0000', '#ff7373', '#ff0000', '#ff7373', '#ff0000'])
  assert.deepEqual(Flow.palette('plugin', true, '#ff0000').slice(0, 2), ['#d90000', '#ea7373'])
  // None of its own: the fallback given.
  assert.equal(Flow.palette('plugin', false, undefined, '#4cc2ff')[0], '#4cc2ff')
  assert.equal(Flow.gradient(['#000', '#fff', '#000']), 'conic-gradient(#000, #fff, #000)')
})

test('the curve is CSS cubic-bezier', () => {
  const ease = Flow.bezier([0.25, 0.1, 0.25, 1])
  // CSS's ease at a few points (as browsers compute it).
  near(ease.y(0.5), 0.8024, 0.001, 'ease at 0.5')
  near(ease.y(0.25), 0.4085, 0.001, 'ease at 0.25')
  assert.equal(ease.y(0), 0)
  assert.equal(ease.y(1), 1)
  // A straight line is a straight line.
  const line = Flow.bezier([1 / 3, 1 / 3, 2 / 3, 2 / 3])
  near(line.y(0.37), 0.37, 1e-6, 'line')
  near(line.dy(0.6), 1, 1e-6, 'line slope')
})
