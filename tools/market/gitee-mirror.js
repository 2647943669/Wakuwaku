// Copies the built market (build-index.js's --out) to a Gitee release, for
// those in China: the same files at https://gitee.com/<owner>/<repo>/releases/
// download/market/<file>, as GitHub has them. The market repository's
// publish workflow runs it after GitHub's upload.
//
//   GITEE_TOKEN=<a Gitee private token, scope projects>
//   node tools/market/gitee-mirror.js --repo <owner/repo> --dir <out> [--changed <file>]
//        [--tag market] [--api https://gitee.com/api/v5] [--site https://gitee.com]
//
// The Gitee repository must be there with a commit on it (made with a
// README). The release is made on its default branch if missing (and its
// tag, if Gitee does not make it with the release). Then the zips Gitee does
// not have yet, and those --changed names, are put up (one of the same name
// taken away first), and index.json last, so an index never names a file
// not there yet. Without GITEE_TOKEN it does nothing. Outside China an
// upload goes at some 25 KB a second, so a big pack takes minutes.
const fs = require('node:fs')
const path = require('node:path')
const crypto = require('node:crypto')

const arg = (argv, name, fallback) => {
  const i = argv.indexOf(`--${name}`)
  return i >= 0 && argv[i + 1] !== undefined ? argv[i + 1] : fallback
}
const sleep = ms => new Promise(r => setTimeout(r, ms))

async function mirror(opts) {
  const { repo, dir, token, tag = 'market' } = opts
  const api = (opts.api || 'https://gitee.com/api/v5').replace(/\/$/, '')
  const site = (opts.site || 'https://gitee.com').replace(/\/$/, '')
  const say = opts.log || (line => console.log(line))
  const wait = opts.retryWait ?? 5000
  const headers = { Authorization: `token ${token}`, 'User-Agent': 'wakuwaku-market' }

  // A call to Gitee's API: tried again on a network error or a 5xx.
  async function call(method, url, body) {
    for (let i = 0; ; i++) {
      let res
      try {
        res = await fetch(url, { method, headers, body })
      } catch (e) {
        if (i >= 2) throw new Error(`${method} ${url}: ${e.message}`)
        await sleep(wait * (i + 1))
        continue
      }
      if (res.status >= 500 && i < 2) {
        await sleep(wait * (i + 1))
        continue
      }
      const text = await res.text()
      let json = null
      try { json = text ? JSON.parse(text) : null } catch {}
      return { status: res.status, ok: res.ok, json, text }
    }
  }
  const form = fields => new URLSearchParams(fields)
  const fail = (what, r) => new Error(`${what}: ${r.status} ${r.text.slice(0, 300)}`)

  const info = await call('GET', `${api}/repos/${repo}`)
  if (!info.ok) throw fail(`Gitee has no ${repo} (make it, with a README)`, info)
  const branch = (info.json && info.json.default_branch) || 'master'

  let got = await call('GET', `${api}/repos/${repo}/releases/tags/${encodeURIComponent(tag)}`)
  let release = got.ok && got.json && got.json.id ? got.json : null
  if (!release) {
    const fields = { tag_name: tag, name: 'Market', body: "Wakuwaku's market: index.json and the packages it lists.", prerelease: 'false', target_commitish: branch }
    let made = await call('POST', `${api}/repos/${repo}/releases`, form(fields))
    if (!(made.ok && made.json && made.json.id)) {
      // The tag first, then the release again.
      const t = await call('POST', `${api}/repos/${repo}/tags`, form({ refs: branch, tag_name: tag, tag_message: 'market' }))
      if (!t.ok) throw fail('the release could not be made, nor its tag', made)
      made = await call('POST', `${api}/repos/${repo}/releases`, form(fields))
      if (!(made.ok && made.json && made.json.id)) throw fail('the release could not be made', made)
    }
    release = made.json
    say(`made the release ${tag} on Gitee (id ${release.id})`)
  }
  const base = `${api}/repos/${repo}/releases/${release.id}/attach_files`

  // What it has already (a page is 100 at most); a name there twice (an
  // upload tried again) is taken away whole when put up again.
  const have = new Map()
  for (let page = 1; ; page++) {
    const r = await call('GET', `${base}?page=${page}&per_page=100`)
    if (!r.ok || !Array.isArray(r.json)) throw fail("the release's files could not be listed", r)
    for (const f of r.json) have.set(f.name, [...(have.get(f.name) || []), f])
    if (r.json.length < 100) break
  }

  const changed = new Set(opts.changed || [])
  const zips = fs.readdirSync(dir).filter(f => f.endsWith('.zip')).sort()
  const todo = [...zips.filter(f => !have.has(f) || changed.has(f)), 'index.json']
  const put = []
  for (const name of todo) {
    for (const old of have.get(name) || []) {
      const d = await call('DELETE', `${base}/${old.id}`)
      if (!d.ok && d.status !== 404) throw fail(`the old ${name} could not be taken away`, d)
    }
    const buf = fs.readFileSync(path.join(dir, name))
    const body = new FormData()
    body.append('file', new Blob([buf]), name)
    say(`uploading ${name} (${(buf.length / 1024).toFixed(1)} KB)…`)
    const up = await call('POST', base, body)
    if (!up.ok) throw fail(`${name} could not be uploaded`, up)
    put.push(name)
  }

  // index.json downloaded back as the app does it (no account, no API).
  let back = null
  if (opts.verify !== false) {
    const want = crypto.createHash('sha256').update(fs.readFileSync(path.join(dir, 'index.json'))).digest('hex')
    const url = `${site}/${repo}/releases/download/${tag}/index.json`
    for (let i = 0; i < 6; i++) {
      try {
        const res = await fetch(url, { headers: { 'User-Agent': 'wakuwaku-market' } })
        if (res.ok) {
          const sha = crypto.createHash('sha256').update(Buffer.from(await res.arrayBuffer())).digest('hex')
          back = sha === want
          if (back) break
        }
      } catch {}
      await sleep(wait)
    }
    say(back ? `checked: Gitee gives the same index.json (${url})` : `warning: ${url} is not the index just put up (yet?)`)
  }
  return { release: release.id, put, verified: back }
}

async function main(argv) {
  const repo = arg(argv, 'repo')
  const dir = arg(argv, 'dir')
  if (!repo || !dir) {
    console.error('usage: node tools/market/gitee-mirror.js --repo <owner/repo> --dir <out> [--changed <file>] [--tag market] [--api <url>] [--site <url>]')
    return 2
  }
  const token = process.env.GITEE_TOKEN
  if (!token) {
    console.log('no GITEE_TOKEN: nothing mirrored to Gitee')
    return 0
  }
  const changedFile = arg(argv, 'changed')
  const changed = changedFile && fs.existsSync(changedFile) ? fs.readFileSync(changedFile, 'utf8').split(/\r?\n/).filter(Boolean) : []
  const r = await mirror({ repo, dir, token, changed, tag: arg(argv, 'tag', 'market'), api: arg(argv, 'api'), site: arg(argv, 'site') })
  console.log(`Gitee: ${r.put.length} file(s) put up to ${repo}'s release (id ${r.release})`)
  return 0
}

module.exports = { mirror }

if (require.main === module) {
  main(process.argv.slice(2)).then(code => { process.exitCode = code }, e => {
    console.error(e.message)
    process.exitCode = 1
  })
}
