# Implementation Plan: Agent 执行理解与证据回放

**Branch**: 当前工作区；不创建分支、不提交 | **Spec**: [spec.md](spec.md) | **Input**: [spec.md](spec.md)

**Gate context**: Product regime； owning slice 为既有私有 `apps/studio` 子模块的 renderer／shared，复用根工作区现有 ACP `sourceReview`。不规划 `apps/cloud` 模块。

## Summary

扩展 `WorkspaceAnalysisPage` 与 `workspaceAnalysisProjection`，以统一 `AnalysisRun` 连接 Aster 原始消息、deferred 证据和 Trace spans。先交付 AgentPlay 式事实回放、必做放射资源图与脱敏导出，再交付 状态理解式状态图、Story 和完整轨迹导出，随后以现有 `sourceReview` 无工具路径交付单次语义注释；不做交互式 AI 问答。全部表面按 `analysis-motion` 合同吸收 Arc 的连续反馈、组件行为和 restrained springs，复用 house 控件与 Motion／CSS。两个上游格式仅是边界适配器；不引入 iframe、Python 服务、第二事实库、第二设计系统或新产品配置。

## Constitution Check

*GATE: 已重读 `.specify/memory/constitution.md`。PASS 表示计划有明确实施与测试约束，不表示功能已经通过验收。*

| Principle | Status | Evidence in the plan |
| --- | --- | --- |
| I. Fail closed | PASS | 覆盖不完整、deferred miss、失效引用、非法状态、越界证据、预算超限和导出未确认均命名拒绝；M1–M4 |
| II. Evidence retained | PASS | `docs/evidence/desktop/<date>-agent-execution-understanding/OUTCOME.md`；M5 记录命令、计数、截图和未验范围 |
| III. Hermetic tests | PASS | Studio renderer/shared sibling Vitest；固定模型输出协议测试；原生资格单列，不依赖 live PostgreSQL |
| IV. Closed sets | PASS | `workspaceAnalysisModel.ts` 的 union + Zod 边界；未知值拒绝；本期不新增 Rust enum |
| V. One workspace / published APIs | PASS | 既有 Studio 子模块和根 ACP；不新增 crate、workspace、SDK 或私有跨 crate 调用 |
| VI. Local schema changes | N/A | 无数据库迁移；派生注释按 sourceRevision 内存保存，原文仍由原 owner 拥有 |
| VII. Shrink-only ratchets | PASS | 不增加 native／Radix select、权限或 facade allowlist；复用现有控件与 IPC |
| VIII. Replaceable transports | PASS | DataApi／Trace IPC／deferred IPC／ACP 留在现有边界；`AnalysisRun` 不含 provider transport |
| IX. One configuration language | N/A | 不新增产品配置；path mode 是单次导出参数，不是持久化配置 |
| XIII. One repository | PASS | 规格在父仓库，Studio 源码只在既有私有子模块；不新建上游 checkout 或仓库 |
| XIV. Names are not guessed | PASS | 任务／文件／模型名来自精确存储字段；模型生成的主题名带 provenance，不用 capitalization／regex 猜名 |
| XV. Meaning is not matched | PASS | 机械解析仅处理命令与格式；动作语义、决策和因果由模型或 typed 记录提供，不用词表匹配 |
| XVI. Pickers are drawn | PASS | 复用 `SearchSelect`、Button、分段控件和 popover；不新增原生 select |
| XVII. Dialogs stay in place | PASS | 检查器与导出确认原地展开／淡入；不使用桌面边缘滑入 dialog |

## Project Structure

### Documentation (this feature)

```text
specs/026-agent-execution-understanding/
├── spec.md
├── plan.md
├── research.md
├── data-model.md
├── tasks.md
└── contracts/
    ├── analysis-run.md
    └── analysis-motion.md
```

`tasks.md` 将里程碑拆成带依赖和 hermetic 测试义务的实施任务；本计划与任务列表都不作为实施证据。

### Source Code

