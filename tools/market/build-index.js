// Builds the market from its packages: each checked (check.js's rules, its
// folder named after its id), zipped the same way every time, and the
// index.json the app reads, listing them. Nothing is written when any
// package fails.
//
//   node tools/market/build-index.js --packages <dir> --out <dir>
//        [--previous <index.json>] [--changed <file>] [--now <ISO time>]
//
//   <dir>/<id>/       one folder per package, its files at its top
//   <out>/<id>-<version>.zip, <out>/index.json
//   --previous   the index as published (default: <out>/index.json when
//                there is one): a version already in it keeps its `updated`
//   --changed    a file to write the zips' names that are new or whose bytes
//                differ from the previous index's, one a line (to upload only
//                those)
//   --now        the time to build at, for tests
//
// index.json:
//   { "format": 1, "generated": "<ISO time>", "items": [ { "id", "kind",
//     "name", "description", "author", "homepage", "version", "api",
//     "category", "sizes", "color", "view", "permissions", "file", "size",
//     "sha256", "icon", "updated" } ] }
// items by kind (plugin, pet, wallpaper), then id. `name` and `description`
// as the manifest has them (a string, or { en, zh }); `homepage`, `api`,
// `category`, `sizes` and `color` null where the package has none (a pack
// has none of the last four); `view` true for a plugin with a view.html;
// `permissions` only what it asks for ({ net: [...], agents: true, ... },
// {} for none); `file` relative to index.json; `icon` the widget's .svg icon
// (widget.icon) of 8 KB or less as a data: URI, else null; `updated` the day (UTC) that version was
// first built.
const fs = require('node:fs')
const path = require('node:path')
const pkg = require('./package')
const { lines } = require('./check')

const KIND_ORDER = ['plugin', 'pet', 'wallpaper']

function itemOf(r, file, zip, updated) {
  const m = r.manifest
  const plugin = r.kind === 'plugin'
  const w = (plugin && m.widget) || {}
  return {
    id: m.id,
    kind: r.kind,
    name: m.name,
    description: m.description,
    author: m.author,
    homepage: m.homepage || null,
    version: m.version,
    api: Number.isInteger(m.api) ? m.api : null,
    category: plugin ? m.category : null,
    sizes: plugin ? w.sizes || ['s', 'm'] : null,
    color: plugin ? w.color || null : null,
    view: plugin && typeof m.view === 'string',
    permissions: plugin ? pkg.permissionsOf(m) : {},
    file,
    size: zip.length,
    sha256: pkg.sha256(zip),
    icon: plugin ? pkg.iconOf(r) : null,
    updated,
  }
}

// Builds into opts.out; { ok, index, changed, failed: [{ dir, result }], notes }.
function build(opts) {
  const now = opts.now ? new Date(opts.now) : new Date()
  if (Number.isNaN(now.getTime())) throw new Error(`not a time: ${opts.now}`)
  const today = now.toISOString().slice(0, 10)
  const dirs = fs.readdirSync(opts.packages, { withFileTypes: true })
    .filter(d => d.isDirectory() && !d.name.startsWith('.'))
    .map(d => path.join(opts.packages, d.name))
    .sort()
  const results = dirs.map(dir => ({ dir, result: pkg.read(dir, { market: true }) }))
  const failed = results.filter(x => x.result.errors.length)
  if (failed.length) return { ok: false, failed, results, index: null, changed: [], notes: [] }

  const prevFile = opts.previous || path.join(opts.out, 'index.json')
  let previous = []
  if (fs.existsSync(prevFile)) {
    const p = JSON.parse(fs.readFileSync(prevFile, 'utf8'))
    if (p && Array.isArray(p.items)) previous = p.items
  } else if (opts.previous) throw new Error(`no such file: ${opts.previous}`)
  const before = new Map(previous.map(i => [i.id, i]))

  const notes = []
  const changed = []
  const items = []
  const zips = []
  for (const { result: r } of results) {
    const zip = pkg.zipOf(r)
    const file = `${r.manifest.id}-${r.manifest.version}.zip`
    const was = before.get(r.manifest.id)
    const same = was && was.version === r.manifest.version
    const updated = same && /^\d{4}-\d{2}-\d{2}$/.test(was.updated) ? was.updated : today
    const item = itemOf(r, file, zip, updated)
    if (!same || was.sha256 !== item.sha256 || was.file !== file) changed.push(file)
    if (same && was.sha256 !== item.sha256) notes.push(`${file}: the same version with other bytes (packed again by another node?); uploaded again`)
    items.push(item)
    zips.push([file, zip])
  }
  items.sort((a, b) => KIND_ORDER.indexOf(a.kind) - KIND_ORDER.indexOf(b.kind) || (a.id < b.id ? -1 : a.id > b.id ? 1 : 0))
  for (const id of before.keys()) if (!items.some(i => i.id === id)) notes.push(`${id}: gone from the market`)

  const index = { format: 1, generated: now.toISOString(), items }
  fs.mkdirSync(opts.out, { recursive: true })
  for (const [file, zip] of zips) fs.writeFileSync(path.join(opts.out, file), zip)
  const tmp = path.join(opts.out, `.index.json.${process.pid}`)
  fs.writeFileSync(tmp, JSON.stringify(index, null, 2) + '\n')
  fs.renameSync(tmp, path.join(opts.out, 'index.json'))
  if (opts.changed) fs.writeFileSync(opts.changed, changed.map(f => f + '\n').join(''))
  return { ok: true, failed: [], results, index, changed, notes }
}

function main(argv) {
  const opts = {}
  for (let i = 0; i < argv.length; i++) {
    const a = argv[i]
    const key = { '--packages': 'packages', '--out': 'out', '--previous': 'previous', '--changed': 'changed', '--now': 'now' }[a]
    if (!key || argv[i + 1] === undefined) {
      console.error('usage: node tools/market/build-index.js --packages <dir> --out <dir> [--previous <index.json>] [--changed <file>] [--now <ISO time>]')
      return 2
    }
    opts[key] = argv[++i]
  }
  if (!opts.packages || !opts.out) return main([])
  const r = build(opts)
  if (!r.ok) {
    for (const { dir, result } of r.failed) console.log(lines(result, dir).join('\n') + '\n')
    console.log(`没有生成：${r.failed.length} 个包没通过 / not built: ${r.failed.length} package(s) failed`)
    return 1
  }
  for (const { dir, result } of r.results) {
    if (result.warnings.length) console.log(lines(result, dir).join('\n') + '\n')
  }
  for (const i of r.index.items) console.log(`${i.file.padEnd(40)} ${pkg.kb(i.size).padStart(9)}  ${i.sha256.slice(0, 12)}  ${i.updated}${r.changed.includes(i.file) ? '  (new)' : ''}`)
  for (const n of r.notes) console.log(`note: ${n}`)
  console.log(`${r.index.items.length} item(s), ${r.changed.length} new or changed → ${path.join(opts.out, 'index.json')}`)
  return 0
}

module.exports = { build, itemOf }

if (require.main === module) process.exitCode = main(process.argv.slice(2))
