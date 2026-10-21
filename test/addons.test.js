// Third-party plugins in the island's page (src/pet/addons.js): a message is
// a plugin's only when it comes from that plugin's frame, whatever it says;
// what each may ask; what each is told, and when; frames started, started
// over, stopped. And the SDK in a frame (src/addon/sdk.js): the waku object
// over messages to the page.
const assert = require('node:assert/strict')
const fs = require('node:fs')
const path = require('node:path')
const vm = require('node:vm')
const { test } = require('node:test')

const A = require('../src/pet/addons')

// A host with fakes: its frames are plain objects; what it posts and asks is
// recorded; timers run when told; invoke answers as `answers` says.
function setup(answers = {}) {
  const posted = []
  const asked = []
  const logs = []
  const dropped = []
  let t = 0
  const timers = []
  const made = []
  const host = A.createHost({
    invoke: (cmd, args) => {
      asked.push({ cmd, args })
      const a = answers[cmd]
      return Promise.resolve(typeof a === 'function' ? a(args) : (a ?? { ok: true }))
    },
    make: (id, src) => {
      const win = { name: `win-${id}-${made.length}` }
      const el = { id, src }
      made.push({ id, src, win, el })
      return { el, win }
    },
    drop: rec => dropped.push(rec.id),
    post: (win, message) => posted.push({ to: win.name, ...message }),
    log: line => logs.push(line),
    now: () => t,
    setTimer: (fn, ms) => {
      timers.push({ fn, at: t + ms, live: true })
      return timers.length
    },
    clearTimer: n => {
      if (n && timers[n - 1]) timers[n - 1].live = false
    },
  })
  const advance = ms => {
    t += ms
    for (const timer of timers) if (timer.live && timer.at <= t) {
      timer.live = false
      timer.fn()
    }
  }
  const flush = () => new Promise(r => setImmediate(r))
  return { host, posted, asked, logs, dropped, made, advance, flush, win: id => made.filter(m => m.id === id).at(-1)?.win }
}

const item = (id, extra = {}) => ({ id, version: '1.0.0', rev: `1.0.0:${id}`, main: 'main.js', perms: { net: [], agents: false, hardware: false, notify: false }, settings: {}, ...extra })

test('frames start once her language is known, one per plugin, at its own first page', () => {
  const s = setup()
  s.host.apply([item('hello'), item('Bad Id'), { nope: 1 }])
  assert.equal(s.made.length, 0, 'not before her language is known')
  s.host.setEnv({ lang: 'en', theme: 'light', quiet: false })
  assert.deepEqual(s.made.map(m => m.src), ['http://wakuplugin.localhost/hello/__waku/main.html'])
  // Ready: told who it is and how things are.
  s.host.receive(s.win('hello'), { waku: 1, t: 'ready' })
  assert.deepEqual(s.posted.at(-1), { to: s.win('hello').name, waku: 1, t: 'init', id: 'hello', version: '1.0.0', lang: 'en', theme: 'light', quiet: false, settings: {}, main: 'main.js' })
})

test('a message is the plugin’s whose frame sent it, whatever it says it is', async () => {
  const s = setup()
  s.host.setEnv({ lang: 'zh' })
  s.host.apply([item('hello'), item('rain-alert', { perms: { net: ['api.open-meteo.com'], notify: true } })])
  // From nobody's frame: nothing.
  assert.equal(s.host.receive({ name: 'stranger' }, { waku: 1, t: 'notify', n: 1, text: 'x', id: 'rain-alert' }), false)
  // From hello's, claiming to be rain-alert, with rain-alert's permission: hello's, refused.
  s.host.receive(s.win('hello'), { waku: 1, t: 'notify', n: 2, text: 'x', id: 'rain-alert' })
  s.host.receive(s.win('hello'), { waku: 1, t: 'storage', n: 3, op: 'set', key: 'k', value: 1, id: 'rain-alert' })
  await s.flush()
  assert.deepEqual(s.posted.filter(p => p.t === 'reply').map(p => [p.n, p.ok, p.error]), [
    [2, false, 'no notify permission'],
    [3, true, undefined],
  ])
  assert.deepEqual(s.asked.map(a => [a.cmd, a.args.id]), [['storage', 'hello']])
  // Not a message of hers at all.
  assert.equal(s.host.receive(s.win('hello'), { t: 'notify' }), false)
  assert.equal(s.host.receive(s.win('hello'), 'notify'), false)
})

