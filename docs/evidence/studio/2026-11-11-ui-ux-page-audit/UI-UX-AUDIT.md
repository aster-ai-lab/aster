# Aster Studio 逐页面 UI/UX 审计：交互反馈与动效（2026-11-11）

只读静态分析，基于 `apps/studio` 干净 checkout `cb3df20eed`（含本轮错误卡/动效相关改动）。未运行 UI、未做运行时帧率/对比度测量；文中时长与延迟均来自代码字面值。父仓库 HEAD `81f657b7b`（本机另有并发会话的 `crates/` 改动，与本审计无关）。

## 评判依据

| 规范 | 用于判断 |
| --- | --- |
| WCAG 2.2 AA **2.3.3 Animation from Interactions** | 交互触发的非必要动效必须可关闭（reduced-motion） |
| WCAG 2.2 **2.2.2 Pause, Stop, Hide** / 2.1 **2.3.1** | 自动启动、>5s 的动效需可停；闪烁 <3Hz 不算违规（如实标注）。 |
| Nielsen **#1 系统状态可见性**、#5 防错、#9 错误识别与恢复 | 操作后是否即时反馈、失败是否可诊断可恢复 |
| Material Motion | 100–300ms、避免纯 linear、避免布局属性动画、动效不阻塞交互 |
| Apple HIG | 尊重 Reduce Motion、动画须有目的、用户控制 |
| 布局稳定性 / CLS | 避免动画改变 height 或等价属性造成内容跳动 |

## 先说做得好的（避免只挑刺）

- **反馈基建收敛**：Toast、ConfirmActionPopup、ErrorBoundary、Skeleton、EmptyState 全在 `@aster-ai-lab/ui`；每窗口一个 `ToastHost`。
- **浮层动效统一**：`popup-motion.css` / `dialog-motion.css` 统一 160ms 进 `cubic-bezier(0.16,1,0.3,1)` / 200ms 出。
- **reduced-motion 覆盖率高**：CSS 约 56/61 个含动画的文件有保护；JS/TS 38 个文件用 `useReducedMotion` / `matchMedia`；75 个 TSX 用 `motion-reduce:`；`useMotionActivity` 在窗口失焦时给 `:root[data-motion-paused]` 全局停动画，是完整正例。
- **a11y 语义广泛**：`role="status"` 130 文件、`aria-busy` 61、`aria-live` 39、`aria-pressed` 85。
- **反馈最完整的页面**：Capabilities（连接器/技能）、FilesPage、ArchiveSettings、UsageSettings、DataSettings、GlobalSearchPanel。
- **有意防闪烁的延迟不算缺陷**：Capabilities 300ms 最小 spinner、WikiRetrievalStatus 400ms preparing 均为合理设计，下文不计入。

---

## 一、逐页面分析

> 严重度：S1 影响主路径/合规；S2 明显体验损失；S3 局部不一致。每行给「反馈 / 动效」两类问题与代码位置。

### P1 工作台首页 / 新建任务空态

| 维度 | 问题 | 位置 | 规范 | 级别 |
| --- | --- | --- | --- | --- |
| 反馈 | **回退 composer 发送链路零忙碌反馈**：`WorkbenchHome.tsx` 点击后连续 `await updateAgent` + `await requestHomeComposerSend` + `finishFallbackSend`，期间按钮不 disabled、无 spinner、无 optimistic、无 toast。主路径 `AgentComposer` 有 `sendingHome`，这条 fallback 没有；慢时易重复提交。 | `WorkbenchHome.tsx:1548-1583`、`1294-1295` | Nielsen #1 | **S1** |
| 反馈 | 空态只有占位符，无示例/能力入口（此前提过） | `WorkbenchHome.tsx` 空态分支 | Nielsen #10 | S3 |
| 动效 | 会话卡 HoverCard `650ms` 才出现；关闭 100ms | `WorkbenchSessionCard.tsx:224,248` | Material 100–300ms | S2 |
| 动效 | 页面骨架 `Skeleton` 交错 90ms、1.8s 后停止（正例，非无限） | `arc/skeleton.module.css` | — | 合规 |

### P2 任务会话页（消息流，问题最集中）

