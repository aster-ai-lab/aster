# Feature Specification: Aster OS 技术收敛执行（W0–W3）

**Feature Branch**: `main`（未要求特性分支；按仓库默认直接落在 main）

**Created**: 2026-10-21

**Status**: Draft

**Input**: 用户描述：「站在架构师角度按技术收敛的方向规划可执行开发计划」→
「落成 specs/005 有界史诗」。范围：2026-10-21 → 2027-03-31（收敛计划
Phase 0 收口 + Phase 1 GA-2），即开发计划 W0–W3 波次。

**Gate context**: 本史诗是 `apps/cloud/docs/convergence/plan.md`（Active,
Phase 0→3）与根 `aster-convergence-plan.html` 已批准决议（DP-1..DP-5，
2026-10-20）在 aster 侧的执行载体，服务其 Phase 0 出口与 GA-2 验收链。
GA-2 是既有产品门（`apps/cloud/ROADMAP.md` W0/APP0 行、
`ga2-workflow-application-checklist.md`）；本史诗自身不新增产品门。上游
内容性修改发生在 `kense-labs/cloud` 与 `kense-labs/api`（子模块），aster
侧交付物是：仓库工程修复、指针提升、决策记录与证据留存。

**路径制度映射**（供 plan 阶段 Constitution Check 使用）：

| 路径 | 制度 |
| --- | --- |
| 根 workspace、`.github/workflows/`、`justfile`、`apps/runtime`、`crates/lane`（根副本） | 产品制度（hermetic 测试、root workspace、`publish = false`）；`crates/lane` 另守共享副本制度 |
| `apps/api`、`apps/cloud`（子模块内容） | 控制面制度（bounded contexts、`#[controller]`、双适配器证明）——在上游仓库履行；aster 只做 gitlink 指针提升 |
| `crates/lane` 三副本（root / api / cloud） | 共享副本制度：单边修改生长漂移集即被 `tests/crate_drift_report.py` 拒绝；本史诗以命名对账（named reconcile）落地 |

## User Scenarios & Testing *(mandatory)*

### User Story 1 - 基线 CI 修绿（Priority: P0）

维护者在 main 上合并任意 PR，三个 workflow（`ci.yml`、`backend.yml`、
`release-aster-cli.yml`）全部通过。当前基线（上游 run @ `eb17c6ce`，
2026-10-20）`check (ubuntu)` 因已退役 `apps/work/src-tauri` 清单失败、
`backend.yml` 的 `fmt` 因 `--profile minimal` 未装 rustfmt 失败、后端
`check` exit 101。本地工作树已含未提交的修复素材。

**Why this priority**: 红 CI 之上的一切证据与合并都不可信；W1–W3 的保留
运行依赖这条底线。窗口：立即 → 2026-11-07。

**Independent Test**: 干净 checkout 上 `cargo check --locked --workspace
--all-targets` 通过；`cargo fmt --all -- --check` 通过（本地 1.97.1）。
远端判据为一次保留的绿 run 链接，记录于 evidence。

**Acceptance Scenarios**:

1. **Given** 本地未提交的在途修复（含 `apps/work` 引用清除），**When**
   按「逐文件 stage」提交并推送，**Then** main 上三个 workflow 全绿。
2. **Given** `backend.yml` 以 `--profile minimal` 安装工具链，**When**
   `fmt` job 运行，**Then** rustfmt 组件被显式供给（`rustup component add
   rustfmt` 或 `--profile default`），job 不得被删除或 `continue-on-error`
   来变绿。
3. **Given** 后端 `check` exit 101（对照 2026-10-20 基线），**When** 复现
   于当前子模块 pin，**Then** 诊断并修复根因；无法复现时记录「基线已随
   在途修复消解」的证据，不得无凭据地宣称已修。

### User Story 2 - Phase 0 变更包落库与指针一致（Priority: P0）

三个仓库的 Phase 0 变更包（`apps/cloud/docs/convergence/
phase0-change-package.md`，状态「已准备、未提交」）落库：子模块侧在上游
合入，aster 侧提交 gitlink 指针提升。

**Why this priority**: 收敛计划的全部后续工作都挂在未提交的变更包上；
「未提交即未发生」。

**Independent Test**: `git submodule status` 的两个 pin 与变更包记录的
目标 rev 一致；根仓库无新增 lockfile（`apps/api`、`apps/cloud` 各自保留
自己的 lockfile，见宪法约束）。

**Acceptance Scenarios**:

1. **Given** 变更包已在上游两个子模块仓库准备就绪，**When** 上游合入并
   回到 aster，**Then** 指针提升提交只包含 gitlink 变更（`git diff
   --cached --name-only` 验证），绝不捎带另一会话的在途编辑。