test('what it asks goes to her in its name, its answers back to its frame', async () => {
  const s = setup({
    fetch: { status: 200, ok: true, headers: {}, text: '{}' },
    storage: { value: 7 },
    remove: { error: 'not running' },
  })
  s.host.setEnv({ lang: 'zh' })
  s.host.apply([item('hello')])
  const w = s.win('hello')
  s.host.receive(w, { waku: 1, t: 'fetch', n: 1, url: 'https://example.com/', method: 'GET', headers: { a: 1 } })
  s.host.receive(w, { waku: 1, t: 'storage', n: 2, op: 'get', key: 'k' })
  s.host.receive(w, { waku: 1, t: 'storage', n: 3, op: 'drop', key: 'k' })
  s.host.receive(w, { waku: 1, t: 'remove', n: 4, sub: 'two' })
  s.host.receive(w, { waku: 1, t: 'agents', n: 5 })
  s.host.receive(w, { waku: 1, t: 'hardware', n: 6 })
  await s.flush()
  assert.deepEqual(s.asked[0], { cmd: 'fetch', args: { id: 'hello', url: 'https://example.com/', method: 'GET', headers: { a: '1' }, body: null } })
  assert.deepEqual(s.asked[2].args.op, 'get', 'an op it has not: get')
  const replies = Object.fromEntries(s.posted.filter(p => p.t === 'reply').map(p => [p.n, p.ok ? p.value : `! ${p.error}`]))
  assert.deepEqual(replies, { 1: { status: 200, ok: true, headers: {}, text: '{}' }, 2: 7, 3: 7, 4: '! not running', 5: '! no agents permission', 6: '! no hardware permission' })
})

test('four requests at once at most; a widget’s words once a second, the last of them', async () => {
  let release
  const s = setup({ fetch: () => new Promise(r => (release = r)) })
  s.host.setEnv({ lang: 'zh' })
  s.host.apply([item('hello')])
  const w = s.win('hello')
  for (let n = 1; n <= 5; n++) s.host.receive(w, { waku: 1, t: 'fetch', n, url: 'https://example.com/' })
  await s.flush()
  assert.deepEqual(s.posted.filter(p => p.t === 'reply').map(p => [p.n, p.error]), [[5, '4 requests at once at most']])
  release({ status: 200, ok: true, headers: {}, text: '' })
  await s.flush()
  // Widgets: the first at once, those within its second as one at its end.
  s.host.receive(w, { waku: 1, t: 'widget', n: 10, sub: '', data: { value: '1' } })
  s.host.receive(w, { waku: 1, t: 'widget', n: 11, sub: '', data: { value: '2' } })
  s.host.receive(w, { waku: 1, t: 'widget', n: 12, sub: '', data: { value: '3' } })
  s.host.receive(w, { waku: 1, t: 'widget', n: 13, sub: 'two', data: { value: 'b' } })
  await s.flush()
  const sent = () => s.asked.filter(a => a.cmd === 'widget').map(a => [a.args.sub, a.args.data.value])
  assert.deepEqual(sent(), [
    [null, '1'],
    ['two', 'b'],
  ])
  s.advance(999)
  await s.flush()
  assert.equal(sent().length, 2)
  s.advance(1)
  await s.flush()
  assert.deepEqual(sent().at(-1), [null, '3'])
  const answered = s.posted.filter(p => p.t === 'reply' && p.n >= 10).map(p => p.n).sort()
  assert.deepEqual(answered, [10, 11, 12, 13])
  // Not JSON, or too big: refused here.
  s.host.receive(w, { waku: 1, t: 'widget', n: 20, data: { big: 'x'.repeat(70_000) } })
  assert.equal(s.posted.at(-1).error, 'a widget is JSON, up to 64 KB')
})

