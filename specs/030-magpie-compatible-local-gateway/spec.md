# Feature Specification: Magpie 兼容的本地 AI 网关

**Feature Branch**: 当前工作分支

**Created**: 2026-10-31

**Status**: Draft

**Input**: 在现有 `aster-gateway` 内嵌实现基础上，使 Aster 的本地 AI 网关在业务语义上能够替代 Magpie，而不是只提供几个相似端点。

**Gate context**: `apps/studio` 与 `apps/gateway` 的 product slice；根 Cargo workspace 与根锁文件；不新增仓库或第二套 Node 网关。

## User Scenarios & Testing *(mandatory)*

### User Story 1 - 任意兼容客户端都能使用统一模型入口 (Priority: P1)

OpenAI、Anthropic、Gemini 客户端使用同一批本地模型和路由组，能够完成模型发现、非流式与流式生成、工具调用和推理字段往返。旧 Aster 客户端的冒号模型 ID 继续工作。

**Why this priority**: 协议和模型寻址是替代 Magpie 的最低契约；缺少 Gemini 或流式语义会破坏现有客户端。

**Independent Test**: 离线协议 fixture 同时调用 Magpie 与 Aster，对照状态码、模型 ID、消息结构、事件顺序、工具调用、推理签名、错误和终止事件，不访问真实付费供应商。

**Acceptance Scenarios**:

1. **Given** 一个启用的模型，**When** 客户端以 `provider/model` 请求，**Then** Aster 路由到正确上游；以 `provider:model` 请求得到相同结果。
2. **Given** 一个路由组，**When** 客户端以 `group/id` 请求，**Then** Aster 使用组内候选；裸路由 ID 的旧行为不变。
3. **Given** OpenAI、Anthropic 和 Gemini 三种客户端，**When** 请求流式工具调用和推理，**Then** 事件顺序、参数、签名与终止语义通过对照测试。
4. **Given** 缺少或错误的客户端密钥，**When** 请求受保护端点，**Then** 返回 401，且不接触上游。

### User Story 2 - 多账户和路由组在失败与配额边界继续服务 (Priority: P1)

用户配置多个供应商账户和路由组后，网关按声明策略选择候选，在可重试失败或配额耗尽时切换；已经承诺响应后不进行可能重复输出的重放。

**Why this priority**: 这是 Magpie 的核心业务价值，不能退化成仅按 URL 转发。

**Independent Test**: 使用两个离线上游和配额 fixture，覆盖固定、顺序、轮转、用量、节奏、权重、免费、快速及嵌套路由，验证选择、粘性和故障切换。

**Acceptance Scenarios**:

1. **Given** 首选上游返回可重试错误且尚未承诺响应，**When** 策略允许切换，**Then** 下一候选收到一次请求且客户端不看到首选错误。
2. **Given** 上游已发送首个语义响应，**When** 后续连接失败，**Then** 不向其他候选重放请求。
3. **Given** 嵌套路由组，**When** 解析候选，**Then** 只展开已声明成员，不猜测模型或账户。
4. **Given** 不满足切换条件的 401/403，**When** 上游拒绝凭据，**Then** 错误原样呈现并记录为配置问题，不盲目切换。

### User Story 3 - 客户端密钥、模型权限和配额可独立管理 (Priority: P1)

用户可创建、轮换和撤销命名客户端密钥，并为其设置模型范围和日/周/月 token 与成本限额；超限请求被拒绝且不消耗上游额度。

**Why this priority**: 局域网共享和多客户端使用依赖独立授权与配额，单一全局密钥无法替代。

**Independent Test**: 两个命名密钥分别验证模型授权、轮换、撤销、周期重置、成本/token 限额和并发请求下的精确扣减。

**Acceptance Scenarios**:

1. **Given** 密钥只允许模型 A，**When** 请求模型 B，**Then** 返回 403，模型 B 上游未被调用。
2. **Given** 密钥达到限额，**When** 再次请求，**Then** 返回 429 和重置信息，不消耗上游。
3. **Given** 密钥轮换或撤销，**When** 使用旧密钥，**Then** 返回 401；新密钥按原授权工作。

### User Story 4 - 订阅、目录和用量具有 Magpie 业务语义 (Priority: P2)

网关可管理多账户订阅，动态刷新模型目录，并按客户端、模型、账户和上游密钥记录 token、成本、缓存、重试与配额窗口。

**Why this priority**: 多账户池和可解释用量是长期替代所需的控制面，而不仅是数据面。

**Independent Test**: 使用订阅 fixture 和本地用量存储，验证目录刷新、账户选择、真实归因、成本、缓存、重试和导出不包含原始凭据。

**Acceptance Scenarios**:

1. **Given** 首选账户不可用，**When** 备用账户回答，**Then** 用量归因到实际回答账户，不追溯到首选账户。
2. **Given** 供应商模型目录变化，**When** 刷新或重启网关，**Then** 只更新已声明供应商的目录，不影响其他供应商。
3. **Given** 一个重试后成功的请求，**When** 查询用量，**Then** 最终请求只计一次成功，失败尝试单独标记且不重复计费。

### User Story 5 - 扩展、隐私和 Agent 配置保持兼容 (Priority: P2)

用户可使用声明的插件、隐私脱敏、Agent 配置 Profile、局域网共享和同步；核心网关不需要恢复 ElysiaJS。

**Why this priority**: 完整替代不仅是请求转发，还包括 Magpie 的扩展与本地控制面。

