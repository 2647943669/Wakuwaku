// One scene as the part of the screen it shows (its view): the wallpaper
// where it lies on the 1600 x 900 screen, under it Windows' acrylic where
// the island asks for it (island_plate), and over it her page's window
// (page.html in a frame) where her home has it. Shot by render.ps1, framed
// by gallery.html.
;(function () {
  const Q = new URLSearchParams(location.search)
  const mode = Q.get('mode') === 'light' ? 'light' : 'dark'
  const scene = window.Scenes.byId(Q.get('scene') || '')
  const screenEl = document.getElementById('screen')
  const pending = document.getElementById('pending')
  if (!scene) {
    screenEl.textContent = `No scene "${Q.get('scene')}".`
    screenEl.style.color = '#fff'
    return
  }
  const [SW, SH] = window.Scenes.SCREEN
  const [vx, vy, vw, vh] = scene.view
  const [wx, wy, ww, wh] = window.Scenes.windowOf(scene)
  Object.assign(screenEl.style, { width: `${vw}px`, height: `${vh}px` })
  document.body.style.width = `${vw}px`

  const wall = document.getElementById('wall')
  Object.assign(wall.style, { left: `${-vx}px`, top: `${-vy}px`, width: `${SW}px`, height: `${SH}px`, backgroundImage: `url("${Q.get('wall') || 'assets/wall.jpg'}")` })

  const frame = document.createElement('iframe')
  Object.assign(frame.style, { left: `${wx - vx}px`, top: `${wy - vy}px`, width: `${ww}px`, height: `${wh}px` })
  frame.src = `page.html?role=${scene.page}&scene=${encodeURIComponent(scene.id)}&mode=${mode}`
  screenEl.append(frame)

  // Windows' acrylic under the island, where the page puts it (backplate.rs).
  let plate = null
  window.addEventListener('message', e => {
    const m = e.data
    if (!m || !m.wk) return
    if ('plate' in m) {
      plate?.remove()
      plate = null
      if (m.plate) {
        plate = document.createElement('div')
        plate.className = `plate${m.plate.dark ? '' : ' light'}`
        Object.assign(plate.style, { left: `${wx - vx + m.plate.x}px`, top: `${wy - vy + m.plate.y}px`, width: `${m.plate.width}px`, height: `${m.plate.height}px`, borderRadius: '8px' })
        frame.before(plate)
      }
    }
    if (m.frozen) {
      pending.remove()
      if (m.errors?.length) {
        const notes = document.createElement('pre')
        notes.id = 'notes'
        notes.textContent = m.errors.join('\n')
        screenEl.append(notes)
      }
      document.documentElement.dataset.frozen = '1'
    }
  })
})()