| Layer / phase | Existing owner and pattern | Planned files |
| --- | --- | --- |
| M1 事实模型 | `pages/knowledge/workspaceAnalysisProjection.ts` 与 sibling test | 修改投影；新增 `workspaceAnalysisModel.ts`、`workspaceAnalysisEvidence.ts`、`workspaceAnalysisAgentPlay.ts` 及聚焦测试 |
| M1 历史与 Trace | `useAgentSessionParts.ts`、`TracePage.tsx`、`shared/data/types/trace.ts` | 复用分页；从 TracePage 提取 `components/chat/trace/useTraceData.ts` 并接回两个消费者；扩展当前页读取所选会话 `traceId` |
| M1 deferred 证据 | `useToolResult.ts`、`AgentRightPane.tsx`、`shared/ai/transport/deferredToolResult.ts` | 将 `findDeferredToolResult` 提取到 `renderer/utils/message/find.ts`；更新 AgentRightPane 与分析检查器，保留原测试 |
| M1 导出 | `pages/agents/planExport.ts` 的 desktop save + browser fallback | 提取通用 `renderer/utils/saveFile.ts` 并接回 planExport；新增 AgentPlay adapter 测试 |
| M1 Schema 验证 | 现有 Zod；`json-schema@0.4.0` 不支持 AgentPlay 的 2020-12 Schema | 生产继续使用 Zod；仅在 devDependencies 增加 `ajv`／`ajv-formats` 供官方 Schema 合同测试，更新 `package.json` 与 `pnpm-lock.yaml` |
| M2 执行 UI 与放射图 | `WorkspaceAnalysisPage.tsx`、现有 `@aster-ai-lab/ui`、`@xyflow/react` | 页面保持组合入口；新增 `workspaceAnalysisResourceGraph.ts`、`WorkspaceAnalysisMap.tsx`、`WorkspaceAnalysisTimeline.tsx`、`WorkspaceAnalysisInspector.tsx`、`WorkspaceAnalysisPlaybackControls.tsx` 及布局／组件测试 |
| M3 状态与 Story | 纯投影先例、已有 xyflow 依赖 | 新增 `workspaceAnalysisStateGraph.ts`、`workspaceAnalysisTrajectory.ts`、`WorkspaceAnalysisStory.tsx`、`WorkspaceAnalysisStateMap.tsx` 与正反例 |
| M4 无工具单次语义注释 | `knowledgeCompileAgent.ts`、`KnowledgeCompileTurn.tsx`、ACP `sourceReview` | 新增 `sessionAnalysisAgent.ts`、`WorkspaceAnalysisTurn.tsx`、`workspaceAnalysisAnnotation.ts`；不新增聊天组件、ACP 工具或 Rust 权限模块 |
| M4 内部会话标识 | `shared/types/knowledgeNote.ts` 的内部 compile 过滤先例 | 新增 `shared/types/agentSessionAnalysis.ts`；在 AgentPage、sessionWorkspace、WorkbenchProject*、WorkspaceWikiCatalog、WorkspaceBoard 和当前分析页的既有内部会话过滤点排除分析助手 |
| Arc 微动效 | `packages/ui` 的 SegmentedControl、Button、CopyButton、EmptyState、rowMotion；根 `DESIGN.md` §6；Motion／CSS | 新增 `pages/knowledge/useWorkspaceAnalysisClock.ts` 共享 playhead；在现有分析组件中接入真实状态交换、相机、Context、导出和 reduced-motion，不新增 motion token 或 Arc 依赖 |
| 全阶段 UI 文案 | `src/renderer/i18n/locales/en-us.json` 与其他 locales | 只新增分析页 key；先同步再翻译全部占位，运行现有 i18n 检查 |
| M5 验收 | 当前 Studio／Aster Test 原生流程 | 仅写 `docs/evidence/desktop/<date>-agent-execution-understanding/`；不改 specs 充当验收 |

新增文件只在对应职责出现时创建；不得一次性 scaffold 全部组件。若 M1 可以用现有文件保持简单，不提前拆 M2／M3 文件。

## Research