| 维度 | 问题 | 位置 | 规范 | 级别 |
| --- | --- | --- | --- | --- |
| 动效 | **交互触发的 830ms 平滑滚动完全无 reduced-motion 保护**：`DEFAULT_FRAMES=50` ≈830ms@60fps，用于引用/目录/回顶跳转；整文件无 `useReducedMotion`。开启「减弱动态」仍被强制经历近 1 秒视口移动。 | `list/useSmoothScrollAnimation.ts:16,40-41,76-124`；调用 `chatVirtualizerRuntime.tsx:1059-1061,1101,1167-1176` | **WCAG 2.3.3**、Material 100–300ms | **S1** |
| 反馈 | **`useMinimumDisplayDuration` 人为延迟真实状态 1000ms / 1200ms**：思考预览与工具组 live header 的新状态被 `setTimeout` 扣住，模型已产出新状态而 UI 最多滞后 1–1.2s 才显示，用户误判卡住。error/waiting 有 bypass（正例），但正常状态持续脱节。 | `blocks/useMinimumDisplayDuration.ts:47-59`；`ThinkingBlock.tsx:16,95-100`；`ToolBlockGroup.tsx:66,394-399` | Nielsen #1、Material | **S1** |
| 动效 | **CSS 语法缺陷：`@keyframes task-pending-shimmer` 未闭合**，3248 开、3289 才闭，中间 3256–3287 五条普通规则被当作无效 keyframe 丢弃，包含代码块折叠箭头的 `transition: transform 180ms` 与 `-90°` 折叠态 → 折叠无角度指示、硬切。 | `task-conversation.css:3248-3289` | 代码正确性、Nielsen #1 | **S1** |
| 动效 | **600ms 聚合 hover 预览**（工具组/推理行/活动组），并 `position: fixed; z-index:80` 覆盖目标行；浮层用 `onPointerDown` 吞掉第一次点击 → 点两下才生效。 | `TaskConversationMessage.tsx:1207,1256-1262,1269-1275,1834,2662`；`task-conversation.css:2600-2602` | Material 100–300ms、动效不阻塞交互 | **S2** |
| 动效 | **定义了动画但未接线 / 被覆盖**：`[data-copy-check]`、`[data-plan-complete]` 220ms 入场动画全库只在 CSS 出现，TSX 未挂属性；`.task-state-glyph` 的 160ms 状态切换被 `:4528-4531` 更高特异性 `animation:none` 覆盖 → 复制成功、工具 running→success 全硬切。 | `task-conversation.css:873-887,889-908,2280-2293,4528-4531`；`TaskConversationMessage.tsx:665,1453-1457` | Nielsen #1 | **S2** |
| 动效 | **CLS：artifact 占位 120px → 加载后 `clamp(320px,58vh,620px)` 硬切**，下方消息被整段推走。已有 `content-visibility` 缓解但不消除高度跳变。 | `task-conversation.css:317,333-340,346-362` | 布局稳定 | **S2** |
| 动效 | **折叠用 `grid-template-rows: 0fr→1fr` + `InlineDisclosure` 400ms/400ms delay（实际 260+260ms）**，等价 height 动画，逐帧重排；长工具输出展开时阅读位置漂移。 | `packages/ui/.../inline-disclosure.tsx:20,25`；`task-conversation.css:1259-1266,2159-2171` | Material 避免布局动画 | **S2** |
| 动效 | **推理预览无条件强制滚到底**：每个流式 chunk 写 `node.scrollTop = height`，无「用户已上滚」判断（对比 `chatVirtualizerRuntime` 有 `userScrollGestureRef` 保护）。用户回看前文会被持续拉回。 | `TaskConversationMessage.tsx:1841-1859` | 用户控制、2.2.2 | **S2** |
| 动效 | `TaskLazyOutput` 折叠后 400ms 才卸载，而动画 260ms → 140ms 占位滞留；`reducedMotion ? 0 : 400`（reduce 有保护，正例） | `TaskConversationMessage.tsx:542-546` | Material | S3 |
| 动效 | **离屏动画暂停仅在 `live` 时启用**：`const live = waiting || compacting`，streaming 期间不创建 IntersectionObserver；且 offscreen 白名单脆弱。长回复滚到别处时 spinner/shimmer 仍跑。 | `TaskConversationMessage.tsx:2782-2795`；`workbenchMotion.css:93-103` | WCAG 2.2.2 | S2 |
| 动效 | 错误态颜色切换无 transition；WorkingStatus `Clock3 ↔ spinner` 图标硬切；代码块 `TaskDisclosure` 用 `!transition-none` 强制无过渡 | `task-conversation.css:1241-1243,1589-1596,2668-2670`；`WorkingStatus.tsx:101-105`；`TaskConversationMessage.tsx:1471` | Nielsen #1 | S2 |
| 动效 | `data-motion-offscreen` Reduce 白名单遗漏 `.task-pending__chevron`、`[data-task-code-toggle] svg` 的 180ms transition | `task-conversation.css:3213,3283,3544-3550` | 2.3.3 | S3 |
| 动效 | 复制成功态复位 1600/1500ms，且 timer 未清理、无防抖 → 连点提前消失、卸载后 setState | `TaskConversationMessage.tsx:1420-1428,2031-2039` | 交互健壮性 | S3 |
| 反馈 | 错误卡无一键重试（自动退避承担）；大量解析/产物链路 `catch` 静默，失败表现为空白而非可诊断错误 | `turn.ts:351,789,800...`、`TaskConversationMessage.tsx:1188,1425,...` | Nielsen #9 | S2 |
| 正例 | `Streamdown animated={false}` 关闭逐字闪烁；`TaskProse` memo 比较器防重放；`keepMountedKeys` 防虚拟列表回收闪烁；`useViewportFollowState` 区分用户滚动与程序滚动、提供「回到底部」按钮 | `TaskConversationMessage.tsx:2332-2346,2362-2370`；`chatVirtualizerRuntime.tsx:424-430,1015-1023` | — | 合规 |

