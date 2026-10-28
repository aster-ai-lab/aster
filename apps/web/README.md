# Aster 官网、文档与下载页

`public/index.html`、`home.css`、`home.js` 和 `cosmic.js` 是 Aster 完整首页，依次展示产品首屏、可切换任务场景、工作连接、安全可控和桌面/CLI 下载。首页沿用 Aster 石墨单色品牌、星环标志与四张真实产品截图，不依赖第三方前端资源。下载区使用 WebGL 绘制三维透视星域，包含螺旋星系盘、深度雾、噪声星云、镜头推进、指针视差和流星拖尾；WebGL 不可用时回退到 Canvas 2D 星空，减少动态效果时仅显示静态画面。

`public/download/` 是可独立访问的静态下载网站，使用 HTML、CSS 和 JavaScript，不需要安装前端框架或读取私有桌面源码。品牌图形直接使用 `public/tenant/aster/logo.svg` 和 `mark-light.png`。工作区、Office、审查和产物追溯使用 `screenshots/` 中四张真实产品截图；截图来自原生 Aster 窗口和隔离演示数据，不包含主实例任务或文件，图片本身不代表网页可以执行这些桌面功能。

页面结构自上而下为：星轨 hero、真实界面功能页签、多场景卡片、五步工作流、接入方式、对比表、常见问题与底部行动区。品牌图形在 hero 中由内联 SVG 实时绘制：星核、双轨道与环绕彗星，全部由 CSS 关键帧驱动。

动效图标同样是内联手绘 SVG，不依赖外部图标库：下载目录只允许发布既定文件，外链只允许仓库域名，因此第三方资源无法引入。图标用描边路径、`pathLength` 归一化与 `stroke-dashoffset` 实现线条书写、括号开合、图层呼吸、指针流动等循环动画，并随 `prefers-reduced-motion` 关闭。

交互层包含滚动进度条、指针光晕、卡片三维倾斜与跟随高光、磁吸按钮、点击涟漪、能力跑马灯、导航滚动高亮和 CLI 卡片里的实时会话回放。所有动效在 `prefers-reduced-motion: reduce` 下停止，脚本关闭时页面内容与下载链接依然完整可见。

从仓库根目录预览：

```sh
python3 -m http.server 4188 --bind 127.0.0.1 --directory apps/web/public
```

打开 <http://127.0.0.1:4188/> 查看首页；下载页位于 <http://127.0.0.1:4188/download/>，文档总入口位于 <http://127.0.0.1:4188/docs/>。

## MDX 文档站

文档按产品、版本、章节组织，`/docs/` 是统一入口。当前提供 Aster Code 9.1.5、9.1.6 和 Aster ACP 9.1.6。Code 包含架构、设计原理、十二章终端教程、Meta Harness 和功能参考；ACP 包含客户端接入与宿主扩展。前后篇和章节导航只在当前产品的当前版本内跳转，默认版本由注册表明确指定。

```text
content/docs/
  catalog.json                 # 产品 → 版本 → 章节的单一注册表
  index.mdx                    # 所有产品入口
  code/9.1.5/
    index.mdx
    design/                    # 设计哲学与 RSI
    tutorial/                  # 十二章终端智能体教程
    meta-harness/              # 接口与使用参考
    _components.mjs            # 该版本的图表及内容组件
    _diagrams/*.mmd             # 该版本的 Pele 源图
scripts/
  docs_catalog.mjs             # 校验产品、版本与公开地址
  build_docs.mjs               # 编译 MDX、生成导航与页内目录
  docs_components.mjs          # 跨产品共享的展示组件
  docs_diagrams.mjs            # 按当前版本读取源图的 Pele 渲染器
public/docs/
  index.html
  code/9.1.5/**/index.html      # 保留的原有版本
  code/9.1.6/**/index.html      # 当前 Core 文档
  acp/9.1.6/**/index.html       # 独立 ACP 产品
  code/**/index.html            # 默认版本的兼容地址
  styles.css
  generated.css
```

MDX 和 React 只在构建时运行。公开页面仅加载一个独立的 navigation.js，按产品、版本在当前标签页保存左侧菜单滚动位置，不拦截链接，也不加载 React。项目导航、移动目录、图示和表格在关闭 JavaScript 后仍可用。SVG 图示与表格可在小屏幕上独立横向滚动，移动导航使用原生 `details` 折叠面板，打印时隐藏站点导航。

Markdown 通过 MDX 官方推荐的 `remark-gfm` 支持 GFM 表格、任务列表、删除线、自动链接和脚注。普通 Markdown 表格与内容组件使用相同的可聚焦横向滚动容器，表头使用语义 scope，对齐通过 CSS 表达；脚注提供往返锚点，任务列表保持静态禁用。

