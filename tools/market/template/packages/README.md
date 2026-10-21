# packages/

每个包一个文件夹：`packages/<id>/`，文件夹名就是它 `manifest.json` 里的 `id`。这里的文件（比如这一份）不是包，不会发布。

示例插件还没放进来：Wakuwaku 仓库的 `examples/plugins/` 写好后，把那三个文件夹（`tally`、`rain-alert`、`agent-board`）复制到这里，作为第一批包。`agent-board` 带一个自绘界面（`view.html`）。壁纸包的示例是 `examples/packs/wallpaper-sample`（三张渐变和一张预览图），也可以一起复制过来。

One folder per package: `packages/<id>/`, named after the `id` in its `manifest.json`. Files here (like this one) are no package and are not published.

The example plugins are not here yet: once the Wakuwaku repository's `examples/plugins/` is written, copy its three folders (`tally`, `rain-alert`, `agent-board`) here as the first packages. `agent-board` draws a view of its own (`view.html`). The example wallpaper pack is `examples/packs/wallpaper-sample` (three gradients and a preview); copy it too if you like.