### P3 命令面板 / 全局搜索 / 搜索任务对话框

| 维度 | 问题 | 位置 | 规范 | 级别 |
| --- | --- | --- | --- | --- |
| 反馈 | **命令菜单 6 处 catch 只写 logger**，无 toast / inline 错误 / 重试 / 错误态；菜单静默变空或不弹出，用户分不清「无命令」和「出错了」。这是所有右键/菜单的公共入口，影响面最大。 | `components/command/CommandMenus.tsx:438-439,458-459,501-502,507-508,542-543,812-813` | Nielsen #1/#9 | **S1** |
| 反馈 | 工作台 palette 执行动作后直接关闭、无 toast（固定 3 项，无空/加载/错态） | `WorkbenchCommandPalette.tsx:48-83` | Nielsen #1 | S3 |
| 反馈 | 消息内搜索无结果仅 `0/0`，匹配数变化无 `aria-live` 播报 | `FindBar.tsx:189-197`；`MessageListSearch.tsx:27,94` | a11y / #1 | S2 |
| 动效 | 搜索面板 hover/focus 反馈齐备；`useDeferredValue` 无显式 debounce（合理） | `GlobalSearchPanel.tsx:346-347,640-649` | — | 合规 |

### P4 设置页（分子页）

| 子页 | 反馈问题 | 动效问题 | 位置 | 级别 |
| --- | --- | --- | --- | --- |
| 通用 / 工作区 | `:50` `.catch(() => {})` 静默；成功无 toast（自动保存，值即反馈，可接受） | 无 | `GeneralSettings/WorkspaceSettings.tsx:50,63,80,99` | S2 |
| 快捷键 | 冲突/占用/失败 toast 齐、恢复默认有 `popup.confirm`（正例） | 无 | `ShortcutSettings.tsx:211,236,286,352-356` | 合规 |
| 用量 | Skeleton / EmptyState / `aria-busy` / 导出失败 toast 齐（正例） | 热力图入场 `520ms`（超 300ms，有 reduce 保护） | `LocalUsageSettings.tsx:133,159,234,381-397`；`animation.css:163` | S3 |
| 模型服务 | 连接检查 checking + warning/success(2000ms)/error + 多处 confirm（正例） | 无 | `useProviderConnectionCheck.ts:77-126` | 合规 |
| **模型配置** | **全页仅一处失败 `toast.error`，无成功反馈、无 Skeleton、无空/加载态** —— 设置里反馈最薄 | 无 | `ModelSettings/ModelSettings.tsx:175-177` | **S2** |
| 网络搜索 | `saving`+Spinner+成功 toast+inline 失败+`aria-busy` 齐；但运行时下载刷新失败静默 | 无 | `HeadlessBrowserForm.tsx:89,108-110,150`；`BrowserRuntimeDownload.tsx:89` | S2 |
| 文档处理 | 保存/校验/删除确认齐；本地模型状态读取失败静默 | 无 | `DocumentParseSettings.tsx:62,83,110,250-251` | S2 |
| 数据 / 归档 | 清缓存 confirm+loading+disabled；归档 success/partial/fail/info 四级 toast、EmptyState、永久删除确认 —— **全仓最完整**（正例） | 无 | `ClearCachePopup.tsx:117,159,195`；`ArchiveSettings.tsx:186-218` | 合规 |
| 关于 | 更新检查有 toast/aria-busy/disabled，但 `:187,:247` 静默 catch | 无 | `AboutSettings.tsx:187,223,247,280` | S2 |
| 设置框架 | **外链 `void openExternalWebsite()` 无 catch → 打开失败完全静默** | 导航 `aria-current`/active/hover/focus-visible 齐 | `SettingsPage.tsx:100,153-163` | S2 |

