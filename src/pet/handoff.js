// The handover between Windows' own taskbar and hers (taskbar.rs), so the
// foot of the screen is never bare. Starting, a picture of Windows' strip,
// taken before hers shows, is laid where the strip is, over it (covered:
// Windows' is hidden under it then, unseen); what is on it, its icons and
// words, crumbles into dust that lifts and settles into hers, each of her
// parts coming as its dust lands, the picture's bare ground fading into
// hers. Without a picture (none to take: Windows' strip hides itself, or was
// hidden already) the dust gathers from above. Quitting, hers crumbles the
// same way and drifts off, Windows' shown again under it once hers is bare.
// Never while the motion is to be kept down (Windows' animations off:
// prefers-reduced-motion): then each step is answered at once.
;(function () {
  if (new URLSearchParams(location.search).get('role') !== 'island') return
  const { pet } = window

  // Her strip's parts that the dust becomes (and crumbles from).
  const PARTS = '#island, #bar-rest .start, #bar-rest .win, #bar-rest .tchev, #bar-rest .ticon, #bar-rest .kbd, #bar-rest .tquick, #bar-rest .tpriv, #bar-rest .face.mon, #bar-rest .tclock, #bar-rest .gear'
  const STRIP_H = 48
  // At most this many grains; their size, and every how many is a glint.
  const MOST = 1500
  const GRAIN = 1.6
  const GLINT_EVERY = 11
  // How long a grain takes (lifting, then settling), and the spread of their starts.
  const SETTLE_MS = 1050
  const SPREAD_MS = 200
  const GLINTS = ['#fff6d6', '#ffd27a', '#ffffff', '#ffe9a8']
  const quiet = () => matchMedia('(prefers-reduced-motion: reduce)').matches

  // --- The layers: the picture where the strip is, the dust over the whole window ------
  function canvasLayer(css) {
    const c = document.createElement('canvas')
    c.style.cssText = `position:fixed;pointer-events:none;${css}`
    document.body.append(c)
    return c
  }

  let picture = null
  let dust = null
  // Each step its own: a picture still loading when the next step comes is
  // not laid (it would stay over hers).
  let step = 0
  const wait = ms => new Promise(r => setTimeout(r, ms))

  // Windows' strip, as its picture shows it, laid over where it is; resolves
  // once it is on the screen (two frames drawn, or a moment: no frames come
  // while the window is hidden).
  async function cover(url) {
    const mine = ++step
    drop()
    const img = new Image()
    img.src = url
    await img.decode()
    if (mine !== step) return
    picture = canvasLayer(`left:0;bottom:0;width:100%;height:${STRIP_H}px;z-index:2147483000`)
    picture.width = img.naturalWidth
    picture.height = img.naturalHeight
    picture.getContext('2d', { willReadFrequently: true }).drawImage(img, 0, 0)
    await Promise.race([new Promise(r => requestAnimationFrame(() => requestAnimationFrame(r))), wait(120)])
  }

  function drop() {
    picture?.remove()
    picture = null
  }

  // What is on the picture, as grains (where, in the page's px, and their
  // colour), and the picture left bare: each column its ground's colour (as
  // a little below its top edge, which no icon reaches).
  function crumble() {
    const c = picture.getContext('2d', { willReadFrequently: true })
    const { width: w, height: h } = picture
    const sx = w / innerWidth
    const sy = h / STRIP_H
    const img = c.getImageData(0, 0, w, h)
    const d = img.data
    const groundRow = Math.min(h - 1, Math.round(4 * sy))
    const ground = new Uint8ClampedArray(w * 3)
    for (let x = 0; x < w; x++) for (let k = 0; k < 3; k++) ground[x * 3 + k] = d[(groundRow * w + x) * 4 + k]
    const stride = Math.max(1, Math.round(1.5 * sy))
    const top = innerHeight - STRIP_H
    const grains = []
    for (let y = Math.round(2 * sy); y < h; y += stride)
      for (let x = 0; x < w; x += stride) {
        const i = (y * w + x) * 4
        const diff = Math.abs(d[i] - ground[x * 3]) + Math.abs(d[i + 1] - ground[x * 3 + 1]) + Math.abs(d[i + 2] - ground[x * 3 + 2])
        if (diff > 60) grains.push({ x: x / sx, y: top + y / sy, c: `rgb(${d[i]},${d[i + 1]},${d[i + 2]})` })
      }
    for (let y = Math.round(2 * sy); y < h; y++)
      for (let x = 0; x < w; x++) {
        const i = (y * w + x) * 4
        d[i] = ground[x * 3]
        d[i + 1] = ground[x * 3 + 1]
        d[i + 2] = ground[x * 3 + 2]
      }
    c.putImageData(img, 0, 0)
    return thin(grains)
  }

  const thin = list => (list.length <= MOST ? list : list.filter((_, i) => i % Math.ceil(list.length / MOST) === 0))

  // Points on her parts, as many as each one's size asks, each with a
  // colour: an icon's own (its picture's pixels), else the words' colour.
  function partsPoints(n) {
    const parts = [...document.querySelectorAll(PARTS)].filter(el => el.getClientRects().length && el.offsetWidth)
    const boxes = parts.map(el => ({ el, r: el.getBoundingClientRect(), colours: coloursOf(el) }))
    const total = boxes.reduce((a, b) => a + b.r.width * b.r.height, 0) || 1
    const out = []
    for (const b of boxes) {
      const k = Math.max(5, Math.round((n * b.r.width * b.r.height) / total))
      for (let i = 0; i < k; i++)
        out.push({ el: b.el, x: b.r.left + b.r.width * (0.15 + 0.7 * Math.random()), y: b.r.top + b.r.height * (0.2 + 0.6 * Math.random()), c: b.colours[(Math.random() * b.colours.length) | 0] })
    }
    return out
  }

  function coloursOf(el) {
    const img = el.querySelector('img')
    if (img?.complete && img.naturalWidth) {
      try {
        const c = document.createElement('canvas')
        c.width = c.height = 16
        const x = c.getContext('2d', { willReadFrequently: true })
        x.drawImage(img, 0, 0, 16, 16)
        const d = x.getImageData(0, 0, 16, 16).data
        const out = []
        for (let i = 0; i < d.length; i += 4) if (d[i + 3] > 160) out.push(`rgb(${d[i]},${d[i + 1]},${d[i + 2]})`)
        if (out.length) return out
      } catch {}
    }
    return [getComputedStyle(el).color || '#ffffff']
  }

  // --- The dust ------------------------------------------------------------------------
  // A grain: from where it was, up to a point a little above, then down to
  // where it goes, a flash there; or, with nowhere to go, up and away.
  let grains = []
  let raf = 0
  let last = 0
  let dpr = 1

  function start() {
    if (!dust) {
      dust = canvasLayer('inset:0;width:100%;height:100%;z-index:2147483001')
      dpr = devicePixelRatio || 1
      dust.width = Math.round(innerWidth * dpr)
      dust.height = Math.round(innerHeight * dpr)
    }
    if (!raf) raf = requestAnimationFrame(frame)
  }

  function frame(now) {
    const dt = Math.min(50, now - (last || now))
    last = now
    const c = dust.getContext('2d')
    c.setTransform(dpr, 0, 0, dpr, 0, 0)
    c.clearRect(0, 0, innerWidth, innerHeight)
    c.globalCompositeOperation = 'lighter'
    grains = grains.filter(g => g.life < g.max + g.delay)
    for (const g of grains) {
      g.life += dt
      const t = (g.life - g.delay) / g.max
      if (t < 0) {
        if (g.showWait) dotAt(c, g, g.sx, g.sy, 1, GRAIN)
        continue
      }
      let x
      let y
      let a = 1
      let r = GRAIN
      if (g.tx === undefined) {
        // Up and away, swaying, fading.
        const e = 1 - (1 - t) * (1 - t)
        x = g.sx + g.dx * e + Math.sin(t * 6 + g.ph) * 6
        y = g.sy - g.rise * e
        a = 1 - t
      } else if (t < 0.32) {
        const e = 1 - Math.pow(1 - t / 0.32, 2)
        x = g.sx + (g.lx - g.sx) * e
        y = g.sy + (g.ly - g.sy) * e
      } else {
        const k = Math.min(1, (t - 0.32) / 0.58)
        const e = k * k * (3 - 2 * k)
        x = g.lx + (g.tx - g.lx) * e
        y = g.ly + (g.ty - g.ly) * e
        if (t > 0.9) {
          a = 1 - (t - 0.9) / 0.1
          r = GRAIN * (1 + (2 * (t - 0.9)) / 0.1)
        }
      }
      dotAt(c, g, x, y, a, r)
    }
    if (grains.length) raf = requestAnimationFrame(frame)
    else stop()
  }

  function dotAt(c, g, x, y, a, r) {
    c.globalAlpha = Math.max(0, Math.min(1, a))
    if (g.glint) {
      const grad = c.createRadialGradient(x, y, 0, x, y, r * 3)
      grad.addColorStop(0, g.c)
      grad.addColorStop(1, 'rgba(255,255,255,0)')
      c.fillStyle = grad
      c.beginPath()
      c.arc(x, y, r * 3, 0, Math.PI * 2)
      c.fill()
    } else {
      c.fillStyle = g.c
      c.fillRect(x - r / 2, y - r / 2, r, r)
    }
  }

  function stop() {
    cancelAnimationFrame(raf)
    raf = 0
    last = 0
    grains = []
    dust?.remove()
    dust = null
  }

  // Grains from these points to those, the later the farther right they
  // start (shown where they are while they wait: they are what was there,
  // unless they come from nowhere); when each part's last grain lands.
  function settle(from, to, showWait = true) {
    const lands = new Map()
    from.forEach((p, i) => {
      const t = to[(i * 7919) % to.length]
      const max = SETTLE_MS + Math.random() * 250
      const delay = (p.x / innerWidth) * SPREAD_MS * 0.7 + Math.random() * SPREAD_MS * 0.3
      const glint = i % GLINT_EVERY === 0
      grains.push({ sx: p.x, sy: p.y, lx: p.x + (Math.random() - 0.5) * 50, ly: p.y - 18 - Math.random() * 70, tx: t.x, ty: t.y, max, delay, life: 0, glint, c: glint ? GLINTS[i % GLINTS.length] : p.c, showWait })
      if (t.el) lands.set(t.el, Math.max(lands.get(t.el) || 0, delay + max * 0.86))
    })
    start()
    return lands
  }

  // Her parts come as their dust lands (hidden till then), the picture's
  // bare ground fading into hers meanwhile.
  function arrive(lands) {
    for (const el of document.querySelectorAll(PARTS)) {
      const at = lands.get(el) ?? SETTLE_MS
      el.animate([{ opacity: 0, transform: 'scale(0.6)' }, { opacity: 1, transform: 'none' }], { duration: 320, delay: Math.max(0, at - 160), easing: 'cubic-bezier(0.3, 1.4, 0.5, 1)', fill: 'backwards' })
    }
    picture?.animate([{ opacity: 1 }, { opacity: 0 }], { duration: 600, delay: 320, easing: 'ease', fill: 'forwards' })
    // Over by then whatever came of the frames (none drawn while the window
    // is hidden or covered: the dust would stay where it was).
    const most = Math.max(SETTLE_MS + 250, ...lands.values())
    setTimeout(() => {
      drop()
      stop()
      pet.taskbar.handoff('done')
    }, most + 260)
  }

  // --- The steps ------------------------------------------------------------------------
  // in: Windows' strip's picture to lay over it (none: nothing to cover).
  // go: Windows' is hidden under it now: the dust.
  // out: quitting: hers crumbles, Windows' to be shown under it (bare).
  pet.onHandoff(async ({ step: next, picture: url } = {}) => {
    if (next === 'in') {
      if (url && !quiet()) {
        try {
          await cover(url)
        } catch {
          drop()
        }
      }
      return pet.taskbar.handoff('covered')
    }
    if (next === 'go') {
      step++
      if (quiet()) {
        drop()
        return pet.taskbar.handoff('done')
      }
      const to = partsPoints(MOST)
      if (picture) return arrive(settle(crumble(), to))
      // Nothing to crumble: the dust gathers from above.
      const from = to.map(() => ({ x: Math.random() * innerWidth, y: Math.random() * (innerHeight - STRIP_H - 40), c: GLINTS[(Math.random() * GLINTS.length) | 0] }))
      return arrive(settle(from, to, false))
    }
    if (next === 'out') {
      step++
      drop()
      if (quiet()) {
        pet.taskbar.handoff('bare')
        return pet.taskbar.handoff('done')
      }
      const row = document.getElementById('island-row')
      const from = partsPoints(MOST)
      for (const el of document.querySelectorAll(PARTS)) el.animate([{ opacity: 0 }], { duration: 1, fill: 'forwards' })
      from.forEach((p, i) => {
        const glint = i % GLINT_EVERY === 0
        grains.push({ sx: p.x, sy: p.y, dx: (Math.random() - 0.5) * 60, rise: 50 + Math.random() * 110, ph: Math.random() * 6, max: 700 + Math.random() * 300, delay: (p.x / innerWidth) * 120, life: 0, glint, c: glint ? GLINTS[i % GLINTS.length] : p.c, showWait: true })
      })
      start()
      // Hers bare: Windows' may show under it; hers fades off it.
      setTimeout(() => pet.taskbar.handoff('bare'), 160)
      row?.animate([{ opacity: 1 }, { opacity: 0 }], { duration: 480, delay: 200, easing: 'ease', fill: 'forwards' })
      setTimeout(() => {
        stop()
        pet.taskbar.handoff('done')
      }, 1000)
    }
  })
})()