2. **Given** 工作树存在大量本会话之外的在途修改，**When** 提交指针提升，
   **Then** 拒绝 `git add -A`；逐文件 stage（宪法 Constraints）。

### User Story 3 - 决策点落地与阻塞发出（Priority: P1）

六个架构决策（D-A 私有 CA 准入、D-B 保留运行凭据/runner、D-C
`apps/runtime` 新包名、D-D postgres digest 镜像、D-E SEV 容量、D-F krun
引导链归属）在期限内出结论：aster 能决定的落为决策记录；需要 owner 或
外部团队的发出工单/issue 并记录编号。

**Why this priority**: 非编码决策是当前关键路径的最长前置（D-A/D-B/D-F
卡 T0.4.2 与 GA-2 绿 run）。

**Independent Test**: 每个决策点有一份记录（决策记录、ADR 或外发 issue
编号）可引用；未决项在 epic 看板上显式标红，不得静默过期。

**Acceptance Scenarios**:

1. **Given** T0.4.2 需要私有 CA 准入结论，**When** owner 拍板，**Then**
   ADR 落入上游 decisions/ 并被指针提升带进 aster。
2. **Given** 某决策到期限（D-A/D-B：2026-11-07；D-C/D-D：2026-11-15）
   仍无结论，**Then** 依赖该决策的下游故事拒绝启动并在看板标注阻塞原因，
   不得带假设推进。

### User Story 4 - T0.4.2 OIDC 联邦 e2e 执行 → Phase 0 出口（Priority: P1）

首次端到端执行 `apps/cloud/docs/convergence/
t0.4.2-oidc-federation-runbook.md`（含 Cloud 侧 SubjectLink
link/revoke/负控），产出证据，关闭收敛计划 Phase 0 的唯一出口项；同批回
填 G0/P0/WEB0 三条 lane 的证据日期（aging 缺口 7→4）。

**Why this priority**: Phase 0 不出口，成员面身份第二写路径不清零，GA-2
的月度评审节奏不成立。

**Independent Test**: 上游控制面路径按双适配器制度提供证明；aster 侧判
据为 runbook 全步骤执行的 OUTCOME.md 落入 `docs/evidence/`（宪法 II）。

**Acceptance Scenarios**:

1. **Given** D-A/D-D 已决且 krun 修复可用（D-F），**When** 执行 runbook
   步骤 1–6，**Then** 任一步骤失败即停止并记录，不得跳步后宣称完成。
2. **Given** e2e 完成，**When** 2026-11 月度评审，**Then** Phase 0 获追
   认；`G0/P0/WEB0` 的 ROADMAP 行带证据日期。
3. **Given** 步骤失败于外部依赖（如 Box 修复未到），**Then** OUTCOME.md
   的 Explicitly not claimed 段点名该外部依赖与重试条件。

### User Story 5 - lane 持久化命令 payload 修复（命名对账）（Priority: P1）

`queue.rs:284` 三副本（`crates/lane`、`apps/api/crates/lane`、
`apps/cloud/crates/lane`）以 `json!({})` 落盘 `StoredCommand.payload`，
重放即丢命令内容——正确性缺陷。以命名对账修复：三副本同一提交语义，上
游两仓库 PR + 根仓库修改 + 指针提升，`tests/crate_drift_report.py` 保持
绿（不生长漂移集）。

**Why this priority**: 这是 B 泳道里唯一的正确性缺陷，且演示三副本修
复的标准链条，为 DP-3 单上游合并铺路。

**Independent Test**: 三副本各带同一回归测试：构造带 payload 的命令 →
持久化 → 重放 → payload 保真（字节相等）。产品/共享副本制度，无需
live PostgreSQL。

**Acceptance Scenarios**:

1. **Given** 三副本同线 284 的缺陷，**When** 修复，**Then** 三份 diff 语
   义一致、测试同套；漂移 ratchet 绿。
2. **Given** 上游某一 PR 未合入，**When** 尝试单边先落根副本，**Then**
   `tests/crate_drift_report.py` 拒绝（漂移集生长），提交不落地。

### User Story 6 - `apps/runtime` 命名与注册收尾（Priority: P2）

