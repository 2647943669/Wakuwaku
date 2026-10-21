// Checks Wakuwaku packages by docs/PLUGINS.md's rules (package.js), and
// says what it found in Chinese and English: the market's own check, and an
// author's before submitting.
//
//   node tools/market/check.js <folder|zip>... [--market] [--against <folder>] [--json]
//
//   --market    as the market's check runs it: a package's folder is named
//               after its id (packages/<id>/)
//   --against   the packages as they are now (the market's main branch, its
//               packages/ folder): a package that changed must have a
//               higher version than the one there
//   --json      the results as JSON instead of the report
//
// The exit code is 1 when any package has an error (warnings do not count),
// 2 when used wrongly. On GitHub Actions each problem is also an annotation.
const fs = require('node:fs')
const path = require('node:path')
const pkg = require('./package')

const KIND = { plugin: ['插件', 'plugin'], pet: ['宠物包', 'pet pack'], wallpaper: ['壁纸包', 'wallpaper pack'] }

// A package against its version on the market's main branch.
function against(r, base) {
  const m = r.manifest
  if (!m || typeof m.id !== 'string' || !/^\d+\.\d+\.\d+$/.test(String(m.version))) return
  const dir = path.join(base, m.id)
  if (!fs.existsSync(dir) || !fs.statSync(dir).isDirectory()) return
  const old = pkg.read(dir)
  const was = old.manifest && old.manifest.version
  if (typeof was !== 'string' || !/^\d+\.\d+\.\d+$/.test(was)) return
  const order = pkg.compareVersions(m.version, was)
  if (order < 0) {
    r.errors.push({ code: 'version-lower', file: 'manifest.json', zh: `version ${m.version} 比已经上架的 ${was} 低`, en: `version ${m.version} is lower than the published ${was}` })
  } else if (order === 0 && !pkg.sameContent(r, old)) {
    r.errors.push({ code: 'version-not-bumped', file: 'manifest.json', zh: `文件改了，version 还是 ${was}：要升版本号，装过的人才会更新`, en: `files changed but version is still ${was}: raise it, or those who installed it never get the update` })
  }
}

// The report of one package, as lines.
function lines(r, label) {
  const m = r.manifest
  const k = KIND[r.kind] || ['?', '?']
  const head = m && typeof m.id === 'string' ? `${m.id} ${typeof m.version === 'string' ? m.version : '?'} · ${k[0]} / ${k[1]}` : '?'
  const out = [`${label}  —  ${head}`]
  const where = p => (p.file ? `${p.file}: ` : '')
  for (const e of r.errors) out.push(`  错误  ${where(e)}${e.zh}`, `  error ${where(e)}${e.en}`)
  for (const w of r.warnings) out.push(`  提醒  ${where(w)}${w.zh}`, `  note  ${where(w)}${w.en}`)
  const n = r.files.length
  if (r.errors.length) out.push(`  ✗ 不通过：${r.errors.length} 个错误，${r.warnings.length} 个提醒 / failed: ${r.errors.length} error(s), ${r.warnings.length} warning(s)`)
  else out.push(`  ✓ 通过：${n} 个文件，打包 ${pkg.kb(r.packed)}，解开 ${pkg.kb(r.unpacked)}${r.warnings.length ? `，${r.warnings.length} 个提醒` : ''} / passed: ${n} files, ${pkg.kb(r.packed)} zipped, ${pkg.kb(r.unpacked)} unpacked${r.warnings.length ? `, ${r.warnings.length} warning(s)` : ''}`)
  return out
}

// GitHub Actions' annotations: on the file where there is one.
function annotations(r, label) {
  const esc = s => String(s).replace(/%/g, '%25').replace(/\r/g, '%0D').replace(/\n/g, '%0A')
  const prop = s => esc(s).replace(/:/g, '%3A').replace(/,/g, '%2C')
  const fileOf = p => {
    const isZip = /\.zip$/i.test(label)
    return (isZip || !p.file ? label : `${label}/${p.file}`).replace(/\\/g, '/')
  }
  const out = []
  for (const e of r.errors) out.push(`::error file=${prop(fileOf(e))},title=${prop(e.code)}::${esc(`${e.en} / ${e.zh}`)}`)
  for (const w of r.warnings) out.push(`::warning file=${prop(fileOf(w))},title=${prop(w.code)}::${esc(`${w.en} / ${w.zh}`)}`)
  return out
}

function check(sources, opts = {}) {
  return sources.map(source => {
    const r = pkg.read(source, { market: opts.market })
    if (opts.against) against(r, opts.against)
    return r
  })
}

function main(argv) {
  const sources = []
  const opts = {}
  let json = false
  for (let i = 0; i < argv.length; i++) {
    const a = argv[i]
    if (a === '--market') opts.market = true
    else if (a === '--json') json = true
    else if (a === '--against') opts.against = argv[++i]
    else if (a.startsWith('--')) { console.error(`unknown option ${a}`); return 2 }
    else sources.push(a)
  }
  if (!sources.length || (opts.against === undefined && argv.includes('--against'))) {
    console.error('usage: node tools/market/check.js <folder|zip>... [--market] [--against <folder>] [--json]')
    return 2
  }
  const results = check(sources, opts)
  if (json) {
    console.log(JSON.stringify(results.map((r, i) => ({ source: sources[i], id: r.manifest && r.manifest.id, version: r.manifest && r.manifest.version, kind: r.kind, files: r.files, packed: r.packed, unpacked: r.unpacked, errors: r.errors, warnings: r.warnings })), null, 2))
  } else {
    results.forEach((r, i) => console.log(lines(r, sources[i]).join('\n') + '\n'))
    if (results.length > 1) {
      const bad = results.filter(r => r.errors.length).length
      console.log(`${results.length} 个包：${results.length - bad} 个通过，${bad} 个不通过 / ${results.length} packages: ${results.length - bad} passed, ${bad} failed`)
    }
    if (process.env.GITHUB_ACTIONS === 'true') results.forEach((r, i) => annotations(r, sources[i]).forEach(l => console.log(l)))
  }
  return results.some(r => r.errors.length) ? 1 : 0
}

module.exports = { check, lines, against }

if (require.main === module) process.exitCode = main(process.argv.slice(2))