### P5 连接器 / 技能（Capabilities）

| 维度 | 现状 | 位置 | 级别 |
| --- | --- | --- | --- |
| 反馈 | `pendingSlug` 防并发 + `aria-busy` + disabled + `PopConfirm tone=danger` + 空态 + success/error toast；**强制 spinner ≥300ms 防闪烁**（正例）。但 `data-ui="capabilities.applying" aria-busy` 仅 `sr-only`，**视觉上无「正在应用」文案**。 | `CapabilitiesPage.tsx:263,351-353,395-409,754,985-1002` | S3 |
| 反馈 | 各对话框 busy 全禁用 + toast 齐（正例） | `ConnectorItemDialog.tsx:183-184,305-343`；`ConnectorManageDialog.tsx:219,255-256` | 合规 |

### P6 自动化 / 定时任务 —— 反馈最差页面

| 维度 | 问题 | 位置 | 规范 | 级别 |
| --- | --- | --- | --- | --- |
| 反馈 | **保存/运行/暂停/恢复/删除全部只改本地 state，零 toast、零 loading/disabled、零 optimistic、零错误边界**（grep 全文件无任何反馈关键词命中）。用户无法确认定时任务是否生效。 | `WorkScheduledTasks.tsx:113-128,212-219,245-248,276-310,350-353,427-431,559-563` | Nielsen #1/#9 | **S1** |
| 反馈 | 删除仅「二次点击按钮变红」的二段确认，未用全局 `popup.confirm` / `ConfirmActionPopup`，是全仓唯一破坏性操作确认不达标的页面。 | `WorkScheduledTasks.tsx:276-310` | Nielsen #5 | **S2** |
| 反馈 | 三个空态齐备、`role="alert"` 表单校验、hover/focus-visible（正例） | 同上 | — | 合规 |

### P7 知识库 / 文件 / 文档资料库

| 页面 | 问题 | 位置 | 级别 |
| --- | --- | --- | --- |
| 知识库主页/分析 | `:131,:171,:302` 静默 catch；分析数据拉取失败无任何提示 | `KnowledgePage.tsx:246,395`；`WorkspaceAnalysisPage.tsx:131,171,302,425-438` | **S2** |
| 知识库检索/安装 | `installCloudKnowledge.ts`、`knowledgeQuery.ts`、`knowledgeSearchState.ts` 吞异常；`WikiRetrievalStatus` 失败只给一行 `role="status"`，**无重试按钮**；preparing 延迟 400ms（有意防闪烁，正例） | `installCloudKnowledge.ts:15,23`；`knowledgeQuery.ts:92`；`WikiRetrievalStatus.tsx:6,14,16-22` | **S2** |
| 文件页 | 删除 confirm + 成功/部分失败/失败 toast + EmptyState + 刷新保留旧列表防闪烁（正例）；两处 catch 未逐行确认 | `FilesPage.tsx:238,277,371-379,419,576,1050-1052` | 合规（待核 2 处） |
| 文档/资料库 | 自建删除确认 Dialog + inline deleteError + `aria-busy`/disabled + 下载 toast；但 `FileEditorWindow.tsx:26`、`HtmlPreviewActions.tsx:89,94`、`SaveDocumentDialog.tsx:61` 静默 catch | `PersonalLibraryPage.tsx:642,1043-1044,1786-1813`；`SaveDocumentDialog.tsx:61,80,117` | S2 |

