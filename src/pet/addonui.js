// Other people's plugins on the Plugins page and in the picker (settings.js,
// picker.js; docs/design/specs/plugins.md §3), as plain logic for the tests:
// what a plugin's row says under its name, the picker's kind it goes under,
// what it may do in words, which cards come first in the picker (one
// installed here and never placed since: 新), the steps of installing one
// from a file and the confirm layer each shows, and a folder's line that
// did not install; the market's list, buttons, layers, words for what went
// wrong, and the widget its cards are drawn from (spec §1, §2, §5). The
// words come from T(key, vars) as the page has them; the markup stays the
// page's.
;(function (root) {
  // The picker's kinds (picker.js CATS) for a manifest's category.
  const CATS = { system: 'system', productivity: 'work', info: 'info' }
  const catOf = a => CATS[a?.category] || 'info'

  // Who made it and which it is: "@lin · v1.2.0" (no author: the version).
  const byline = a => [a?.author ? `@${a.author}` : '', a?.version ? `v${a.version}` : ''].filter(Boolean).join(' · ')

  // A setting's label by its key (what is to be filled in first).
  const labelOf = (a, key) => (Array.isArray(a?.settings) ? a.settings.find(f => f.key === key)?.label : '') || key || ''

  // A time of day as her clock says it: 10:12.
  const pad = n => String(n).padStart(2, '0')
  const clockOf = ms => {
    const d = new Date(ms)
    return Number.isFinite(d.getTime()) ? `${pad(d.getHours())}:${pad(d.getMinutes())}` : ''
  }

  // Why the watchdog stopped it (addons.rs view: stopped { why, since,
  // times }), in words: not answering, since when and how many times (spec
  // §3: 无响应，已停用 · 10:12 起 3 次没回答); too busy (§8: 太忙，已停用);
  // its page loading again and again. Without the watchdog's say (an older
  // one): not responding, and what it said.
  function stoppedWords(a, T) {
    const s = a?.stopped
    if (!s || typeof s !== 'object') return [T('addons.stopped'), a?.said].filter(Boolean).join(' · ')
    if (s.why === 'busy') return T('addons.busy')
    if (s.why === 'crashed') return T('addons.crashed')
    const time = clockOf(s.since)
    return [T('addons.stopped'), time && s.times > 0 ? T('addons.stoppedSince', { time, n: s.times }) : ''].filter(Boolean).join(' · ')
  }

  // What its row says under its name (spec §3), as the built-ins' do
  // (settings.js pluginNote), with who made it and its version after: one
  // that waits for a newer Wakuwaku says so; one the watchdog stopped, why
  // (stoppedWords), with 重新启用; put nowhere, 没放; put somewhere:
  // starting, a setting to fill in first, what went wrong while it shows
  // nothing, or 运行中. placed: put somewhere; mine: its widgets up now.
  // { text, bad, revive }.
  function statusOf(a, { placed = false, mine = [], T }) {
    const by = byline(a)
    const tail = text => [text, by].filter(Boolean).join(' · ')
    if (a.state === 'unsupported') return { text: tail(T('addons.newerApp')), bad: true }
    if (a.state === 'stopped') return { text: tail(stoppedWords(a, T)), bad: true, revive: true }
    if (!placed) return { text: tail(T('pl.unplaced')) }
    if (a.state === 'off') return { text: tail(T('pl.starting')) }
    if (a.state === 'needs') return { text: T('pl.needs', { what: labelOf(a, a.missing) }), bad: true }
    if (!mine.length) return a.said ? { text: T('pl.trouble', { line: a.said }), bad: true } : { text: tail(T('pl.starting')) }
    return { text: tail(T('pl.running')) }
  }

  // What it may do, a line each (its detail, the install's 它会：): the
  // web, by the hosts named; the agents' sessions; the hardware's readings;
  // nudges. None: an empty list (the page says it shows its own alone).
  // list: names in a row as the language says them (I18n.list).
  function permLines(perms, T, list = names => names.join(', ')) {
    const p = perms && typeof perms === 'object' ? perms : {}
    const out = []
    if (Array.isArray(p.net) && p.net.length) out.push({ icon: 'net', text: T('addons.perm.net', { hosts: list(p.net) }) })
    if (p.agents) out.push({ icon: 'agents', text: T('addons.perm.agents') })
    if (p.hardware) out.push({ icon: 'hardware', text: T('addons.perm.hardware') })
    if (p.notify) out.push({ icon: 'notify', text: T('addons.perm.notify') })
    return out
  }
  // Those asked about before installing (spec §2): all but the web.
  const isSensitive = perms => !!(perms && (perms.agents || perms.hardware || perms.notify))

  // --- The picker ------------------------------------------------------------------------
  // The cards in their order, one installed here and not placed since
  // first (fresh: its key, `plugin:<id>`), the rest as they were.
  const freshFirst = (cards, fresh) => [...cards.filter(c => fresh(c.key)), ...cards.filter(c => !fresh(c.key))]
  // Its badge where 已放 N goes: that, or 新 for a fresh one put nowhere yet.
  const badgeOf = (n, fresh, T) => (n ? { text: T('picker.placed', { n }), fresh: false } : fresh ? { text: T('addons.new'), fresh: true } : { text: '', fresh: false })

  // Those installed while the page has been up and not placed since, by id:
  // 新 in the picker until put somewhere once (spec §2). The page's alone.
  function makeNews() {
    const ids = new Set()
    return {
      add: id => ids.add(String(id)),
      has: id => ids.has(String(id)),
      // Put somewhere: new no more.
      placed: id => ids.delete(String(id)),
      ids: () => [...ids],
    }
  }

  // --- Installing from a file (spec §3) --------------------------------------------------
  // Step by step, each a plain object; next(state, event) the one after it:
  //   idle        nothing under way; done: { id, name } once one went in
  //   picking     Windows' dialog open (addon_manage pick)
  //   looking     the .zip looked at (inspect), path
  //   ask         安装「X」？ path, summary
  //   installing  path, summary (the ask, its buttons off)
  //   refused     装不了 X.zip: file, refused { reason, file, detail }
  // Events: pick; picked { path } (null: closed); looked { path, summary };
  // go; cancel; installed { result }.
  const IDLE = { step: 'idle' }
  const fileOf = path => String(path || '').split(/[\\/]/).pop()

  // What inspect said, as a step: one to ask about (a plugin), or why not
  // (a pack: not yet; no answer at all: the file not read).
  function verdict(path, summary) {
    const file = summary?.file || fileOf(path)
    if (!summary || typeof summary !== 'object') return { step: 'refused', file, refused: { reason: 'read', file: null, detail: '' } }
    if (!summary.ok) return { step: 'refused', file, refused: summary.refused || { reason: 'read', file: null, detail: '' } }
    if (summary.kind !== 'plugin') return { step: 'refused', file, refused: { reason: 'kind', file: null, detail: '' } }
    return { step: 'ask', path, summary }
  }

  function next(s, e) {
    const step = s?.step || 'idle'
    switch (e?.type) {
      case 'pick':
        return step === 'idle' ? { step: 'picking' } : s
      case 'picked':
        if (step !== 'picking') return s
        return e.path ? { step: 'looking', path: e.path } : IDLE
      case 'looked':
        return step === 'looking' && e.path === s.path ? verdict(s.path, e.summary) : s
      case 'go':
        return step === 'ask' ? { step: 'installing', path: s.path, summary: s.summary } : s
      case 'cancel':
        return step === 'ask' || step === 'refused' ? IDLE : s
      case 'installed': {
        if (step !== 'installing') return s
        const r = e.result
        if (r && r.ok) return { step: 'idle', done: { id: r.id, name: r.name || s.summary.name } }
        return { step: 'refused', file: r?.file || fileOf(s.path), refused: r?.refused || { reason: 'write', file: null, detail: r?.error || '' } }
      }
    }
    return s
  }

  // Why a package will not go in (addons.rs Refused), in words: a head (in
  // the amber note, stressed) and the rest; the Rust side's own words kept
  // where they are a developer's (a manifest's fault), its sizes read.
  function refusedWords(r, T) {
    const file = r?.file || ''
    const detail = String(r?.detail || '')
    switch (r?.reason) {
      case 'program':
        return { head: T('addons.no.programHead'), text: T('addons.no.program', { file }) }
      case 'type':
        return { head: T('addons.no.typeHead'), text: T('addons.no.type', { file }) }
      case 'path':
        return { head: '', text: T('addons.no.path', { file }) }
      case 'big': {
        const m = /over (\d+) MB( unpacked)?/.exec(detail)
        return { head: '', text: m ? T(m[2] ? 'addons.no.bigUnpacked' : 'addons.no.big', { n: m[1] }) : T('addons.no.bigAny') }
      }
      case 'files': {
        const m = /^(\d+) files, (\d+) at most/.exec(detail)
        return { head: '', text: T('addons.no.files', { n: m ? m[1] : '', most: m ? m[2] : '500' }) }
      }
      case 'zip':
        return { head: '', text: T(detail === 'ZIP64' ? 'addons.no.zip64' : detail === 'encrypted' ? 'addons.no.encrypted' : 'addons.no.zip') }
      case 'manifest':
        return { head: '', text: /no manifest\.json/.test(detail) ? T('addons.no.noManifest') : T('addons.no.manifest', { detail: cut(detail, 120) }) }
      case 'kind':
        return { head: '', text: T('addons.no.kind') }
      case 'write':
        return { head: '', text: detail ? T('addons.no.write', { detail: cut(detail, 120) }) : T('addons.no.writeAny') }
      default:
        return { head: '', text: detail ? T('addons.no.read', { detail: cut(detail, 120) }) : T('addons.no.readAny') }
    }
  }

  // The confirm layer a step shows (null: none), as the page draws it:
  // its title, the amber note (head stressed), what it will do, a note,
  // the line under it (a link to the docs: docs), its buttons ([what,
  // label, kind], kind al: the accent one, no: the red one; busy: all off).
  function layerOf(s, T, list) {
    if (s?.step === 'ask' || s?.step === 'installing') {
      const m = s.summary || {}
      const will = permLines(m.perms, T, list)
      return {
        kind: 'ask',
        title: T('addons.installAsk', { name: m.name || m.id || '' }),
        warn: { head: T('addons.unreviewedHead'), text: T('addons.unreviewedText') },
        willHead: T('addons.will'),
        will: will.length ? will.map(x => x.text) : [T('addons.perm.none')],
        notes: [m.replaces ? T('addons.replaces', { version: m.replaces }) : '', m.supported === false ? T('addons.needsNewer') : ''].filter(Boolean),
        line: null,
        busy: s.step === 'installing',
        buttons: [
          ['cancel', T('addons.cancel'), ''],
          ['go', T(s.step === 'installing' ? 'addons.installing' : 'addons.installAnyway'), 'al'],
        ],
      }
    }
    if (s?.step === 'refused') {
      const external = s.refused?.reason === 'program' || s.refused?.reason === 'type'
      return {
        kind: 'refused',
        title: T('addons.cannot', { file: s.file || '' }),
        warn: refusedWords(s.refused, T),
        willHead: '',
        will: [],
        notes: [],
        line: { text: T(external ? 'addons.refusedExternal' : 'addons.refusedDocs'), docs: T('addons.docs') },
        busy: false,
        buttons: [['cancel', T('addons.gotIt'), 'al']],
      }
    }
    return null
  }

  // Taking one away, asked first (spec §3: 卸载「X」？它存的数据也一起删掉。).
  const uninstallLayerOf = (a, T, busy = false) => ({
    kind: 'uninstall',
    title: T('addons.uninstallAsk', { name: a?.name || a?.id || '' }),
    warn: null,
    willHead: '',
    will: [],
    notes: [T('addons.uninstallNote')],
    line: null,
    busy,
    buttons: [
      ['cancel', T('addons.cancel'), ''],
      ['uninstall', T('addons.uninstall'), 'no'],
    ],
  })

  // --- What did not go in ----------------------------------------------------------------
  // A folder in plugins whose manifest will not do (addons.rs broken_view),
  // as one quiet line: 「plugins\foo 没装上：…」, the reason in words where
  // she knows it, else as the Rust side says it (for the one who wrote it),
  // short.
  function brokenLine(b, T) {
    const folder = `plugins\\${b?.folder || ''}`
    const reason = String(b?.reason || '')
    let why = cut(reason, 80)
    let m
    if (reason === 'no manifest.json') why = T('addons.why.noManifest')
    else if ((m = /^its folder is (\S+), its id (\S+)$/.exec(reason))) why = T('addons.why.folder', { folder: m[1], id: m[2] })
    else if ((m = /^(.+): not in its folder$/.exec(reason))) why = T('addons.why.missing', { file: m[1] })
    return T('addons.broken', { folder, why })
  }

  // Words cut to n characters, with … where they were longer.
  function cut(text, n) {
    const chars = [...String(text ?? '')]
    return chars.length > n ? `${chars.slice(0, n - 1).join('')}…` : chars.join('')
  }

  // What its page says of it after who made it (spec §2's 自绘界面 too):
  // its version and author, its kind in the library, and that it draws a
  // view of its own.
  const aboutOf = (a, T) => [byline(a), T(`picker.cat.${catOf(a)}`), a?.view ? T('addons.ownView') : ''].filter(Boolean).join(' · ')

  // --- The market (market.rs; spec §1, §2, §3's banner, §5) ------------------------------
  // Its categories, the chips over its list: all, widgets (plugins), pets, wallpapers.
  const MARKET_CATS = ['all', 'widget', 'pet', 'wallpaper']
  const marketCat = item => (item?.kind === 'pet' ? 'pet' : item?.kind === 'wallpaper' ? 'wallpaper' : 'widget')
  // Words the index gives as { en, zh } (or a string): hers, else English.
  const wordsIn = (v, lang) => (v && typeof v === 'object' ? v[lang] || v.en || '' : String(v ?? ''))

  // A link that opens the settings on the market, by category (spec §1):
  // widgets:market (all), widgets:market:widget (the picker's 获取更多…),
  // :pet and :wallpaper (the Look page's, phase 4). null for any other.
  function marketLink(next) {
    const m = /^widgets:market(?::(widget|pet|wallpaper))?$/.exec(String(next ?? ''))
    return m ? { cat: m[1] || 'all' } : null
  }

  // Those its list shows: of the category, with the words looked for in
  // either language's name or description, its author or its id, any case.
  function marketList(items, { cat = 'all', query = '' } = {}) {
    const q = String(query).trim().toLowerCase()
    const said = i => [i.name?.en, i.name?.zh, i.name, i.description?.en, i.description?.zh, i.description, i.author, i.id].filter(w => typeof w === 'string')
    return (Array.isArray(items) ? items : []).filter(i => (cat === 'all' || marketCat(i) === cat) && (!q || said(i).some(w => w.toLowerCase().includes(q))))
  }

  // Whether one version is newer than another, part by part (1.10.0 after 1.9.0).
  function isNewer(a, b) {
    const x = String(a ?? '').split('.').map(Number)
    const y = String(b ?? '').split('.').map(Number)
    for (let i = 0; i < Math.max(x.length, y.length); i++) if ((x[i] || 0) !== (y[i] || 0)) return (x[i] || 0) > (y[i] || 0)
    return false
  }

  // The button at its row's end, and on its page (spec §2): a download
  // under way, how far (busy: its share, then 校验中… and 安装中…); here and
  // older, 更新; here, 已安装 (on its page: 添加小工具… for a plugin, 去
  // 「外观」里选 for a pack); else 安装. installed: the version here (null:
  // none); job: its download's last word (market:progress).
  function marketButton(item, { installed = null, job = null, detail = false, T }) {
    if (job && !['done', 'failed'].includes(job.stage)) {
      const pct = job.total ? Math.min(100, Math.floor((100 * (job.got || 0)) / job.total)) : 0
      const fetching = job.stage === 'download'
      return { what: 'busy', label: fetching ? `${pct}%` : T(job.stage === 'verify' ? 'mk.verify' : 'mk.installing'), pct: fetching ? pct : 100, primary: false, disabled: true }
    }
    if (installed && isNewer(item?.version, installed)) return { what: 'update', label: T('mk.update'), pct: 0, primary: true, disabled: false }
    if (installed && !detail) return { what: 'installed', label: T('mk.installed'), pct: 0, primary: false, disabled: true }
    if (installed) return item?.kind === 'plugin' ? { what: 'add', label: T('pl.add'), pct: 0, primary: false, disabled: false } : { what: 'look', label: T('mk.toLook'), pct: 0, primary: false, disabled: false }
    return { what: 'install', label: T('mk.install'), pct: 0, primary: true, disabled: false }
  }

  // A size as the market says it: 48 KB, 3.2 MB.
  const sizeOf = (n, T) => (n >= 1048576 ? T('mk.mb', { n: (n / 1048576).toFixed(1) }) : T('mk.kb', { n: Math.max(1, Math.round((n || 0) / 1024)) }))
  // Its line in the list, under its name: its kind, 自绘界面, what it is.
  const marketAbout = (item, T, lang) => [T(`mk.kind.${item?.kind}`), item?.view ? T('addons.ownView') : '', wordsIn(item?.description, lang)].filter(Boolean).join(' · ')
  // Its page's, under its name: @author · v1.2.0 · 48 KB · 小工具 (· 自绘界面).
  const marketByline = (item, T) => [item?.author ? `@${item.author}` : '', item?.version ? `v${item.version}` : '', item?.size ? sizeOf(item.size, T) : '', T(`mk.kind.${item?.kind}`), item?.view ? T('addons.ownView') : ''].filter(Boolean).join(' · ')

  // What `wanted` asks for beyond what `had` has (a newer version's new
  // permissions), as permissions: a host none of those had covers (the
  // same name, or under a *. one, as addons.rs host_allowed), each flag
  // not had.
  function permsBeyond(had, wanted) {
    const h = had && typeof had === 'object' ? had : {}
    const w = wanted && typeof wanted === 'object' ? wanted : {}
    const rules = Array.isArray(h.net) ? h.net : []
    const covered = host => rules.some(r => r === host || (r.startsWith('*.') && host.endsWith(r.slice(1)) && host.length > r.length - 1))
    return { net: (Array.isArray(w.net) ? w.net : []).filter(x => !covered(x)), agents: !!w.agents && !h.agents, hardware: !!w.hardware && !h.hardware, notify: !!w.notify && !h.notify }
  }
  const asksAny = p => !!(p.net.length || p.agents || p.hardware || p.notify)

  // Installing one that asks for more than the web (spec §2): 安装「X」？
  // 它会：…, 取消 / 安装, over the panel. null for one asking for none of it:
  // it goes straight in (the hosts it reaches are on its page). Updating
  // one here (had: what the one here may do): asked only when the new one
  // asks for anything more, that alone listed (spec §5).
  function marketAskOf(item, T, list, lang, had = null) {
    if (!item) return null
    const name = wordsIn(item.name, lang)
    const more = had ? permsBeyond(had, item.perms) : item.perms
    if (had ? !asksAny(more) : !isSensitive(more)) return null
    return {
      kind: 'ask',
      title: T(had ? 'mk.updateAsk' : 'addons.installAsk', { name }),
      warn: null,
      willHead: T(had ? 'mk.updateWill' : 'addons.will'),
      will: permLines(more, T, list).map(x => x.text),
      notes: [],
      line: null,
      busy: false,
      buttons: [
        ['cancel', T('addons.cancel'), ''],
        ['mkgo', T(had ? 'mk.update' : 'mk.install'), 'al'],
      ],
    }
  }

  // What an update newly asks for (market.rs: "net:<host>", "agents",
  // "hardware", "notify") as permissions, for permLines.
  const permsOfNew = list => {
    const l = Array.isArray(list) ? list : []
    return { net: l.filter(p => String(p).startsWith('net:')).map(p => String(p).slice(4)), agents: l.includes('agents'), hardware: l.includes('hardware'), notify: l.includes('notify') }
  }
  // The line under 获取更多 when updates wait for the person (spec §3):
  // 「N 个插件有更新」 and 全部更新; null with none.
  const updatesBanner = (updates, T) => (Array.isArray(updates) && updates.length ? { text: T('mk.updates', { n: updates.length }), button: T('mk.updateAll') } : null)
  // 全部更新, asked first when any of them asks for more (spec §5): each of
  // those, its versions and what it newly asks for; how many others there
  // are. null: none asks for more (they go at once).
  function updatesAskOf(updates, T, list) {
    const all = Array.isArray(updates) ? updates : []
    const asking = all.filter(u => Array.isArray(u.newPerms) && u.newPerms.length)
    if (!asking.length) return null
    const rest = all.length - asking.length
    return {
      kind: 'ask',
      title: T('mk.updatesAsk', { n: all.length }),
      warn: null,
      willHead: T('mk.updatesHead'),
      will: asking.map(u => T('mk.updateLine', { name: u.name, from: u.from, to: u.to, perms: permLines(permsOfNew(u.newPerms), T, list).map(p => p.text).join(T('mk.permSep')) })),
      notes: rest > 0 ? [T('mk.updatesRest', { n: rest })] : [],
      line: null,
      busy: false,
      buttons: [
        ['cancel', T('addons.cancel'), ''],
        ['mkall', T('mk.updateAll'), 'al'],
      ],
    }
  }

  // Why the market will not show (its index's error, market.rs), in words:
  // not open yet (404: closed, no error at all); not reached, as her
  // updates say it, with the Network proxy to look at (proxy); its index
  // unreadable. null: no error.
  function marketTroubleOf(err, T) {
    if (!err || typeof err !== 'object') return null
    if (err.code === 'closed') return { closed: true, text: T('mk.closed'), note: T('mk.closedNote'), proxy: false }
    if (err.code === 'bad') return { closed: false, text: T('mk.errorBad', { detail: cut(err.detail || '', 160) }), note: '', proxy: false }
    return { closed: false, text: T('mk.error', { detail: cut(err.detail || '', 240) }), note: T('mk.proxyHint'), proxy: true }
  }
  // Why one did not go in (market.rs install's error), in words; '' for one called off.
  function marketFailOf(err, T) {
    const detail = cut(err?.detail || '', 160)
    switch (err?.code) {
      case 'pack':
        return T('mk.no.pack')
      case 'download':
        return T('mk.no.download', { detail })
      case 'size':
        return T('mk.no.size')
      case 'digest':
        return T('mk.no.digest')
      case 'mismatch':
        return T('mk.no.mismatch', { detail })
      case 'refused': {
        const w = refusedWords(err.refused || { reason: 'read', detail: err.detail }, T)
        return T('mk.no.refused', { detail: `${w.head}${w.text}` })
      }
      case 'notFound':
        return T('mk.no.notFound')
      case 'busy':
        return T('mk.no.busy')
      case 'cancelled':
        return ''
      default:
        return T('mk.no.other', { detail })
    }
  }
  // Installed: the word at the panel's foot (spec §2): a plugin, where to
  // put it; a pack, the Look page.
  const marketDoneOf = (item, T, lang) => T(item?.kind === 'plugin' ? 'addons.installed' : 'mk.installedPack', { name: wordsIn(item?.name, lang) })

  // What its widget would say (the index's sample), as her pages take a
  // widget of a plugin's: its name for a label where it gives none, its
  // sizes the manifest's, its colour and icon there unless it gives its
  // own, its own .svg as its picture; for her card templates (wdesk.js) to
  // draw before it is installed. null: no sample.
  function sampleWidget(item, lang) {
    const s = item?.sample
    if (!s || typeof s !== 'object') return null
    const icon = s.icon || item.glyph || 'dot'
    const w = {
      id: item.id,
      plugin: item.id,
      from: 'addon',
      on: true,
      label: s.label || wordsIn(item.name, lang),
      value: s.value || '',
      icon,
      color: s.color || item.color || '',
      private: false,
      detail: { ...(s.detail && typeof s.detail === 'object' ? s.detail : {}), sizes: sampleSizes(item) },
    }
    if (icon === 'dot' && item.icon) w.iconUrl = item.icon
    return w
  }
  // The sizes its cards are drawn at: those it has of s, m, l.
  const sampleSizes = item => ['s', 'm', 'l'].filter(z => (Array.isArray(item?.sizes) && item.sizes.length ? item.sizes : ['s', 'm']).includes(z))

  const api = {
    CATS, catOf, byline, labelOf, clockOf, stoppedWords, statusOf, aboutOf, permLines, isSensitive, freshFirst, badgeOf, makeNews, IDLE, verdict, next, refusedWords, layerOf, uninstallLayerOf, brokenLine, cut, fileOf,
    MARKET_CATS, marketCat, wordsIn, marketLink, marketList, isNewer, marketButton, sizeOf, marketAbout, marketByline, permsBeyond, marketAskOf, permsOfNew, updatesBanner, updatesAskOf, marketTroubleOf, marketFailOf, marketDoneOf, sampleWidget, sampleSizes,
  }
  if (typeof module !== 'undefined' && module.exports) module.exports = api
  else {
    // The page's own: those installed since it came up (settings.js marks
    // them, picker.js shows 新 until placed).
    api.news = makeNews()
    root.AddonUI = api
  }
})(this)
