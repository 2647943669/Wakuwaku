// Other people's plugins on the Plugins page and in the picker (settings.js,
// picker.js; docs/design/specs/plugins.md §3), as plain logic for the tests:
// what a plugin's row says under its name, the picker's kind it goes under,
// what it may do in words, which cards come first in the picker (one
// installed here and never placed since: 新), the steps of installing one
// from a file and the confirm layer each shows, and a folder's line that
// did not install. The words come from T(key, vars) as the page has them;
// the markup stays the page's.
;(function (root) {
  // The picker's kinds (picker.js CATS) for a manifest's category.
  const CATS = { system: 'system', productivity: 'work', info: 'info' }
  const catOf = a => CATS[a?.category] || 'info'

  // Who made it and which it is: "@lin · v1.2.0" (no author: the version).
  const byline = a => [a?.author ? `@${a.author}` : '', a?.version ? `v${a.version}` : ''].filter(Boolean).join(' · ')

  // A setting's label by its key (what is to be filled in first).
  const labelOf = (a, key) => (Array.isArray(a?.settings) ? a.settings.find(f => f.key === key)?.label : '') || key || ''

  // What its row says under its name (spec §3), as the built-ins' do
  // (settings.js pluginNote), with who made it and its version after: one
  // that waits for a newer Wakuwaku says so; one not responding, turned off
  // (and when and why, once the watchdog says: its said), with 重新启用;
  // put nowhere, 没放; put somewhere: starting, a setting to fill in first,
  // what went wrong while it shows nothing, or 运行中. placed: put
  // somewhere; mine: its widgets up now. { text, bad, revive }.
  function statusOf(a, { placed = false, mine = [], T }) {
    const by = byline(a)
    const tail = text => [text, by].filter(Boolean).join(' · ')
    if (a.state === 'unsupported') return { text: tail(T('addons.newerApp')), bad: true }
    if (a.state === 'stopped') return { text: [T('addons.stopped'), a.said, by].filter(Boolean).join(' · '), bad: true, revive: true }
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

  const api = { CATS, catOf, byline, labelOf, statusOf, permLines, isSensitive, freshFirst, badgeOf, makeNews, IDLE, verdict, next, refusedWords, layerOf, uninstallLayerOf, brokenLine, cut, fileOf }
  if (typeof module !== 'undefined' && module.exports) module.exports = api
  else {
    // The page's own: those installed since it came up (settings.js marks
    // them, picker.js shows 新 until placed).
    api.news = makeNews()
    root.AddonUI = api
  }
})(this)
