# Aster 官网、文档与下载页

整站只有一套实现：Astro 静态构建。源码经 `scripts/prepare_astro.mjs` 写入构建缓存，再由 Astro 生成 `dist/`。GitHub Pages 只发布 `dist/` 中通过检查的公开文件。

## 目录职责

```text
apps/web/
  content/
    home.html                 # 首页 HTML 与下载占位符
    download.html             # 下载页 HTML 与下载占位符
    releases.json             # 六个安装包的固定发布快照
    docs/                     # MDX、产品注册表、Mermaid 与讲解数据
  src/pages/                  # 首页、下载页、法律页与文档 Astro 路由
  public/                     # 手工维护的 CSS、浏览器脚本、图片、字体和许可证
  scripts/                    # 构建器、渲染器与发布检查
  tests/                      # Python、DOM、Canvas 和文档测试
  node_modules/.cache/        # 临时文档与公开资源缓存，不提交
  dist/                       # 唯一最终站点输出，不提交
```

`public/` 不再存放生成 HTML，也不保存生成的导航脚本、图示 SVG、搜索索引或 `generated.css`。这些文件全部由构建器写入缓存，再进入 `dist/`。`src/prefs/tenantInfo.ts` 仍由仓库的租户工具使用，不属于旧官网残留。

## 首页与下载页

首页使用 `content/home.html`、`public/home.css`、`public/home.js` 和 `public/cosmic.js`。首屏保留「Aster，让想法落地」的原位逐字输入，下载区保留紫色双旋臂星系、分层穿行星点、循环聚散、半透明玻璃卡片、指针视差、Canvas 2D 回退和减少动态模式。

下载页使用 `content/download.html` 与 `public/download/` 中的静态资源。四张截图来自隔离演示数据；网页本身不提供桌面应用能力。首页和下载页只通过相对路径引用公开资源，兼容 GitHub Pages 的 `/aster/` 项目路径。

安装包地址使用 `content/releases.json` 中的 `{{download.*}}` 占位符。Astro 在构建时调用 `renderDownloadDocument` 渲染两个 HTML 源文件，不再生成 `public/index.html` 或 `public/download/index.html`。

刷新公开 Release 快照：

```sh
node apps/web/scripts/build_releases.mjs --refresh
npm --prefix apps/web run check
```

刷新命令只更新 `content/releases.json` 并验证模板，不写旧静态产物。选择逻辑排除草稿、预发布和未完成上传的资产，并保持六个桌面与 CLI 目标的顺序和版本一致。

## 文档站

`content/docs/catalog.json` 是产品、默认版本和章节的唯一注册表。每个版本声明 `id`、`label`、`sourceRef` 和有序的 `pages`；MDX 位于对应产品与版本目录。公开地址为 `/docs/<产品>/<版本>/<章节>/`，不含版本的地址呈现显式默认版本并指向固定版本 canonical。

文档构建统一完成：

- 编译 MDX、GFM 表格、代码高亮和章节导航。
- 从 Mermaid 与讲解数据生成 Pele SVG、源码漫游和图示样式。
- 生成全文搜索索引。
- 从 `scripts/docs_navigation.mjs` 打包浏览器导航脚本。
- 校验 `SourceRef` 的源码快照、精确片段和行号。

公开文档不加载 React 或 Astro 客户端运行时，不执行内联脚本，也不请求远程字体或浏览器端 Release API。移动导航使用原生 `details`，关闭 JavaScript 后仍可阅读正文和打开文档链接。

新增或修改文档后，只编辑 `content/docs/` 中的源文件并运行构建。不要手写或提交缓存及 `dist/` 中的 HTML、CSS、SVG、JS 或 JSON。`assets/readme/` 中由同一 Mermaid 源生成的 README SVG 是唯一需要随源码提交的生成图。

当前 Code 9.1.6 的 RSI 与 Meta Harness 章节增加可拖动的球形证据图谱。RSI 默认只显示图谱、轻量案例入口与操作；点击节点才展开内部细节与证据卡，关闭时返回。专注按钮将同一个图谱移入原生模态窗口，`?view=graph` 可直接进入专注视图。球面微点和网格是背景纹理，不代表实验记录；实际关系使用带方向光点的曲线，内部窥探保留全局图与跨层连接。动效与讲解可分别暂停，减少动态效果时保留静态关系，离开视口后停止绘制。

