# Aster 任务消息流 UI 形态清单（2026-11-11）

只读静态侦察，基于当前 checkout（父仓库 `85e640c6d`，`apps/studio` gitlink `07ffdfcf5`、工作树 `42303bb224` + 7 个未提交改动）。未修改任何文件、未运行 UI。

**路径缩写**：`$R` = `apps/studio/src/renderer`，`$S` = `apps/studio/src/shared`。

## 0. 口径：为什么没有「一个总数」

消息流不是一套渲染器，而是 **8 张正交的表**；一条消息在屏幕上会同时命中其中多张表各一项（例如「失败消息行」= R18 = B10 = L17，同一处被计数三次）。因此：

- **枚举条目合计 222 项**（31+42+32+15+27+20+22+20+13），这是"代码里可数出的渲染分支/组件形态数"，**不是互斥状态数**。
- **互斥口径**下真正独立的是：消息行外观 31 种、可见内容块 30 种（42 减去 12 种代码明确 `return null` 的隐藏 part）、工具卡片 47 类（32 注册 + 15 旁路）、工具基础状态 7 值、消息状态 4 值、流状态 7 值。
- 实际屏幕形态 = 各维度取值的**组合**，远大于条目数；这也解释了为什么截图里一屏能同时出现 10 多种内部机制。

### 决定分流的两套渲染器（最关键的结构事实）

`$R/components/chat/messages/frame/MessageContent.tsx:27`：

| 条件 | 内容渲染器 |
| --- | --- |
| `workbenchConversation && role === 'assistant' && AssistantMessage`（任务页助手消息） | `$R/pages/agents/taskConversation/TaskConversationMessage.tsx` → `TaskConversationTurn`（transcript 流水行） |
| 任务页 user 消息、所有非 workbench 场景 | `$R/components/chat/messages/blocks/MessagePartsRenderer.tsx`（part switch） |
| 共用外壳 | `frame/MessageFrame.tsx`、`MessageHeader.tsx`、`list/MessageGroup.tsx`、`list/MessageAnchorLine.tsx` |

状态权威：`MessageStatus = pending | success | error | paused`（`$S/data/types/message.ts:538`）；`TopicStreamStatus = pending | streaming | done | aborted | awaiting-approval | awaiting-form | error`（`$S/ai/transport/stream.ts:85`）；工具状态 `pending | streaming | invoking | done | error | cancelled | waiting`（`$R/types/mcpTool.ts:11` + UI 派生 `waiting`）。

---

## 表 A · 消息行形态（31）

