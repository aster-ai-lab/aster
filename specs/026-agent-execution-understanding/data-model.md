# Data Model: Agent 执行理解与证据回放

## 1. 所有权与分层

`AnalysisRun` 是 Studio renderer 的只读派生模型。原始消息、parts、deferred 工具结果和 Trace spans 仍由原 owner 拥有；本功能不复制第二份事实库，也不把语义注释写回被分析会话。

```text
原始持久化／运行时数据
  messages + stats + parts + deferred refs + trace spans
                    ↓
             AnalysisRun（事实层）
                    ↓
       states / phases / actions / links（结构层）
                    ↓
          annotations（可失效的解释层）
```

AgentPlay 与完整轨迹文档均为边界导出。原始证据默认只在本地按需解析；脱敏导出不得反向成为原文来源。

## 2. 封闭集合

TypeScript 使用 union + Zod 边界 Schema；外部输入遇到未知值直接拒绝。若后续进入 Rust，对应枚举必须提供 `parse()` / `as_str()`，不得使用字符串自由转换。

```ts
type AnalysisEventKind =
  | 'prompt' | 'response' | 'read' | 'search' | 'edit' | 'exec' | 'verify'
  | 'tool' | 'mcp' | 'skill' | 'subagent' | 'task' | 'llm'
  | 'error' | 'delay' | 'compaction' | 'artifact' | 'model_switch'

type AnalysisEventStatus = 'running' | 'success' | 'error' | 'unknown'
type AnalysisHistoryStatus = 'loading' | 'partial' | 'complete' | 'failed'
type AnalysisBehavior = 'explore' | 'plan' | 'edit' | 'execute' | 'verify' | 'diagnose'
type AnalysisBranchStatus = 'attempted' | 'retained' | 'proposed' | 'rejected'
type AnalysisLinkKind = 'sequence' | 'informs' | 'review_request' | 'review_return'
type AnalysisMove = 'insight' | 'ruleout' | 'decision' | 'pivot' | 'open'
type AnalysisProvenance = 'observed' | 'model_inferred' | 'user_authored'
type AnalysisPathMode = 'relative' | 'dirs_only' | 'hashed'
```

页面模式、镜头和导出类型同样使用封闭集合。显示文案不改变底层值。

## 3. 事实层

### 3.1 EvidenceRef

`EvidenceRef` 是可解析的本地指针，不是原文容器。使用 discriminated union，在入口校验一次后向内传递 typed 值。

```ts
type EvidenceRef =
  | {
      kind: 'message_part'
      sessionId: string
      messageId: string
      partIndex: number
    }
  | {
      kind: 'tool_result'
      sessionId: string
      topicId: string
      messageId: string
      toolCallId: string
    }
  | {
      kind: 'trace_span'
      sessionId: string
      traceId: string
      spanId: string
    }
```

约束：

- `tool_result` 复用 `DeferredToolResultRef` 的 `topicId/messageId/toolCallId`；解析前必须确认它属于当前 `sessionId`。
- `message_part` 使用原消息 ID 和原 part 索引；不得因分页、筛选或语义分组重新编号。
- `trace_span` 使用原 `traceId/spanId`；不得把 UI tree node ID 当作新事实 ID。
- 引用不含工具输出、文件内容、提示词或模型密钥。

### 3.2 AnalysisEvent

```ts
interface AnalysisEvent {
  id: string                 // 来自源身份；不是当前数组下标
  seq: number                // 每次完整投影后的连续顺序，仅用于回放
  kind: AnalysisEventKind
  status: AnalysisEventStatus
  evidence: EvidenceRef[]

  timestamp?: number
  durationMs?: number
  agentId: string
  taskId?: string
  parentToolCallId?: string
  model?: string
  category?: string

  paths: string[]
  errorClass?: string
  retryOf?: string           // 另一个稳定 event.id；仅来自确定性的相同调用证据
  signals?: Array<
    | { kind: 'delay'; source: 'duration' | 'inactivity'; durationMs: number }
    | { kind: 'model_switch'; from: string; to: string }
  >
  tokenUsage?: {
    input?: number
    cacheWrite?: number
    cacheRead?: number
    output?: number
    context?: number
  }
}
```

