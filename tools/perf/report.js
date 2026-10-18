// The A/B table of a performance run (docs/PERFORMANCE.md sections 4 and 5):
// per scene and metric, A's and B's medians, B/A, and whether B passes.
//
//   node tools/perf/report.js <a.json> <b.json> [--md <out.md>] [--json <out.json>]
//
// a.json and b.json are run.ps1's, one per side: { label, version, commit,
// machine, scenes: { S1: { quiet, runs: [{ totals, animations, event? }] },
// S7: { starts: [{ kind, ... }] } } }. The table goes to stdout (Markdown);
// the exit code is 1 when anything fails.
//
// The rules, B against A by medians:
//   Mcyc/s, ms/s   B <= 110% of A, or B - A < 0.5 ms/s ("A itself small");
//                  for cycles the 0.5 ms/s is turned into cycles at A's own
//                  cycles per ms of CPU (all A's runs together)
//   wakeups/s      B <= 120% of A
//   private MB     B - A < 30 MB
//   GPU %          quiet scenes only: B <= 110% of A, or within 0.1 point
//                  (a VM's GPU numbers are mostly 0)
//   animations     quiet scenes only: B has none running endlessly, but for
//                  those the scene allows
//   start (S7)     process start to page ready: warm B - A < 100 ms, cold < 300 ms
const fs = require('node:fs')

const METRICS = [
  { key: 'mcycPerSec', name: 'Mcyc/s', digits: 1 },
  { key: 'msPerSec', name: 'ms/s', digits: 2 },
  { key: 'wakeupsPerSec', name: 'wakeups/s', digits: 0 },
  { key: 'privateMB', name: 'private MB', digits: 0 },
  { key: 'workingSetMB', name: 'working set MB', digits: 0 },
  { key: 'gpuPct', name: 'GPU %', digits: 2 },
]
const SEGMENTS = [
  { key: 'procToUp', name: 'start → up' },
  { key: 'procToReady', name: 'start → page ready', rule: true },
  { key: 'readyToTaken', name: 'page ready → taskbar taken' },
  { key: 'procToTaken', name: 'start → taskbar taken' },
]

function median(xs) {
  const v = xs.filter((x) => typeof x === 'number' && Number.isFinite(x)).sort((a, b) => a - b)
  if (!v.length) return null
  const mid = Math.floor(v.length / 2)
  return v.length % 2 ? v[mid] : (v[mid - 1] + v[mid]) / 2
}

function fmt(x, digits) {
  if (x === null || x === undefined) return '–'
  return x.toFixed(digits)
}

function ratio(a, b) {
  if (a === null || b === null) return '–'
  if (a === 0) return b === 0 ? '1.00' : '–'
  return (b / a).toFixed(2)
}

// Cycles per ms of CPU: the machine's rated clock (QueryProcessCycleTime
// counts at it); else A's runs' cycles over their CPU time, all together.
// Not each run's own: a quiet process's CPU time is counted in 15.6 ms
// ticks, and work begun on a tick and done before the next is mostly not
// counted, so its ms/s reads low.
function cyclesPerMs(side) {
  if (side.machine && side.machine.mhz > 0) return side.machine.mhz / 1000
  let cyc = 0
  let ms = 0
  for (const scene of Object.values(side.scenes || {})) {
    for (const run of scene.runs || []) {
      if (!run.totals) continue
      cyc += run.totals.mcycPerSec || 0
      ms += run.totals.msPerSec || 0
    }
  }
  return ms > 0 ? cyc / ms : 3
}

function judge(metric, a, b, quiet, perMs) {
  if (a === null || b === null) return { pass: null }
  switch (metric) {
    case 'mcycPerSec': {
      const limit = Math.max(a * 1.1, a + 0.5 * perMs)
      return { pass: b <= limit, limit }
    }
    case 'msPerSec':
      return { pass: b <= Math.max(a * 1.1, a + 0.5), limit: Math.max(a * 1.1, a + 0.5) }
    case 'wakeupsPerSec':
      return { pass: b <= a * 1.2, limit: a * 1.2 }
    case 'privateMB':
      return { pass: b - a < 30, limit: a + 30 }
    case 'gpuPct':
      if (!quiet) return { pass: null }
      return { pass: b <= Math.max(a * 1.1, a + 0.1), limit: Math.max(a * 1.1, a + 0.1) }
    default:
      return { pass: null }
  }
}

function mark(pass) {
  if (pass === null || pass === undefined) return ''
  return pass ? 'pass' : '**FAIL**'
}

