# README 图形

根目录 README 使用 Hairline 的装配工作台：固定底座对应内核边界，
三个工位表示任务中的推演、执行与核验。这是过程隐喻；五个实际组合部件在 README 中单独列出。

- `meta-harness.svg`：GitHub 使用的静态预览，无脚本或外部资源。
- `hairline-workbench.html`：自包含交互页，下载后直接用浏览器打开；按 **play** 或用 `?play=1` 播放巡览。
- `source/workbench.js`：图形的可编辑源文件。
- `source/render.mjs`：保留原版引擎与页面，校验交互后导出静态 SVG。

图形遵循 Hairline 的固定命中区域、按距离错开、有限抬升、真实遮挡和离散缓动规则。
巡览依次选择 `plan`、`execute`、`verify`，最后回到 `rest`；底座保持固定。
滑块只控制工位抬升幅度，播放尊重系统的减少动态效果设置。
图形不连接 Aster，也不代表运行状态或验证证据。

## 架构解剖图

根目录 README 的“运行机制”使用四张拆分静态图，均无脚本或外部资源：

- `task-control-loop.svg`：模型提案、权限判定、工具执行、事实日志与完成门禁。
- `meta-harness-assembly.svg`：宿主配方、五个默认组件、Moore 组件与动作 key。
- `extension-surfaces.svg`：提示、上下文、Skills、工具、Hooks 和宿主组件的接入位置。
- `verification-recovery.svg`：修改摘要、验证证据、资源寿命、恢复与未决外部操作。

可编辑源文件为 `source/render_anatomy.py`，只使用 Python 标准库。
在仓库根目录运行以下命令重建四张 SVG：

```sh
python assets/readme/source/render_anatomy.py
```

## 重建

需要 Node、安装好的 `hairline-create` 技能，以及 Chrome 或该技能检查器使用的 Chromium。
技能安装方式见 [Hairline Skills](https://hairline.lucasmarkes.com/skills)。
脚本复用技能提供的构建、校验与浏览器辅助函数，不向项目添加 npm 依赖。
在仓库根目录运行：

```sh
node assets/readme/source/render.mjs
```

技能不在默认 Codex 目录时，将技能目录作为唯一参数传入。
图形引擎和页面必须保持原样。编辑图形后，再用该技能的 `look.mjs` 检查：

```sh
node ~/.codex/skills/hairline-create/look.mjs \
  assets/readme/hairline-workbench.html \
  --answer 0,0,36 --edge=-70,20,25 --edge=65,25,30
```

检查器生成明暗主题、240px、幅度两端的图版及运动序列；这些临时预览不用于 README。
还应在 900px 和 360px 宽度检查完整 README。交互 HTML 由 GitHub 作为源码展示，需下载后打开。
上游页面的暗色线条对比度偏低，阅读交互图建议选择 `light`；README 的静态预览自带白色背景。

## 来源

引擎、页面和辅助脚本来自 [Hairline](https://github.com/lucasmarkes/hairline)，
安装时对应提交 `37d2e78dcb333ab30c3338e28ed4a690364ea014`。
生成页保留内核校验摘要；MIT 许可见 [hairline-license.txt](hairline-license.txt)。