`input/output/detail` 不进入可分享事件。检查器通过 `evidence` 按需取得原文；页面可使用已有、明确有界的标题或路径元数据。

稳定 ID 规则：

- 消息／part 事件：`message:<messageId>:part:<partIndex>`。
- 消息级模型统计：`message:<messageId>:stats`。
- Trace 事件：`span:<traceId>:<spanId>`。
- 同一源事实被多个投影使用时仍只有一个 `AnalysisEvent.id`。

`seq` 在合并后按原会话顺序生成；分页追加不得改变事件 ID。时间相同的事件保持原 transcript 顺序，再按 Trace 原顺序合并，不用名称猜测先后。

`signals` 是从真实测量派生的标记，不新增重复执行事件。实测耗时达到 10 秒生成 `delay/duration`；只有同一消息内相邻工具、起止时间完整且时钟可比较时才生成 `delay/inactivity`。模型切换只比较同一 Agent 的 response／llm 模型序列，保留精确的 `from`／`to`，不从工具名称或文本猜测。

### 3.3 AnalysisCoverage

```ts
interface AnalysisCoverage {
  history: AnalysisHistoryStatus
  hasOlder: boolean
  loadedMessageCount: number
  deferredEvidenceCount: number
  unavailableEvidenceCount: number
  truncated: boolean
  reasons: string[]         // 封闭错误码，不是任意用户提示
}
```

只有 `history === 'complete' && hasOlder === false && truncated === false` 才能声明“完整执行”。总消息数没有权威来源时保持未知，不通过当前页数量推算。

### 3.4 AnalysisRun

```ts
interface AnalysisRun {
  sessionId: string
  sourceRevision: string
  events: AnalysisEvent[]
  coverage: AnalysisCoverage
  agents: Array<{
    id: string
    parentId?: string
    kind: 'main' | 'subagent' | 'helper' | 'unknown'
    model?: string
  }>
  tasks: Array<{
    id: string
    title?: string
    startEventId: string
    endEventId: string
  }>
  metrics: {
    turns: number
    toolCalls: number
    errors: number
    files: number
    totalTokens: number | null
    durationMs: number | null
    costUsd: number | null
  }
  resources: AnalysisResourceGraph
  graph?: AnalysisGraph
}
```

`sourceRevision` 绑定会话 ID、消息 IDs／updatedAt、Trace 读取代次和覆盖状态。任何源变化或会话切换都使旧注释结果失效；不要求把原文哈希暴露到导出中。

### 3.5 AnalysisResourceGraph（必做）

```ts
type AnalysisResourceCategory =
  | 'files' | 'shell' | 'mcp' | 'models' | 'outputs' | 'subagents' | 'errors'

interface AnalysisResourceNode {
  id: string
  category: AnalysisResourceCategory
  parentId?: string
  label: string
  eventIds: string[]
  status: 'success' | 'error' | 'unknown'
}

interface AnalysisResourceLink {
  sourceNodeId: string
  targetNodeId: string
  eventIds: string[]
}

interface AnalysisResourceGraph {
  centerNodeId: string
  nodes: AnalysisResourceNode[]
  links: AnalysisResourceLink[]
  layoutRevision: string
}
```

- 中心节点是当前任务；内圈仅包含有真实资源的分类，外圈是文件、机械解析的命令 verb、MCP、模型、产物、子 Agent 和错误节点。
- 节点 label 来自精确路径、工具／模型／任务记录或封闭错误类型，不用名称推测实体。
- 一个资源被多个事件使用时只保留一个资源节点，`eventIds` 保留全部真实使用关系，不重复累计执行。
- `layoutRevision` 由 run 与视口尺寸决定；筛选和镜头只改变淡化状态，不改变节点集合或径向坐标。
- 当前节点路径由 `links` 与中心、分类和相关资源计算；没有真实 link 时不补造关系。

## 4. 结构与解释层

### 4.1 AnalysisGraph

```ts
interface AnalysisGraph {
  scope: string
  states: AnalysisState[]
  phases: AnalysisPhase[]
  actions: AnalysisAction[]
  transitions: AnalysisTransition[]
  links: AnalysisLink[]
  annotations: AnalysisAnnotation[]
}
```

