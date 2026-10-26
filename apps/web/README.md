# Aster 官网与下载页

`public/index.html`、`home.css`、`home.js` 和 `cosmic.js` 是 Aster 完整首页，依次展示产品首屏、可切换任务场景、工作连接、安全可控和桌面/CLI 下载。首页沿用 Aster 石墨单色品牌、星环标志与四张真实产品截图，不依赖第三方前端资源。下载区使用 WebGL 绘制三维透视星域，包含螺旋星系盘、深度雾、噪声星云、镜头推进、指针视差和流星拖尾；WebGL 不可用时回退到 Canvas 2D 星空，减少动态效果时仅显示静态画面。

`public/download/` 是可独立访问的静态下载网站，使用 HTML、CSS 和 JavaScript，不需要安装前端框架或读取私有桌面源码。品牌图形直接使用 `public/tenant/aster/logo.svg` 和 `mark-light.png`。工作区、Office、审查和产物追溯使用 `screenshots/` 中四张真实产品截图；截图来自原生 Aster 窗口和隔离演示数据，不包含主实例任务或文件，图片本身不代表网页可以执行这些桌面功能。

页面结构自上而下为：星轨 hero、真实界面功能页签、多场景卡片、五步工作流、接入方式、对比表、常见问题与底部行动区。品牌图形在 hero 中由内联 SVG 实时绘制：星核、双轨道与环绕彗星，全部由 CSS 关键帧驱动。

动效图标同样是内联手绘 SVG，不依赖外部图标库：下载目录只允许发布既定文件，外链只允许仓库域名，因此第三方资源无法引入。图标用描边路径、`pathLength` 归一化与 `stroke-dashoffset` 实现线条书写、括号开合、图层呼吸、指针流动等循环动画，并随 `prefers-reduced-motion` 关闭。

交互层包含滚动进度条、指针光晕、卡片三维倾斜与跟随高光、磁吸按钮、点击涟漪、能力跑马灯、导航滚动高亮和 CLI 卡片里的实时会话回放。所有动效在 `prefers-reduced-motion: reduce` 下停止，脚本关闭时页面内容与下载链接依然完整可见。

从仓库根目录预览：

```sh
python3 -m http.server 4188 --bind 127.0.0.1 --directory apps/web/public
```

打开 <http://127.0.0.1:4188/> 查看新首页；原下载页仍位于 <http://127.0.0.1:4188/download/>。

校验和生成部署目录：

```sh
python3 -m unittest discover -s apps/web/tests -p 'test_*.py' -v
python3 apps/web/scripts/check_site.py
node --check apps/web/public/download/app.js
node --check apps/web/public/download/canvas.js
node --check apps/web/public/home.js
node --check apps/web/public/cosmic.js
python3 apps/web/scripts/stage_site.py target/aster-site
```

交互回归使用公共 UI 工作区已有的 `jsdom` 开发依赖；已安装该工作区依赖时直接运行：

```sh
node --test apps/web/tests/download.test.cjs apps/web/tests/canvas.test.cjs apps/web/tests/cosmic.test.cjs apps/web/tests/home.test.cjs
```

也可以隔离安装测试依赖并显式指定，不需要私有源码：

```sh
npm install --prefix target/download-test-deps --ignore-scripts --no-save --package-lock=false jsdom@26.1.0
JS_DOM_MODULE="$PWD/target/download-test-deps/node_modules/jsdom" node --test apps/web/tests/download.test.cjs apps/web/tests/canvas.test.cjs apps/web/tests/cosmic.test.cjs apps/web/tests/home.test.cjs
```

DOM 回归验证交互状态和事件；Canvas 测试用受控绘图上下文和动画时钟验证调度。它们不替代真实浏览器的像素、布局、原生对话框焦点和绘图效果检查。

输出目录必须不存在。脚本只复制首页的四个文件、下载页的四个文件、四张 PNG 截图、三份 Ubuntu 字体及其完整许可证、两个 Aster 品牌文件，共 18 个文件；根入口是完整首页，`download/` 继续独立发布，不复制其他租户、源码、依赖或用户数据。所有资源路径均兼容 GitHub Pages 的 `/aster/` 项目路径。Canvas 是下载页的独立装饰层：减少动态时静态绘制，页面隐藏或首屏离开视口时停止动画，没有第三方绘图库。

资源清单位于 `scripts/site_contract.py`。截图只允许 `workspace.png`、`office.png`、`review.png` 和 `lineage.png`，每张不超过 2 MiB；检查覆盖 PNG 签名、IHDR 尺寸和头部校验码，并要求 HTML 显式引用四张图片。检查不代替完整图片解码或截图内容审查。字体从本地 Ubuntu 字体包复制，页面使用本地 `@font-face`，`fonts/Ubuntu-LICENSE.txt` 随站点完整发布。下载目录出现其他文件或符号链接会拒绝发布。

下载链接的封闭清单位于 `scripts/site_contract.py`，对应已存在的公开稳定包。新增或调整链接前应核实 GitHub Release 的版本、CPU 和文件名；下载页应区分当前公开包与尚未发布的新界面。静态检查不访问网络，也不证明远端下载可用性或浏览器交互效果。

`.github/workflows/aster-pages.yml` 在 `main` 的相关文件变化时部署，也支持手动触发。它仅检出公共仓库，DOM 测试依赖隔离在 runner 临时目录，校验后只上传精确静态目录。Pages 需由仓库管理员启用 GitHub Actions 来源；本脚本不修改远端设置。

浏览器回归在上述 4188 静态服务启动后运行：

```sh
a3s-test check apps/web/tests/download.acl --json
a3s-test run apps/web/tests/download.acl --browser-driver standalone --json
```

该套件覆盖桌面与移动端的功能标签、真实截图展示与放大、下载弹层、常见问题和回顶按钮；需要已安装兼容的 `agent-browser`。