### P8 小程序 / 浏览器

| 页面 | 问题 | 位置 | 级别 |
| --- | --- | --- | --- |
| 小程序列表/新建/安装 | 反馈齐备（新建 loading/disabled/toast、删除确认、拖包安装 toast、visibility optimistic + 失败 toast）—— 正例 | `NewMiniAppPanel.tsx:119,135,157,176`；`MiniAppsPage.tsx:138,219,240` | 合规 |
| 小程序运行页 | **错误态只有一行文案，无重试按钮、无 toast**；加载用 BeatLoader | `MiniAppPage.tsx:126-134,137-148,153-165` | **S2** |
| 浏览器壳 | `BrowserPage` 自身不渲染任何加载/错误反馈，全靠 `WebviewNavigation`（其 `role="progressbar"`、reload→stop、toast.error 齐） | `BrowserPage.tsx:14-20,65-81`；`WebviewNavigation.tsx:262,279,288,293,467-471` | S3 |

### P9 消息内搜索 / 多选 / 会话列表

| 维度 | 问题 | 位置 | 级别 |
| --- | --- | --- | --- |
| 反馈 | 框选高亮仅 300ms、无结果提示弱、匹配数变化无 aria-live；批量删除有 confirm+toast（正例） | `SelectionBox.tsx:123-124`；`FindBar.tsx:189-197`；`useMessageSelectionController.ts:227-352` | S2 |
| 反馈 | `MessageGroupMenuBar.tsx:106` 批量重试失败依赖可选 `notifyError?.`，宿主未注入时静默（推测，未核实注入） | `MessageGroupMenuBar.tsx:106` | S3 |

---

## 二、跨页面系统性问题

| # | 问题 | 证据 | 规范 |
| --- | --- | --- | --- |
| X1 | **动效 token 双轨**：`.arc` scope（`packages/ui/.../arc/foundation.css:99-112`）定义 `--duration-*`/`--ease-*`，22 个 arc 组件用变量；renderer 侧（4548 行 `task-conversation.css`、`animation.css`）几乎全硬编码 160/120/200/240ms。`SendMessageButton.css:29`、`ComposerStopButton.css:41` 写 `var(--duration-standard, 240ms)` 带 fallback，说明变量在该 DOM 树上根本未定义。另有 `theme.css:428,598` 第三套命名。 | 见左 | 设计系统一致性 |
| X2 | **基础无限动画无 reduce 保护**：`animation.css:17` pulse 1.5s、`:86` rotate 0.75s linear、`:235` shimmer 3s linear、`:252` reveal 0.5s linear；`index.css:146` `.loader flash 0.5s infinite alternate`（1Hz，**低于 3Hz 阈值，不构成 WCAG 2.3.1 违规**）；`LoadingIcon.tsx:13` 直接消费无保护的 rotate。 | 见左 | 2.3.3 / HIG |
| X3 | **5 个 CSS 文件完全无 reduce 保护**：`index.css`、`CommandListPopover.css`、`richtext.css`、`scrollbar-quiet.css`、`theme.css` | 见左 | 2.3.3 |
| X4 | **Spinner / BeatLoader 不受兜底**：`primitives/spinner.tsx:27-32` 与 `components/Spinner.tsx:31-36` 的 `repeat: Infinity` 未读 `useReducedMotion`；`react-spinners` BeatLoader（10 处）既不匹配 `animate-spin/pulse` 类也不在 offscreen 白名单 → reduce 下仍跑（**依据调用侧推断，未读包内实现**）。 | 见左 | 2.3.3 / 2.2.2 |
| X5 | **延迟集群各自为政**：Tooltip 500ms（`primitives/tooltip.tsx:287`）、模型详情 HoverCard **1500ms**（`ModelSelectorDetailCard.tsx:27,300`）、会话卡 650ms、链接卡 500/100ms、聚合预览 600ms、通用 HoverCard 无显式 openDelay（推测走 Radix 默认）。正例：`MessageTokens.tsx:108` 200ms、`AgentRightPane.tsx:1661` 120ms。 | 见左 | Material 100–300ms / Nielsen #1 |
| X6 | **进度/成功态时长不一致**：trace `ProgressBar.tsx:15` 300ms vs arc `progress.module.css:3` 480ms；Toast 默认 5000ms，error 常驻、loading→success 2000ms；copy 成功态 1500/1600ms。 | 见左 | 一致性 |
| X7 | **全局焦点样式被抑制**：`index.css:73-75 @layer app { *:focus { outline-style: none } }`，只在 `.arc` 内由 `foundation.css:163-168` 用 `:focus-visible !important` 补回。非 arc 组件（46 个文件有 focus-visible）无全局保证。 | 见左 | WCAG 2.4.7 |
| X8 | **重复定义易漂移**：`components.css` 与 `styles-legacy-compat.css` 各 4 个相同 infinite 动画；`task-conversation.css` 内 `task-tool-spin` 定义在 `:1158/:3165/:3397` 三处且时长 700ms/700ms/0.8s 不一致。 | 见左 | 可维护性 |
| X9 | **`tailwind.css:140`** 在 reduce 下把启动页旋转换成 `pageLoaderPulse 1.2s infinite` —— 仍是无限动画（降低强度而非静止），与严格 Reduce Motion 标准有张力。 | 见左 | 2.3.3 |
| X10 | **静默 catch 群**：命令菜单 6 处、自动化整页、知识库检索/安装、设置多处、任务产物解析链路。共同表现为「操作了什么都没发生」。 | 见各页 | Nielsen #1/#9 |