| # | 形态 | 触发条件 | 关键组件 |
| --- | --- | --- | --- |
| R01 | 用户消息·气泡 | `role!=='assistant' && messageStyle==='bubble' && !multiSelect` | `frame/MessageFrame.tsx` |
| R02 | 用户消息·带头像 header | `messageStyle!=='bubble'` 或多选 | `frame/MessageFrame.tsx`、`MessageHeader.tsx` |
| R03 | 用户消息·附件外置行 | `hoistAttachments` + `file` part | `frame/MessageFileGroup.tsx`、`MessageAttachments.tsx` |
| R04 | 助手消息·普通（显示身份） | `role==='assistant'` 且非 workbench | `frame/MessageHeader.tsx` |
| R05 | 助手消息·workbench 隐藏身份 | `workbenchConversation`，`data-avatar-hidden` | `frame/workbenchAssistantMessage.tsx` |
| R06 | 助手消息·任务页 transcript | `MessageContent` 分流命中 | `TaskConversationMessage.tsx:2672` |
| R07 | 编辑态消息行 | `editingMessageId === message.id` | `frame/MessageFrame.tsx:296` |
| R08 | 多选模式消息行 | `selection.isMultiSelectMode` | `frame/MessageFrame.tsx:188` |
| R09 | 上下文边界分隔行 | `message.isContextBoundary`（`data-clear`） | `frame/MessageFrame.tsx:201` |
| R10 | 多模型组·fold | `isAssistantMultiModelGroup` + `fold` | `list/MessageGroup.tsx:296` |
| R11 | 多模型组·grid | `grid` + `isGrouped` | `list/MessageGroup.tsx:332` |
| R12 | 多模型组·horizontal / vertical | 对应 `multiModelMessageStyle` | `list/MessageGroup.tsx:423` |
| R13 | 流式/处理中助手行 | `isActiveTurnProcessing` / `streaming` | `blocks/MessagePartsRenderer.tsx:1487` |
| R14 | 已落定但流仍活跃（awaiting） | `taskTurnPresentation().awaiting` | `frame/taskTurnPresentation.ts:17` |
| R15 | 审批/待确认行 | `isApprovalAnchor` / `confirming` | `taskConversation/WorkingStatus.tsx:67` |
| R16 | 暂停行 | `status==='paused'` 且无可见 part | `MessagePartsRenderer.tsx:1677` |
| R17 | 空内容占位行 | 无可见 entry 且处理中 | `blocks/PlaceholderBlock.tsx` |
| R18 | 失败·含错误 part 行 | `data-error` / 工具 `error` | `blocks/ErrorBlock.tsx`、`TaskErrorNote` |
| R19 | 已完成过程组·折叠 | `collapseCompletedToolHistory` | `blocks/MessageProcessGroup.tsx:143` |
| R20 | 活动过程组行 | `phase='active'` | `blocks/MessageProcessGroup.tsx:116` |
| R21 | 连续工具聚合行（通用） | `groupPartEntries()` 连续 `tool-*` | `blocks/ToolBlockGroup.tsx:584` |
| R22 | 连续 2 工具组（任务页） | `clusterTurnActivity()` rest.length===1 | `TaskConversationMessage.tsx:2373` |
| R23 | 活动聚合组（任务页，≥3 工具） | rest.length>=2 | `TaskConversationMessage.tsx:2590` |
| R24 | 单工具行（任务页） | `TaskTurnItem{kind:'tool'}` 未聚合 | `TaskConversationMessage.tsx:578` |
| R25 | 文件活动行（任务页） | `clusterTaskFileActivities()` | `taskConversation/TaskFileActivity.tsx` |
| R26 | WorkingStatus 行 | `waiting || confirming` | `taskConversation/WorkingStatus.tsx` |
| R27 | 任务页 tail 附加区 | `proof` / `workspaceProductsForTurn()` | `TaskAutoReview.tsx`、`TaskFollowUpSuggestions.tsx` |
| R28 | 子代理折叠列表行 | `subagentEntries.length>0` | `MessagePartsRenderer.tsx:1705` |
| R29 | 会话结果 / 报告产物卡片行 | `sessionToolResponses` / `canRenderReportArtifacts` | `tools/agent/SessionResultCards.tsx`、`ReportArtifacts.tsx` |
| R30 | 渲染错误兜底行 | 任意块抛错 | `blocks/BlockErrorFallback.tsx` |
| R31 | 送达 / 自主轮次 badge 行 | `message.delivery` / `turnOrigin` | `frame/MessageHeader.tsx:58,260` |

## 表 B · 内容块 / part 类型（42，其中 12 种隐藏）

可见 30 种：`text` 正文（B01）、任务页 prose（B02）、`reasoning` 通用（B03）、任务页推理行（B04）、`data-code`（B05）、图片 `file`（B06）、非图片 `file`（B07）、工具 part 通用（B08）、任务页工具行（B09）、`data-error`（B10）、`data-form` 问答表单（B11）、`data-video`（B12）、`data-compact`（B13）、`data-compaction-anchor` 多态（B14）、`data-conversation-reset`（B15）、`data-agent-session-fork`（B16）、`data-translation`（B17）、`data-retry`（B18）、连续图片组（B31）、连续工具组（B32）、连续 video 组（B33）、过程组外壳（B34）、占位块（B35）、块错误兜底（B36）、子代理/会话结果/报告产物（B37）、hover 活动预览（B38）、文本内围栏衍生形态（B39，含 json-render 表单 / HTML 卡 / 审查发现卡 / mermaid / 代码 / 文档预览）、引用 pill 与上标（B40）、内联产物预览（B41）、暂停空 markdown（B42）。

**代码明确 `return null` / 隐藏的 12 种**：B19 `data-context-usage`、B20 `data-agent-task-event`、B21 `data-task-plan`、B22 `data-knowledge-scope`、B23 `data-review-source`、B24 `data-review-progress`、B25 `data-workbench-proof`（消息体内不渲染，驱动消息级复审卡）、B26 `data-clear`（由 frame 层画分隔线）、B27 `data-agent-plan`（任务页只进 hover 预览）、B28 `source-url`/`source-document`/`step-start`、B29 `data-citation`、B30 未知 part type。

关键文件：`$R/components/chat/messages/blocks/MessagePartsRenderer.tsx`（switch 在 636–825 行）、`$S/data/types/uiParts.ts:157-183`（part 键全集）、`$R/components/chat/messages/utils/messagePartLayouts.ts:39-56`（`HIDDEN_PART_TYPES`）。

