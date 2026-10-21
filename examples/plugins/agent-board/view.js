// Agent board's view: the ranking its main.js gives as its widget's view
// ({ day, total, rows: [{ project, minutes, on }] }), the busiest first,
// three in a medium card, eight in a large one. A plain script (not a
// module), drawn at once: waku.data is there before this runs.
const board = document.getElementById('board')
const words = {
  zh: { head: '今天 agent 干活', total: n => `共 ${n} 分钟`, none: '今天还没开工', min: n => `${n} 分` },
  en: { head: 'Agents at work today', total: n => `${n} min in all`, none: 'Nothing yet today', min: n => `${n} min` },
}
const esc = s => String(s ?? '').replace(/[&<>"']/g, c => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' })[c])

function draw() {
  const s = words[waku.lang] || words.en
  const data = waku.data || {}
  const rows = Array.isArray(data.rows) ? data.rows.slice(0, waku.size === 'l' ? 8 : 3) : []
  board.className = waku.size
  if (!rows.length) {
    board.innerHTML = `<div class="hd">${esc(s.head)}</div><div class="none">${esc(s.none)}</div>`
    return
  }
  const most = Math.max(1, ...rows.map(r => Number(r.minutes) || 0))
  // The bars as they were (an update grows them from there), new ones from nothing.
  const was = new Map([...board.querySelectorAll('.row')].map(el => [el.dataset.project, el.querySelector('.bar').style.width]))
  board.innerHTML = `<div class="hd"><span>${esc(s.head)}</span><span class="total">${esc(s.total(data.total || 0))}</span></div><div class="rows">${rows
    .map(
      (r, i) =>
        `<div class="row${i === 0 ? ' first' : ''}${r.on ? ' on' : ''}" data-project="${esc(r.project)}"><span class="name">${esc(r.project)}</span><span class="track"><i class="bar" style="width:${was.get(String(r.project)) || '0%'}"></i></span><span class="min">${esc(s.min(r.minutes))}</span></div>`,
    )
    .join('')}</div>`
  // Next frame: each bar to its length, once (its transition, then still).
  requestAnimationFrame(() => {
    board.querySelectorAll('.row').forEach((el, i) => {
      el.querySelector('.bar').style.width = `${Math.max(4, (100 * (Number(rows[i].minutes) || 0)) / most)}%`
    })
  })
}

waku.on('data', draw)
waku.on('lang', draw)
draw()
// Its main told it is up, and at which size (waku.on('view') there).
waku.send({ shown: waku.size })