function report(A, B) {
  const rows = []
  const fails = []
  const perMs = cyclesPerMs(A)
  const order = (k) => {
    const m = /^S(\d+)(.*)$/.exec(k)
    return m ? Number(m[1]) * 10 + (m[2] ? 1 : 0) : 999
  }
  const names = [...new Set([...Object.keys(A.scenes || {}), ...Object.keys(B.scenes || {})])].sort((x, y) => order(x) - order(y))
  for (const name of names) {
    const sa = (A.scenes || {})[name] || {}
    const sb = (B.scenes || {})[name] || {}
    const title = sb.title || sa.title || ''
    if (sa.starts || sb.starts) {
      for (const kind of ['warm', 'cold']) {
        for (const seg of SEGMENTS) {
          const pick = (s) => median((s.starts || []).filter((x) => x.kind === kind).map((x) => x[seg.key]))
          const a = pick(sa)
          const b = pick(sb)
          if (a === null && b === null) continue
          let pass = null
          if (seg.rule && a !== null && b !== null) pass = b - a < (kind === 'warm' ? 100 : 300)
          rows.push({ scene: name, title, metric: `${kind}: ${seg.name} ms`, a: fmt(a, 0), b: fmt(b, 0), ratio: ratio(a, b), pass })
          if (pass === false) fails.push(`${name} ${kind} ${seg.name}`)
        }
      }
      continue
    }
    const quiet = Boolean(sb.quiet ?? sa.quiet)
    for (const m of METRICS) {
      const a = median((sa.runs || []).map((r) => r.totals && r.totals[m.key]))
      const b = median((sb.runs || []).map((r) => r.totals && r.totals[m.key]))
      if (a === null && b === null) continue
      const { pass } = m.key === 'workingSetMB' ? { pass: null } : judge(m.key, a, b, quiet, perMs)
      rows.push({ scene: name, title, metric: m.name, a: fmt(a, m.digits), b: fmt(b, m.digits), ratio: ratio(a, b), pass })
      if (pass === false) fails.push(`${name} ${m.name}`)
    }
    // Endless animations: the most any run saw, and whether B's are allowed.
    const most = (s) => {
      const counts = (s.runs || []).map((r) => (r.animations ? r.animations.count : null)).filter((x) => x !== null)
      return counts.length ? Math.max(...counts) : null
    }
    const a = most(sa)
    const b = most(sb)
    if (a !== null || b !== null) {
      const disallowed = (sb.runs || []).some((r) => r.animations && r.animations.disallowed > 0)
      const pass = quiet && b !== null ? !disallowed : null
      rows.push({ scene: name, title, metric: 'endless animations', a: a === null ? '–' : String(a), b: b === null ? '–' : String(b), ratio: '', pass })
      if (pass === false) fails.push(`${name} endless animations`)
    }
    // A scene's event (S6: the ring after "done"): what the window cost, all of it.
    const ev = (s, k) => median((s.runs || []).map((r) => r.event && r.event.totals && r.event.totals[k] * r.event.seconds))
    const ea = ev(sa, 'mcycPerSec')
    const eb = ev(sb, 'mcycPerSec')
    if (ea !== null || eb !== null) {
      const secs = median((sb.runs || []).map((r) => r.event && r.event.seconds)) || median((sa.runs || []).map((r) => r.event && r.event.seconds))
      const pa = judge('mcycPerSec', ea, eb, false, perMs * (secs || 1))
      rows.push({ scene: name, title, metric: `event, Mcyc in ${fmt(secs, 0)} s`, a: fmt(ea, 0), b: fmt(eb, 0), ratio: ratio(ea, eb), pass: pa.pass })
      if (pa.pass === false) fails.push(`${name} event Mcyc`)
      const ma = ev(sa, 'msPerSec')
      const mb = ev(sb, 'msPerSec')
      const pm = ma !== null && mb !== null ? { pass: mb <= Math.max(ma * 1.1, ma + 0.5 * (secs || 1)) } : { pass: null }
      rows.push({ scene: name, title, metric: `event, CPU ms in ${fmt(secs, 0)} s`, a: fmt(ma, 0), b: fmt(mb, 0), ratio: ratio(ma, mb), pass: pm.pass })
      if (pm.pass === false) fails.push(`${name} event ms`)
    }
  }
  return { rows, fails, perMs }
}

function markdown(A, B, { rows, fails, perMs }) {
  const who = (s) => `${s.label || '?'}${s.commit ? ` (${s.commit})` : ''}`
  const out = []
  out.push(`A: ${who(A)}  B: ${who(B)}`)
  if (B.machine) out.push(`Machine: ${B.machine.name || '?'}, ${B.machine.cpus || '?'} CPUs, ${(B.machine.displays || []).join(' + ')}`)
  out.push(`Medians of ${Math.max(...Object.values(B.scenes || {}).map((s) => (s.runs || []).length), 0)} runs a side; 0.5 ms/s taken as ${(0.5 * perMs).toFixed(2)} Mcyc/s`)
  out.push('')
  out.push('| Scene | Metric | A | B | B/A | Result |')
  out.push('|---|---|---:|---:|---:|---|')
  let last = ''
  for (const r of rows) {
    const scene = r.scene === last ? '' : `${r.scene} ${r.title}`.trim()
    last = r.scene
    out.push(`| ${scene} | ${r.metric} | ${r.a} | ${r.b} | ${r.ratio} | ${mark(r.pass)} |`)
  }
  out.push('')
  out.push(fails.length ? `FAIL: ${fails.join('; ')}` : 'All pass.')
  return out.join('\n')
}

function main(argv) {
  const files = []
  const opts = {}
  for (let i = 0; i < argv.length; i++) {
    if (argv[i] === '--md' || argv[i] === '--json') opts[argv[i].slice(2)] = argv[++i]
    else files.push(argv[i])
  }
  if (files.length !== 2) {
    console.error('node tools/perf/report.js <a.json> <b.json> [--md <out.md>] [--json <out.json>]')
    return 2
  }
  const [A, B] = files.map((f) => JSON.parse(fs.readFileSync(f, 'utf8').replace(/^﻿/, '')))
  const result = report(A, B)
  const md = markdown(A, B, result)
  console.log(md)
  if (opts.md) fs.writeFileSync(opts.md, md + '\n')
  if (opts.json) fs.writeFileSync(opts.json, JSON.stringify({ a: A.label, b: B.label, rows: result.rows, fails: result.fails }, null, 2))
  return result.fails.length ? 1 : 0
}

if (require.main === module) process.exitCode = main(process.argv.slice(2))
module.exports = { median, judge, report, markdown }
