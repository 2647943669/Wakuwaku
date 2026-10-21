# The market's tools

The market is a public repository of packages (plugins, pet packs, wallpaper packs; `docs/PLUGINS.md`) and an `index.json` built from them, published as release files on GitHub and copied to Gitee for China. These tools check packages, build the index, serve a fake market for the app's tests, and copy the market to Gitee. Node 18 or newer, no dependencies.

| File | What it does |
|---|---|
| `package.js` | Reads a package from its folder or its `.zip` and checks it by `docs/PLUGINS.md`'s rules: the manifest, file types by kind, programs by their first bytes (MZ/PE, ELF, Mach-O 32/64/fat, whatever the name), names that would land outside (`..`, absolute, drives, devices), one top folder taken away, sizes and file counts by kind, reserved ids, `net` hosts. Also writes the zip the market publishes, the same bytes for the same files. |
| `check.js` | `node tools/market/check.js <folder\|zip>... [--market] [--against <folder>] [--json]`: a report per package in Chinese and English. Exit code 1 on any error; warnings (no icon, a description over 60 characters, a `view` without `m` or `l` in `widget.sizes`, what `widget.sample` has that she would drop or cut, a pack's preview over 64 KB, bytes that do not look like their type…) do not fail it, and it says when the market's page will draw no cards (no `widget.sample`) or no picture (no `preview.*`). `--market`: the folder is named after its id. `--against`: the market's packages as on `main`; a package that changed must raise its version. |
| `build-index.js` | `node tools/market/build-index.js --packages <dir> --out <dir> [--previous <index.json>] [--changed <file>]`: checks every `<dir>/<id>/`, and only when all pass writes `<out>/<id>-<version>.zip` and `<out>/index.json`. A version already in the previous index (default: `<out>/index.json`) keeps its `updated`; `--changed` lists the zips that are new or changed, to upload only those. |
| `fake-market.js` | `node tools/market/fake-market.js <dir> [port]`: a market on 127.0.0.1 (port 47392) for the app's tests, with a 404, a 500, a slow or no answer, a wrong SHA-256 and a redirect switched on at will. See below. |
| `gitee-mirror.js` | Puts the built market on a Gitee release (the market repository's `publish.yml` runs it). |
| `template/` | The market repository's files: its README, `.github/workflows/check.yml` and `publish.yml`, `.github/wakuwaku-ref`, and `packages/`. |

## Before you submit

```
node tools/market/check.js --market path/to/packages/<id>
node tools/market/check.js my-plugin.zip            # a zip as you would send it
```

Or with a clone of the market, as its check runs: `node <Wakuwaku>/tools/market/check.js --market --against <a checkout of main>/packages packages/<id>`.

## index.json

```json
{ "format": 1, "generated": "2026-10-21T08:00:00.000Z", "items": [ {
    "id": "rain-alert", "kind": "plugin",
    "name": { "en": "Rain alert", "zh": "下雨提醒" }, "description": { "en": "…", "zh": "…" },
    "author": "lin", "homepage": "https://github.com/lin/rain-alert", "version": "1.2.0", "api": 1,
    "category": "info", "sizes": ["s", "m"], "color": "#8cc8ff", "view": true,
    "permissions": { "net": ["api.open-meteo.com"], "notify": true },
    "file": "rain-alert-1.2.0.zip", "size": 1085, "sha256": "7812d1ac…",
    "icon": "data:image/svg+xml;base64,…", "glyph": null,
    "sample": { "label": "Dresden", "value": "70%", "detail": { "extra": "17:00" } },
    "preview": null, "updated": "2026-10-21" } ] }
```

- Items by kind (`plugin`, `pet`, `wallpaper`), then id.
- `name` and `description` as the manifest has them: a string, or `{ en, zh }` with `en` always there.
- `homepage`, `api`, `category`, `sizes` and `color` are `null` where the package has none; a pack has no `api`, `category`, `sizes` or `color`.
- `view` is true for a plugin with a `view.html`.
- `permissions` holds only what is asked for, `{}` for none.
- `file` is relative to `index.json`.
- `icon` is the plugin's `.svg` icon (`widget.icon`) of 8 KB or less as a `data:` URI, else `null` (one of her own icons by name, too). Her page draws it as a picture (a CSS mask in one colour, as she draws a plugin's own icon everywhere), never as markup; the check refuses an icon with scripts, event attributes or outside references.
- `glyph` is the name of her own icon when `widget.icon` is one, else `null`.
- `sample` is the manifest's `widget.sample` (what its widget would show, in the `POST /widget` format, 4 KB at most, checked as she takes a widget), else `null`: the market's page draws the plugin's cards from it. The app takes it as a widget again (what is not a widget's dropped, words cut).
- `preview` is a pack's `preview.webp`, `.png` or `.jpg` (at its top level, 64 KB or less) as a `data:` URI, else `null`. A plugin has none.
- `updated` is the day (UTC) that version was first built.

