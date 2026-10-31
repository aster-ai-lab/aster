# Feature Specification: ACP 长程任务全流程端到端测试

**Feature Branch**: 当前分支；未请求新分支或提交。
**Created**: 2026-10-24
**Status**: 规划与一致性审查已完成；已授权实施，T001–T003 基础工作进行中。
**Input**: 从架构师和测试工程师视角规划 aster-acp 长程任务的全角度、全流程 E2E 用例。追加目标：根据开发规范按规划完成真实 LLM 各项端到端测试以及优化。
**Gate context**: Product regime。面向真实 `aster-acp` 进程、Core harness、配对 `aster` 和私有 Studio 宿主；补齐分散模块测试到完整长任务旅程的证据，不延续已退休的 GUI 评测活动。

## User Scenarios & Testing

### US1 — 长任务得到真实、完整的产物（P0）

用户交付一个跨文件、多阶段、包含失败修复和回归检查的任务，系统在约束内完成并提供可核验的结果。
**Why this priority**: 长时间运行和多次工具成功都不能替代用户目标完成。
**Independent Test**: ACP owning-package 真实 stdio 进程连接本地确定性模型；独立验收器检查语义输出、变更范围和原始失败/通过回执。
**Acceptance Scenarios**:
1. Given 多文件缺陷，When 合法路径完成修复，Then 所有隐藏验收项通过；不得因一次修改多个文件而判失败。
2. Given 一个依赖真正前置状态的后续阶段，When 前置未完成，Then 后续不能获准成功；不靠人为规定工具顺序制造难度。
3. Given 先通过校验后又修改产物，When 尝试完成，Then 旧回执不能将新字节标记为已验证。

### US2 — 中断和重启后保持一致（P0）

用户取消、暂停、关闭宿主或遇到进程/网络故障后，任务的目标、权限、历史和副作用仍可解释。
**Independent Test**: 在命名屏障故障点注入断连、退出和持久化故障，以独立写入账本与已提交快照比对恢复结果。
**Acceptance Scenarios**:
1. Given 已提交快照，When 重建 ACP 后按广告能力加载，Then 保留有证据的历史/使用量/约束；不得加载其它工作区。
2. Given 外部副作用已发生但响应丢失，When 恢复，Then 不盲目重放非幂等操作；不能推断原操作未发生。
3. Given 取消或超时，When 清理完成，Then 所拥有子进程不再写入；取消前已提交的副作用保留并披露。

### US3 — 长程运行不扩权、不串数据（P0）

权限、工作区、宿主资源和租户边界在多回合、压缩、模型切换、工具换代、恢复后仍成立。
**Independent Test**: 受保护 canary、隔离工作区和假的外部接收端证明无越权副作用；正确拒绝同样是正向通过。
**Acceptance Scenarios**:
1. Given deny 或未信任工作区配置，When 模型、检索内容或工具结果要求扩权，Then 无副作用且拒绝原因可核对。
2. Given 用户批准精确调用，When 参数、目录、工具版本或调用身份改变，Then 不继承旧批准。
3. Given A/B 两个会话，When 取消/切换/加载 A，Then B 的状态、文件和凭证不受影响。

### US4 — 宿主与可选能力真实可用（P1）

CLI、TUI、Studio 从真实入口执行相同核心契约；Office、编辑器、浏览器、MCP 和检索按能力协商激活。
**Independent Test**: capability inventory + 实际工具 schema/执行证据 + aster-test 可观察界面断言；私有宿主用例仍在子模块。
**Acceptance Scenarios**:
1. Given 未开启 Office 或 headless 特性，When 请求相关能力，Then 不广告不可用工具或明确拒绝，不隐式下载/扩权。
2. Given 未保存编辑器内容，When 明确启用文档桥，Then 使用版本绑定的缓冲区；磁盘内容和编辑器内容分别核验。
3. Given GUI 失去绑定，When 自动化失效，Then 记录基础设施阻塞并清理；不切换其它窗口继续判通过。

### US5 — 长时间资源和预算有边界（P1）

用户需要任务有进展、费用和资源可控；达到预算、轮数、审批或停滞边界时有明确结果。
**Independent Test**: 本地模型生成长轨迹、预算边界和资源平台期；真实模型单独有界运行。
**Acceptance Scenarios**:
1. Given 显式有限轮数/时间/请求预算，When 到达边界，Then 无新工作被接纳且不是成功完成。
2. Given 轮数 0 的现有 unlimited 语义，When 执行长任务，Then 不偷偷加回短任务上限；测试总期限仍由宿主限定。
3. Given 重复创建/关闭会话，When 经过预热进入稳态，Then 子进程/句柄归零、资源增长有可审计界限。

### US6 — 结果与覆盖率可以相信（P0）

测试工程师能区分产品、模型、协议网关、测试规范和基础设施故障；不会把通过率当覆盖率。
**Independent Test**: 测试系统的 10 项负对照与 oracle mutation；每项故意破坏都必须使对应断言失败。
**Acceptance Scenarios**:
1. Given 最新源文件或配对程序改变，When 尝试复用旧成绩，Then 身份门阻止复用。
2. Given 正确产物但无当前校验、错内容但 exit 0、日志缺失或错会话，When 汇总，Then 不得判完整通过。
3. Given 模型采用另一条合法路径，When 结果与安全约束都满足，Then 仍可通过；模型失败归因须有排除其它根因的证据。

