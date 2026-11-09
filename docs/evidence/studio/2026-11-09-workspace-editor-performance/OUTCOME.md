# 工作区编辑器性能：诊断、局部优化与诚实的边界

状态：**完成诊断 + 落地一项确定优化；「每个组件深度优化」未完成，见 §6**。用户反馈「工作区每个组件都要深度优化，现在 markdown 等编辑器打开、编辑特别是滚动非常卡」。

## 1. 组件链路盘点（不是凭感觉改）

| 层 | 文件 | 作用 |
| --- | --- | --- |
| 工作区标签宿主 | `pages/agents/WorkbenchTaskWorkspace.tsx` | 逐个标签渲染 `WorkbenchFileEditor`；`html` 标签已用 React 19 `<Activity mode={active ? 'visible' : 'hidden'}>`，editor 标签**没有** |
| 文件编辑器 | `pages/agents/WorkbenchFileEditor.tsx` | 注释明确「visited editors stay mounted to preserve drafts」——**多个已访问编辑器常驻**；带 `active` / `deferInitialLoad` |
| 文档打开视图 | `pages/documents/DocumentOpenView.tsx` | `surfaceMode: 'preview' | 'source'`，text 类文件走 source 编辑器，markdown 可切 preview |
| 代码编辑器 | `pages/documents/WorkspaceCodeEditor.tsx` → `monacoCodeEditor.tsx` | Monaco 0.56；`automaticLayout: true`、`largeFileOptimizations: true`、heavy 文件走 `largeMonacoOverrides` |
| Markdown 预览 | `components/FilePreview/plugins/markdown/MarkdownFilePreview.tsx` → `components/markdown/StaticMarkdown.tsx` → `AppMarkdown.tsx` | `StaticMarkdown` 全量渲染整篇 markdown（上限 2 MiB），source 模式走已虚拟化的 `CodeViewer` |
| 代码查看器 | `components/CodeViewer.tsx` | **已优化**：`@tanstack/react-virtual` 行虚拟化 + debounce + memo；不在本次改动范围 |

## 2. 根因分析

**确定的反模式**：`monacoCodeEditor.tsx` 的 `automaticLayout: true`。Monaco 该选项会用定时器周期性读取容器尺寸并强制同步布局；而 `WorkbenchFileEditor` 刻意让所有访问过的编辑器保持挂载，于是**每多开一个文件就多一个定时布局源**，滚动/编辑时相互叠加。同文件里 `html` 标签用 `Activity` 隔离、editor 标签没有，进一步放大。

**其它候选（本轮未证实，见 §4）**：
- `StaticMarkdown` 对 2 MiB 文档全量 DOM 渲染，无分块/离屏跳过；
- `DocumentOpenView.tsx:531` 的 `useEffect` 依赖 `text`，每次按键都会执行并用内联 `async` 回调调用 `onDirtyChange`（父组件用 ref 记录 dirty，未观察到父级重渲染，收益未证实）；
- 非活动 editor 标签未用 `Activity` 隔离。

## 3. 已实施（唯一保留的产品改动）

`src/renderer/pages/documents/monacoCodeEditor.tsx`：

- `automaticLayout: true` → **`false`**；
- 新增 `ResizeObserver` 观察宿主节点，尺寸变化时用 `requestAnimationFrame` 合并后调用 `editor.layout()`；
- cleanup 里 `disconnect()` + `cancelAnimationFrame(layoutFrame)`；
- `typeof ResizeObserver !== 'undefined'` 保护，jsdom 环境不报错。

这样布局只在容器真的变化（切换标签、窗口缩放、面板展开）时发生，去掉了常驻编辑器的周期性同步布局。与后续若给 editor 标签加 `Activity` 天然配合：隐藏→显示时尺寸变化会触发一次 `layout()`。

## 4. 尝试过但**已回撤**的改动（不假绿）

`src/renderer/assets/styles/markdown.css` 曾加过：

```css
.markdown.static-markdown > * { content-visibility: auto; contain-intrinsic-size: auto 120px; }
```

为验证收益做了两次隔离 Chromium 测量（Playwright + CDP）：