详见 [research.md](research.md)、[data-model.md](data-model.md)、[contracts/analysis-run.md](contracts/analysis-run.md) 和 [contracts/analysis-motion.md](contracts/analysis-motion.md)。

关键先例与偏差：

- 保留 `WorkspaceAnalysisPage` 与纯 projection，不建立第二分析服务。
- 保留 `useAgentSessionParts` 的 deferred-output 边界，不为“完整回放”改成首屏全量原文。
- `KnowledgeGraphView` 是笔记 force graph，不适合版本 DAG 或放射资源布局；两类分析图都使用已有 `@xyflow/react`，不新增依赖。
- `sourceReview` 已提供固定输入、无工具、隔离上下文；单次语义注释直接复用，不照搬 `knowledge_compile` 的 candidate-write 权限，也不增加聊天协议。
- Arc 行为以本机 371 项目录审阅和根 `DESIGN.md` §6 为准，直接复用 house SegmentedControl、Button、CopyButton、EmptyState、rowMotion 与 Motion／CSS；不批量安装 Arc，不使用 Pro 源码，不把资源网络误用为 Sunburst。
- 官方 AgentPlay Schema 是 2020-12，仓库现有 `json-schema` 包明确不支持新 draft；因此只增加 test-only Ajv，生产模型不引入第二套 validator。

## UI／UX Composition

以 **1440 × 900** 桌面窗口为基准，保留现有“分析、百科、产物、记忆、看板”入口，只在分析页内部切换“理解决策／回放执行”，不增加第二套任务详情页。

```text
┌────────────────────────────────────────────────────────────────┐
│ 任务标题 / 覆盖状态      理解决策 · 回放执行      镜头 / 导出 │ 48 px
├────────────────────────────────────────────────────────────────┤
│ Turns · Calls · Errors · Tokens · Duration · 当前镜头指标      │
├───────────────────────────────────────┬────────────────────────┤
│ 决策模式：版本／阶段／动作 Map         │ 当前节点／事件检查器    │
│ 执行模式：分类内圈＋资源外圈放射图     │ 摘要 · 证据 · 指标      │
│                                       │ 原文按需展开            │
├───────────────────────────────────────┴────────────────────────┤
│ 播放 / 暂停   1× 4× 16×   当前步骤 / 总步骤   回到实时        │
├────────────────────────────────────────────────────────────────┤
│ Tasks / Decisions / Events / Tokens / Context                  │
└────────────────────────────────────────────────────────────────┘
```

- **理解决策**：Map 在版本树与执行顺序间切换；Story 与检查器同步展开状态、阶段、动作和原始记录；“只看决策”只折叠显示，不删除事件。
- **回放执行**：放射图中心为当前任务，内圈为 Files／Shell／MCP／Models／Outputs／Subagents／Errors，外圈为真实资源；支持拖动、缩放和当前路径高亮。筛选只淡化节点并保持坐标，时间线与检查器共享选择。
- **共享检查器**：桌面宽 320 px，顺序为当前结论、事实属性、证据、指标；原始输出按需展开。点击任何引用只更新同一选择与游标。
- **视觉**：白色工作面、`#efefef` 分隔、`#f2f3f5` 选中、Aster 语义状态色、30／32 px 控件；不复制上游执行回放或状态理解工具的独立主题。
- **Arc 微动效**：按 [analysis-motion](contracts/analysis-motion.md) 使用 `agent-timeline` 的共享时钟、`tool-call` 的原位展开、`line-replay` 的无 overshoot 重定向、`context-window` 的真实压缩解释和动作组件的稳定反馈；所有循环在 hidden／offscreen／paused／settled 停止，reduce 立即显示等价终态。
- **窄屏**：1024／768 px 收缩地图与检查器比例，320 px 改为原地展开底部详情；所有图形保留可访问列表，键盘与 reduced-motion 不依赖 Canvas 动画。

## Delivery Phases

估算以一名熟悉 Studio React／Rust 的工程师、每人日约 6 小时有效开发为基准；它是排期假设，不是已消耗工期或发布承诺。每个阶段先写能暴露目标回归的最小测试。

