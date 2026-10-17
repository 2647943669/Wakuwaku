// The gallery's scenes: each one surface of hers, in one state, as a part of
// a 1600 x 900 screen (its view), drawn by her real page (page.html) with
// the world a scene sets (fake-tauri.js, fixtures.js) and what it then does
// (steps: a hover, a click, an event); shot in Windows' dark and light mode
// (render.ps1), and shown together in gallery.html.
//
// A scene:
//   id        its name, and the PNG's (<id>-dark.png, <id>-light.png)
//   section   which part of the sheet (SECTIONS)
//   title     what it shows, as the sheet labels it
//   page      the page that draws it: island (index.html, her home's window),
//             desk (desk.html, the desktop's widgets), bar (bar.html, her
//             taskbar on another display)
//   home      her home: taskbar (a strip along the bottom) or island (the top)
//   view      [x, y, w, h]: the part of the screen shot, in screen px
//   world     F => ({...}): the world it starts from (Fake.set; F: Fixtures)
//   steps     async p => {...}: what it does once the page is up (p: boot.js)
//   budget    the page's time in ms (virtual: a timer of 3 s takes none of
//             the clock's) until it is shot; the page is frozen before it
//   settle    ms after the steps before the page is frozen (default 500)
//
// Node reads the list (render.ps1): node scenes.js --list.
;(function (root) {
  const SCREEN = [1600, 900]
  const STRIP = 48
  // Her home's window on the screen (island.rs): the taskbar's the screen
  // wide and the strip with its room above (640); the island's at the top
  // in the middle (760 wide, as tall as it grows: kept tall here).
  const WINDOWS = {
    taskbar: [0, SCREEN[1] - STRIP - 640, SCREEN[0], STRIP + 640],
    island: [(SCREEN[0] - 760) / 2, 0, 760, 720],
    desk: [0, 0, SCREEN[0], SCREEN[1]],
    // Her taskbar on another display (bars.rs): the strip alone.
    bar: [0, SCREEN[1] - STRIP, SCREEN[0], STRIP],
  }
  const windowOf = s => (s.page === 'desk' || s.page === 'bar' ? WINDOWS[s.page] : WINDOWS[s.home || 'taskbar'])

  const SECTIONS = [
    ['taskbar', '任务栏条带'],
    ['capsule', '任务栏上的胶囊'],
    ['island', '屏幕顶部的灵动岛'],
    ['flow', '流光提醒'],
    ['menus', '菜单'],
    ['settings', '设置'],
    ['controls', '控件'],
    ['widgets', '小工具'],
    ['desktop', '桌面'],
  ]

  // The strip's foot is the screen's: a view's top this far above it.
  const above = h => SCREEN[1] - h

  // What every scene on her home's page starts with: widgets, the plugins,
  // the taskbar's programs, tray and keyboard, Windows 11's layout.
  const home = F => ({
    widgets: F.widgets('monitor', 'weather', 'today', 'inbox-a1', 'stock-aapl', 'deploy'),
    plugins: F.PLUGINS,
    windows: F.WINDOWS,
    tray: F.TRAY,
    keys: F.KEYS,
    taskbarLayout: F.TASKBAR.win11,
  })
  // A session in a mood, the island left to say it as it would.
  const inMood = (mood, extra = {}) => F => ({ ...home(F), sessions: [F.session(mood)], asks: mood === 'waiting' ? [F.ASK] : [], ...extra })
  // An ending as a dot alone (endCue none): the closed island as it stays.
  const ended = mood => F => ({ ...inMood(mood)(F), config: { endCue: 'none' } })
  const busy = F => ({ ...home(F), sessions: [F.session('working'), ...F.others()] })

  const openIsland = async p => {
    p.hover('#island')
    await p.wait(400)
  }
  const openCard = async p => {
    await openIsland(p)
    p.click('#island-expanded [data-widget="weather"]')
    await p.wait(500)
  }
  const openIsleMenu = async p => {
    await openIsland(p)
    p.context('#island-expanded [data-widget="weather"]', 60, 12)
    await p.wait(300)
  }

  // The capsule's and the island's states, each for one home.
  const states = (section, home, view) => [
    { id: `${section}-idle`, section, title: '收起 · 空闲（小工具轮流显示）', page: 'island', home, view: view.closed, world: F => ({ ...inMood('idle')(F), home }) },
    { id: `${section}-working`, section, title: '收起 · 干活中', page: 'island', home, view: view.closed, world: F => ({ ...inMood('working')(F), home }) },
    { id: `${section}-waiting`, section, title: '等你 · 询问卡片', page: 'island', home, view: view.ask, world: F => ({ ...inMood('waiting')(F), home }), settle: 900 },
    { id: `${section}-done`, section, title: '收起 · 搞定（留一个小点）', page: 'island', home, view: view.closed, world: F => ({ ...ended('done')(F), home }) },
    { id: `${section}-error`, section, title: '收起 · 出错（留一个小点）', page: 'island', home, view: view.closed, world: F => ({ ...ended('error')(F), home }) },
    { id: `${section}-open`, section, title: '展开 · 会话和小工具', page: 'island', home, view: view.open, world: F => ({ ...busy(F), home }), steps: openIsland },
    { id: `${section}-card`, section, title: '展开 · 天气的卡片打开', page: 'island', home, view: view.card, world: F => ({ ...busy(F), home }), steps: openCard },
  ]

  const LIST = [
    // --- The taskbar's strip ---------------------------------------------------------------
    ...[
      ['mica', 'Mica'],
      ['clear', '透明'],
      ['black', '纯色'],
    ].map(([material, name]) => ({
      id: `taskbar-${material}`,
      section: 'taskbar',
      title: `任务栏 · ${name}`,
      page: 'island',
      home: 'taskbar',
      view: [0, above(88), 1600, 88],
      world: F => ({ ...home(F), material, sessions: [F.session('working')] }),
    })),
    {
      id: 'taskbar-side-screen',
      section: 'taskbar',
      title: '副屏的任务栏 · 有询问在等时她在开头',
      page: 'bar',
      view: [0, above(88), 1600, 88],
      world: F => ({ ...home(F), material: 'mica', asks: [F.ASK] }),
    },

    // --- Her capsule on the taskbar, and her island at the top ------------------------------
    ...states('capsule', 'taskbar', {
      closed: [0, above(80), 440, 80],
      ask: [0, above(230), 520, 230],
      open: [0, above(420), 520, 420],
      card: [0, above(660), 520, 660],
    }),
    ...states('island', 'island', {
      closed: [480, 0, 640, 60],
      ask: [500, 0, 600, 210],
      open: [520, 0, 560, 400],
      card: [520, 0, 560, 600],
    }),

    // --- The flow light ----------------------------------------------------------------------
    // A turn done: the ring paused part of the way round, then the dot it leaves.
    {
      id: 'flow-ring',
      section: 'flow',
      title: '流光 · 光环转到一半',
      page: 'island',
      home: 'taskbar',
      view: [0, above(80), 440, 80],
      world: F => ({ ...home(F), sessions: [F.session('working')] }),
      steps: async p => {
        await p.wait(300)
        p.set({ sessions: [p.F.session('done')] })
        p.push()
        await p.wait(50)
        p.freeze({ flow: 1100 })
      },
    },
    {
      id: 'flow-dot',
      section: 'flow',
      title: '流光 · 播完留下的小点',
      page: 'island',
      home: 'taskbar',
      view: [0, above(80), 440, 80],
      budget: 6000,
      world: F => ({ ...home(F), sessions: [F.session('working')] }),
      steps: async p => {
        await p.wait(300)
        p.set({ sessions: [p.F.session('done')] })
        p.push()
        await p.wait(3400)
      },
    },
    {
      id: 'flow-ring-island',
      section: 'flow',
      title: '流光 · 顶部灵动岛上',
      page: 'island',
      home: 'island',
      view: [480, 0, 640, 60],
      world: F => ({ ...home(F), home: 'island', sessions: [F.session('working')] }),
      steps: async p => {
        await p.wait(300)
        p.set({ sessions: [p.F.session('done')] })
        p.push()
        await p.wait(50)
        p.freeze({ flow: 1100 })
      },
    },

    // --- Menus -------------------------------------------------------------------------------
    // Her own right-click menu is Windows' (tray.rs): not a page's, not here.
    {
      id: 'menu-capsule',
      section: 'menus',
      title: '胶囊里小工具的右键菜单',
      page: 'island',
      home: 'taskbar',
      view: [0, above(460), 720, 460],
      world: busy,
      steps: openIsleMenu,
    },
    {
      id: 'menu-taskbar',
      section: 'menus',
      title: '任务栏上小工具的右键菜单',
      page: 'island',
      home: 'taskbar',
      view: [880, above(420), 720, 420],
      world: F => ({ ...home(F), taskbarLayout: F.TASKBAR.widgets, widgets: F.widgets('monitor', 'weather', 'today', 'inbox-a1', 'stock-aapl', 'stock-nvda', 'countdown'), sessions: [F.session('working')] }),
      steps: async p => {
        await p.wait(200)
        p.context('[data-entry="wtw"]', 12, 12)
        await p.wait(300)
      },
    },
    {
      id: 'menu-app',
      section: 'menus',
      title: '程序按钮的右键菜单',
      page: 'island',
      home: 'taskbar',
      view: [480, above(360), 640, 360],
      world: F => ({ ...home(F), sessions: [F.session('working')] }),
      steps: async p => {
        await p.wait(200)
        p.context('#bar-rest [data-app="code"]', 20, 20)
        await p.wait(400)
      },
    },

    // --- The settings ------------------------------------------------------------------------
    ...[
      ['now', 'general', '通用'],
      ['widgets', 'plugins', '插件'],
    ].map(([tab, id, name]) => ({
      id: `settings-${id}`,
      section: 'settings',
      title: `设置 · ${name}`,
      page: 'island',
      home: 'taskbar',
      view: [0, above(660), 560, 660],
      world: F => ({ ...busy(F), widgets: F.widgets('monitor', 'today', 'weather', 'stock-aapl', 'stock-nvda', 'pomodoro', 'countdown', 'inbox-a1', 'deploy') }),
      steps: async p => {
        p.emit('island:settings', { tab })
        await p.wait(600)
      },
    })),

    // --- Controls, from the settings ---------------------------------------------------------
    {
      id: 'controls',
      section: 'controls',
      title: '开关、分段、按钮、输入框、下拉',
      page: 'island',
      home: 'taskbar',
      view: [0, above(660), 560, 660],
      world: busy,
      steps: async p => {
        p.emit('island:settings', { tab: 'now' })
        await p.wait(600)
        p.specimen()
        await p.wait(100)
        p.click('#island-settings .s-body [data-mail-agent]')
        await p.wait(200)
      },
    },

    // --- Widgets -----------------------------------------------------------------------------
    {
      id: 'widgets-sizes',
      section: 'widgets',
      title: '桌面上的小 / 中 / 大：天气、系统监控、Claude 今天、脚本小工具（字母标记）',
      page: 'desk',
      view: [0, 0, 1380, 740],
      budget: 4000,
      world: F => ({ widgets: F.WIDGETS, deskLayout: F.DESK.sizes }),
    },
    {
      id: 'widgets-taskbar',
      section: 'widgets',
      title: '任务栏上：一个中档、一个轮播、一个叠放',
      page: 'island',
      home: 'taskbar',
      view: [760, above(88), 840, 88],
      world: F => ({ ...home(F), taskbarLayout: F.TASKBAR.widgets, widgets: F.widgets('monitor', 'weather', 'today', 'inbox-a1', 'stock-aapl', 'stock-nvda', 'countdown'), sessions: [F.session('working')] }),
    },
    {
      id: 'widgets-stack-open',
      section: 'widgets',
      title: '任务栏上的叠放点开',
      page: 'island',
      home: 'taskbar',
      view: [880, above(400), 720, 400],
      world: F => ({ ...home(F), taskbarLayout: F.TASKBAR.widgets, widgets: F.widgets('monitor', 'weather', 'today', 'inbox-a1', 'stock-aapl', 'stock-nvda', 'countdown'), sessions: [F.session('working')] }),
      steps: async p => {
        await p.wait(200)
        p.click('.tw-stack')
        await p.wait(500)
      },
    },

    // --- The desktop -------------------------------------------------------------------------
    {
      id: 'desktop',
      section: 'desktop',
      title: '单个、叠放、工作台（展开和折叠），在壁纸上',
      page: 'desk',
      view: [0, 0, 1240, 620],
      budget: 4000,
      world: F => ({ widgets: F.WIDGETS, deskLayout: F.DESK.scene }),
    },
  ]

  const DEFAULT_BUDGET = 3000

  // What render.ps1 needs of each, as plain data.
  const plain = s => ({
    id: s.id,
    section: s.section,
    title: s.title,
    page: s.page,
    home: s.home || 'taskbar',
    view: s.view,
    window: windowOf(s),
    budget: s.budget || DEFAULT_BUDGET,
  })

  const api = { SCREEN, STRIP, SECTIONS, LIST, WINDOWS, DEFAULT_BUDGET, windowOf, byId: id => LIST.find(s => s.id === id), plain, list: () => LIST.map(plain) }

  if (typeof module !== 'undefined' && module.exports) {
    module.exports = api
    if (require.main === module && process.argv.includes('--list')) {
      const out = JSON.stringify({ screen: SCREEN, sections: SECTIONS, scenes: api.list() }, null, 1)
      const at = process.argv[process.argv.indexOf('--list') + 1]
      if (at && !at.startsWith('-')) require('node:fs').writeFileSync(at, out)
      else process.stdout.write(out)
    }
  } else {
    root.Scenes = api
  }
})(this)