test('settings, language, theme and quiet told as they change; a changed plugin started over', () => {
  const s = setup()
  s.host.setEnv({ lang: 'zh', theme: 'dark', quiet: false })
  s.host.apply([item('hello', { settings: { Step: 1 } })])
  s.host.receive(s.win('hello'), { waku: 1, t: 'ready' })
  s.posted.length = 0
  s.host.apply([item('hello', { settings: { Step: 2 } })])
  s.host.setEnv({ lang: 'zh', theme: 'light', quiet: true })
  s.host.setEnv({ lang: 'zh', theme: 'light', quiet: true })
  assert.deepEqual(s.posted.map(p => [p.what, p.value]), [
    ['settings', { Step: 2 }],
    ['theme', 'light'],
    ['quiet', true],
  ])
  // Another version (or its manifest edited): a frame of its own again.
  s.host.apply([item('hello', { rev: '1.1.0:x', settings: { Step: 2 } })])
  assert.deepEqual(s.dropped, ['hello'])
  assert.equal(s.made.length, 2)
  // Gone from the list: stopped.
  s.host.apply([])
  assert.deepEqual(s.dropped, ['hello', 'hello'])
  assert.equal(s.host.recs.size, 0)
})

test('the agents to those that may and listen, once a second at most, and only what a plugin needs', () => {
  const s = setup()
  s.host.setEnv({ lang: 'zh' })
  s.host.apply([item('watcher', { perms: { agents: true } }), item('hello')])
  for (const id of ['watcher', 'hello']) {
    s.host.receive(s.win(id), { waku: 1, t: 'ready' })
    s.host.receive(s.win(id), { waku: 1, t: 'listen', what: 'agents' })
  }
  const told = () => s.posted.filter(p => p.what === 'agents')
  assert.deepEqual(told().map(p => p.to), [s.win('watcher').name], 'only one that may, at once as it listens')
  const session = { id: 's1', agent: 'codex', project: 'pet', mood: 'working', since: 5, detail: 'Editing a.rs', name: 'secret task', reply: 'secret words', jump: true }
  s.host.setAgents([session])
  assert.deepEqual(told().at(-1).value, [{ id: 's1', agent: 'codex', project: 'pet', mood: 'working', since: 5, detail: 'Editing a.rs' }])
  s.host.setAgents([{ ...session, mood: 'done' }])
  assert.equal(told().length, 2, 'the next within its second waits')
  s.advance(1000)
  assert.equal(told().at(-1).value[0].mood, 'done')
  s.host.setAgents([{ ...session, mood: 'done' }])
  s.advance(5000)
  assert.equal(told().length, 3, 'the same again: nothing told')
  assert.deepEqual(A.agentsOf([{ id: 'x', mood: 'dancing', detail: { key: 'k' } }, null, { id: '' }]), [{ id: 'x', agent: 'claude', project: '', mood: 'idle', since: null, detail: '' }])
})

test('the hardware read while one that may listens, slower while nobody is about', async () => {
  const s = setup({ hardware: { cpu: 12, memory: 40, gpu: null, netDown: 1, netUp: 2 } })
  s.host.setEnv({ lang: 'zh' })
  s.host.apply([item('meter', { perms: { hardware: true } })])
  s.host.receive(s.win('meter'), { waku: 1, t: 'ready' })
  s.advance(60_000)
  assert.equal(s.asked.length, 0, 'nobody listens: never read')
  s.host.receive(s.win('meter'), { waku: 1, t: 'listen', what: 'hardware' })
  s.advance(5000)
  await s.flush()
  assert.deepEqual(s.posted.filter(p => p.what === 'hardware').map(p => p.value.cpu), [12])
  s.host.setEnv({ quiet: true })
  s.advance(5000)
  await s.flush()
  assert.equal(s.asked.length, 1, 'quiet: three times as long')
  s.advance(10_000)
  await s.flush()
  assert.equal(s.asked.length, 2)
  s.host.apply([])
  s.advance(60_000)
  await s.flush()
  assert.equal(s.asked.length, 2, 'stopped: no more')
})

