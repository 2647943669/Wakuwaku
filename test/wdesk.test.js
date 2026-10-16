const assert = require('node:assert/strict')
const { test } = require('node:test')

const WDesk = require('../src/pet/wdesk')

const now = new Date(2026, 9, 16, 14, 30).getTime()
const dark = { lang: 'zh', light: false, detailed: true, hour: 14, now }
const light = { ...dark, light: true }

const weather = {
  id: 'weather',
  plugin: 'weather',
  label: 'Dresden · 阴',
  value: '11°',
  detail: {
    sizes: ['s', 'm', 'l'],
    extra: '8°~16°',
    series: { points: [10, 11, 13, 15, 16, 14, 11], unit: '°' },
    rows: [
      ...['10时', '12时', '14时', '16时', '18时', '20时', '22时', '0时'].map((when, i) => ({ kind: 'hour', when, icon: i === 2 ? 'sun' : 'cloud', value: `${10 + i}°` })),
      { kind: 'day', when: '明天', icon: 'rain', value: '9°~14°', name: '小雨' },
      { kind: 'day', when: '后天', icon: 'cloud', value: '7°~12°', name: '阴' },
    ],
  },
}
const monitor = {
  id: 'monitor',
  builtIn: true,
  label: { key: 'widget.monitor' },
  value: { parts: [{ icon: 'cpu', text: '17%' }, { icon: 'memory', text: '44%' }, { icon: 'down', text: '7.4K' }] },
  detail: {
    sizes: ['s', 'm', 'l'],
    series: { points: [12, 17, 20, 17], unit: '%', hi: 100, lo: 0 },
    rows: [
      { kind: 'cpu', value: '17%', text: '3.4 GHz', name: '8', unit: '16' },
      { kind: 'memory', value: '44%', text: '14.1 / 32 GB' },
      { kind: 'gpu', value: '23%', name: 'RTX 4070', text: '52°' },
      { kind: 'net', value: '7.4K', text: '337K', points: [7400, 8000, 6000] },
      { kind: 'uptime', value: '273600' },
    ],
  },
}
const aapl = { id: 'stock-aapl', plugin: 'stock', label: '苹果', value: '230.40 +0.80%', color: '#ff5c6c', detail: { sizes: ['s', 'm', 'l'], series: { points: [228, 229, 230.4], hi: 231, lo: 227 }, rows: [{ code: 'AAPL', price: '230.40', change: '+0.80%', value: '4,812 万' }] } }
const nvda = { id: 'stock-nvda', plugin: 'stock', label: '英伟达', value: '145.10 -1.30%', color: '#34d27b', detail: { sizes: ['s', 'm', 'l'], series: { points: [147, 146, 145.1] }, rows: [{ code: 'NVDA', price: '145.10', change: '-1.30%' }] } }
const inbox = {
  id: 'inbox-a',
  from: 'mail',
  label: 'Gmail',
  value: { key: 'mail.unread', vars: { n: '13' } },
  private: true,
  detail: { sizes: ['s', 'm', 'l'], rows: [{ from: 'Anna', subject: '周五见：我订好了那家餐厅', at: new Date(2026, 9, 16, 10, 42).getTime() }, { from: 'GitHub', subject: '#52 merged', at: new Date(2026, 9, 15, 9, 0).getTime() }] },
}
const today = { id: 'today', builtIn: true, label: { key: 'widget.today' }, value: { key: 'widget.todayValue', vars: { turns: '197', time: '2:01', approvals: '3' } }, detail: { sizes: ['s', 'm', 'l'], series: { points: [0, 0, 0, 0, 0, 0, 0, 0, 0, 2, 5, 9, 14, 11, 18, 0, 0, 0, 0, 0, 0, 0, 0, 0] } } }
const tokens = { id: 'tokens', builtIn: true, label: { key: 'widget.tokens' }, value: { key: 'widget.tokensValue', vars: { total: '1.7B', output: '3.4M' } } }
const pomodoro = { id: 'pomodoro', label: '专注', value: '18:42', color: '#ff5c6c', detail: { sizes: ['s', 'm'], extra: '之后休息 5 分钟', series: { points: [1122], hi: 1500, lo: 0 }, rows: [{ kind: 'round', value: '3', unit: '4' }] } }
const countdown = { id: 'countdown', label: 'v0.3.1 发布', value: '3 天', detail: { sizes: ['s'], extra: '10 月 19 日 周日', rows: [{ kind: 'days', value: '3', unit: '天' }] } }
const script = { id: 'ci-main', plugin: 'ci', label: 'CI main', value: '通过', color: '#34d27b', detail: '3 分钟前' }
const all = [weather, monitor, aapl, nvda, inbox, today, tokens, pomodoro, countdown, script]
const ctx = c => ({ ...c, all })
const text = html => html.replace(/<[^>]+>/g, ' ').replace(/&[a-z#0-9]+;/g, ' ').replace(/\s+/g, ' ').trim()

test('every widget draws in every size it has, dark and light, without a hole', () => {
  for (const w of all)
    for (const size of ['s', 'm', 'l'])
      for (const c of [dark, light]) {
        const html = WDesk.faceHTML(w, size, ctx(c))
        assert.ok(html.includes('class="hd"'), `${w.id} ${size}: its head`)
        assert.ok(!/undefined|NaN|\[object/.test(html), `${w.id} ${size}: ${text(html)}`)
      }
})

test('the weather: its sky and place small; six hours medium; the curve, seven hours and the next days large', () => {
  const s = text(WDesk.faceHTML(weather, 's', ctx(dark)))
  assert.match(s, /11°/)
  assert.match(s, /阴 · Dresden/)
  assert.match(s, /8°~16°/)
  const m = WDesk.faceHTML(weather, 'm', ctx(dark))
  assert.equal((m.match(/<b>\d+°<\/b>/g) || []).length, 6)
  const l = WDesk.faceHTML(weather, 'l', ctx(dark))
  assert.equal((l.match(/<b>\d+°<\/b>/g) || []).length, 7)
  assert.match(l, /<svg class="wspark" width="292" height="64"/)
  assert.match(text(l), /明天 9°~14° 小雨/)
})

test('the monitor: a ring small, three columns medium, the curve and the hardware large; what is not read says so', () => {
  assert.match(WDesk.faceHTML(monitor, 's', ctx(dark)), /<svg class="wring" width="58"/)
  const m = text(WDesk.faceHTML(monitor, 'm', ctx(dark)))
  assert.match(m, /CPU 17 %/)
  assert.match(m, /GPU 23 %/)
  assert.match(m, /↓ 7.4K · ↑ 337K/)
  const l = WDesk.faceHTML(monitor, 'l', ctx(dark))
  assert.match(l, /width="292" height="70"/)
  assert.match(text(l), /8 核 16 线程/)
  // No drive read (a VM): its line greyed with "no data".
  assert.match(text(l), /磁盘 暂无数据/)
  assert.match(text(l), /运行 3 天 4 小时/)
})

test("stocks: the first small, three lines medium, the others under the curve large; rises in the script's colour", () => {
  const s = WDesk.faceHTML(aapl, 's', ctx(dark))
  assert.match(text(s), /AAPL 230.40 \+0.80%/)
  // Red up (the script said so, as the stock plugin's colours setting is): the chip red.
  assert.match(s, /--t:#ff6b6b/)
  assert.match(WDesk.faceHTML(aapl, 's', ctx(light)), /--t:#c42b3c/)
  const m = WDesk.faceHTML(aapl, 'm', ctx(dark))
  assert.equal((m.match(/class="row sline"/g) || []).length, 2)
  assert.match(m, /--t:#34d27b/)
  const l = text(WDesk.faceHTML(aapl, 'l', ctx(dark)))
  assert.match(l, /成交 4,812 万/)
  assert.match(l, /NVDA 英伟达 145.10 -1.30%/)
})

test('mail: unread and the newest small; letters medium and large, private blurred unless the specifics show', () => {
  const s = WDesk.faceHTML(inbox, 's', ctx({ ...dark, detailed: false }))
  assert.match(text(s), /13 封未读/)
  assert.match(s, /<span class="wpriv hid">Anna：周五见：我订好了那家餐厅<\/span>/)
  assert.doesNotMatch(WDesk.faceHTML(inbox, 's', ctx(dark)), /wpriv/)
  const m = WDesk.faceHTML(inbox, 'm', ctx(dark))
  assert.equal((m.match(/class="mailrow"/g) || []).length, 2)
  assert.match(text(m), /13 封未读/)
  assert.match(text(m), /10:42/)
  assert.match(text(m), /昨天/)
})

test("Claude today: its turns small; the three numbers and the hours medium; tokens' curve and the busiest not counted yet", () => {
  const s = text(WDesk.faceHTML(today, 's', ctx(dark)))
  assert.match(s, /197/)
  assert.match(s, /批准 3 次 · 1.7B token/)
  const m = WDesk.faceHTML(today, 'm', ctx(dark))
  assert.match(m, /<svg class="wbars" width="292" height="30"/)
  assert.match(text(m), /1.7B token/)
  const l = text(WDesk.faceHTML(today, 'l', ctx(dark)))
  assert.match(l, /每小时的 token 暂无数据/)
  assert.match(l, /最忙的会话 暂无数据/)
})

test('the pomodoro: its ring small and medium, which round, what comes after; no pause it cannot do', () => {
  assert.match(WDesk.faceHTML(pomodoro, 's', ctx(dark)), /<svg class="wring" width="84"/)
  const m = WDesk.faceHTML(pomodoro, 'm', ctx(dark))
  assert.match(m, /<svg class="wring" width="96"/)
  assert.match(text(m), /第 3 个/)
  assert.match(text(m), /今天 3 \/ 4/)
  assert.match(text(m), /之后休息 5 分钟/)
  assert.match(m, /class="wbtn off"/)
})

test('the countdown: the days, half-size unit, the event and its date', () => {
  const s = WDesk.faceHTML(countdown, 's', ctx(dark))
  assert.match(s, /<div class="big">3<small>天<\/small><\/div>/)
  assert.match(text(s), /v0.3.1 发布 10 月 19 日 周日/)
})

test('nothing to say: greyed with its name; loading a glint; stopped amber', () => {
  const none = WDesk.emptyHTML({ type: 'plugin', of: 'weather' }, '天气', 'none', dark)
  assert.match(text(none), /天气 暂无数据/)
  assert.match(WDesk.emptyHTML({ type: 'widget', of: 'x' }, 'X', 'loading', dark), /class="wshim"/)
  assert.match(WDesk.emptyHTML({ type: 'widget', of: 'x' }, 'X', 'error', dark), /werr/)
})