**Independent Test**: 在隔离插件宿主和临时 Agent 配置目录中验证中间件、脱敏往返、配置格式保留、共享授权和同步恢复。

**Acceptance Scenarios**:

1. **Given** 插件超时或抛出错误，**When** 处理请求，**Then** 按契约保留原始请求或明确拒绝，不执行未授权副作用。
2. **Given** 含密钥的提示与工具参数，**When** 完成请求和流式响应，**Then** 上游只见占位符，客户端和文件得到原值，磁盘不出现明文。
3. **Given** 带注释和自定义顺序的 Agent 配置，**When** 切换 Profile，**Then** 仅目标字段变化，其他内容与格式保留。

## Edge Cases

- 无可用模型或候选时，返回明确的 unavailable，不选择未声明模型。
- 配置或目录不可读时拒绝启动，不以空目录软启动。
- 端口冲突时启动失败并保留停止状态，不写 `enabled=true`。
- 客户端断开时取消上游工作；已承诺响应的流不可被其他候选接管。
- 配额扣减与并发请求必须保持精确，不允许超限后补记账。
- 脱敏不可用且策略要求脱敏时，请求失败关闭，不发送原文。
- 上游 401/403 与可重试 408/429/5xx 必须区分，不把所有错误都当作切换理由。

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: 网关必须内嵌 `aster-gateway`，不得恢复 ElysiaJS 或新增第二套 Node 网关。
- **FR-002**: 支持 Magpie 的 `provider/model`、`group/id`，并保留 Aster 旧寻址。
- **FR-003**: 支持 OpenAI Chat、OpenAI Responses、Anthropic Messages 和 Gemini 的声明端点、流式事件、工具调用和推理语义。
- **FR-004**: 实现声明的客户端密钥、模型授权、周期配额及原子扣减；未授权和超限请求不得触达上游。
- **FR-005**: 实现声明的多账户、路由策略、嵌套组与故障切换；响应承诺后禁止重放。
- **FR-006**: 目录、账户和用量只来自显式配置或响应，不猜测名称、供应商、价格或账户。
- **FR-007**: 用量记录必须归因到实际上游账户与密钥，不存原始凭据。
- **FR-008**: 插件和脱敏必须失败关闭；不得把不可信插件权限隐式扩展到主机。
- **FR-009**: 旧 Aster 配置、模型 ID 和已有 OpenAI/Anthropic 客户端必须保持可用。
- **FR-010**: 每个行为阶段必须有独立离线验收，不允许只以 UI 启动成功或文档声明替代。

### Key Entities *(include if the feature involves data)*

- **GatewayClientKey**: 命名客户端凭据、模型范围、周期配额和状态，由桌面本地配置拥有者管理。
- **ProviderAccount**: 上游账户、凭据引用、供应商和配额状态；原始 secret 不进入日志或用量记录。
- **RouteGroup**: 命名候选、策略、嵌套关系和会话粘性，由本地路由配置拥有者管理。
- **UsageAttempt**: 单次上游尝试的状态、账户、密钥引用、错误和重试关系。
- **UsageRequest**: 客户端请求的最终状态、协议、模型、token、成本和缓存，由本地用量存储管理。

## House Checklist *(mandatory — from Agents.md)*

- [ ] New behavior has a small hermetic test in the owning package
- [ ] Closed sets: every new enum has `parse()` / `as_str()`
- [ ] No production `unwrap()` / `expect()` without an infallibility comment
- [ ] Internal `aster-*` dependencies stay on a path in this repository
- [ ] Product configuration, if any, is ACL parsed by `aster-acl`
- [ ] Evidence target: `docs/evidence/local-gateway/<date-slug>/OUTCOME.md` when a landing is recorded

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: 本计划契约表中的 P1 协议用例全部通过；Magpie 与 Aster 的状态、结构、流式事件和错误语义一致。
- **SC-002**: 所有路由策略和嵌套组合通过离线选择测试；响应承诺后的重放次数为 0。
- **SC-003**: 客户端密钥授权、轮换、撤销及周期配额通过并发测试；未授权/超限上游调用次数为 0。
- **SC-004**: 用量归因与实际上下游一致；日志和导出中的原始凭据出现次数为 0。
- **SC-005**: 现有 Aster 回归测试保持通过，`package.json` 与锁文件不重新出现 ElysiaJS。
- **SC-006**: 在声明支持的桌面平台完成原生端到端验证；未运行的平台明确记录，不宣称全覆盖。

## Assumptions

- 兼容基线固定为本机 Magpie `0.1.2141`；升级参考版本必须显式审查并更新契约。
- 真实供应商凭据只用于可选实时验收，不作为默认测试依赖。
- 协议兼容指业务语义兼容，不要求不同供应商的错误文本或时延逐字一致。
- 插件若包含 JavaScript，必须使用独立、明确的隔离宿主；这不等于恢复 ElysiaJS 服务。

## Explicitly Not Claimed

- 当前实现已经是 Magpie 的完美替代品；本文件是后续工作契约。
- 不承诺与 Magpie 相同的 UI、TUI、资源占用或性能数字。
- 不在没有凭据时宣称订阅供应商的实时兼容性。
- 不复制 Magpie 的遥测、远程账户或云端控制面。
- 不用旧 ElysiaJS 的 Gemini、Cherry REST、MCP 或远程配对实现冒充新兼容层。
