# Wakuwaku 插件市场

[Wakuwaku](https://github.com/zhoupengjie/Wakuwaku) 的插件、宠物包和壁纸包。每个包是 `packages/` 下的一个文件夹。合并进 `main` 后，目录 `index.json` 和每个包的 zip 会自动发到这个仓库的 release「market」，并同步一份到 Gitee，给国内用。Wakuwaku 在「插件 › 获取更多」里读它们。

## 投稿

1. Fork 这个仓库。
2. 加一个文件夹 `packages/<id>/`。文件夹名就是 `manifest.json` 里的 `id`，包里的文件直接放在这个文件夹里，`manifest.json` 在最外层。
3. 提交前先在本地检查一遍（要 Node 18 或更新）：
   ```
   git clone --depth 1 https://github.com/zhoupengjie/Wakuwaku
   node Wakuwaku/tools/market/check.js --market packages/<id>
   ```
   有「错误」就过不了；「提醒」不挡，但请看一下。
4. 提 PR。一个 PR 只放一个包。说一句它做什么；有小工具的话，附一张它在任务栏或桌面上的截图。
5. **更新**：改 `packages/<id>/` 里的文件，同时把 `version` 升上去（比如 1.2.0 → 1.2.1）。文件改了版本号没升，检查不过，因为装过的人收不到更新。

## 规则

完整的格式和规则在 Wakuwaku 仓库的 [docs/PLUGINS.md](https://github.com/zhoupengjie/Wakuwaku/blob/main/docs/PLUGINS.md)。要点：

- **永远不收可执行程序。** `.exe`、`.dll`、`.bat`、`.ps1` 之类不收；改了名字的程序也不收，检查按文件开头的字节认（Windows 的 MZ/PE、Linux 的 ELF、macOS 的 Mach-O）。要跑程序的，用本地接口 `POST /widget` 自己运行，它不进市场。
- 插件只有网页文件：`manifest.json`、`main.js`，可选 `view.html`、样式、图片、字体、声音、`.wasm`。它跑在沙盒里，读不了你的文件，也运行不了程序。
- 宠物包：`manifest.json`、`pet.json`、`spritesheet.webp` 和图片。壁纸包：`manifest.json` 和图片（`.jpg .jpeg .png .webp`）。两种都可以带 `.md`、`.txt`，都没有代码。
- 大小：插件打包后 ≤ 5 MB、解开 ≤ 20 MB；资源包 ≤ 50 MB、解开 ≤ 200 MB；最多 500 个文件。
- 联网只能访问 `permissions.net` 里写的网站：完整域名或 `*.域名`，不写端口，不能是 IP、`localhost` 或局域网。
- `id` 不能和 Wakuwaku 内置的重名。
- 小工具可以在 `widget.sample` 里写一份示例数据（`POST /widget` 的格式），市场的详情页用它画出卡片；资源包可以在最外层带一张 64 KB 以内的 `preview.webp`（或 `.png`、`.jpg`），详情页显示它。都不是必须的。

## 审核

先是 CI 自动检查（`.github/workflows/check.yml`：格式、文件类型、程序、大小、版本号），通过后维护者再看一遍代码和它要的权限，然后合并。合并后自动发布，通常几分钟内出现在 Wakuwaku 里；Gitee 那份会慢一些。

---

# Wakuwaku's market

Plugins, pet packs and wallpaper packs for [Wakuwaku](https://github.com/zhoupengjie/Wakuwaku). Each package is a folder in `packages/`. Once merged into `main`, the catalogue `index.json` and each package's zip are published to this repository's release "market", and copied to Gitee for China. Wakuwaku reads them under Plugins › Get more.

## Submitting

1. Fork this repository.
2. Add a folder `packages/<id>/`, named after the `id` in its `manifest.json`, with the package's files directly in it (`manifest.json` at its top).
3. Check it on your computer first (Node 18 or newer):
   ```
   git clone --depth 1 https://github.com/zhoupengjie/Wakuwaku
   node Wakuwaku/tools/market/check.js --market packages/<id>
   ```
   An error stops it; a warning does not, but have a look.
4. Open a pull request, one package per pull request. Say what it does; for a widget, add a screenshot of it on the taskbar or the desktop.
5. **Updating**: change the files in `packages/<id>/` and raise its `version` (1.2.0 → 1.2.1). Changed files with the same version fail the check: those who installed it would never get them.

## Rules

The whole format and its rules are in the Wakuwaku repository's [docs/PLUGINS.md](https://github.com/zhoupengjie/Wakuwaku/blob/main/docs/PLUGINS.md). In short:

- **Programs are never accepted.** No `.exe`, `.dll`, `.bat`, `.ps1` and the like; nor a program under another name: files are checked by their first bytes (Windows' MZ/PE, Linux's ELF, macOS's Mach-O). Something that needs a program runs it itself and posts to her local `POST /widget`; it is not in the market.
- A plugin is web files only: `manifest.json`, `main.js`, and optionally `view.html`, styles, images, fonts, sounds, `.wasm`. It runs in a sandbox, with no reach to your files or other programs.
- A pet pack: `manifest.json`, `pet.json`, `spritesheet.webp` and images. A wallpaper pack: `manifest.json` and images (`.jpg .jpeg .png .webp`). Both may carry `.md` and `.txt`; neither has code.
- Size: a plugin 5 MB zipped and 20 MB unpacked; a pack 50 MB and 200 MB; 500 files at most.
- The web only to the hosts in `permissions.net`: full names or `*.domain`, no port, no IP address, no `localhost` or local network.
- An `id` none of Wakuwaku's own.
- A widget may give example data in `widget.sample` (the `POST /widget` format): the market's page draws its cards from it. A pack may carry a `preview.webp` (or `.png`, `.jpg`) of 64 KB or less at its top: the market's page shows it. Neither is needed.

## Review

The CI check comes first (`.github/workflows/check.yml`: format, file types, programs, size, version). Then the maintainer reads the code and the permissions it asks for, and merges. Publishing follows by itself, in Wakuwaku within minutes; Gitee's copy a little later.

---

## 维护者 / For the maintainer

- `.github/wakuwaku-ref`：检查和打包用 Wakuwaku 哪个版本的 `tools/market/`。规则随 Wakuwaku 的版本走，Wakuwaku 发了改规则的版本再改这一行。/ Which Wakuwaku release's `tools/market/` checks and builds the market; raise it when a release changes the rules.
- PR 改了 `packages/` 以外的文件（尤其 `.github/`），CI 会提醒：这些只由维护者改，别合并别人改的。/ A pull request that changes anything outside `packages/` (above all `.github/`) gets a warning: only the maintainer changes those.
- 下架一个包：删掉它的文件夹。已发布的 zip 留在 release 里，目录里不再有它。/ Taking a package down: delete its folder. Its published zips stay on the release; the index no longer lists it.
- Gitee：secret `GITEE_TOKEN`（Gitee 私人令牌，权限 projects），可选 variable `GITEE_REPO`（`owner/repo`，默认和这里同名）。没有 token 就不同步。/ Gitee: the secret `GITEE_TOKEN` (a Gitee private token, scope projects), and optionally the variable `GITEE_REPO` (`owner/repo`, by default this repository's). Without the token nothing goes to Gitee.