“演进历程”参考 [Arc UI Timeline](https://uiarc.dev/components/timeline)、[Stepper](https://uiarc.dev/components/stepper) 与 [Accordion](https://uiarc.dev/components/accordion) 的连接线、当前阶段环和展开交互，用现有原生 DOM / Web Animations 实现。默认收起，打开时仅展示当前案例的纵向时间轴；阶段切换同步图谱，来源按需展开。论文案例按算法阅读顺序呈现，不伪造事件时间、完成状态或运行指标。DGM、STOP 标为文献流程重建，Aster 预算修复标为工程记录，应用映射另行注明。案例数据在 `_rsi-cases.mjs`，展示组件与 Core 数据在 `_harness-demo.mjs`。静态 Pele 图和完整文字来源保留，关闭脚本后仍可阅读；浏览器增强复用 `navigation.js`，不增加运行时依赖，也不连接模型或会话。

RSI 与 Core 节点的内部视图采用“对象 → 作者指定的关系类别 → 原始记录”三层结构。`attachGroups` 在构建时核对每条记录只归属一次；案例导览路径同样核对已声明的节点与有向关系。讲解回放共享一个状态所有者：在全局启动按案例阶段推进，在节点内启动则回放当前记录；光点到达后同步说明与一次亮环，暂停冻结进度，手动选择和案例切换取消旧回放。减少动态效果时逐条手动阅读，不运行自动计时器。缩小上下文使用全分辨率几何重绘，文字层保持独立；节点与记录使用 12–13px 阅读字号，记录水平排版。案例、返回和播放位于独立工具栏，场景另占一行；细节圆按剩余画布高度缩放，给证据卡留出空间。投影标签避让其它节点与条件文字，并用细引线保留其原始位置。

三个 RSI 案例各有明确的场景、基线、目标、判断条件与结果。全局节点用角色标签区分场景、决策、方案和结果；有向关系包含箭头、分支条件和回到下一轮的反馈关系。条件标签可钻取目标结果。DGM 的准入条件来自论文 §3 / A.4，不能把低分等同于无效；STOP 的三个策略节点是论文汇总，不是同轮并行日志；Aster 的三个额度结果是固定提交的配置合同。

## 本地开发

使用与 Pages 工作流相同的 Node.js 24.12.0（内置 npm 11.6.2），从仓库根目录安装独立官网依赖。更新 `package-lock.json` 时也使用此版本，避免不同 npm 版本对可选依赖的处理导致 CI 安装失败。

```sh
npm --prefix apps/web ci --workspaces=false --ignore-scripts --no-audit --no-fund
```

启动开发服务器：

```sh
npm --prefix apps/web run dev -- --host 127.0.0.1 --port 7100
```

入口分别为：

- <http://127.0.0.1:7100/>
- <http://127.0.0.1:7100/download/>
- <http://127.0.0.1:7100/docs/>
- <http://127.0.0.1:7100/terms/>
- <http://127.0.0.1:7100/privacy/>

## 构建与测试

```sh
npm --prefix apps/web run build
npm --prefix apps/web run check
npm --prefix apps/web test
```

三个命令的职责固定：

- `build`：生成文档与公开资源缓存，并由 Astro 输出 `dist/`。
- `check`：重新构建，确认生成结果稳定，再检查发布资源边界。
- `test`：执行 `check`，然后运行全部 Python、JavaScript 语法、DOM、Canvas、首页、下载、Release 和产品文档测试。

DOM 测试可以复用 `packages/ui` 的 `jsdom`，也可以使用隔离依赖：

```sh
npm install --prefix target/web-test-deps --ignore-scripts --no-save --package-lock=false jsdom@26.1.0
JS_DOM_MODULE="$PWD/target/web-test-deps/node_modules/jsdom" npm --prefix apps/web test
```

`python3 apps/web/scripts/stage_site.py target/aster-site` 会再次检查 `dist/`，并只复制 `scripts/site_contract.py` 明确允许的公开文件。目标目录必须不存在。

## 部署

`.github/workflows/aster-pages.yml` 监听 `apps/web/**`、相关 README 资源和工作流自身，也响应正式发布及指定发布工作流。PR 使用提交内的 Release 快照；主分支部署先刷新公开 Release 数据，再执行同一个 `npm --prefix apps/web test`。

CI 与本地不再维护不同的测试清单。验证通过后，工作流只上传暂存目录，不复制 MDX、依赖、缓存、其他租户资源或用户数据。
