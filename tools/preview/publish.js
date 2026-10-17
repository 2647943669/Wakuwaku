// The playground as it is published (a page with its files beside it), and
// served the same way here to try it:
//
//   node tools/preview/publish.js build <folder>        the files, files.json, the size
//   node tools/preview/publish.js serve [port] [folder]  http://127.0.0.1:<port>/
//
// build copies into <folder> what the playground needs, under the paths it
// is published at: playground.html (the page: its content alone, the host
// wraps it), preview/ (the stand-in and the playground's own scripts),
// pet/ and shared/ (her pages and every script and style they load, read
// from their own HTML), pets/claude-chan/ (her sprites), assets/ (the
// wallpaper). Her two pages get a viewport meta (published HTML starts with
// its own); nothing else is changed. files.json maps each published path but
// the page to its copy; the total size is printed.
//
// serve builds into a temporary folder (or serves <folder>) and answers only
// what files.json has (so a file left out shows here too), the page wrapped
// in a document as the host would.
const fs = require('node:fs')
const path = require('node:path')
const http = require('node:http')
const os = require('node:os')

const HERE = __dirname
const REPO = path.join(HERE, '..', '..')
const PET = path.join(REPO, 'src', 'pet')
const PREVIEW = ['page.html', 'boot.js', 'fake-tauri.js', 'fixtures.js', 'scenes.js', 'playground.js']
const PAGES = ['index.html', 'desk.html']

// Her pages' scripts and styles, as their HTML loads them (src/pet relative).
function pageFiles(page) {
  const html = fs.readFileSync(path.join(PET, page), 'utf8')
  return [...html.matchAll(/<(?:script|link)\b[^>]*\b(?:src|href)="([^"]+)"/g)].map(m => m[1]).filter(p => !/^[a-z]+:/i.test(p))
}

// Published path -> where it comes from (a file, or text made here).
function plan() {
  const out = new Map()
  out.set('playground.html', { file: path.join(HERE, 'playground.html') })
  for (const f of PREVIEW) out.set(`preview/${f}`, { file: path.join(HERE, f) })
  out.set('assets/wall.jpg', { file: path.join(HERE, 'assets', 'wall.jpg') })
  out.set('pets/claude-chan/spritesheet.webp', { file: path.join(REPO, 'pets', 'claude-chan', 'spritesheet.webp') })
  for (const page of PAGES) {
    const html = fs.readFileSync(path.join(PET, page), 'utf8')
    out.set(`pet/${page}`, { text: html.replace(/(<meta charset="utf-8"\s*\/?>)/i, '$1\n    <meta name="viewport" content="width=device-width, initial-scale=1" />') })
    for (const ref of pageFiles(page)) {
      const abs = path.resolve(PET, ref)
      const rel = path.relative(path.join(REPO, 'src'), abs).split(path.sep).join('/')
      out.set(rel, { file: abs })
    }
  }
  return out
}

function build(folder) {
  const dir = path.resolve(folder)
  fs.mkdirSync(dir, { recursive: true })
  const files = {}
  let total = 0
  for (const [pub, from] of plan()) {
    const to = path.join(dir, ...pub.split('/'))
    fs.mkdirSync(path.dirname(to), { recursive: true })
    if (from.file) fs.copyFileSync(from.file, to)
    else fs.writeFileSync(to, from.text)
    total += fs.statSync(to).size
    if (pub !== 'playground.html') files[pub] = to
  }
  fs.writeFileSync(path.join(dir, 'files.json'), JSON.stringify(files, null, 2))
  return { dir, files, total }
}

const TYPES = { '.html': 'text/html; charset=utf-8', '.js': 'text/javascript; charset=utf-8', '.css': 'text/css; charset=utf-8', '.json': 'application/json', '.jpg': 'image/jpeg', '.png': 'image/png', '.webp': 'image/webp' }

// The page as the host wraps it: its content in a document of its own.
const wrap = content => `<!doctype html>\n<html lang="zh-CN">\n<head>\n<meta charset="utf-8">\n<meta name="viewport" content="width=device-width, initial-scale=1, viewport-fit=cover">\n</head>\n<body>\n${content}\n</body>\n</html>\n`

function serve(port, folder) {
  const { dir, files } = folder && fs.existsSync(path.join(folder, 'files.json')) ? { dir: path.resolve(folder), files: JSON.parse(fs.readFileSync(path.join(folder, 'files.json'), 'utf8')) } : build(folder || fs.mkdtempSync(path.join(os.tmpdir(), 'wk-playground-')))
  const server = http.createServer((req, res) => {
    const url = new URL(req.url, 'http://x')
    let pub = decodeURIComponent(url.pathname).replace(/^\/+/, '')
    if (!pub) pub = 'playground.html'
    // For trying it at a width the browser's window cannot be (headless
    // Edge's is 500 px at least): the page in a frame that wide (?w=&h=&q=).
    if (pub === '__frame') {
      const w = Number(url.searchParams.get('w')) || 400
      const h = Number(url.searchParams.get('h')) || 860
      const q = url.searchParams.get('q') || ''
      res.writeHead(200, { 'content-type': TYPES['.html'], 'cache-control': 'no-store' })
      return res.end(`<!doctype html><meta charset="utf-8"><style>html,body{margin:0;background:#777}iframe{display:block;border:0}</style><iframe width="${w}" height="${h}" src="/playground.html${q ? `?${q.replace(/"/g, '')}` : ''}"></iframe>`)
    }
    if (pub === 'playground.html') {
      res.writeHead(200, { 'content-type': TYPES['.html'], 'cache-control': 'no-store' })
      return res.end(wrap(fs.readFileSync(path.join(dir, 'playground.html'), 'utf8')))
    }
    const file = files[pub]
    if (!file || !fs.existsSync(file)) {
      res.writeHead(404, { 'content-type': 'text/plain' })
      return res.end(`not published: ${pub}`)
    }
    res.writeHead(200, { 'content-type': TYPES[path.extname(file).toLowerCase()] || 'application/octet-stream', 'cache-control': 'no-store' })
    fs.createReadStream(file).pipe(res)
  })
  server.listen(port, '127.0.0.1', () => console.log(`playground: http://127.0.0.1:${port}/  (${dir})`))
}

const [cmd, a, b] = process.argv.slice(2)
if (cmd === 'build' && a) {
  const { dir, files, total } = build(a)
  console.log(`${Object.keys(files).length + 1} files into ${dir}: ${total} bytes (${(total / 1048576).toFixed(2)} MB)`)
} else if (cmd === 'serve') {
  serve(Number(a) || 8765, b)
} else {
  console.log('node tools/preview/publish.js build <folder> | serve [port] [folder]')
  process.exitCode = 1
}
