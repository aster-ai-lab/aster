# Feature Specification: Agent 执行理解与证据回放

**Feature Branch**: 当前工作区；不创建分支

**Created**: 2026-10-29

**Status**: Draft

**Scope correction (2026-10-30)**: 根据最新审查，Story／状态树、镜头切换、语义注释与内部分析会话、完整轨迹及回放导出、资源列表、相机重置、底部事件详情、回到实时、全部信号快捷跳转、右侧事件计数、时间线标题和步骤／时间轴切换均移出当前范围。当前仅保留执行放射图、事件检查器、播放／步进／速度、筛选、固定步骤时间线、真实证据以及 Delay／Model switch 信号；项目任务选择入口保留。上述扩展的专属实现、测试和原生脚本已删除，后续验收以此修正为准。

**Input**: User description: "为 Aster 分析页规划 AgentPlay 执行事实层与完整轨迹语义理解层的原生融合，包含统一事件与证据模型、隐私边界、UI/UX、分阶段文件改造和验收测试。放射资源图必做，交互式 AI 问答不做；保留单次 AI 语义注释。充分利用 Arc UI 的微动效设计哲学和组件行为，复用 Aster house 组件而非引入第二设计系统。"

**Gate context**: Product regime， owning slice 为 `apps/studio` 知识工作区的任务／项目分析页。该功能没有独立 roadmap 行；它直接扩展现有 `WorkspaceAnalysisPage`，解决当前只有事件列表、缺少执行诊断、放射资源关系、版本理解和证据注释的问题。

## User Scenarios & Testing *(mandatory)*

### User Story 1 - 回放并诊断一次真实执行 (Priority: P1)

工程师打开任务或项目中的某个执行记录，查看完整的工具、编辑、模型、错误、压缩和产物事件，按任务、模型或子 Agent 缩小范围，并播放或拖动时间线定位失败与重试。

**Why this priority**: 这是 AgentPlay 能力的主要价值，也是后续状态理解的事实基础；没有可靠事件流，语义总结不可信。

**Independent Test**: Studio renderer 的投影与组件测试使用同一组真实结构夹具，验证事件顺序、指标、过滤、镜头、播放、选择同步和会话切换；播放期间不发出历史或模型请求。

**Acceptance Scenarios**:

1. **Given** 一个包含主 Agent、子 Agent、编辑、失败测试、重试和压缩的会话， **When** 用户打开分析页并选择失败事件， **Then** 时间线、资源视图和检查器选中同一事件，并显示实测耗时、错误和父子关系。
2. **Given** 历史尚未全部加载， **When** 用户查看覆盖状态或开始播放， **Then** 页面明确显示当前覆盖范围，不把已加载片段称为完整执行。
3. **Given** 用户切换到另一个任务， **When** 旧任务的延迟响应随后到达， **Then** 新任务的游标、筛选和镜头不被旧响应修改。

### User Story 2 - 从摘要下钻到原始证据 (Priority: P1)

工程师从运行摘要或事件检查器查看原始消息、工具输入输出和文件证据；对于延迟输出，只在使用时加载。工程师可以导出脱敏执行地图，而不暴露提示词、回复、命令参数或文件内容。

**Why this priority**: 可核验证据和隐私是执行审计可用的前提，不能让摘要替代事实，也不能为导出牺牲本地数据边界。

**Independent Test**: Studio renderer／shared 合同测试覆盖 deferred 输出命中、永久缺失、会话隔离，以及包含秘密字符串和敏感路径的脱敏导出；完整原文导出需要显式确认。

**Acceptance Scenarios**:

1. **Given** 一个工具输出只有 deferred 引用， **When** 用户打开原始证据， **Then** 系统通过现有结果通道按需读取；若引用已失效，显示“原始输出不可用”，不显示为空输出。
2. **Given** 用户选择 AgentPlay 脱敏导出， **When** 生成文档， **Then** Schema 校验通过，秘密原文和命令参数零出现，路径遵循所选 `relative`、`dirs_only` 或 `hashed` 模式。
3. **Given** 用户选择包含原始记录的完整轨迹导出， **When** 尚未确认原文范围， **Then** 系统拒绝导出并说明需要确认的隐私范围。