### M0：证据覆盖与黄金夹具（1–2 人日）

1. 建立一个小型完整夹具：两个用户回合、主／子 Agent、Read／Edit、失败 Verify、确定性重试、模型变化、压缩、产物和 deferred 输出。
2. 建立缺失夹具：旧消息无 runtimeTiming、无 Token、无快照、永久 deferred miss、历史分页失败。
3. 核对 `SpanEntity` 的模型请求／工具 span 字段与 Trace cursor 完成语义；不能证明 per-request 的字段保持消息级粒度。
4. 把 8 个秘密标记加入隐私夹具，覆盖 prompt、response、reasoning、tool input、tool output、file content、command args、subagent prompt。

**退出条件**：夹具通过原消息类型构造；所有已知缺口都能用“未知／局部”表达；没有为了填满模型而修改原始类型或制造字段。

### M1：统一事实、证据与脱敏导出（3–4 人日）

1. 引入 `AnalysisRun`、稳定事件身份、覆盖和 sourceRevision；保留当前 `buildWorkspaceAnalysis` 行为直到页面迁移完成，再删除无消费者的兼容入口。
2. 合并消息统计与 Trace spans；未知测量保持 `null`，同一事实不重复累计。
3. 提取 deferred 查找与 Trace 读取复用点，完成会话切换和 stale-response 隔离。
4. 实现本地证据解析与 AgentPlay 脱敏 adapter；复用通用文件保存。
5. 用 Ajv 对官方 `agentplay/v1` Schema 做测试校验，生产入口继续由 Zod 封闭校验。

**退出条件**：现有页面行为不回归；隐私夹具零泄漏；局部分页不宣称完整；M1 单元／合同测试通过。此阶段不先上线空的新地图或 Story 按钮。

### M2：AgentPlay 式执行回放与放射资源图（5–6 人日）

1. 将页面拆成摘要、镜头／筛选、播放、时间线、放射资源图和检查器；所有组件接收同一个 run 与游标。
2. 实现 Tasks／Events／Tokens／Context 轨道、步骤与时间轴、1×／4×／16×、错误／重试／延迟／产物跳转。
3. 实现任务、模型和子 Agent 镜头；全会话与当前镜头指标明确区分。
4. 用纯函数生成中心—分类内圈—资源外圈图：文件、命令、MCP、模型、产物、子 Agent 和错误均来自真实事件并去重；无记录时不创建占位事实节点。
5. 使用 xyflow 实现径向布局、拖动、缩放、当前路径高亮和共享事件选择；筛选只淡化不匹配节点，节点集合与坐标保持不变。
6. 完成 deferred 原文检查器、键盘、可访问资源／事件列表、窄屏和中英文。

**退出条件**：User Story 1／2／4 的交互验收通过；七类资源与至少 30 节点夹具满足 SC-005；20,000 事件夹具播放 100 步无历史／模型请求，无常驻全量 DOM 行。

### M3：状态理解式状态图与 Story（4–5 人日）

1. 从真实 Diff／哈希／产物构建 states、phases、actions、transitions 和 links；缺证据时保留动作视图。
2. 实现 System states → Work phases → Actions → Raw records 四级下钻、版本树／执行顺序和“只看决策”。
3. Story、Map、时间线和检查器共享选中项；回滚生成新时间状态，不形成时间环。
4. 增加完整轨迹导出；原文范围确认、Schema／引用校验失败均不写出文件。
5. 保持观察事实与注释分离；无注释时也能浏览确定性 Story。

**退出条件**：User Story 3 通过；合法迁移、回滚、缺失快照、未知引用和非 DAG 均有正反例；不把测试成功单独视为状态变化。

### M4：可选单次语义注释（2–3 人日）

