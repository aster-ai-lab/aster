# Feature Specification: 本地 9.1.6 集成与 Aster 任务评测

**Feature Branch**: 当前工作区；未请求另建分支或提交本轮集成改动。
**Created**: 2026-10-26
**Status**: Ready for implementation
**Input**: 将 Studio 接入最新本地 a3s-code-core / a3s-code-acp 9.1.6，以不占用鼠标的 CUA 模式运行 Aster，完成全部评测并检查效率。用户已将原路径 `../tasts` 明确更正为 `../tasks`。
**Gate context**: Product regime；沿用根 Cargo 工作区、私有 Studio 子模块和现有任务评测工具。此规格直接承接本次明确的集成、规划及执行请求，不承接已延期的 020 业务验收产品功能。

## User Scenarios & Testing

### User Story 1 - 确认运行的确实是本地 9.1.6（Priority: P0）

Studio 嵌入本地 Core，并启动本轮构建的 ACP 与 Office 配套程序。版本文字之外，还要核对依赖来源、程序路径、摘要与实际运行进程。

**Why this priority**: 旧 sidecar 或另一构建目录会使实机结果与本轮代码脱节。
**Independent Test**: 依赖检查及 `agent_plane/launcher.rs` 的精确路径拒绝测试；错误的显式二进制路径不能回退到 PATH。
**Acceptance Scenarios**:

1. Given 根工作区本地 9.1.6 源码，When 构建并启动 Aster 测试实例，Then Core/ACP 仅解析到本仓库，运行 sidecar 摘要与构建回执一致。
2. Given 不可执行或不匹配的显式 sidecar，When 启动，Then 明确拒绝，不借用其他已安装版本。

### User Story 2 - 后台操作不干扰用户窗口（Priority: P0）

使用独立 Aster 实例和自己的窗口；保持现有用户实例、会话和鼠标不变。所有测试输入使用明确的后台投递。

**Independent Test**: `tests/e2e/task-catalog/cua_guard_test.py` 及后台驱动测试证明坐标、指针、前台投递、会触发鼠标点击的 addressed hotkey 和不明确的投递结果被拒绝。
**Acceptance Scenarios**:

1. Given 用户原有 Aster 窗口，When 启动测试 fork，Then 原进程保留，新实例有自己的身份和资料目录。
2. Given 前台投递或坐标动作，When 测试驱动收到请求，Then 在输入发生前拒绝；不降级成前台点击。

### User Story 3 - 28 项任务有独立验收（Priority: P0）

范围为 `list.xlsx` 的 25 项，加上 T01、git-leak-recovery、regex-log。Aster 只拿到题目及允许的输入，不接触答案、评分程序或已有解法。

**Independent Test**: 工作区准备测试拒绝答案泄漏、符号链接和越界输入；评分器的错误数值、错行归属、损坏文件、空结果等反例不能通过。
**Acceptance Scenarios**:

1. Given 含核对答案的任务目录，When 准备作业，Then 仅复制精确白名单文件，原始资料摘要保持不变。
2. Given 未完成 OCR、未冻结评分标准或不完整产物，When 汇总，Then 显示未完成及原因，不计为通过。
3. Given 正则和 Git 任务的参考方案及测试，When 执行，Then 测试侧保留这些资料，Aster 只操作准备好的隔离夹具。

### User Story 4 - 修复后正确性和效率都有记录（Priority: P1）

保留第一次失败，按实际错误修复 owning module，复验后记录耗时、模型调用、工具调用、token 与重试；不以少做检查换取较短耗时。

**Independent Test**: 回执汇总测试拒绝把失败、超时、旧源码结果、部分任务或文件存在当作全部通过。
**Acceptance Scenarios**:

1. Given 同一输入和评分标准的失败，When 修改实现，Then 增加有因果关系的小测试，单独复验失败案例，再验证受影响任务。
2. Given 28 个当前结果，When 报告完成，Then 每项有当前构建身份、终态、独立验收与实际效率数据。

### Edge Cases

- 旧二进制、旧 renderer、端口被别的产品占用、模型不可用、上下文或工具不支持图片。
- 过期 AX token、窗口消失、用户移动鼠标、取消、丢失终态和许可请求。
- 扫描 PDF、损坏 Office 文件、输出可打开但数值错误、答案进入输入目录。
- T01 事件乱序/重复、布尔金额、预算并列最优；Git 清理误伤合法对象；正则跨行误匹配。

## Requirements