| 测量 | 结果 |
| --- | --- |
| 400 个轻段落，同步循环强制布局 | 两组都 ~0ms，测不出差异（同步循环没有真实渲染帧） |
| 400 个轻段落，rAF 逐帧滚动 60 帧 ×3 轮 | 两组都被锁在 60fps（总耗时 ~1000ms），**0.0% 差异** |
| 300 个重块（标题+表格+代码块），CDP `Performance.getMetrics` | 无 cv：`LayoutDuration 0ms / TaskDuration 33.5ms`；有 cv：`LayoutDuration 17.9ms / TaskDuration 50.4ms` —— **反而更差**（滚动时新进入视口的块要做布局，headless 下跳过绘制的收益体现不出来） |

**结论：没有证据支持这条 CSS 优化，且 `content-visibility` 对 Chromium 的可访问性/可见性判定有潜在风险，已完整回撤**，`git diff` 中不再包含该文件。

## 5. 验证

| 检查 | 结果 |
| --- | --- |
| `WorkspaceCodeEditor.test.tsx` | **2/2 通过**（`automaticLayout` 未被单测断言） |
| rsbuild build | **EXIT=0**（19.2s，类型检查通过） |
| markdown 相关 E2E `document-message-content` / `html-workspace` / `workspace-motion` | **12/12 通过** |
| `run-perf-layout-contracts.mjs` | **185/185 通过** |
| `git diff --check` | 通过 |
| 完整主功能 E2E `playwright.workbench-office.config.ts` | **`E2E_EXIT=0`，139 passed / 0 failed / 55 skipped** |

**一次环境故障已排除**：中途 `/tmp/aster-e2e-dist/windows/main/index.html` 被外部清理，导致某轮完整 E2E 从第 30 个用例起整片 20s 超时（**不是改动回归**）。重新 `rsbuild build` 恢复产物（HTML + 4 个 JS 引用全部存在、预览 200）后重跑，得到上表的有效结果。

## 6. Explicitly not claimed

- **未完成「每个组件深度优化」**：只改了 Monaco 布局策略。工作区还有 Office（docx/xlsx/pptx/pdf）、HTML、终端、diff、图片、markdown 预览等组件，本轮没有逐一优化，也没有逐一量化。
- **没有量化到真窗口的 FPS / 输入延迟 / longtask**：两次隔离测量都没测出收益，因此**不宣称「滚动变快了」**。Monaco 优化的收益来自机制推理（去掉定时强制布局），需要一次真实多标签场景的 CDP `PerformanceObserver` longtask / 帧率测量才能定量确认，这是下一步最该做的事。
- **未改 `StaticMarkdown` 全量渲染**、未给 editor 标签加 `Activity`、未动 `onDirtyChange`：都涉及草稿生命周期或产品交互取舍，且缺乏证据，不擅自扩大范围。
- **未修 markdown 编辑器「打开慢」**：`MarkdownFilePreview` 的读文件/首屏链路未测量、未优化。
- **未宣称覆盖率变化**：本轮未跑 agent-desktop 逐页审查，Gate A / Gate B 数字未变。
- 工作树里另有上一轮 `LocalUsageSettings.tsx` 的用量表虚拟化改动（未提交），与本轮无关。

## 7. 环境与数据

- 静态预览 4318 使用本轮最新 `rsbuild build` 产物（HTML 引用 4 个 JS 全部存在，无测试桩）；未启动真窗口，未改 `/Applications` pin。
- 完整主功能 E2E 日志：`/tmp/aster-final-e2e.log`（有效结果 139/0/55）；隔离测量脚本为一次性 Node + Playwright（未入库）。
- 工作树产品改动：`monacoCodeEditor.tsx`（本轮）+ `LocalUsageSettings.tsx`（上一轮用量页虚拟化，未提交）。未提交、未 push。

## 8. 建议的下一步（按可验证程度排序）

1. **先量化**：写一个真实工作区多标签场景的 Playwright 性能 spec —— 打开 3–4 个 markdown 文件、注入 1–2 MiB 内容，用 CDP `PerformanceObserver` 测滚动 longtask 与帧间隔；用「回退到 HEAD」跑同一 spec 得到前后对比。
2. 若证实 Monaco 收益，再给 `WorkbenchTaskWorkspace.tsx` 的 editor 标签加 `<Activity mode={active ? 'visible' : 'hidden'}>`，与 html 标签一致，消除非活动编辑器参与布局。
3. 对 `StaticMarkdown` 做**分块渲染**（按标题/段落切片 + 占位高度），这比 `content-visibility` 更可控，且能同时改善打开与滚动；需同步 markdown 相关单测与 `document-message-content.spec.ts`。
4. 把上述指标固化成回归门槛（例如「4 个编辑器常驻时滚动 longtask ≤ X ms」），防止再次劣化。