## How the app reads it

- The base is `https://github.com/<owner>/wakuwaku-plugins/releases/download/market/`, and the same path on `https://gitee.com/`. The app fetches `<base>index.json`, then `<base><file>` for an install.
- GitHub first, Gitee first in China (as the updater decides it: `updateFrom`, or `in_china()`). The other is tried when the first fails.
- A downloaded zip is installed only when its SHA-256 is the index's `sha256` (and its size the index's `size`), and its manifest's `id` and `version` are the item's. Then it is checked by the same rules as a zip the user chose.
- A `404` for `index.json` (the repository not made yet) is the market not open yet, not an error.
- `WAKUWAKU_MARKET_URL` replaces the base (one base, no second place), for tests with the fake market; redirects to `http://` are followed then. `WAKUWAKU_MARKET_LOOK=<first>[,<every>]` sets the update looks in seconds (3 minutes after start, then daily, by default).

## The fake market

```
node tools/market/build-index.js --packages examples/plugins --out <dir>
node tools/market/fake-market.js <dir>          # or the packages folder: built first
WAKUWAKU_MARKET_URL=http://127.0.0.1:47392/releases/download/market/
```

It serves `<dir>` at `/releases/download/market/<file>` (under any prefix, as GitHub's `/<owner>/<repo>/…`) and at `/<file>`. What goes wrong is `missing` (404), `fail` (500), `slow` (2 s, then 64 KB/s), `stall` (no answer), `badsha` (the last byte changed) and `redirect` (a 302 to `/storage/<file>`), each for every file (`=1`) or one (`=<file>`, `=*.zip`). They are set when it starts (`FAKE_MARKET=slow,badsha=rain-alert-1.2.0.zip`), while it runs (`GET /mode?slow=1`, `0` to undo), for one client by its base (`http://127.0.0.1:47392/@missing=index.json/releases/download/market/`), or on one request (`?badsha`). JS tests can start it in-process: `require('./tools/market/fake-market').start(dir, { port: 0 })` resolves to `{ base, close() }`.

## Setting up the market repository

1. On GitHub, create the public repository `wakuwaku-plugins` (empty).
2. Copy `template/`'s files into it (`.github/` and `.gitattributes` too). Put the Wakuwaku release whose `tools/market/` should check it in `.github/wakuwaku-ref` (a tag that has this folder), and push to `main`. `publish.yml` makes the release `market` and publishes the first index.
3. Protect `main`: pull requests only, the `check` workflow required.
4. On Gitee, create the repository (the same name, public) with a README, so it has a commit to tag.
5. Create a Gitee private token (scope `projects`) and add it to the GitHub repository as the secret `GITEE_TOKEN`. If the Gitee repository is not `<same owner>/wakuwaku-plugins`, set the repository variable `GITEE_REPO` to its `owner/name`.
6. Run `publish` once by hand (Actions › publish › Run workflow) and check `https://gitee.com/<owner>/wakuwaku-plugins/releases/download/market/index.json`.

The zips are built with the node version the workflows pin, so the same package keeps the same SHA-256. When a newer node packs a version differently, `build-index.js` says so and uploads it again.