test('a frame loading its page again starts over; one that keeps on is left stopped until it changes', () => {
  const s = setup()
  s.host.setEnv({ lang: 'zh' })
  s.host.apply([item('hopper')])
  const load = () => s.host.loaded('hopper', s.host.recs.get('hopper').el)
  load()
  assert.equal(s.made.length, 1, 'its first load is its own')
  for (let i = 0; i < 3; i++) {
    load()
    load()
  }
  assert.equal(s.made.length, 4, 'started over each time')
  load()
  assert.equal(s.host.recs.size, 0, 'the fourth time in a minute: stopped')
  assert.ok(s.logs.at(-1).includes('stopped'))
  assert.ok(s.asked.some(a => a.cmd === 'said' && a.args.id === 'hopper'))
  s.host.apply([item('hopper')])
  assert.equal(s.host.recs.size, 0, 'held while it is the same')
  s.host.apply([item('hopper', { rev: 'edited' })])
  assert.equal(s.host.recs.size, 1, 'changed: tried again')
  // A load of a frame since dropped means nothing.
  s.host.loaded('hopper', { stale: true })
  assert.equal(s.made.length, 5)
})

test('its log lines and errors: thirty a minute at most, errors said on the Plugins page', async () => {
  const s = setup()
  s.host.setEnv({ lang: 'zh' })
  s.host.apply([item('chatty')])
  const w = s.win('chatty')
  for (let i = 0; i < 40; i++) s.host.receive(w, { waku: 1, t: 'log', line: `line ${i}` })
  assert.equal(s.logs.length, 30)
  assert.equal(s.logs[0], 'plugin chatty: line 0')
  s.advance(60_000)
  s.host.receive(w, { waku: 1, t: 'error', line: 'TypeError: x is undefined (main.js:3)' })
  await s.flush()
  assert.deepEqual(s.asked.at(-1), { cmd: 'said', args: { id: 'chatty', line: 'TypeError: x is undefined (main.js:3)' } })
  assert.equal(s.host.ping('chatty'), true)
  assert.equal(s.host.ping('nobody'), false)
})

// --- The SDK, in a frame of its own ---------------------------------------------------------

function frame() {
  const sent = []
  const handlers = { message: [], error: [], unhandledrejection: [] }
  const parent = { postMessage: m => sent.push(JSON.parse(JSON.stringify(m))) }
  const timers = []
  const window = {
    parent,
    location: { href: 'http://wakuplugin.localhost/hello/__waku/main.html' },
    addEventListener: (type, fn) => handlers[type]?.push(fn),
    RTCPeerConnection: function () {},
  }
  const context = {
    window,
    location: window.location,
    URL,
    JSON,
    Promise,
    Error,
    TypeError,
    Object,
    Number,
    String,
    Array,
    Map,
    Set,
    Math,
    setTimeout: (fn, ms) => timers.push({ fn, ms }) && timers.length,
    clearTimeout: n => n && timers[n - 1] && (timers[n - 1].fn = null),
  }
  vm.createContext(context)
  vm.runInContext(fs.readFileSync(path.join(__dirname, '..', 'src', 'addon', 'sdk.js'), 'utf8'), context, {
    filename: 'sdk.js',
    // Its main, as the frame would load it: here, nothing.
    importModuleDynamically: () => Promise.resolve(new vm.SyntheticModule([], () => {}, { context })),
  })
  const deliver = data => handlers.message.forEach(fn => fn({ source: parent, data }))
  return { sent, window, deliver, timers, handlers, parent }
}