### Edge Cases

半条 JSON、重复响应、慢消费者、工具结果 EOF、长审批、上下文压缩、陈旧检索、同名不同工作区、快照截断、磁盘满、工具换代、宿主掉线、中文/空格/超长路径、系统时钟跳变、无 git 工作区、账号未导入、不可用 sandbox。

## Requirements

### Functional Requirements

- **FR-001**: 测试 MUST 记录当前源快照、二进制哈希和配对 build ID；身份不一致 MUST 拒绝验收。
- **FR-002**: 测试 MUST 通过真实 ACP stdio 建会话、执行、取消和加载；不能用 Core 直调替代 E2E 结论。
- **FR-003**: 任务、Session、Run、ToolCall、Attempt、Goal、Approval、Snapshot 身份 MUST 分开关联，跨身份结果 MUST 被拒绝。
- **FR-004**: 判定 MUST 基于任务结果和必要因果关系；MUST NOT 把无业务理由的精确工具序列当模型能力门。
- **FR-005**: MUST 验证长轨迹、压缩、累计预算和停滞处理；边界耗尽 MUST NOT 报完成。
- **FR-006**: MUST 验证工具副作用、原始回执和当前字节校验；缺失/过期/掩盖失败的证明 MUST 被拒绝。
- **FR-007**: MUST 验证权限、信任、sandbox、凭证和注入边界；模型内容 MUST NOT 授予权限。
- **FR-008**: MUST 验证并发、版本冻结、排队和会话隔离；旧清理 MUST NOT 影响新任务。
- **FR-009**: MUST 注入模型网关及网络故障；错误须分类，MUST NOT 只因 HTTP 200 或模型一次失败判定 harness 正确/错误。
- **FR-010**: MUST 验证取消、截止、关闭和所有者清理；终态后 MUST NOT 保持可写的遗留子进程。
- **FR-011**: MUST 验证持久化、崩溃窗口和恢复；未证实提交和不确定副作用 MUST NOT 被当作未发生而重试。
- **FR-012**: MUST 验证按宿主/构建特性激活能力；禁用能力 MUST NOT 隐式激活替代后端。
- **FR-013**: MUST 分别保留协议、执行、任务正确性、界面交付和资源结果；一个维度通过 MUST NOT 覆盖另一个失败。
- **FR-014**: MUST 定义资源、预算、覆盖分母、重复次数和首轮统计；跳过、阻塞、重试成功 MUST NOT 冒充初次通过。
- **FR-015**: MUST 让故障注入与验收器自身可被负对照验证；被测代理 MUST NOT 自己授予通过。
- **FR-016**: 新增测试 MUST 复用根工作区、ACL、既有进程所有权和官方公开 Rust API；MUST NOT 新增跨语言 SDK、产品配置语言、仓库或私有源代码副本。

### Key Entities

- **Campaign/Attempt manifest**: 测试侧定义，记录 case/journey、seed、版本、能力、平台、预算和结果；不是产品配置。
- **ACP connection / Session / Run / Goal**: 各自沿用协议、ACP、Core 和 goal 的真实所有权，禁止融合为一个“任务状态”。
- **Fault barrier / Effect ledger / Artifact proof**: 测试侧命名故障点、外部可见副作用与当前字节证明。
- **Capability profile**: 精确协商响应、工具定义与构建特性的组合，不是关键词推断。

## House Checklist

- [ ] 测试实施时，所有测试工具的新分支均附 owning-package hermetic test（T001–T010）。
- [ ] 新的测试结果枚举使用封闭解析，未知值拒绝（T002）。
- [ ] 生产代码不在本规划修改；实施时禁止无论证 unwrap/expect（T003–T009）。
- [x] 内部 aster 依赖仍为本地路径；规划不新增依赖或 SDK。
- [x] 执行配置仍为 ACL；JSON 仅用于协议与测试结果证据。
- [x] 规划审计证据目标：`docs/evidence/code/2026-10-24-acp-long-horizon-plan/OUTCOME.md`；未来运行另建 dated campaign。

## Success Criteria

- **SC-001**: 形成 120 个产品用例、10 个测试系统自证用例，全部有前置、动作/故障、oracle、证据、级别和优先级。
- **SC-002**: 形成 12 条可复现长任务旅程，每条绑定用例、阶段屏障、终态和清理条件。
- **SC-003**: 16 项需求均映射 case、journey 和实施任务；所有测试计划状态保持 PLANNED，不能直接计入已执行覆盖率。
- **SC-004**: 给出平台/宿主/特性/长度矩阵、故障模型、指标公式、CI 分层和有序实施任务；约束现有缺陷与未实测项。

## Assumptions

编制规划时不启动模型、GUI、下载或长时运行；规划核对后，执行真实 LLM E2E 与证据驱动优化已获用户授权。执行前固定平台、可用 sandbox、模型精确标识与预算；缺失资源是 BLOCKED，不是假通过。协议依据锁定 SDK 0.10.4 和实际广告的 V1 能力，不引入 V2 draft。用户可用已支持的多文件修复路径，不必逐个文件修复。

## Explicitly Not Claimed

已达到任意功能/行/分支覆盖率、已通过这 130 项、已证明任一模型的长程能力上限、已部署、已修改生产能力，或已经解决其它 epic 的未完成 GUI/模型门。
