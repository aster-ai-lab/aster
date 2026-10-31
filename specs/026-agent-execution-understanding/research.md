# Research: AgentPlay × 状态理解 × Aster

调研日期：2026-10-29。上游版本只用于固定本计划依据，不表示仓库已引入其代码。

## 1. 上游依据

| 项目 | 固定版本 | 阅读材料 | 与本功能直接相关的能力 |
| --- | --- | --- | --- |
| 状态理解参考 | 设计调研快照 | 状态图、读取层与 Viewer 指南 | 状态／阶段／动作／原文四级探索、版本树与执行顺序、Story、决策折叠、证据链接、完整轨迹导出；Smart Context 本期不采用 |
| AgentPlay | `d8b81d0f9178165a5a3e43e871aa83e676e09af0` (`main`) | [README](https://github.com/Entelligence-AI/agentplay/tree/d8b81d0f9178165a5a3e43e871aa83e676e09af0)、[format](https://github.com/Entelligence-AI/agentplay/blob/d8b81d0f9178165a5a3e43e871aa83e676e09af0/docs/format.md) | 跨 Agent 事件模型、资源地图、多轨回放、任务／模型／子 Agent 镜头、错误／重试／延迟信号、严格脱敏 `agentplay/v1` |

许可证分别为 MIT 与 Apache-2.0。本计划首先实现独立投影和格式适配，不复制完整 Viewer；若后续复制任何源码，必须单独保留版权和 NOTICE，并重新审查上游改动。

## 2. 能力互补与边界

| 问题 | AgentPlay | 状态理解 | Aster 融合决定 |
| --- | --- | --- | --- |
| 发生了什么 | 确定性事件、时间、资源、指标和结果 | 原始事件归档 | Aster 事实层拥有唯一事件流 |
| 为什么转向 | 不擅长表达版本与因果 | 状态、分支、决策、`informs` | 结构层只通过事件引用建立 |
| 能否核查 | 分享地图故意剥离原文 | 原文与摘要并存、引用回跳 | 原文留在本地，按需解析；两种导出分离 |
| 如何回放 | 多轨、步进、镜头、异常导航 | Story 引导播放 | 共享游标，决策与执行两种视图 |
| 如何看资源关系 | 放射资源地图是核心交互 | 状态／动作图为主 | 放射资源图列为 P1 必做，与同一事件和检查器联动 |
| 如何问答 | 无核心证据问答 | 两轮 Smart Context | 用户明确排除交互式 AI 问答；只保留单次语义注释 |
| 如何处理版本 | 文件 touch 和执行结果 | 快照、迁移、lineage、测量 | 没有观察效果证据就降级，不伪造 DAG |

### 核心研究结论

不能把 `agentplay/v1` 直接转换成完整轨迹。前者为安全分享删除自由原文，后者的原文核查和逐字 quote 需要这些证据。融合点必须在脱敏前的 Aster 统一模型，两个上游格式只做适配器。

状态理解参考中的 reading layer 是 analyst interpretation，不是额外观察事实。其结构校验能发现漏引用、非逐字 quote 和反向 `informs`，不能证明总结语义正确。因此 Aster 必须保留 `model_inferred`、源修订和原始证据，并允许无模型的局部视图。

## 3. Aster 现有先例

| 关注点 | 已有实现 | 复用决定 |
| --- | --- | --- |
| 当前分析页 | `apps/studio/src/renderer/pages/knowledge/WorkspaceAnalysisPage.tsx` | 保留页面入口、指标、筛选、播放和检查器；扩展为共享游标的双模式页面 |
| 纯数据投影 | `apps/studio/src/renderer/pages/knowledge/workspaceAnalysisProjection.ts` 及 sibling test | 扩展为 `AnalysisRun`，不新增并行 projection owner |
| 长会话分页 | `apps/studio/src/renderer/hooks/useAgentSessionParts.ts` | 复用 load-all、原消息身份和覆盖状态；不在首屏强制传输全部工具输出 |
| deferred 输出 | `apps/studio/src/renderer/hooks/useToolResult.ts`、`src/shared/ai/transport/deferredToolResult.ts` | 复用 SWR 去重、永久 miss 和 `ai.tool.get_result`；分析检查器按需读取 |
| deferred 查找 | `AgentRightPane.tsx` 内 `findDeferredToolResult` | 因第二个真实消费者出现，提取到现有 message utility 并保留 AgentRightPane 行为 |
| 子 Agent 关系 | `agentRightPaneProjection.ts` 的 `buildAgentToolFlowProjection` | 复用 `parentToolCallId`、messageId/partIndex 和子流程语义，不复制计数 |
| 消息统计 | `src/shared/data/types/message.ts` | 使用实测 `runtimeTiming`、spans、Token、context 和 actualModels；旧标量仅作兼容回退 |
| Trace 数据 | `components/chat/trace/TracePage.tsx`、`src/shared/data/types/trace.ts` | 提取可复用读取 hook，让 TracePage 与分析页共享原 span；不复制轮询或把树节点当事实 |
| 图谱 | `KnowledgeGraphView.tsx`、`KnowledgeGraphLayer.tsx` | 现有 WebGL force graph 服务笔记关系，不适合版本 DAG 或放射资源图；使用已有 `@xyflow/react` 实现两类确定性布局，不新增依赖 |
| 后台 Agent session | `knowledgeCompileAgent.ts`、`KnowledgeCompileTurn.tsx` | 复用独立会话创建、精确 sender 绑定和取消模式；不复用百科任务文本或候选写权限 |
| 无工具模型隔离 | `crates/code/acp/src/host_context.rs`、`agent/session_open.rs` 的 `sourceReview` | 单次语义注释采用该路径；宿主提供有界节点与原文，模型无文件或 Shell 工具 |
| 候选写权限 | `crates/code/acp/src/knowledge_compile.rs` | 不复用：它允许 wiki staging candidate writes，会话分析只需要 `sourceReview` 的更低权限 |
| 内部会话过滤 | `isKnowledgeCompileWorkbenchRow` 及 Agent／Knowledge 调用点 | 新增明确分析目的标识，并在相同列表边界排除；不借用 knowledge-compile 名称 |
| UI／选择器 | `@aster-ai-lab/ui`、根 `DESIGN.md`、现有 `SearchSelect` | 原生 Aster 控件与视觉规范；不使用两个上游 Viewer 的主题或 iframe |
| 测试 | `workspaceAnalysisProjection.test.ts`、frontend testing guide | 最低充分层级：纯投影／状态机单元测试，联动组件测试，跨进程与原生旅程单独验收 |

## 4. 关键设计选择

### D1：统一 `AnalysisRun`，不上游格式直通

选择：原始 Aster 数据 → `AnalysisRun` → AgentPlay／完整轨迹导出。

理由：保留稳定证据引用和原始数据所有权，避免脱敏后信息不可恢复，也避免两套指标和顺序。

### D2：先元数据完整，再原文按需

选择：分页加载全部消息元数据用于回放；deferred 输出只在检查器、导出确认或单次语义注释需要时读取。

理由：当前 `useAgentSessionParts` 明确使用 `deferToolOutputs: true`。 eagerly 改为全文传输会放大首屏负载，也违背现有虚拟化边界。

### D3：无快照则无版本

选择：用真实 Diff、哈希、产物和效果证据生成状态；历史数据不足时只显示动作。

理由：测试成功、退出码 0、一次 Read 或模型声称“已修复”均不证明 tracked scope 已改变。

### D4：单次语义注释采用 `sourceReview`

选择：宿主一次性提供有界节点与原文，模型只返回 `AnalysisAnnotation` 候选；无聊天、追问或 Smart Context。

理由：保留 Story／决策说明所需的模型能力，同时遵守用户“不做 AI 问答”的范围，并避免新增 ACP 工具、文件权限、Python 转换服务或第二个模型客户端。

### D5：一个页面、两种联动模式

选择：决策视图与执行视图共享事件、检查器和游标；不是两个标签页各嵌一个 Viewer。

理由：用户应从“为什么转向”直接下钻到“哪次测试失败”，而不是重新搜索同一事件。

### D6：放射资源图是执行视图必做画布

选择：中心为任务，内圈为资源分类，外圈为真实资源；节点通过 `eventIds` 与时间线和检查器联动，筛选只淡化并保持径向布局。

理由：资源关系不是可选装饰，而是定位“此步接触了哪些资源”的主要入口；其验收同时覆盖布局、交互和事实去重。

## 5. 已排除方案

- **嵌入上游 Web Viewer／iframe**：造成双事实源、主题和隐私边界不一致，排除。
- **运行 AgentPlay Python 或外部转换技能**：引入额外进程和运行时，且 Aster 已有原数据，排除。
- **交互式 AI 问答／Smart Context**：用户明确排除；不保留聊天入口或后续追问协议。
- **复用 knowledge compile candidate-write 权限**：超出只读分析需要，排除。
- **用关键词／正则判断业务语义、决策或名称**：违反 Names／Meaning 原则；机械命令解析与模型语义判断严格分开。
- **为所有历史会话伪造完整状态树**：证据不足，改为显式降级。
- **新增数据库保存派生注释**：当前没有跨重启需求，避免陈旧语义与 schema 扩散；按 sourceRevision 内存缓存并延期评估。

## 6. 实施前已关闭的问题

- 完整原文是否已在首屏可用：否，deferred 输出必须按需解析。
- 是否需要新的 ACP 写权限：否，使用 `sourceReview`。
- 是否需要新的图形依赖：否，已有 `@xyflow/react`；现有笔记 WebGL 图不适合版本 DAG 或放射资源布局。
- 是否需要外部 Agent 全量解析器才能交付 Aster 回放：否，先服务 Aster 原会话；外部导入另行扩展。
- 是否允许无模型使用：是，事实回放、放射资源图、证据检查和脱敏导出不得依赖 AI。

## 7. Arc UI 微动效与组件依据

本机已有 [Arc 完整目录审阅与落地记录](../../docs/evidence/local-model-control/2026-10-28/OUTCOME.md)：2026-10-28 快照包含 235 components + 136 blocks，共 371 项；逐项审阅保存在 `arc-ui-research/`。根 `DESIGN.md` §6 已融合 Arc 的 motion roles、组件配方、打断、reduced-motion 和渲染约束。本功能直接继承这些 house 规则，不重新复制公开预览。

### 采用的行为，而非批量安装

| Arc 参考 | 本功能采用点 | 边界 |
| --- | --- | --- |
| `agent-timeline` | 多 Agent 时间轴、playhead、span 选择、单一 CSS／motion clock、离屏停止 | 真实 Aster 事件驱动；20,000 事件不照搬“小几百 spans”的整图模式 |
| `line-replay` | 无 overshoot 的播放重定向、手动 scrubber、关键时刻和性能模型 | 不自动播放；不按帧 React render 或播报 |
| `tool-call` | 稳定工具行、原位展开、状态 glyph、按需挂载输入输出 | 长列表使用虚拟化；筛选不重播 stagger |
| `agent-status`／`agent-run`／`agent-plan` | 真实活动、等待冻结、单次确认、完成后保留失败细节 | 不把历史记录伪装为运行中，不虚构计划步骤或完成百分比 |
| `context-window` | 实测上下文与 compaction 的有界解释 | 不根据文本长度虚构 Token，不重播历史压缩 |
| `action-button`／`copy-button`／`confirm-morph` | 保存与复制的原位 pending／result、稳定目标、失败恢复 | 实际 Promise 决定状态，取消不显示成功 |
| `SegmentedControl`、`EmptyState`、`rowMotion` | house 已有选中底板、首次进入与 reduced-motion | 直接复用 `packages/ui`，不创建第二组件系统 |
| `sunburst` | 仅借鉴隐藏数据表和连续导航思路 | 不采用组件：资源网络不是 part-to-whole 占比，避免错误语义 |
| `task-breakdown` | 不采用 | 本页不是可编辑 PM 计划，不把 Agent 执行误当项目拆解 |

### 实施决定

新增 [contracts/analysis-motion.md](contracts/analysis-motion.md)，将每个动效绑定到真实触发器、状态 owner、打断、停止和 reduced-motion。播放与放射相机采用共享逻辑游标 + 本地 motion value，不进入事实模型；所有循环在隐藏、离屏、暂停、会话切换和 settled 后停止。

## 8. M0 Trace 覆盖实测（2026-10-29）

- `SpanEntity` 原始合同包含 `modelName` 与可选 `usage`；usage 分别记录 `prompt_tokens`、`completion_tokens`、`total_tokens`，并保留可选输入／输出细节。分析投影可直接消费这些字段，无需从 UI tree 文本反推。
- `TraceDataCursor` 由 `historyVersion` 与 `liveRevision` 组成；`reset` 替换快照，非 reset 只携带 live changes。现有 TracePage 测试已证明 cursor 传递、reset 清空、切换 Trace 和 pending IPC 时不重叠轮询。
- 新增原始 span 合同测试，`TraceTreeModel` 在 reset 后保留模型身份和 600／200／800 Token 记录。`TracePage.test.tsx` 当前共 8 个测试通过。
- 上游未记录 per-request 字段的会话仍按消息级 `metadata.stats` 投影，不能从 span 名称或文本猜测模型、Token 或成本。