test('the SDK says it is ready, and is given its waku at init', async () => {
  const f = frame()
  assert.deepEqual(f.sent, [{ waku: 1, t: 'ready' }])
  assert.equal(f.window.RTCPeerConnection, undefined, 'WebRTC taken away')
  assert.equal(f.window.waku, undefined)
  // From anyone but the page holding it: nothing.
  f.handlers.message.forEach(fn => fn({ source: {}, data: { waku: 1, t: 'init', id: 'x' } }))
  assert.equal(f.window.waku, undefined)
  f.deliver({ waku: 1, t: 'init', id: 'hello', version: '1.0.0', lang: 'en', theme: 'light', quiet: false, settings: { Step: 2 }, main: 'main.js' })
  const waku = f.window.waku
  assert.equal(waku.id, 'hello')
  assert.equal(waku.lang, 'en')
  assert.equal(waku.settings.Step, 2)
  assert.throws(() => {
    'use strict'
    waku.settings.Step = 3
  })
  // A second init changes nothing.
  f.deliver({ waku: 1, t: 'init', id: 'other', version: '9.9.9', lang: 'zh', settings: {}, main: 'x.js' })
  assert.equal(f.window.waku.id, 'hello')
})

test('the SDK asks by message and gives back what the page answers', async () => {
  const f = frame()
  f.deliver({ waku: 1, t: 'init', id: 'hello', version: '1.0.0', lang: 'zh', theme: 'dark', quiet: false, settings: {}, main: 'main.js' })
  const waku = f.window.waku
  const got = waku.storage.get('count')
  const set = waku.widget.set('two', { value: 3, fn: () => 1 })
  const fetched = waku.fetch('https://api.open-meteo.com/v1/x', { headers: { accept: 'application/json' } })
  const asks = f.sent.filter(m => m.n)
  assert.deepEqual(asks.map(m => m.t), ['storage', 'widget', 'fetch'])
  assert.deepEqual(asks[1].data, { value: 3 }, 'as JSON has it')
  assert.deepEqual(asks[2].headers, { accept: 'application/json' })
  f.deliver({ waku: 1, t: 'reply', n: asks[0].n, ok: true, value: 41 })
  f.deliver({ waku: 1, t: 'reply', n: asks[1].n, ok: false, error: 'label: some words' })
  f.deliver({ waku: 1, t: 'reply', n: asks[2].n, ok: true, value: { status: 200, ok: true, headers: {}, text: '{"a":1}' } })
  assert.equal(await got, 41)
  await assert.rejects(set, /label: some words/)
  const r = await fetched
  assert.equal(r.status, 200)
  assert.deepEqual(await r.json(), { a: 1 })
  assert.throws(() => waku.on('nope', () => {}), /waku.on/)
})

test('the SDK’s every: at least a second, three times as long while nobody is about', () => {
  const f = frame()
  f.deliver({ waku: 1, t: 'init', id: 'hello', version: '1.0.0', lang: 'zh', theme: 'dark', quiet: false, settings: {}, main: 'main.js' })
  let calls = 0
  const stop = f.window.waku.every(0.1, () => calls++)
  assert.equal(f.timers.at(-1).ms, 1000)
  f.deliver({ waku: 1, t: 'event', what: 'quiet', value: true })
  assert.equal(f.timers.at(-1).ms, 3000)
  f.timers.at(-1).fn()
  assert.equal(calls, 1)
  stop()
  const last = f.timers.at(-1)
  assert.equal(last.fn, null, 'stopped')
  let seen = null
  f.window.waku.on('settings', v => (seen = v))
  f.deliver({ waku: 1, t: 'event', what: 'settings', value: { City: 'Dresden' } })
  assert.deepEqual({ ...seen }, { City: 'Dresden' })
  assert.equal(f.window.waku.settings.City, 'Dresden')
  // Its listening for the agents is told the page.
  f.window.waku.on('agents', () => {})
  assert.deepEqual(f.sent.at(-1), { waku: 1, t: 'listen', what: 'agents' })
})