> **2026-10-21 证据修正**：改名已完成。`apps/runtime` 的包名是
> `aster-gateway-runtime`（`apps/runtime/Cargo.toml:2`）且已是根 workspace
> 成员（根 `Cargo.toml` members）；`aster-runtime` 现仅属于
> `crates/runtime`（拷贝的 exec runtime）。原「撞名待改名」的前提不复存
> 在，D-C 撤销。残余：registry-builder 仍未注册（依赖 pinned extension
> 0.1.3 的决定）；`lib.rs` 文档注释已随本切片对齐包名；线上身份字符串
> （health 的 `service: "aster-runtime"`、user_agent）保留待消费方澄清。

Gateway /v1 数据面二进制的命名一致性收尾：包名、模块文档、注册状态三
者对齐；registry-builder 注册决定同批处理。

**Why this priority**: 结构一致性，但不阻塞 GA-2。

**Independent Test**: `cargo metadata` 中包名唯一；`cargo check
--locked --workspace --all-targets` 通过。

**Acceptance Scenarios**:

1. **Given** 包名已唯一，**When** 校验，**Then** `aster-gateway-runtime`
   与 `aster-runtime` 各自唯一可解析，无歧义。
2. **Given** 后续引入新的内部 `aster-*` 依赖，**When** 解析，**Then** 依赖
   只指向本仓库路径；远端依赖出现即被 `tests/workspace_dependencies.py`
   拒绝。

### User Story 7 - runtime usage/analytics 数据面诚实化（Priority: P2）

`apps/runtime/src/http/router.rs:255` 的 `/v1/api/usage/batch` 以 200 返
回空 batch 冒充导出（`"stub: lifecycle export not yet wired"`）；
`usage` 硬编码 `lifecycle_events: 0`、`analytics` 恒空。

**Why this priority**: 面向消费者的 API 返回假数据违反 fail-closed 语义；
实现真实导出与显式降级二选一，消除沉默欺骗。

**Independent Test**: endpoint 契约测试：未接线时返回 501 且 reason 命名
（如 `usage_export.not_wired`）；返回 200 当且仅当 records 来自真实生
命周期导出。

**Acceptance Scenarios**:

1. **Given** 生命周期导出未接线，**When** 客户端调用 usage/analytics，
   **Then** 拒绝并命名原因（501 + reason），不再返回 200 假空集。
2. **Given** 后续接线真实导出，**When** 有记录，**Then** 200 仅携带真实
   records；schema 字段为闭集（宪法 IV）。

### User Story 8 - 保留 CI 基建与 GA-2 验收链 aster 侧支撑（Priority: P2）

GA-2 验收链（W0.3 真实供应商收尾 → HumanTask 全环 → 23 节点目录 →
APP0.3 匿名 Chatbot 经 Gateway 流式交付）主体在上游。aster 侧交付：依赖
D-B 的保留绿 run 管道（复用 `just middleware-up`/`backend-test` 栈）、
G0 构建日志契约与 Box 的外联 issue（D-F 同渠道）、以及每次上游 GA-2 进
展的指针提升 + 证据落档。

**Why this priority**: GA-2 是收敛计划的目标门（2027-03-31）；aster 的角
色是保留运行基建与证据托管，不代上游声明 Verified。

**Independent Test**: 一次完整保留绿 run 的 run 链接与 artifact 存档记
入 `docs/evidence/`；`ga2-workflow-application-checklist.md` 末行由
「Open (retention pending green run)」转为保留通过（该声明落上游仓库）。

**Acceptance Scenarios**:

1. **Given** D-B 就绪（token 或自托管 runner），**When** 触发保留管道，
   **Then** 全链绿 run 被保留且可追溯；无法读取私有子模块时以命名错误
   失败，不得跳过检出后假绿。
2. **Given** 上游 W0.3/HumanTask/目录/Chatbot 任一环落地，**When** 回到
   aster，**Then** 指针提升与 evidence 同步；aster 的 ROADMAP/文档不替
   上游写 Verified 行。

### User Story 9 - krun 自有修复轨（Priority: P1；2026-10-21 日终增补）

> 增补背景：owner 于 2026-10-21 指示「fork 到 aster-ai-lab、在其上修、
> 切换到自有 fork」，替代 US-3 中 T014 的外发 issue 路径。T020-T022 已
> 在 tasks.md Phase C+ 承载。

libkrun（aster-ai-lab 自有 Rust fork）的引导链故障转为 aster-ai-lab 自有
资产修复：fork、插桩（`KRUN_BOOT_ABORT` 早退具名）、Box 侧保留失败
引导日志、fork 修复合并后重 vendor 切换。

**Why this priority**: T0.4.2 的三解阻塞之一；插桩与日志保留是取证前
提，重 vendor 前一切诊断都靠它们。

**Independent Test**: `vm::boot_diag` 三用例（拷贝/替换/no-op，3/3 绿）；
fork `cargo check -p libkrun` exit 0（占位 init、check-only、未发布）。