### User Story 3 - 理解版本、尝试与决策 (Priority: P2)

技术负责人从 Story 概览开始，查看代码或产物状态、工作阶段、动作和原始记录四个层级；在版本树与执行顺序之间切换，并只保留关键决策、排除、转向和未决问题。

**Why this priority**:完整轨迹的核心增量是解释“为什么这样走”，但它必须建立在 P1 的事件与证据合同之上。

**Independent Test**: 纯状态图测试验证合法迁移、回滚新状态、缺失快照降级、引用完整性和 DAG 拒绝规则；组件测试验证四级下钻、Story／Map／时间线联动及“只看决策”。

**Acceptance Scenarios**:

1. **Given** 一组具有真实 Diff 或哈希证据的编辑与验证事件， **When** 用户打开版本视图， **Then** 只有具有观察效果的 Edit／Execute 形成状态迁移，测试和读取不会单独形成新版本。
2. **Given** 历史会话缺少状态快照， **When** 无法证明版本关系， **Then** 页面降级为动作与 Story 视图，不伪造版本树、回滚或指标对比。
3. **Given** 一个语义注释引用未知或越界事件， **When** 导入或生成该注释， **Then** 系统拒绝发布该注释并保留原始事件视图。

### User Story 4 - 用放射资源图穿透执行关系 (Priority: P1)

工程师以当前任务为中心，查看 Files、Shell、MCP、Models、Outputs、Subagents、Errors 等分类内圈，以及真实文件、命令、工具、模型、子 Agent 和产物外圈节点。用户可以拖动、缩放、选择节点并高亮当前路径，再联动定位时间线与原始证据。

**Why this priority**: 放射资源图是执行关系的必要入口，不能延期为可选增强；它让用户同时看清“做了什么”和“哪些资源参与了这一步”。

**Independent Test**: 纯布局测试验证分类、节点、去重和径向位置；组件测试验证拖动、缩放、当前路径、筛选淡化、选择同步及可访问列表，不依赖截图判断正确性。

**Acceptance Scenarios**:

1. **Given** 一个包含七类资源和至少 30 个真实节点的会话， **When** 用户选择外圈文件或错误节点， **Then** 地图高亮该节点与中心及相关节点的路径，时间线和检查器定位同一事件。
2. **Given** 用户应用事件筛选或子 Agent 镜头， **When** 部分节点不匹配， **Then** 地图只淡化不匹配节点并保留稳定布局，不删除节点后重新排图。
3. **Given** 某类资源没有真实记录， **When** 渲染放射图， **Then** 不创建占位事实节点，其他分类和可访问资源列表仍可使用。

### Edge Cases

