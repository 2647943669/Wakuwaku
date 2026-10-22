// Rain alert: will it rain in the next hours, and from when (Open-Meteo, no
// key needed). Its City is looked up once (Open-Meteo's geocoding), then the
// hourly chance of rain every ten minutes. The widget shows the highest
// chance in the hours it looks ahead, as her weather does its temperature:
// "<city> · 下雨", the value, the hours as a curve (M) and as rows, and a
// few words on when. With Alert on, it nudges her widget once before a
// rainy spell that starts within two hours. Permissions: those two hosts,
// and notify (docs/PLUGINS.md).
const GEO = 'https://geocoding-api.open-meteo.com/v1/search'
const FORECAST = 'https://api.open-meteo.com/v1/forecast'
// The chance (%) from which an hour counts as rainy.
const RAINY = 50

const t = (zh, en) => (waku.lang === 'zh' ? zh : en)
let place = null
let placeOf = ''

async function where(city) {
  const key = `${city}|${waku.lang}`
  if (place && placeOf === key) return place
  const r = await waku.fetch(`${GEO}?name=${encodeURIComponent(city)}&count=1&language=${waku.lang}&format=json`)
  if (!r.ok) throw new Error(`geocoding: HTTP ${r.status}`)
  const hit = (await r.json()).results?.[0]
  if (!hit) throw new Error(t(`找不到这个城市：${city}`, `no city called ${city}`))
  ;[place, placeOf] = [hit, key]
  return place
}

// The place's hour now, as Open-Meteo writes its hours (timezone=auto).
function hourThere(offsetSeconds) {
  return `${new Date(Date.now() + offsetSeconds * 1000).toISOString().slice(0, 13)}:00`
}

async function look() {
  const { City, Hours, Alert } = waku.settings
  try {
    const p = await where(City)
    const r = await waku.fetch(`${FORECAST}?latitude=${p.latitude}&longitude=${p.longitude}&hourly=precipitation_probability&forecast_days=2&timezone=auto`)
    if (!r.ok) throw new Error(`forecast: HTTP ${r.status}`)
    const data = await r.json()
    const times = data.hourly?.time || []
    const chances = data.hourly?.precipitation_probability || []
    const first = Math.max(0, times.indexOf(hourThere(data.utc_offset_seconds || 0)))
    const hours = Number(Hours) || 6
    const ahead = chances.slice(first, first + hours).map(c => Number(c) || 0)
    const most = Math.max(0, ...ahead)
    const rainy = ahead.findIndex(c => c >= RAINY)
    const when = rainy < 0 ? null : times[first + rainy].slice(11, 16)
    const extra = when === null ? t(`${hours} 小时内不太会下`, `dry for ${hours} h`) : rainy === 0 ? t('现在就可能下', 'rain likely now') : t(`${when} 起可能下雨`, `rain from ${when}`)
    const rows = ahead.slice(0, 6).map((c, i) => {
      const h = Number(times[first + i].slice(11, 13))
      return { kind: 'hour', when: waku.lang === 'zh' ? `${h}时` : `${String(h).padStart(2, '0')}:00`, icon: c >= RAINY ? 'rain' : 'cloud', value: `${c}%` }
    })
    waku.widget.set({
      label: `${p.name} · ${t('下雨', 'Rain')}`,
      value: `${most}%`,
      detail: { series: { points: ahead, unit: '%', hi: 100, lo: 0 }, rows, extra },
    })
    if (Alert && when !== null && rainy <= 2) {
      const spell = times[first + rainy]
      if ((await waku.storage.get('told')) !== spell) {
        await waku.storage.set('told', spell)
        await waku.notify(rainy === 0 ? t(`${p.name}现在可能下雨`, `${p.name}: rain likely now`) : t(`${p.name} ${when} 起可能下雨`, `${p.name}: rain likely from ${when}`))
      }
    }
  } catch (err) {
    waku.widget.set({ label: City, value: '—', detail: { extra: String(err.message).slice(0, 24) } })
    // Said under it on the Plugins page.
    throw err
  }
}

waku.on('settings', look)
waku.on('lang', look)
waku.every(600, look)
look()
