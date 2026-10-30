# Contract: AnalysisRun、证据与互操作

本合同冻结 `specs/026-agent-execution-understanding` 的边界。实现可以拆文件，不得改变事实所有权、拒绝条件或隐私语义。

## 1. 输入与投影

### 输入

- 当前 Aster `sessionId` 及其精确 workspace 绑定。
- `useAgentSessionParts` 返回的分页消息、加载状态和原消息 IDs。
- 当前会话的可选 `traceId` 与现有 Trace IPC 读取结果。
- 原 parts 中的 `DeferredToolResultRef`，由 `ai.tool.get_result` 按需解析。

### 投影规则

1. 在外部边界校验一次；内部只传 typed `AnalysisRun`。
2. 保留原 transcript 顺序和源身份；筛选、镜头、Story 和地图不修改 `events`。
3. 合并 Trace 时使用其实测时间与原顺序；缺失时间不补造。
4. 一个源事实只累计一次。动作、阶段和 Story 引用不增加执行计数。
5. `totalTokens`、`durationMs`、成本等指标没有测量时为 `null`，不是 `0`。
6. 历史未完成时允许局部回放，但所有汇总和标题必须带覆盖状态，不得写“完整”。
7. 会话切换以 `sessionId + sourceRevision` 使旧请求失效；旧响应不得写入新 run。

### 拒绝

- `analysis.invalid_session_scope`：会话、workspace 或证据不属于当前范围。
- `analysis.duplicate_event_id`：不同源事实产生相同稳定 ID。
- `analysis.invalid_event_order`：输入无法形成确定性的原顺序。
- `analysis.history_incomplete`：调用方要求完整投影但覆盖不完整。

## 2. 证据解析

### 请求

`EvidenceRef` 只能由当前 run 提供。UI 不手工拼接一个跨会话 `tool_result`。

- `message_part`：从当前已加载消息读取；必要时通过现有单消息 GET 获取完整 part。
- `tool_result`：复用 `isDeferredToolOutput`、`useToolResult` 和 `ai.tool.get_result`；`topicId` 必须对应当前会话。
- `trace_span`：从当前 `traceId` 的原始 span 集合读取；UI tree node 仅作展示身份。

### 结果

```ts
type EvidenceResult =
  | { status: 'available'; ref: EvidenceRef; value: unknown }
  | { status: 'deferred'; ref: EvidenceRef }
  | { status: 'unavailable'; ref: EvidenceRef; reason: 'missing' | 'out_of_scope' | 'stale' }
```

`unavailable` 不得被序列化成空输出。用户可重试仅限暂时性读取错误；现有 `useToolResult` 的永久 miss 不重试语义保持不变。

## 3. 播放、选择与镜头

页面持有唯一：

```ts
{
  sessionId
  selectedEventId
  selectedNodeId?: string
  cursorSeq: number
  filter: AnalysisFilter
  lens: AnalysisLens
  playback: 'stopped' | 'playing' | 'following'
  speed: 1 | 4 | 16
}
```

- 节点与事件有明确映射；选中节点会解析到其代表事件，不另建第二游标。
- 过滤和镜头改变可播放集合，不改变原始指标；指标另提供“全会话／当前镜头”两个明确范围。
- 播放只递增本地 `cursorSeq`，不得请求消息、工具输出或模型。
- 打开原始证据可以触发一次 deferred 读取；这不属于播放自动行为。
- 用户手动拖动后退出实时跟随；点击“回到实时”才恢复。
- 切换会话时清空选择、游标、镜头和注释，并停止播放。

键盘：Space 播放／暂停，左右键一步，Shift+左右键十步；输入框、编辑器和屏幕阅读器交互区域不得被全局拦截。

### 放射资源图

- 中心为当前任务，内圈为真实资源分类，外圈为真实文件、命令、MCP、模型、产物、子 Agent 和错误节点。
- 节点与事件通过 `eventIds` 关联；选择资源节点时解析到当前或代表事件，地图、时间线和检查器共享选择。
- 初次布局按中心、分类、外圈计算径向坐标；同一 `layoutRevision` 下筛选和镜头不得重新排图。
- 不匹配节点只淡化，当前路径保持清晰；不得通过删除节点制造“筛选完成”的假象。
- 支持拖动、缩放和点击空白取消选择；这些操作不修改事实事件。
- 缺失分类不创建占位事实节点；可访问资源列表提供相同节点与选择能力。

## 4. 状态图与注释

### 状态编译

- 只有实际效果的 Edit／Execute 可以产生 transition。
- 连续编辑在第一次验证、测量、审查或明确选择之前可合并为一个状态。
- Verify／Diagnose／Explore 的结果附着于当前状态，不冒充代码变化。
- proposed／rejected 分支只能包含未执行建议；已尝试后放弃使用 attempted + `ruleout`。
- 回滚是新的时间状态，lineage 可指向历史来源，但 transitions 不成环。
- 证据不足时省略 state／transition，保留 events 和 actions。

### 注释发布

注释只有在以下检查全部通过后才进入 `AnalysisGraph.annotations`：