代码块通过 [Shiki 的 Rehype 插件](https://shiki.style/packages/rehype) 在构建时高亮，使用 GitHub Light High Contrast 配色。声明代码语言后，构建器提供语言标识、可聚焦的滚动区域和完整原文；纯文本使用 `text`。ACL 的 TextMate 规则只着色词法单元，依据 `crates/acl/src/lexer.rs`，不代替 ACL 解析或校验。新出现的内置语言由 Shiki 在构建时加载。

运行时架构、执行时序、Meta Harness 装配、终端分层、确认流程、RSI 改进闭环和 ACP 边界，共七类图使用 [Pele](https://pele.run/)。Mermaid 源文件放在 `content/docs/<产品>/<版本>/_diagrams/`，图注与其他带产品含义的内容组件放在该版本的 `_components.mjs`。渲染器按当前产品和版本读取源文件，生成独立的稳定 ID。构建器禁用图内链接与图片，生成带标题和描述的 SVG；主题变量在 `public/docs/styles.css` 的 `.diagram .pele` 中统一维护，沿用白底、石墨文字、浅灰分组和 Ubuntu 字体。图与代码的生成样式合并到 `public/docs/generated.css`，不使用内联样式、CDN 或浏览器运行时。

`/docs/code/9.1.5/tutorial/` 是围绕同一 `tutorial-agent` 练习工程的十二章教程。页面源码位于 `content/docs/code/9.1.5/tutorial/`；工程文件由读者按章节建立，网站构建不创建 Rust 工作区成员或运行模型。前四章建立会话、流式事件、终端和确认交互，后续接入验证、保存恢复、Harness、上下文、工具、记忆、目标和资源收尾。注册表将教程和 Meta Harness 参考分组，两个系列共用项目导航，公开地址保持独立。

使用文档站自己的构建依赖与锁文件，避免依赖桌面或其他工作区：

```sh
npm --prefix apps/web ci --workspaces=false --ignore-scripts --no-audit --no-fund
npm --prefix apps/web run build
npm --prefix apps/web run check
```

生成的 HTML 随 MDX 一同提交，静态预览无需 Node。CI 的 `check` 会拒绝源码与 HTML 不同步的改动，不会在发布时悄悄覆盖旧产物。只编译注册表中明确列出的文档，不扫描其他目录。MDX 是可执行的仓库源码，只能由受信任的作者维护；构建器不接受用户上传或远程 MDX。

新增产品（子项目）和版本时：

1. 在 `content/docs/catalog.json` 的 `projects` 添加唯一的 `id`、`title`、`label`、`description`、`defaultVersion` 和 `versions`。默认版本必须明确存在，不按版本号或数组位置推断。
2. 每个版本声明 `id`、`label`、`sourceRef` 和有序的 `pages`；需要版本专有组件时，再填写 `components: "_components"`。源码放在 `content/docs/<产品>/<版本>/`。版本间各自保存 MDX、组件和源图；新增版本可复制上一版，再在新目录修改。
3. `sourceRef` 指定运行时源码使用的真实 Git 标签、提交或分支。当前文档以 `main` 为源码引用；归档时应选择已经存在的固定 ref。文中的本仓库 `blob/main`、`tree/main` 引用会按当前版本改写，编辑文档的链接仍指向网站主分支。
4. 每项 `pages` 填写 `slug`、`file`、`label`、`title` 和 `description`，可用非空 `section` 给相邻章节分组。第一项必须是 `slug: ""` 的版本首页；`file` 相对该版本目录且省略 `.mdx`，例如 `guide/start`；`slug` 为空或以 `/` 结尾。同一版本内 slug 与 file 唯一，不同版本可以使用相同名称。
5. MDX 导出 `meta`，包含 `title`、`description`、`label` 和 ISO 日期 `updated`，正文从二级标题开始。专题首页用 `<ChapterCards section="专题名称" />` 列出同组后续文章。
6. 运行 `build`，提交源文件、注册表与生成产物，再运行检查。删除章节或调整默认版本时，构建器只清理由它生成且已经失效的 HTML；其他手写文件不会被删除，发布检查会拒绝未登记的额外文件。

公开地址为 `/docs/<产品>/<版本>/<章节>/`。不含版本的 `/docs/<产品>/<章节>/` 继续呈现显式默认版本，并把 canonical 指向固定版本地址；目录、前后篇和正文相对链接都会留在当前版本。分享需要长期稳定的链接时使用带版本地址。

产品切换进入目标产品的默认版本；版本切换优先保留相同 slug，目标版本没有本篇时明确提示并打开版本首页。两个切换器使用可键盘操作的原生 `details`，不使用 `<select>` 或浏览器脚本。桌面上的源码资料与版本信息位于右栏底部；窄屏移到正文末尾，不覆盖内容。

注册表同时驱动产品入口、版本选项、章节导航、面包屑、前后篇、canonical 和 Python 发布清单。Node 测试使用两产品、两版本的独立 MDX、组件与源图验证隔离。外链仅接受本仓库、四份框架官方文档及 RSI 章节明确引用的两篇论文；菜单滚动位置仅存 sessionStorage；禁用 JavaScript 或浏览器存储时仍可正常阅读和导航。新增参考资料需要在 `scripts/site_contract.py` 登记。远程脚本、字体与样式始终不允许。

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
node --test apps/web/tests/download.test.cjs apps/web/tests/canvas.test.cjs apps/web/tests/cosmic.test.cjs apps/web/tests/home.test.cjs apps/web/tests/docs.test.mjs
```

也可以隔离安装测试依赖并显式指定，不需要私有源码：

```sh
npm install --prefix target/download-test-deps --ignore-scripts --no-save --package-lock=false jsdom@26.1.0
JS_DOM_MODULE="$PWD/target/download-test-deps/node_modules/jsdom" node --test apps/web/tests/download.test.cjs apps/web/tests/canvas.test.cjs apps/web/tests/cosmic.test.cjs apps/web/tests/home.test.cjs apps/web/tests/docs.test.mjs
```

DOM 回归验证交互状态和事件；Canvas 测试用受控绘图上下文和动画时钟验证调度。它们不替代真实浏览器的像素、布局、原生对话框焦点和绘图效果检查。

输出目录必须不存在。脚本只复制首页的四个文件、下载页的四个文件、四张 PNG 截图、三份 Ubuntu 字体及其完整许可证、三个 Aster 品牌文件，以及文档注册表中的 130 个 HTML 地址（79 篇文档，含默认版本兼容地址）、文档样式、生成样式和菜单滚动增强脚本，目前共 152 个文件；根入口是完整首页，`download/` 和 `docs/` 独立访问，不复制其他租户、MDX 源码、依赖或用户数据。所有资源路径均兼容 GitHub Pages 的 `/aster/` 项目路径。Canvas 是下载页的独立装饰层：减少动态时静态绘制，页面隐藏或首屏离开视口时停止动画，没有第三方绘图库。

资源清单位于 `scripts/site_contract.py`。截图只允许 `workspace.png`、`office.png`、`review.png` 和 `lineage.png`，每张不超过 2 MiB；检查覆盖 PNG 签名、IHDR 尺寸和头部校验码，并要求 HTML 显式引用四张图片。检查不代替完整图片解码或截图内容审查。字体从本地 Ubuntu 字体包复制，页面使用本地 `@font-face`，`fonts/Ubuntu-LICENSE.txt` 随站点完整发布。下载目录出现其他文件或符号链接会拒绝发布。

下载链接的封闭清单位于 `scripts/site_contract.py`，对应已存在的公开稳定包。新增或调整链接前应核实 GitHub Release 的版本、CPU 和文件名；下载页应区分当前公开包与尚未发布的新界面。静态检查不访问网络，也不证明远端下载可用性或浏览器交互效果。

`.github/workflows/aster-pages.yml` 在 `main` 的相关文件变化时部署，也支持手动触发。它仅检出公共仓库，DOM 测试依赖隔离在 runner 临时目录，校验后只上传精确静态目录。Pages 需由仓库管理员启用 GitHub Actions 来源；本脚本不修改远端设置。

浏览器回归在上述 4188 静态服务启动后运行：

```sh
a3s-test check apps/web/tests/download.acl --json
a3s-test run apps/web/tests/download.acl --browser-driver standalone --json
```

该套件覆盖桌面与移动端的功能标签、真实截图展示与放大、下载弹层、常见问题和回顶按钮；需要已安装兼容的 `agent-browser`。

文档浏览回归使用独立的 4192 端口：

```sh
python3 -m http.server 4192 --bind 127.0.0.1 --directory apps/web/public
a3s-test check apps/web/tests/docs.acl --json
a3s-test run apps/web/tests/docs.acl --browser-driver standalone --json
```

它覆盖文档总入口、入门章节、Meta Harness 能力目录及七个专题、十二章终端智能体教程、菜单滚动保持、产品版本切换、分组导航、移动目录展开、Pele 图、代码高亮与相邻页面跳转。A3S Test 的截图与页面错误记录用于浏览器证据；构建同步、项目地址、发布隔离、对比度和无脚本可读性分别由 Node 与 Python 检查。

能力覆盖检查使用 `code/9.1.6/_coverage.json` 与 `acp/9.1.6/_coverage.json`，逐项对应 Core 的合同 ID、规范操作、tier、宿主责任和 ACP 接受的 method 集合。增加接口时同步索引、参考章节和测试。这项检查只验证文档与接口目录的对应关系；`apps/tutorial-agent` 另有编译、固定模型集成测试和离线终端回归，真实模型与外部服务需单独验收。
