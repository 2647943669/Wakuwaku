// Agent board: for each project, how long its agents' sessions worked
// today (their mood "working"), kept in its storage across restarts and
// started afresh at midnight. Its widget says which project was busiest
// and for how many minutes (tier 1: everywhere her widgets go); its view
// (view.html, tier 2: the desktop's medium and large cards, the capsule's
// card) draws them all as a ranking. It asks for the agents' sessions
// alone (permissions.agents), and nothing of the web.
const words = {
  zh: { name: 'Agent 排行', busiest: p => `最忙：${p}`, minutes: n => `${n} 分钟`, extra: (n, total) => `${n} 个项目 · 共 ${total} 分钟`, none: '今天还没开工' },
  en: { name: 'Agent board', busiest: p => `Busiest: ${p}`, minutes: n => `${n} min`, extra: (n, total) => `${n} projects · ${total} min in all`, none: 'Nothing yet today' },
}
const say = () => words[waku.lang] || words.en

// Today's date as a day's name, in this computer's time zone.
const today = () => new Date().toLocaleDateString('sv')

// Each project's time today (ms), counted up to the last look; the
// sessions working now, each with its project and since when it was counted.
let day = today()
let totals = {}
let working = new Map()

const kept = await waku.storage.get('today')
if (kept && kept.day === day && kept.totals && typeof kept.totals === 'object') totals = kept.totals

// The time since the last look added to each session's project; past
// midnight, a new day from nothing.
function count() {
  const now = Date.now()
  if (today() !== day) {
    day = today()
    totals = {}
    for (const w of working.values()) w.since = now
    return
  }
  for (const w of working.values()) {
    totals[w.project] = (totals[w.project] || 0) + Math.max(0, now - w.since)
    w.since = now
  }
}

function show() {
  const s = say()
  const ranked = Object.entries(totals)
    .map(([project, ms]) => ({ project, minutes: Math.floor(ms / 60000), on: [...working.values()].some(w => w.project === project) }))
    .filter(r => r.minutes > 0 || r.on)
    .sort((a, b) => b.minutes - a.minutes || a.project.localeCompare(b.project))
  const total = ranked.reduce((n, r) => n + r.minutes, 0)
  const top = ranked[0]
  waku.widget.set({
    label: top ? s.busiest(top.project) : s.name,
    value: top ? s.minutes(top.minutes) : '—',
    detail: {
      extra: top ? s.extra(ranked.length, total) : s.none,
      rows: ranked.slice(0, 6).map(r => ({ name: r.project, value: s.minutes(r.minutes) })),
      series: ranked.length > 1 ? { points: ranked.slice(0, 12).map(r => r.minutes), unit: 'min' } : undefined,
    },
    // For view.html: the ranking, the busiest first.
    view: { day, total, rows: ranked.slice(0, 12) },
  })
}

function save() {
  return waku.storage.set('today', { day, totals })
}

// The sessions as they are now: those working counted from now on, those
// that stopped counted up to now.
function take(list) {
  count()
  const next = new Map()
  for (const a of Array.isArray(list) ? list : []) {
    if (a.mood !== 'working') continue
    next.set(a.id, working.get(a.id) || { project: a.project || '?', since: Date.now() })
  }
  working = next
  show()
}

waku.on('agents', take)
waku.on('lang', show)
// What its view says (waku.send): that it showed, at which size.
waku.on('view', (message, from) => {
  if (message?.shown) waku.log(`its view is up (${message.shown}${from.sub ? `, ${from.sub}` : ''})`)
})
waku.every(30, () => {
  count()
  show()
  save()
})
take(await waku.agents.list())
