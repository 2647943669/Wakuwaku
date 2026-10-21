// A market on 127.0.0.1 for trying the app's market without publishing
// anything: a folder served where GitHub and Gitee keep the market's files
// (<base>/releases/download/market/<file>, and /<file> too), with what can go
// wrong switched on.
//
//   node tools/market/fake-market.js <dir> [port]       (port 47392; 0 for any)
//
// <dir> is a built market (index.json and the zips, build-index.js's --out),
// or packages (one folder per package): those are built first, into a
// temporary folder. Start the app with the base it prints:
//
//   WAKUWAKU_MARKET_URL=http://127.0.0.1:47392/releases/download/market/
//
// and it reads <base>index.json and <base><item.file>.
//
// What goes wrong (each on for every file, or for one: =<file>, or =*.zip):
//   missing    404
//   fail       500
//   slow       the answer starts 2 s late and comes at 64 KB a second
//   stall      no answer at all (for timeouts)
//   badsha     the file's last byte changed: same size, another SHA-256
//              (every file but index.json when on for all)
//   redirect   a 302 to /storage/<file>, as GitHub sends a download on to
//              its storage
// set by any of:
//   FAKE_MARKET=slow,badsha=rain-alert-1.2.0.zip            when it starts
//   GET /mode?slow=1 (0 to undo; /mode alone says them)     while it runs
//   a base that carries them, for one client only:
//     http://127.0.0.1:47392/@missing=index.json/releases/download/market/
//   a query on one request: ?slow, ?badsha=x.zip
// Every request is logged.
const fs = require('node:fs')
const http = require('node:http')
const os = require('node:os')
const path = require('node:path')

const MODES = ['missing', 'fail', 'slow', 'stall', 'badsha', 'redirect']
const PORT = 47392

// "slow,badsha=x.zip" as { slow: '1', badsha: 'x.zip' }.
function parseModes(text) {
  const out = {}
  for (const part of String(text || '').split(',').map(s => s.trim()).filter(Boolean)) {
    const [name, value] = part.split('=')
    if (MODES.includes(name)) out[name] = value === undefined || value === '' ? '1' : value
  }
  return out
}

// Whether a mode set to `value` is on for `file`.
function on(value, file, mode) {
  if (!value || value === '0') return false
  if (value === '1') return !(mode === 'badsha' && file === 'index.json')
  if (value.startsWith('*')) return file.endsWith(value.slice(1))
  return value === file
}

const TYPES = { '.json': 'application/json; charset=utf-8', '.zip': 'application/zip' }

// A market folder to serve: <dir> itself, or <dir>'s packages built.
function marketDir(dir) {
  if (fs.existsSync(path.join(dir, 'index.json'))) return { dir, built: false }
  const packages = fs.readdirSync(dir, { withFileTypes: true }).filter(d => d.isDirectory() && fs.existsSync(path.join(dir, d.name, 'manifest.json')))
  if (!packages.length) throw new Error(`${dir} has no index.json and no packages`)
  const { build } = require('./build-index')
  const out = fs.mkdtempSync(path.join(os.tmpdir(), 'wakuwaku-market-'))
  const r = build({ packages: dir, out })
  if (!r.ok) {
    const { lines } = require('./check')
    throw new Error(`packages do not pass:\n${r.failed.map(f => lines(f.result, f.dir).join('\n')).join('\n')}`)
  }
  return { dir: out, built: true }
}