**Acceptance Scenarios**:

1. **Given** fork main 带插桩（`02e1414`）+ Box 保留日志，**When** 失败
   引导发生，**Then** `<home>/diag/failed-boots/<box_id>/logs/` 含具名
   `KRUN_BOOT_ABORT` 阶段行（端到端声明待 T0.4.2 重跑）。
2. **Given** fork 合并行为修复，**When** 重 vendor（T021），**Then**
   `vendor/libkrun` 为 fork 确定性导出、`build.rs` 哈希与
   SOURCE-PROVENANCE 同步、漂移可溯。

### Edge Cases

- 子模块上游在史诗期间前进：一律以指针提升跟进，禁止在 aster 内直接改
  子模块内容后提交（会制造游离提交）；上游拒收时记录并以 pin 冻结等待。
- 某决策过期未决：下游故事 fail closed（拒绝启动），看板标注，不静默降
  级范围。
- 保留运行中途失败：artifact 部分恢复按 2026-10-18 run-control-audit 的
  先例——未重验的原始宿主检查不得计入绿。
- 三副本对账期间某一上游 PR 被要求改动：三副本同步重出，ratchet 语义以
  「三份一致」为准，不以「两份一致」放行。

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: main 分支三个 workflow 必须在每次合并时全绿；必须拒绝以删
  除 job、`continue-on-error`、或放宽触发条件的方式制造绿（宪法 VII 收缩
  单向棘轮同样适用于 CI 配置）。
- **FR-002**: `backend.yml` 的 fmt job 必须显式供给 rustfmt 组件；工具链
  安装行与组件需求不一致时必须在 job 内失败而非依赖本地缓存。
- **FR-003**: 后端 `check` 失败必须先在当前子模块 pin 上复现再修复；无法
  复现时必须在 evidence 中记录「基线已消解」的对照（提交范围 + run 链
  接），不得无证据宣称修复。
- **FR-004**: 指针提升提交必须只含 gitlink；必须拒绝捎带任何其他文件
  （`git diff --cached --name-only` 先行验证，宪法 Constraints）。
- **FR-005**: 六个决策点各必须有一份可引用的记录；到期限未决时，依赖方
  必须拒绝启动并以命名阻塞标注。
- **FR-006**: T0.4.2 runbook 的执行证据必须含全步骤结果与「Explicitly
  not claimed」段；任一步骤失败必须停止执行，禁止跳步宣称完成（宪法 II、
  XII）。
- **FR-007**: lane payload 修复必须以三副本一致的一次对账落地；单边修改
  导致漂移集生长时必须被 `tests/crate_drift_report.py` 拒绝。
- **FR-008**: `apps/runtime` 改名必须由 ADR 记录旧名→新名映射，且新名在
  `cargo metadata` 中唯一；必须拒绝两个同名 `aster-runtime` 包同时可解析。
- **FR-009**: usage/analytics 端点在导出未接线时必须返回 501 与命名原因，
  必须拒绝以 200 + 空 records 冒充导出；真实导出的 response schema 必须
  是闭集。
- **FR-010**: 保留运行管道在无法读取私有子模块时必须以命名错误失败，必
  须不得跳过检出后产生不可追溯的绿。
- **FR-011**: 本史诗全部行为性落地必须随 evidence
  `docs/evidence/<domain>/<date-slug>/OUTCOME.md`（命令、计数、not
  claimed 段）；证据目录不得删除。
- **FR-012**: 本史诗不得引入第二种产品配置语言；若 D-A 的私有 CA 准入产
  物构成产品行为配置，必须以 ACL（`.acl`，仅经 `aster-acl` 解析）表达
  （宪法 IX）。

### Key Entities

- **子模块 pin**：`apps/cloud`（`kense-labs/cloud`）、`apps/api`
  （`kense-labs/api`）的 gitlink；本史诗的「上游进展」以 pin 前进表达。
- **Phase 0 变更包**：`apps/cloud/docs/convergence/phase0-change-package.md`
  所述三仓库提交集；aster 侧对应指针提升提交。
- **保留绿 run**：可追溯的 CI 运行（链接 + artifact），GA-2 checklist 的
  关闭判据。
- **`StoredCommand.payload`**：`crates/lane`（三副本）的持久化命令负载，
  修复后必须保真。
- **决策记录（D-A..D-F）**：架构决策的最小记录单元（ADR 或 issue 编号），
  下游故事的启动前置。

## House Checklist *(mandatory — from Agents.md)*