- 历史分页失败：保留已加载事件并标记覆盖不完整；不得宣称完整回放或完整指标。
- 工具输出延迟引用失效：证据区显示不可用原因；不得把缺失当成空字符串或成功结果。
- 时间、Token、成本或模型数据缺失：对应值显示未知；不得用零值或公开价格补造精确数据。
- 外部 `agentplay/v1` 或完整轨迹文档包含未知枚举、失效引用或超限输入：在入口一次性拒绝并指出合同错误。
- 同一事件被多个动作引用：保留一个执行事实，不重复计数、不复制原始记录。
- 回滚到历史内容：创建新的时间状态；不得形成指向过去的时间环。
- 20,000 事件长会话：使用虚拟列表／Canvas 和本地播放；不得为每个动画帧请求历史或创建全量 DOM。
- 分析助手会话：使用明确内部目的标记并从普通任务列表排除；不得写回被分析的原会话。
- 快速连续切换、拖动中步进、播放中隐藏或挂载后切换 reduced-motion：从当前位置重定向或立即显示终态，不得重播、跳回起点、遗留循环或丢失操作。

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: 系统 MUST 从 Aster 原始 `messages`、消息统计、parts、父子工具关系和 Trace spans 生成统一 `AnalysisRun`；不得让 AgentPlay 与完整轨迹各自维护不同事实来源。
- **FR-002**: 每个执行事件 MUST 具有会话内稳定 ID、顺序、类型和 `EvidenceRef`；缺失时间或测量值 MUST 保持未知，不得重排、补零或虚构。
- **FR-003**: 系统 MUST 明确历史覆盖状态；分页未完成或失败时 MUST 拒绝“完整执行”表述，并保留可用的局部视图。
- **FR-004**: 原始工具输出 MUST 通过现有 deferred 结果通道按需读取；失效引用 MUST 返回可用性错误，不得退化为空输出。
- **FR-005**: Map、Story、时间线和检查器 MUST 共享选择与播放游标；播放 MUST 只推进本地事件，切换任务 MUST 使旧异步结果失效。
- **FR-006**: 系统 MUST 提供任务、事件类型、模型和子 Agent 镜头；不得为不同镜头重新复制事件或重复累计指标。
- **FR-007**: 状态迁移 MUST 仅来自具有观察效果证据的 Edit／Execute；缺少快照、Diff、哈希或产物证据时 MUST 降级为动作视图并拒绝推断版本关系。
- **FR-008**: 语义注释 MUST 与观察事实分离，携带来源和证据 ID；未知、越界、循环或无证据引用 MUST 被拒绝，不得显示为事实。
- **FR-009**: 系统 MUST 支持 AgentPlay 风格脱敏导出；文档不得包含提示词、助手或推理原文、工具输入输出、文件内容或命令参数，非法路径模式 MUST 被拒绝。
- **FR-010**: 含原始记录的完整轨迹导出 MUST 与脱敏导出分离，并在导出前显式确认范围；未确认或 Schema 校验失败 MUST 不写出文件。
- **FR-011**: 系统 MUST 提供以当前任务为中心的放射资源图，内圈为资源分类、外圈为真实资源节点，支持拖动、缩放、当前路径高亮和共享事件选择；筛选 MUST 只淡化不匹配节点并保持布局，缺失资源 MUST 不创建占位事实节点。
- **FR-012**: 分析助手 MUST 使用独立内部会话并带有封闭目的标识；普通任务列表 MUST 排除这些会话，且不得向被分析会话追加消息。
- **FR-013**: 系统 MUST 在入口校验外部轨迹和模型结构化输出；未知封闭集合值、重复 ID、失效引用和超限记录 MUST 以命名原因拒绝。
- **FR-014**: 页面 MUST 使用 Aster 设计系统与绘制式选择器，提供键盘播放／步进、可访问事件列表、窄屏详情和中英文；不得新增原生 `<select>` 或边缘滑入桌面对话框。
- **FR-015**: 本期实现 MUST 留在现有仓库与 Studio 子模块，不引入 Python 服务、iframe Viewer、新模型客户端、新 Cargo／Node workspace 或新产品配置语言。
- **FR-016**: 系统 MUST 按 `contracts/analysis-motion.md` 采用 Arc 的连续反馈、组件行为和有界微动效，复用 house `SegmentedControl`、Button、CopyButton、EmptyState、rowMotion 与现有 Motion／CSS；每个动效 MUST 有真实触发器、状态 owner、打断和 reduced-motion 结果，禁止自动旋转、装饰性循环、逐字正文、滚动成本、Sunburst 资源占比语义和每帧 React render。

### Key Entities *(include if the feature involves data)*

- **AnalysisRun**: 一个会话的统一只读执行投影，由 Studio renderer 拥有；包含事件、覆盖、镜头、指标和可选语义图。
- **AnalysisEvent**: 一次观察到的执行事实；以消息 ID、part 索引、工具调用 ID 或 span ID 建立稳定身份，不重复存储原文。
- **EvidenceRef**: 指向本地消息、part、deferred 工具结果或 Trace span 的 typed 引用；不包含导出默认允许的原文。
- **AnalysisState / Phase / Action**:完整轨迹式状态、工作阶段和动作；仅通过事件引用建立，Studio 不把它们写回原始消息。
- **AnalysisTransition / Link**: 有证据的状态迁移、顺序、信息依赖和审查交接；非法关系在投影边界被拒绝。
- **AnalysisAnnotation**: 模型或用户产生的解释层，包含 provenance、来源修订和证据引用；与观察事件分开失效。
- **Coverage**: 当前历史加载、证据可用性和截断状态；决定页面能否声明完整执行。