- `sourceRevision` 等于当前 run。
- target 存在且类型正确。
- 所有 evidence ID 存在并处于 target 的合法范围。
- `informs` 不逆向时间；sequence／review 链接端点类型合法。
- quote 逐字存在于所引用事件的本地原文。
- `model_inferred` 保持可见标记，不能在 UI 中改写为观察事实。

失败时保留事实视图并返回 `analysis.invalid_annotation`，不部分发布一份看似完整的 Story。

## 5. AgentPlay 导出

输出格式：`agentplay/v1`，按其公开 JSON Schema 校验。

| Aster | AgentPlay |
| --- | --- |
| `AnalysisRun.sessionId` | `session.id`，附本地 producer 前缀 |
| `AnalysisRun.metrics` | `totals`；未知字段省略，不补零 |
| `AnalysisRun.agents/tasks` | `agents/tasks` |
| `AnalysisEvent.seq` | `events[].seq` |
| 相对首事件时间 | `events[].t` |
| 封闭事件类型 | 对应 `kind`；无等价类型使用 `tool`，不发明上游 kind |
| 文件元数据 | `repos/files/targets`；遵循 path mode |
| 实测模型统计 | `llm`／`model_switch`／tokens；没有 per-request 数据时保持消息级粒度 |
| `AnalysisEvent.signals` | 分离的 `delay`／`model_switch` 信号事件；保留 `from`／`to`／`dur`，并累计 `delays`／`delayMs` |

禁止写入：prompt／response／reasoning 原文、工具输入输出、文件内容、命令参数、子 Agent prompt、用户图片。命令 verb 仅由机械命令解析生成；不能确认时省略。

导出失败返回 `analysis.export_redaction_failed` 或 `analysis.export_schema_invalid`，不留下声称成功的文件。

## 6. 完整轨迹导出

输出格式：`coding-agent-state-trajectory`，`version: 1`。

- `states/phases` 映射到 `states/branches`；`actions/transitions/links/timeline` 保持原 ID 和引用。
- 原始记录只在用户明确确认原文范围后写入。
- 原始记录保持原语言，不随界面或注释翻译。
- 无证据状态、不完整 transition、未知引用、非 DAG 时间图均拒绝导出。
- 只含脱敏事件的文档必须标记证据缺口，不得声称包含完整原文检查或完整语义注释所需的证据。

## 7. 单次语义注释（非问答）

复用 ACP `sourceReview`：固定宿主输入、无工具、隔离上下文。不得把原会话权限、文件工具或模型密钥交给分析助手。本功能只生成一次 Story／决策注释，不提供聊天输入、追问或 Smart Context。

### 宿主输入

```json
{
  "sessionId": "...",
  "sourceRevision": "...",
  "scope": { "kind": "node|run", "id": "..." },
  "nodes": [],
  "evidence": [
    { "eventId": "...", "text": "..." }
  ],
  "inputBudget": 0
}
```

宿主只提供当前注释所需的已知节点和原文；输入超过预算时以 `analysis.context_budget_exceeded` 拒绝，不得静默截断或擅自改成全量轨迹。

### 模型输出与发布

模型只能返回符合 `AnalysisAnnotation` 的候选，不得返回自由聊天或后续问题。宿主校验 target、证据 ID、合法范围、sourceRevision 和逐字 quote 后，才可发布为 `model_inferred` 注释。

- 注释中的证据引用可定位 Story、Map、时间线和放射资源图，但不生成回答式结论卡片。
- 取消或切换会话后，迟到结果被丢弃。
- 无模型连接时，事实回放、放射资源图、状态结构和脱敏导出继续可用。
- 分析助手使用内部目的标记，不写回原会话，不出现在普通任务列表。

## 8. UI 与验收合同

- 同一时间只有一个分析页面事实源；不嵌入两个 Viewer。
- Aster 原生控件、字体、颜色和 30／32 px 控件密度；不使用 AgentPlay 黄绿色主题。
- 决策视图与执行视图共享检查器；窄屏改为原地展开详情，不做桌面边缘滑入 dialog。
- 放射资源图是执行视图的必做主画布；七类资源夹具中，外圈节点选择、当前路径和筛选淡化后的稳定坐标必须通过组件断言。
- 事件列表与资源列表始终提供可访问的语义表示；Canvas 不是唯一信息通道。
- 20,000 事件夹具中播放 100 步的网络／模型请求均为 0。
- 每个失败场景保留可观察错误码和局部可用视图；不得用空白页掩盖合同错误。

## 9. 命名错误码

封闭集合：

- `analysis.invalid_session_scope`
- `analysis.invalid_input`
- `analysis.duplicate_event_id`
- `analysis.invalid_event_order`
- `analysis.history_incomplete`
- `analysis.evidence_unavailable`
- `analysis.stale_source_revision`
- `analysis.invalid_state_transition`
- `analysis.invalid_annotation`
- `analysis.context_budget_exceeded`
- `analysis.export_confirmation_required`
- `analysis.export_redaction_failed`
- `analysis.export_schema_invalid`

UI 文案可翻译，错误码不得随语言变化。