- [x] 新行为各有 owning package 的小型 hermetic 测试（US-5 回归测试三副
  本同套；US-7 契约测试；US-1 以本地 check/fmt + 保留绿 run 判定；US-4
  按控制面制度在上游履行双适配器证明，aster 侧仅存证）
- [x] 闭集：US-7 的 501 reason 与 response schema 为命名闭集；本史诗不
  新增需要 `parse()`/`as_str()` 的枚举（如 plan 阶段发现新增，按宪法 IV
  补齐）
- [x] 生产路径无新 `unwrap()`/`expect()`（除非构造级不可失败论证注释）
- [x] 内部 `aster-*` 依赖保持本仓库路径解析（US-6 改名后由
  `tests/workspace_dependencies.py` 把关）
- [x] 产品配置如有新增（D-A 产物）仅 ACL、仅 `aster-acl` 解析
- [x] 证据目标：每故事落 `docs/evidence/<domain>/<date-slug>/OUTCOME.md`
      （域跟随现有目录集：`development/`（US-1/2/6/7）、`execution/`
      （US-3/4/8）、US-5 的对账证据随上游仓库记录）

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: main 上三个 workflow 连续绿（保留 run 链接入 evidence）；
  本地 `cargo check --locked --workspace --all-targets` 与
  `cargo fmt --all -- --check` 通过。
- **SC-002**: Phase 0 变更包三仓库落库；`git submodule status` 与包内记
  录一致；根仓库无新增 lockfile。
- **SC-003**: 六个决策点各有记录或外发编号；到期未决者看板显式标红。
- **SC-004**: T0.4.2 OUTCOME.md 含 runbook 步骤 1–6 全结果；Phase 0 于
  月度评审追认关闭；`G0/P0/WEB0` 证据日期回填（aging 无日期 lane
  3→0）。
- **SC-005**: lane 三副本回归测试通过且 `tests/crate_drift_report.py` 绿。
- **SC-006**: `cargo metadata` 无重复 `aster-runtime` 包名（依赖 D-C 落
  地）。
- **SC-007**: usage/analytics 契约测试通过：未接线 501 + 命名原因，接线
  后 200 仅真实 records。
- **SC-008**: 至少一次完整保留绿 run 存档（依赖 D-B 落地）。

## Assumptions

- `kense-labs/cloud`、`kense-labs/api` 在窗口内接受史诗相关的上游 PR；
  Box 仓库（krun、日志契约）由 D-F 外联渠道响应。
- owner 在 D-A/D-B（2026-11-07）与 D-C/D-D（2026-11-15）期限内给出结
  论；D-E（SEV）不影响本史诗（GA-2 无 TEE 依赖）。
- GA-2 验收链的上游交付（W0.3 收尾、HumanTask、目录派发、Chatbot）按
  收敛计划 Phase 1 推进；本史诗不替其背书。
- 子模块 pin 冻结策略：史诗内不追非必要上游前进。

## Explicitly Not Claimed

- 不从 aster 声明任何上游 lane 的 Verified（W0.3、FN0、GA-2 等）；声明
  权在上游 ROADMAP 行。
- 不解冻 CELL0/EV0/AR0/I0 Track B+（DP-4 维持）；不执行 DP-2 NATS 迁
  移与 Phase 2/3 任何条目（后续史诗承接）。
- 不改变 SEV/TEE 容量决策；不引入新语言 SDK 或第二个 Cargo workspace；
  不恢复 `apps/work` 或其任何源。
- 不处理 Desktop 渲染器 324 个上游 TS 诊断与 Desktop ROADMAP 未放行行
  （建议独立史诗）；vec 量化、Gateway MM0–3/rollout、发现后端 P4.1 占位
  等登记为 backlog，不属本史诗。
- macOS 执行资格（打包、网络中介、Python 环境等）不在本史诗范围。

## [NEEDS CLARIFICATION]

- **D-A（私有 CA 准入）**：产品侧信任链调整 vs 测试联邦换公共 CA——两方
  案的安全边界不同，需 owner 拍板后方可执行 T0.4.2 runbook 步骤 1。
- **D-B（保留运行凭据）**：GHA 环境 + `ASTER_LABS_READ_TOKEN` vs 自托管
  runner——影响 SC-008 的实现路径与运维成本。
- **D-C（`apps/runtime` 新包名）**：已解决（2026-10-21 核验：包名
  `aster-gateway-runtime` 已落地且已注册；见 US-6 的证据修正）。残余的
  registry-builder 注册（pinned extension 0.1.3）另记。
- **D-D（postgres digest）**：镜像代理 vs 重新 pin，阻塞 T0.4.2 步骤
  4–6 的可执行性。
