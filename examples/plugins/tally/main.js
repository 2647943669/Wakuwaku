// Tally: counts up while its widget is placed, and remembers how far it got
// (waku.storage), as its settings say: by how much, how often, and what it
// is called. It asks for no permissions: its own widget, its settings and
// its storage are all it uses. Zip this folder and install it from the
// Plugins page (docs/PLUGINS.md).
let count = (await waku.storage.get('count')) || 0
let recent = []
let stop = null

function show() {
  const { Word, Every } = waku.settings
  const every = waku.lang === 'zh' ? `每 ${Every} 秒` : `every ${Every} s`
  waku.widget.set({
    // None given: the plugin's name.
    label: Word || undefined,
    value: String(count),
    detail: { series: { points: recent.length ? recent : [count] }, extra: every },
  })
}

async function tick() {
  count += Number(waku.settings.Step) || 1
  recent = [...recent, count].slice(-20)
  await waku.storage.set('count', count)
  show()
}

// Its pace as set, from now.
function start() {
  if (stop) stop()
  stop = waku.every(Number(waku.settings.Every) || 5, tick)
  show()
}

waku.on('settings', start)
waku.on('lang', show)
start()