## 表 C1 · 工具渲染器注册项（32）

注册表 `$R/components/chat/messages/tools/agent/toolRendererRegistry.tsx:26-59`，全部位于 `tools/agent/`：

`Agent`、`Read`、`Task`、`TaskOutput`、`TaskStop`、`Bash`、`Search`、`Glob`、`TodoWrite`、`WebSearch`、`Grep`、`Write`、`WebFetch`、`Edit`、`MultiEdit`、`BashOutput`、`NotebookEdit`、`ExitPlanMode`、`Skill`、`ToolSearch`、`ListMcpResources`、`ReadMcpResource`、`TaskCreate`、`TaskGet`、`TaskUpdate`、`TaskList`、`SendMessage`、`TeamCreate`、`TeamDelete`、`EnterWorktree`、`ExitWorktree`、`Workflow`。

其中 8 项复用 `StructuredAgentTool.tsx`（仅 label、无展开体），6 项复用 `TaskTool.tsx` 导出。`AgentToolsType` 共 33 值，唯一未注册的是 `AskUserQuestion`（走独立卡片）。未命中时 `renderTool()` 返回 `{label:null}`。

## 表 C2 · 注册表外仍出现在消息流的工具 UI（15 类）

`AskUserQuestionCard`、`ToolApprovalOutcome`（拒绝）、`meta/MessageMetaTool`、`mcp/MessageMcpTool`、`agent/UnknownToolRenderer`、`knowledge/MessageKnowledgeSearch`、`webSearch/MessageWebSearch`、`painting/MessageGenerateImage`、`AgentExecutionTimeline → AgentToolCallCard → AgentToolDisclosure`、`SessionCreateTool`/`SessionSendTool`、`ToMarkdownTool`、`NavigateTool`、`MessageChannelConfigTool`（渠道二维码）、`CreateAgentTool`/`PrepareDiagnosticReportTool`（内联结果）、`ReportArtifacts`/`SessionResultCards`（帧级结果）。

折叠原语三套：`tools/shared/ToolDisclosure.tsx`（含 `DeferredDisclosureContent` 延迟挂载）、`primitives/Disclosure.tsx`（Accordion）、`agent/AgentToolDisclosure.tsx`（展开态持久化）。

## 表 C3 · 工具行状态 / 呈现（27）

1 折叠（默认）、2 展开、3 单次工具行、4 多次聚合「N 个工具调用」、5 聚合组内平铺、6 进行中 `invoking`/`streaming`（骨架/微光/pulse）、7 成功 `done`（indicator 默认隐藏）、8 失败 `error`（TriangleAlert + Tooltip）、9 失败行单行布局（错误文本进 header）、10 **等待审批 `waiting`（消息流工具行消失，composer 接管）**、11 聚合头 waiting 优先级、12 被拒绝（CircleX + reason）、13 取消/中断 `cancelled`、14 延迟挂载 deferred mount、15 MCP 分级延迟（40/120/220ms + idle callback）、16 React.lazy/Suspense、17 流式参数 partial JSON、18 输出截断 TruncatedIndicator、19 耗时显示、20 自动审批 ShieldCheck、21 进度环、22 子代理流程行（5 图标态）、23 子任务聚合行、24 压缩锚点跳过/中断标记、25 thinking 头 / 完成态过程组、26 无数据兜底「此工具暂无可用数据」、27 `$deferredToolResult` 延迟加载。

基础状态值 7 个：`pending | streaming | invoking | done | error | cancelled | waiting`（`mapPartStateToStatus` 见 `tools/toolResponse.ts:82-102`；`settleToolResponse` 把未终局强制投影为 `cancelled`）。

## 表 D · 列表容器状态（20）

L1 空会话欢迎覆盖层（`ConversationGreeting`，`MessageList` 无专用空态）、L2 初始加载骨架（3 组 Skeleton，延迟 160ms）、L3 历史分页触顶加载、L4 流式追加 live tail（历史层/live 层分离）、L5 虚拟列表回收与占位（`data-message-index`、`keepMountedKeys`、`freezeSpacerRef`）、L6 滚动位置恢复（`chat.scroll_anchor.<topicId>`）、L7 视口跟随 / 阅读冻结（following/reading）、L8「回到底部」浮动按钮、L9 导航按钮组、L10 锚点线 / 轮次跳转轨（≥5 轮、列宽≥700px）、L11 消息大纲浮层、L12 多选模式（复选框 + 底部胶囊条）、L13 拖拽框选、L14 全选等待历史分页、L15 消息内搜索与命中高亮（CSS Custom Highlights）、L16 分隔（上下文清除 / 压缩 / 会话重置 / 时间戳 / 轮次 tick）、L17 整条消息错误卡（含 `withTerminalErrorFallback`）、L18 行内生成中占位、L19 底部覆盖层让位 inset、L20 会话切换重挂载。