- `AnalysisState`：`id`、`title`、`eventIds`、`snapshotEvidence[]`、可选 `lineage` 和 `assessment`。
- `AnalysisPhase`：`id`、`stateId`、`title`、`eventIds`、`status: AnalysisBranchStatus`。
- `AnalysisAction`：`id`、`stateId`、`phaseId`、`behavior`、`actorId`、`outcome`、`eventIds`。
- `AnalysisTransition`：`sourceStateId`、`targetStateId`、`actionId`、`changes[]`；每个 change 均有 before／after 证据。
- `AnalysisLink`：`sourceActionId`、`targetActionId`、`kind`、`eventIds` 和解释。

### 4.2 状态证据

`snapshotEvidence` 只记录可验证的组件、内容哈希、Diff、产物或测量引用。未知值为 `null`，不能把“没有读取到”当成“内容为空”。

状态迁移必须同时满足：

1. 对应动作是已尝试的 `edit` 或 `execute`。
2. 动作实际成功；退出码 0 本身不足以证明目标范围发生变化。
3. 具有目标 scope 内的 Diff、哈希、产物或其他观察效果。
4. source／target 快照差异完整且证据 ID 存在。
5. 时间迁移形成 DAG；恢复历史内容生成新状态。

读取、搜索、失败写入、无变化写入、测试、审查和未执行建议均不单独产生状态。

### 4.3 AnalysisAnnotation

```ts
interface AnalysisAnnotation {
  id: string
  targetType: 'run' | 'state' | 'phase' | 'action'
  targetId: string
  provenance: AnalysisProvenance
  sourceRevision: string
  move?: AnalysisMove
  topic?: string
  takeaway?: string
  reason?: string
  evidenceEventIds: string[]
  quotes: Array<{
    eventId: string
    text: string             // 必须从该事件原文逐字复制
  }>
}
```

注释与事件分存。`model_inferred` 不得升级为 `observed`；源修订不匹配、证据越界、引用不存在或 quote 非逐字时，整份注释不发布。

## 5. 导出模型

### 5.1 AgentPlay 脱敏文档

从事实层生成 `agentplay/v1`：保留会话、指标、repo／文件元数据、Agent、Task 和事件；剥离所有自由原文。无法机械确认的命令 verb 省略，不猜测。路径模式使用 `AnalysisPathMode`，`hashed` 缺少用户提供的密钥时按上游合同降级为 `dirs_only` 并明确显示。

### 5.2 完整轨迹

从事实层、结构层和已确认原文生成 `coding-agent-state-trajectory` version 1。只有显式确认后才把原始消息、工具输入输出和附件写入 `timeline`／`sources`。未确认时只能导出不含原文的本地草稿，且不得宣称包含完整原文证据。

## 6. 内存与性能边界

- `AnalysisRun` 保存规范化元数据和引用，不复制完整工具输出。
- 原始输出通过现有 SWR／IPC 缓存按需解析；缓存 miss 是明确的可用性状态。
- 时间线视口使用虚拟列表；Tokens／Context 使用 Canvas；播放只修改游标。
- 20,000 事件不意味着 20,000 个常驻 React 行或每帧全图重排。
- 本期注释结果按 `sourceRevision` 保存在当前页面内存；跨重启持久化明确延期。

## 7. 视图动效状态不进入事实模型

Arc 微动效只操作视图状态，不向 `AnalysisRun` 写入“正在播放”“已完成动画”或装饰性进度：

- 逻辑游标、播放速度和镜头属于页面状态机；playhead 与相机插值属于本地视图。
- 原始事件状态是唯一业务状态；动画结束不得把事件改为完成，也不得提前确认导出。
- `prefers-reduced-motion` 只改变呈现方式，不改变事件、选择、引用、导出权限或可访问内容。
- 播放时钟、rAF 和 motion value 不进入 `sourceRevision`，不触发历史读取或模型请求。
- 详细触发器、打断和停止条件见 [contracts/analysis-motion.md](contracts/analysis-motion.md)。