1. 创建独立内部分析会话，复用精确 sender 绑定；所有普通任务列表排除该封闭目的类型。分析会话共享原任务 workspace 时，过滤只能依据 session `description` 的内部目的，不得按共享 workspace path 隐藏原项目。
2. 通过 `sourceReview` 一次性提供有界节点和原文；输入超预算时拒绝，不引入证据选择对话或 Smart Context。
3. 模型只返回 `AnalysisAnnotation` 候选；宿主校验 target、证据 ID、sourceRevision 和逐字 quote 后发布，不生成聊天或追问。
4. 将模型注释标为 `model_inferred`，源变化即失效；引用点击同步 Story／Map／时间线／放射资源图。
5. 关闭、取消或切换会话时丢弃迟到结果；无模型连接不影响 M1–M3，页面不提供聊天输入。

**退出条件**：语义注释合同通过；未知 ID、越界引用、非逐字 quote、超预算和陈旧修订均不发布；原会话消息数保持不变，分析助手不出现在普通任务列表。

### Arc 微动效与组件整合（2–3 人日）

1. 按 `agent-timeline`／`line-replay` 行为实现共享 playhead clock：无每帧 React render、无 overshoot 重定向，hidden／offscreen／paused／settled 停钟。
2. 用 house SegmentedControl、Button、CopyButton、EmptyState、rowMotion 落实选中、工具行、检查器和导出反馈；实际事件与保存 Promise 决定状态。
3. 为放射相机和 Context 加入连续但可打断的过渡；筛选不重排，真实压缩只有一次有界 sweep，不使用 Sunburst。
4. 完成 `analysis-motion` 9 类验收矩阵、连续 5 次切换和挂载后 reduced-motion；不新增 motion token、Arc 依赖或装饰性 autoplay。

**退出条件**：FR-016 与 SC-006 通过；所有动效具有真实触发器、owner、打断、停止和 reduced-motion 结果。

### M5：端到端资格与证据（2–3 人日）

1. 在隔离 Aster 实例中运行失败测试→放射图错误节点→重试→证据定位、状态→决策→原文两条完整旅程。
2. 验证项目任务选择、长会话、deferred miss、取消、重启后的无持久化行为、20,000 事件和中英文。
3. 在 1440／1024／768／320 CSS 宽度检查布局、键盘、焦点、播放／相机动效和 reduced motion；截图只证明布局，行为另由断言证明。
4. 记录实际命令、通过计数、模型与脚本模型差异、二进制／源码身份、所有未验平台与限制。

**退出条件**：规格 SC-001–SC-006 均有对应证据；未执行项明确写 `Explicitly not claimed`，不以组件测试代替原生资格。

**合计估算**：19–26 人日。M1 后可根据真实 Trace／deferred 覆盖缺口重估 M2；M3 前不得通过扩大语义推断来缩短工期。

## Verification Plan

### Hermetic suites

在 `apps/studio` 运行精确文件测试，不使用 `pnpm test <path>`：

- `pnpm test:renderer src/renderer/pages/knowledge/__tests__/workspaceAnalysisProjection.test.ts`
- `pnpm test:renderer src/renderer/pages/knowledge/__tests__/workspaceAnalysisAgentPlay.test.ts`
- `pnpm test:renderer src/renderer/pages/knowledge/__tests__/workspaceAnalysisResourceGraph.test.ts`
- `pnpm test:renderer src/renderer/pages/knowledge/__tests__/WorkspaceAnalysisMap.test.tsx`
- `pnpm test:renderer src/renderer/pages/knowledge/__tests__/useWorkspaceAnalysisClock.test.ts`
- `pnpm test:renderer src/renderer/pages/knowledge/__tests__/workspaceAnalysisStateGraph.test.ts`
- `pnpm test:renderer src/renderer/pages/knowledge/__tests__/workspaceAnalysisAnnotation.test.ts`
- `pnpm test:renderer src/renderer/pages/knowledge/__tests__/WorkspaceAnalysisPage.test.tsx`
- `pnpm test:renderer src/renderer/components/chat/trace/__tests__/useTraceData.test.ts`
- `pnpm test:renderer src/renderer/hooks/__tests__/useAgentSessionParts.test.ts`
- `pnpm test:shared`（仅在修改 shared Schema／目的标识时运行聚焦文件；必要时使用 `pnpm exec vitest run --project shared <files>`）