- **FR-001**: MUST 使用本地 Core/ACP 9.1.6 和根锁文件，核对实际 sidecar 身份；MUST 拒绝名称相同但来源不符的版本。
- **FR-002**: MUST 使用独立 Aster 实例和后台 CUA；MUST 拒绝坐标、鼠标、前台切换及无法证明后台投递的输入，不结束用户窗口。
- **FR-003**: MUST 固定 28 项显式清单与输入摘要，只提供白名单输入；MUST 拒绝答案、参考解法、评分程序和其他工作区混入。
- **FR-004**: MUST 让真实 Aster/ACP/模型执行任务；MUST 拒绝用脚本响应、预制答案、直接代写产物或浏览器预览替代实机结果。
- **FR-005**: MUST 用源资料和独立检查验证文本、表格、图表及文件内容；MUST 拒绝用关键词散落、文件存在或模型自述作为充分证据。
- **FR-006**: MUST 完成扫描件处理与缺失评分器，原阻塞项仍未完成时 MUST 拒绝全部通过结论。
- **FR-007**: MUST 在当前权限、沙箱和原 Run 中执行并等待终态；MUST 拒绝越界访问、隐式 yolo、并行启动未结算原执行或吞掉清理失败。
- **FR-008**: MUST 为 T01 保留输入不变/重复一致/变体验证，为两个代码任务保留其行为测试；MUST 拒绝只适配公开样本或读取参考解法。
- **FR-009**: MUST 记录每次尝试及实际时间、调用和 token；MUST 拒绝修改评分标准或放宽既有超时来制造改进。
- **FR-010**: MUST 保留无关 dirty 状态、私有子模块边界及既有 API；MUST 拒绝复制 Studio 源码到父仓库、另建宿主工作区、导入旧会话或新增外语 SDK。

### Key Entities

- **BuildReceipt**：Core/ACP/Studio/Office 来源、摘要、功能与启动路径；评测证据持有，不增加执行权限。
- **CaseManifest**：题目、精确输入/禁止项、来源身份、输出合同及超时；仅测试数据，不是产品配置。
- **AttemptReceipt**：case、构建、运行、产物、独立断言、后台动作和效率记录；保留失败，不合成成功。

## House Checklist

- [ ] 新行为的小型 owning-package hermetic 测试：T002–T005、T008。
- [ ] 新闭集明确解析与拒绝未知值：T003–T005、T008；不添加推测业务含义的解析器。
- [ ] 新生产代码无未经证明的 `unwrap/expect`：T008–T009。
- [ ] 所有内部 A3S 依赖保持本地、单一根工作区：T001–T002、T009。
- [ ] 产品配置仍由 a3s-acl 解析；测试 JSON 不授予权限：T002–T003、T009。
- [ ] 证据目标：`docs/evidence/desktop/2026-10-26-local-916-task-evaluation/OUTCOME.md`：T009。

## Success Criteria

- **SC-001**：实际 Aster、Core、ACP、Office 构建及运行身份一致；相关依赖和拒绝测试通过。
- **SC-002**：全部输入动作的后台投递有回执，前台/坐标/指针动作数量为零；用户原实例仍在。
- **SC-003**：28/28 项取得当前独立通过证据，零跳过、零沿用历史通过；原始任务输入无修改，答案未进入模型工作区。
- **SC-004**：每项保留耗时、可获得的调用/token 与重试数据；缺失计量明确为空，不捏造数值。使用既有 600/1200 秒 Office 预算、代码任务自带预算；T01 初始预算 900 秒，调整前说明实际依据。
- **SC-005**：所有修复有 owning-module 反例；记录受影响回归和未验证范围，完整仓库既有问题不混入本次修复。

## Assumptions

- 用户已明确评测根目录为 `/Users/roylin/code/tasks`。
- 现有可信宿主配置可供测试实例使用；不复制凭据或用户会话，不经旧 ToYou 账户代理路线。
- 只读查看源资料和答案用于评测，受测 Aster 始终拿不到答案和解法。
- 本轮已授权规划、集成和执行；未要求将新 Studio 改动提交或发布。

## Explicitly Not Claimed

- 未经实机验证的通过、任意任务的永久正确性或所有模型/平台的资格。
- 新的验收产品、调度器、模型代理、配置语言、SDK 或独立测试框架。
- 读取真实泄漏凭据或清理 Aster/用户仓库；Git 恢复任务只操作本仓库内准备的测试夹具。
- 迁移用户数据、修复全部历史改动或自动提交本轮私有 Studio 改动。