## House Checklist *(mandatory — from Agents.md)*

- [x] New behavior has a small hermetic test in the owning package
- [x] Closed sets: every new enum has `parse()` / `as_str()`；TypeScript 边界使用等价封闭 Schema／union 与未知值拒绝，Rust 若新增枚举才使用 `parse()` / `as_str()`
- [x] No production `unwrap()` / `expect()` without an infallibility comment
- [x] Internal `a3s-*` dependencies stay on a path in this repository
- [x] Product configuration, if any, is ACL parsed by `a3s-acl`；本期不新增产品配置
- [x] Evidence target: `docs/evidence/desktop/<date>-agent-execution-understanding/OUTCOME.md` when the landing is recorded

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: 计划新增的至少 12 个投影、状态图、放射布局、脱敏和语义注释正反例全部通过，现有 `workspaceAnalysisProjection` 的 3 个测试保持通过；未知测量和失效证据各有明确反例。
- **SC-002**: 一个 20,000 事件夹具可保留全部事件并完成过滤、镜头切换和本地播放；播放 100 步产生 0 次历史读取和 0 次模型请求，事件列表不创建 20,000 个常驻 DOM 行。
- **SC-003**: 脱敏测试夹具中的 8 个秘密原文／参数标记在 AgentPlay 导出中出现次数为 0；所有导出事件引用可解析且 JSON Schema 校验通过。
- **SC-004**: 两条隔离原生旅程通过：失败测试→放射图错误节点→重试→证据定位，以及状态→决策→原始记录；另验证窄屏、键盘、中英文和切换任务后的 stale-response 隔离。
- **SC-005**: 包含 7 类资源和至少 30 个真实节点的夹具中，径向位置、拖动、缩放和当前路径均通过组件断言；任意外圈节点可定位同一事件，筛选后节点数量与布局坐标保持不变。
- **SC-006**: `analysis-motion` 验收矩阵中的 9 类场景全部有组件行为测试；连续 5 次模式／镜头切换不出现双底板或重播，播放 hidden／settled 后活动循环为 0，挂载后切换 reduced-motion 立即得到等价终态，且不存在每帧 React render。

## Assumptions

- 本期首先服务 Aster 自己保存的会话；通用外部 Agent 日志发现与解析不是第一阶段依赖。
- AgentPlay 与完整轨迹的公开格式用于互操作和导出；不把其脱敏文档当作原始证据库。
- 现有消息／part 元数据足以交付事件级回放；无法证明的历史状态保持未知，不通过新增推测规则补齐。
- 单次 AI 语义注释是用户显式触发的可选能力，只用于 Story／决策说明；基础事件回放、放射资源图、证据检查和脱敏导出不依赖模型。
- 派生注释在本期不要求跨重启持久化；源修订变化会使其失效，避免新增数据库和陈旧语义事实。
- Arc UI 是行为与微动效参考；实现复用 Aster house 组件和已审阅的公开目录，不依赖 Pro 源码或第二套设计系统。

## Explicitly Not Claimed

- 不做交互式 AI 问答、追问、Smart Context 或两轮证据选择对话。
- 不承诺逐 Token、逐流式状态录像，不承诺从所有历史会话恢复完整版本树。
- 不保证模型生成的决策、因果或总结语义正确；结构校验和证据引用不等于事实核验。
- 不在本期接入 Claude、Codex、Cursor 等所有外部 Agent 的本地日志解析器。
- 不嵌入上游完整 Viewer，不复制其黄绿色主题，不引入其 Python／转换技能运行时。
- 不提供跨重启注释缓存、团队共享服务、远程公开分享或自动上传。
- 不把 AgentPlay 的公开价格估算当作 Aster 真实账单；没有实测成本时显示未知。
- 不批量安装 Arc 组件，不使用 Sunburst 表达资源占比，不添加自动旋转、持续 shimmer、逐字正文、装饰粒子或成功庆祝动画。