---

## 三、优先修复建议（按收益/成本排序）

1. **补 `@keyframes` 闭合**（`task-conversation.css:3255` 加 `}`），一次性恢复 5 条规则，包含折变箭头过渡与折叠态指示。零风险、收益最高。
2. **`useSmoothScrollAnimation` 接 `useReducedMotion`**：reduce 时改瞬时；正常时长从 50 帧（830ms）压到 12–18 帧（200–300ms）。直接解决 WCAG 2.3.3 S1。
3. **取消 `useMinimumDisplayDuration` 的 1000/1200ms 扣留**：改 150–250ms trailing throttle + 立即首帧，保留 error/waiting bypass。
4. **统一 hover/tooltip 延迟**：1500ms → 200ms、650/600/500ms → 200–300ms；把 600ms 聚合预览浮层改 `pointer-events: none` 消除吞点击。
5. **自动化页补反馈**：save/run/pause/resume/delete 加 loading/disabled + 成功/失败 toast，删除换 `popup.confirm`。
6. **命令菜单 catch 补 inline 错误态 + 重试**。
7. **回退 composer 加 `sending` busy**。
8. **补 5 个无保护 CSS 文件的 reduce 块**，给 Spinner/BeatLoader 包 `motion-reduce:animate-none`。
9. **消除 CLS**：artifact loading 保持同一 `clamp()` 高度；折叠改 `transform/clip-path` 或固定高度。
10. **把 renderer 硬编码时长迁到全局 token**，去掉 `var(..., fallback)` 假引用；统一 `focus-visible` 全局基线。

## 四、边界（Explicitly not claimed）

- 本报告为**静态代码阅读**，未运行 UI、未用性能面板测帧率/CLS、未做屏幕阅读器实测、未量化对比度。
- 标注「推测」的项：Radix HoverCard 默认延迟、`react-spinners`/`streamdown` 内部动画实现、CSS 层叠覆盖的运行时结果（`@keyframes` 吞规则、`animation:none` 覆盖）；其中 `@keyframes` 未闭合的**结构缺陷**是代码确认，运行时后果依据 CSS 规范推断。
- 未核实的部分：FilesPage 两处 catch、useChatWriteActions 的多处 catch、SkillsSettings hooks 层反馈、`notifyError?.` 宿主是否注入。
- 未评估：视觉风格、文案质量、国际化、纯 Rust 侧行为。
- 本轮**未修改任何产品代码**。