最终受影响文件清单以实际改动为准，不要求为了匹配计划增加空测试文件。

### 必测回归

- 分页 prepend 后事件 ID、选择与原顺序稳定。
- deferred 永久 miss 不重试、不显示为空输出，不跨会话读取。
- 历史失败只显示局部覆盖；未知 Token／成本不是 0。
- 同一事件被多个动作或资源节点引用不重复计数。
- 放射图七类资源正确去重；筛选前后节点数量与径向坐标不变，选择外圈节点定位同一事件。
- 无快照不生成状态；测试成功不生成代码版本；回滚不产生时间环。
- AgentPlay 文档 8 个秘密标记零出现，官方 Schema 通过。
- 完整原文导出未确认不保存；保存取消不报告成功。
- 播放 100 步无网络；任务切换使旧消息、Trace、证据和模型响应失效。
- 未知注释引用、非逐字 quote、超预算和陈旧修订均不发布；分析助手不写原会话且列表不可见，页面无聊天入口。
- Arc 9 类动效场景均有真实触发器与 owner；连续 5 次切换不双底板／重播，播放／相机打断不跳回起点，hidden／settled 后循环为 0，挂载后 reduced-motion 立即等价，无每帧 React render。

### 仓库与 UI gates

- 在 Studio 运行 `pnpm lint`（覆盖 typecheck、i18n 和 format）；只对本功能新增 key 使用 `pnpm i18n:sync` 并翻译全部占位。
- 运行受影响的 AgentRightPane、TracePage、planExport、内部会话过滤及 house SegmentedControl／Button／CopyButton／EmptyState 既有测试，证明复用没有回归。
- `pnpm deps:check` 与 renderer build 验证 test-only Ajv 不影响 bundle；不修改 patched 依赖家族，不增加 Arc 或第二动画库依赖。
- 根仓库保持 `cargo fmt --all -- --check`；若最终只改 TypeScript／docs，不虚构 Rust 行为测试。
- 不运行 live PostgreSQL、不运行无关 cloud 套件、不调用真实模型作为 hermetic 通过条件。

### Evidence

在 `docs/evidence/desktop/<date>-agent-execution-understanding/OUTCOME.md` 记录：

- 实际运行的命令、每项通过计数与失败重跑。
- 夹具来源、事件／秘密标记／资源分类与节点数量、导出校验结果、布局坐标稳定性和网络请求计数。
- Arc 动效的真实触发器、状态 owner、打断、停止、reduced-motion 和高频渲染检查；未使用组件或模拟数据单列。
- 原生旅程、宽度、语言、键盘和截图；未执行的平台、性能和真实模型质量单独列出。
- `Explicitly not claimed`：交互式 AI 问答、逐 Token 回放、所有历史完整版本树、外部 Agent 全解析、跨重启注释、语义正确性保证、Arc 批量组件或 Pro 源码落地。

## Requirement Coverage

| Requirement | Delivery |
| --- | --- |
| FR-001–FR-004 | M0、M1 |
| FR-005–FR-006 | M2 |
| FR-007–FR-008 | M3、M4 |
| FR-009 | M1 |
| FR-010 | M3 |
| FR-011 | M2、Arc 微动效整合 |
| FR-012 | M4 |
| FR-013 | M1、M3、M4 各入口 |
| FR-014 | M2、M3、Arc 微动效整合、M5 |
| FR-015 | 全阶段 Constitution Check 与 deps／build gates |
| FR-016 | Arc 微动效整合、M5 |

## Deviations and Deferred Scope

无宪法偏差。

交互式 AI 问答、Smart Context、外部 Agent 日志发现／解析、跨重启注释持久化、团队共享、逐 Token 录像、全历史完整版本树和真实模型语义质量承诺均不进入本期。放射资源图和 Arc 微动效不在 deferred scope；不批量安装 Arc、不采用 Sunburst 资源占比、自动旋转或装饰性 autoplay。若实现证据表明这些 defer 会阻碍 P1 回放，先回到 spec 修订范围，不得静默新增数据库、权限或运行时。