主文件：`$R/components/chat/messages/MessageList.tsx`、`list/MessageVirtualList.tsx`、`list/chatVirtualizerRuntime.tsx`、`list/useScrollPositionMemory.ts`、`list/useViewportFollowState.ts`、`list/MessageListSearch.tsx`、`list/SelectionBox.tsx`、`list/MessageAnchorLine.tsx`。

## 表 E · 会话状态覆盖层（22）

S1 正在生成、S2 `aria-busy` 区域、S3 Stop 取消生成按钮、S4 后台任务停止按钮、S5 完成态、S6 中断/取消态、S7 错误态、S8 上下文压缩提示（5 种子状态）、S9 API 重试行（含倒计时）、S10 重试/故障转移状态块、S11 任务进行中 WorkingStatus（elapsed + tok/s + popover）、S12 等待确认态、S13 token 用量条/实时速率、S14 token/成本详情 HoverCard、S15 引用面板（"N sources" pill + 侧栏）、S16 右侧详情面板（resource/files/browser/status/trace/flow/terminal）、S17 任务进度条、S18 审批浮层（**在 composer，不在消息流**）、S19 表单挂起态、S20 AskUserQuestion 卡、S21 送达/来源徽标、S22 会话初始化 / 不可用。

## 表 F · 行内操作（20）

A1 编辑/从这里编辑、A2 复制、A3 重新生成、A4 指定模型重新生成、A5 翻译（含语言重试/删除译文/中止）、A6 反馈/设为上下文分支、A7 保存到笔记、A8 保存为文件、A9 导出子菜单（图片/MD/Word/Notion/语雀/Joplin/Obsidian/思源/纯文本）、A10 删除（可带确认）、A11 更多操作菜单、A12 新分支/复制分支到新话题、A13 派生会话 fork、A14 进入多选、A15 兄弟分支切换、A16 定位/高亮消息、A17 错误卡上的重试/诊断/移除、A18 多模型组级操作（4 布局切换）、A19 附件/文件卡操作、A20 代码块/表格操作（导出 Excel）。

统一前提：`showMenuBar = !hideMenuBar && !hideMessageMenuBar && !isEditing && !isStreamTarget && !isApprovalAnchor`；多数按钮还依赖 `useMessageListActions` 是否注入对应 action。

## 表 G · 产物卡片（13）

P1 任务产物列表（含 lineage 版本）、P2 内联 Office 预览（deferred/loading/ready/failed）、P3 内联图片预览、P4 下载按钮、P5 变更文件列表 + diff、P6 HTML 产物卡、P7 文档内容卡、P8 文档选区审阅卡、P9 附件卡片、P10 自动审查/审查发现卡、P11 计划/JSON/Mermaid/diff 预览、P12 保存到任务发现、P13 后续建议（「接下来」）。

---

## 与用户截图相关的观察

截图（任务会话页）同时命中：R06 transcript、R23 活动聚合组、R26 WorkingStatus、R27 tail、R18/B10/L17 错误卡、R31 时间戳与 token 统计、A2/A7/A9 行内操作、C3 工具行状态、S11/S13 速率与用量。也就是说**用户看到的一屏是 8 张表里至少 10 个条目的叠加**，而其中「询问用户 / 列出文件 / N 次工具调用 / 目录 UUID / 词元速率 / 机器门禁诊断」全部属于执行轨迹与内部记账，不是成果叙事。

## 复核说明 / 未覆盖

- 全为静态代码阅读，未运行 UI、未跑测试验证每条分支。
- 三路侦察标注的推测项：`CitationsList` 的 JSX 挂载链、Stop 按钮内部渲染、审批 composer 的按钮布局、完成/中断态是否有隐藏横幅、部分具体工具卡片的 `SkeletonValue`/截断行为。
- 代码中**未发现**：按日历日期分组的分隔条、已删除/撤回消息的占位 UI、`system` 角色的专门分支（渲染只按 `assistant` 二分）。
- 可选能力（`useMessageListActions` 注入与否）会改变按钮显隐，未穷举所有 host 组合。
- 历史源码副本（`apps/studio/docs/evidence/.../source-v*`）、`docs/evidence/evaluation-hardening-*/` 下的测试快照未纳入。