// Starts it: { base, port, modes, close() }. opts: port (0 for any), modes
// ("slow,badsha=x.zip" or { slow: '1' }), log (false for none), slowWait
// and slowRate (ms before the answer, bytes a second).
function start(dir, opts = {}) {
  const { dir: root, built } = marketDir(dir)
  const modes = typeof opts.modes === 'object' && opts.modes ? { ...opts.modes } : parseModes(opts.modes)
  const log = opts.log === false ? () => {} : opts.log || (line => console.log(line))
  const slowWait = opts.slowWait ?? 2000
  const slowRate = opts.slowRate ?? 64 * 1024
  const sockets = new Set()
  const timers = new Set()
  const later = (ms, fn) => { const t = setTimeout(() => { timers.delete(t); fn() }, ms); timers.add(t) }

  const server = http.createServer((req, res) => {
    const url = new URL(req.url, 'http://x')
    let p = decodeURIComponent(url.pathname)
    const said = []
    const done = (status, why) => log(`${req.method} ${req.url} ${status}${why ? ` (${why})` : ''}${said.length ? ` [${said.join(', ')}]` : ''}`)

    if (p === '/mode') {
      for (const [k, v] of url.searchParams) if (MODES.includes(k)) modes[k] = v === '' ? '1' : v
      for (const k of Object.keys(modes)) if (modes[k] === '0') delete modes[k]
      res.writeHead(200, { 'Content-Type': 'application/json' })
      res.end(JSON.stringify(modes))
      return done(200)
    }

    // Modes for this request: the server's, the base's (/@…/), the query's.
    const here = { ...modes }
    const prefix = p.match(/^\/@([^/]*)(?=\/)/)
    if (prefix) {
      Object.assign(here, parseModes(prefix[1]))
      p = p.slice(prefix[0].length)
    }
    for (const [k, v] of url.searchParams) if (MODES.includes(k)) here[k] = v === '' ? '1' : v

    let file = null
    let stored = false
    let m
    if ((m = p.match(/^\/storage\/([^/]+)$/))) { file = m[1]; stored = true }
    else if ((m = p.match(/^(?:\/.*)?\/releases\/download\/market\/([^/]+)$/))) file = m[1]
    else if ((m = p.match(/^\/([^/]+)$/))) file = m[1]
    if (!file || file.startsWith('.') || file.includes('..') || !fs.existsSync(path.join(root, file)) || !fs.statSync(path.join(root, file)).isFile()) {
      res.writeHead(404, { 'Content-Type': 'text/plain' })
      res.end('Not Found')
      return done(404)
    }
    const is = mode => on(here[mode], file, mode) && (said.push(mode), true)

    if (is('stall')) return done('-', 'no answer')
    if (is('missing')) {
      res.writeHead(404, { 'Content-Type': 'text/plain' })
      res.end('Not Found')
      return done(404)
    }
    if (is('fail')) {
      res.writeHead(500, { 'Content-Type': 'text/plain' })
      res.end('Internal Server Error')
      return done(500)
    }
    if (!stored && is('redirect')) {
      const back = prefix ? prefix[0] : ''
      res.writeHead(302, { Location: `${back}/storage/${encodeURIComponent(file)}${url.search}` })
      res.end()
      return done(302)
    }
    let body = fs.readFileSync(path.join(root, file))
    if (is('badsha') && body.length) {
      body = Buffer.from(body)
      body[body.length - 1] ^= 1
    }
    const headers = { 'Content-Type': TYPES[path.extname(file)] || 'application/octet-stream', 'Content-Length': body.length }
    if (!is('slow')) {
      res.writeHead(200, headers)
      res.end(req.method === 'HEAD' ? undefined : body)
      return done(200)
    }
    later(slowWait, () => {
      res.writeHead(200, headers)
      if (req.method === 'HEAD') return res.end()
      const chunk = Math.max(1, Math.round(slowRate / 4))
      let at = 0
      const next = () => {
        if (res.destroyed) return
        res.write(body.subarray(at, at + chunk))
        at += chunk
        if (at >= body.length) return res.end()
        later(250, next)
      }
      next()
    })
    done(200)
  })
  server.on('connection', s => { sockets.add(s); s.on('close', () => sockets.delete(s)) })

  return new Promise((resolve, reject) => {
    server.once('error', reject)
    server.listen(opts.port ?? PORT, '127.0.0.1', () => {
      const port = server.address().port
      resolve({
        port,
        root,
        built,
        modes,
        base: `http://127.0.0.1:${port}/releases/download/market/`,
        close: () => new Promise(done => {
          for (const t of timers) clearTimeout(t)
          for (const s of sockets) s.destroy()
          server.close(() => {
            if (built) fs.rmSync(root, { recursive: true, force: true })
            done()
          })
        }),
      })
    })
  })
}

async function main(argv) {
  const [dir, port] = argv
  if (!dir) {
    console.error('usage: node tools/market/fake-market.js <dir> [port]')
    return 2
  }
  const m = await start(dir, { port: port === undefined ? PORT : Number(port), modes: process.env.FAKE_MARKET })
  if (m.built) console.log(`built ${dir} into ${m.root}`)
  console.log(`fake market: ${m.base}`)
  console.log(`  WAKUWAKU_MARKET_URL=${m.base}`)
  console.log(`  modes: ${Object.keys(m.modes).length ? JSON.stringify(m.modes) : 'none'} (GET /mode?<${MODES.join('|')}>=1 to change)`)
  const stop = () => m.close().then(() => process.exit(0))
  process.on('SIGINT', stop)
  process.on('SIGTERM', stop)
  return 0
}

module.exports = { start, parseModes, MODES, PORT }

if (require.main === module) {
  main(process.argv.slice(2)).then(code => { if (code) process.exitCode = code }, e => {
    console.error(e.message)
    process.exitCode = 1
  })
}
