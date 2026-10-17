# 样例页（gallery）

用**真实的页面代码**（`src/pet` 的 index.html、desk.html、bar.html 和它们的脚本、样式）把她的各个界面渲染出来，深色、浅色各一份，用无头 Edge 截成 PNG。它是 `docs/design/style.md` 那套风格的「样子」，以后也是本地 UI 回归测试的底子。

不改产品代码：页面在浏览器里照常跑，只是 Tauri 换成了一个替身（`fake-tauri.js`）。

## 怎么跑

```powershell
pwsh tools/preview/render.ps1 -Out <输出文件夹>                    # 全部场景，深色和浅色
pwsh tools/preview/render.ps1 -Out <文件夹> -Only capsule-*,flow-* -Modes dark
pwsh tools/preview/render.ps1 -Open                                 # 在 Edge 里打开活的 gallery.html，不截图
```

- 每个场景出两张：`<id>-dark.png`、`<id>-light.png`（默认 2 倍像素，`-Scale 1` 出原大）；再拼成两张总览：`overview-dark.png`、`overview-light.png`。
- 需要 node（读场景列表）和 Edge；6 个 Edge 并行，60 多张约 40 秒。不在屏幕上画任何东西，也不碰正在跑的她。
- 截早了（页面还没冻住）的图左上角有个品红方块，页面出错的图底部有一条红色说明；脚本最后会列出来，退出码是 1。
- 只看一页：`shot.html?scene=<id>&mode=dark`；直接看页面本身并打出调试信息：`page.html?role=island&scene=<id>&mode=dark&debug=1&probe=%23island`。用 file:// 打开时 Edge 要带 `--allow-file-access-from-files`（`-Open` 会带上）。

## 文件

| 文件 | 做什么 |
|---|---|
| `fake-tauri.js` | Tauri 的替身：`window.__TAURI__` / `__TAURI_INTERNALS__`。里面有一份可改的「世界」（`Fake.world`）：她的家（任务栏 / 灵动岛）、条带的边和材质、深浅色和强调色、各种心情的会话、等你的询问、小工具和大小、系统监控读数、桌面布局、设置。`Fake.set()` 改它，`Fake.push()` 按 Rust 那边的样子把事件发给页面，`Fake.emit()` 发任意事件；页面的 `invoke` 从世界里回答。时钟固定从 14:42 开始走，所以每次截出来一样。 |
| `fixtures.js` | 样例数据：会话、询问、小工具（天气、系统监控、Claude 今天、邮件、股票、番茄钟、倒计时、两个没图标的脚本小工具）、插件、程序按钮、托盘、桌面和任务栏布局。 |
| `scenes.js` | 场景表。node 也能读：`node scenes.js --list`。 |
| `boot.js` + `page.html` | 读进真实的页面文件（去掉 CSP，链接改成绝对路径），先装替身再跑它自己的脚本；加载完把场景的世界发过去、跑场景的步骤，然后**冻住**：动画跑到头（或停在场景指定的那一刻），清掉所有计时器。 |
| `shot.html` + `shot.js` | 一个场景 = 屏幕（1600×900）上的一块：底下是壁纸，上面是她的窗口（iframe 里的 page.html），放在她的家该在的位置。 |
| `gallery.html` | 所有场景活的样子，按分区排：`gallery.html?mode=light&only=capsule-*`。 |
| `render.ps1` | 截图 + 拼总览 + 检查。 |
| `assets/wall.jpg` | 仓库 `wallpapers/gpt-cloud-terrace.png` 缩到 1920×1080。 |

## 加一个场景

在 `scenes.js` 的 `LIST` 里加一项：

```js
{
  id: 'capsule-review',                 // 也是 PNG 的名字
  section: 'capsule',                   // SECTIONS 里的一个分区
  title: '收起 · 改好了',
  page: 'island',                       // island（index.html）、desk（desk.html）、bar（bar.html）
  home: 'taskbar',                      // taskbar 或 island
  view: [0, 820, 440, 80],              // 截屏幕上的哪一块 [x, y, w, h]
  world: F => ({ ...home(F), sessions: [F.session('review')], config: { endCue: 'none' } }),
  steps: async p => {                   // 可选：页面起来以后做什么
    p.hover('#island')                  // 也有 p.click、p.context（右键）、p.emit、p.set + p.push、p.wait
    await p.wait(400)
  },
  budget: 3000,                         // 可选：截图前页面的（虚拟）时间
}
```

然后 `render.ps1 -Only capsule-review` 看一眼。要停在动画中间就在步骤里 `p.freeze({ flow: 1100 })`（流光转到 1.1 秒）；没调就在步骤后 0.5 秒自动冻住。新数据放进 `fixtures.js`，新的「Rust 会发的东西」放进 `fake-tauri.js`。

以后做回归测试：时间、动画、计时器都固定了，同一台机器上两次截图几乎一样（2026-10-17 试过：62 张里 56 张逐字节相同，其余 6 张各有 1–6 个像素差 1，是模糊和抗锯齿的噪声）。拿 `render.ps1` 的输出跟一份基准比的时候，给每个像素留一点容差就行。时钟按本机时区走，换了时区的机器要重新截基准。
