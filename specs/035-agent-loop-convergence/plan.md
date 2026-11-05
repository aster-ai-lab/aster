# Implementation Plan: 自主循环与长程任务去重机制融合

**Branch**: 当前 `main` checkout | **Spec**: [spec.md](spec.md) | **Input**: 本次设计、代码审查与用户开发计划请求。
**Gate context**: `aster-core` 产品路径及根工作区共享库 `aster-effect`；两者均采用 owning-package hermetic tests。无 control-plane、桌面或发布改动。
**Status**: FOLLOWUP_EVALUATED_WITH_LIMITS。首轮结果保留；用户授权的T014–T017正在实施和验收。

2026-11-01追加目标：通用优化与防过拟合（T022–T026）ACTIVE。仍沿用下表的产品路径、同一事实来源、兼容公开API、原预算和原验收器约束。候选实现先统一现有压缩预算及成功/失败输出边界，再处理实测确认的无效再次压缩。零模型/任务/答案特判。真实模型跨原失败数据任务、工程和原生文件任务验证；全部首次尝试及未知用量保留。任何可靠性门或独立任务未过，不能宣称完整目标达成。

## Summary

首轮修正现有调度器和事实投影：重复拒绝保留副作用，统计跨压缩保持，持续新观察不再只因参数相同被拦截。复用既有重复阈值、拒绝/状态循环终止、Goal continuation、完成门和用量计量。先复现，再修复，再验证实际入口与消耗。模型训练和额外提示实验后置。

## Constitution Check

以下 PASS 表示设计符合约束，不表示实现或测试已通过。

| Principle | Status | Evidence in the plan |
| --- | --- | --- |
| I. Fail closed | PASS | T003 保留拒绝来源和不确定效果；T006 不把缺少观察当新进展。 |
| II. Evidence retained | PASS | T001/T002 保留 baseline/red；T012 写原始命令、计数及 Explicitly not claimed。 |
| III. Tests follow regime | PASS | T002–T009 都在 effect/Core/ACP 的实际 owner 中验证，不要求 PostgreSQL。 |
| IV. Closed sets | PASS | 复用现有 outcome/completeness；如有边界枚举，仅使用封闭解析并测未知值。 |
| V. Workspace / public APIs | PASS | 同一根 workspace；T004 检查既有 trait 实现和结构体字面量兼容。 |
| VI. Local schema changes | PASS | 无独立存储；派生状态重放。必要的可选事实字段由 effect owning schema 处理并测旧日志。 |
| VII. Shrink-only ratchets | PASS | 不加 allowlist，不改冻结任务、grader、原超时或模型以求通过。 |
| VIII. Replaceable transports | PASS | 无 provider-specific 或 ACP wire 字段；T009 走原 stdio。 |
| IX. One configuration language | PASS | 复用既有 ACL 和重复阈值，不新增开关。 |
| X. Commands / tests | PASS | 新单测放 sibling 文件；不向生产文件追加大段测试。 |
| XI. Independent judgment | PASS | R1–R3 先复现；反证成立时撤销对应修复，记录原因。 |
| XII. Understanding / adversarial review | PASS | C01–C22 包括副作用、合法复验、新观察和恢复负对照。 |
| XIII. One repository | PASS | 不新建 repo/worktree，不自动提交，不引入多 agent 执行流程。 |
| XIV. Names are not guessed | PASS | 工具名来自注册表/原始调用，路径来自真实回执。 |
| XV. Meaning is not matched | PASS | 比较类型化身份与观察，不解析自然语言或命令关键词判断进展。 |
| XVI. Pickers are drawn | N-A | 无 UI。 |
| XVII. Dialogs stay in place | N-A | 无 UI。 |

## Project Structure

### Documentation

```text
specs/035-agent-loop-convergence/
├── spec.md       # 范围、用户场景与要求
├── research.md   # 现有 owner、静态发现、取消的方案
├── plan.md       # 接线、用例、验证与交付门
└── tasks.md      # 12 个有依赖的实施任务
```

不另建 contracts/data-model 文件；本轮无新产品协议或独立存储。必要的数据兼容决策留在本计划及对应实现证据。

### Source Code

| Layer / owner | Existing paths | Planned change / new files |
| --- | --- | --- |
| 通用调度、原始回执 | `crates/effect/src/coding.rs`、`coding/tool_effects.rs`、`lib.rs` | 修正准备/拒绝顺序、统计投影与必要接线；公开 re-export 必须显式。新增 sibling `coding/convergence_tests.rs`，扩展原 `revalidation_tests.rs`。 |
| Core 准备 adapter | `crates/code/core/src/fact_control/executor_tools.rs`、`receipts.rs` | 接入宿主拒绝与规范化观察，保留权限、参数、钩子、取消和 Run 绑定。新增 `receipts/tests/convergence_preparation.rs`，注册于 `receipts/tests.rs`。 |
| 新信息/循环/结算 | `fact_control/receipts/novelty.rs`、`refusal.rs`、`goal_evidence.rs` | 复用既有判据；只改接线所需部分，不另建 state machine。扩展原 novelty/refusal sibling tests。 |
| 会话入口 | `agent_api/conversation_runtime/owned_fact.rs`、`child_run.rs` | 复用原构造；只有缺少接线时修改。新增 `agent_api/tests/fact_log/convergence.rs` 并注册于 `fact_log.rs`。 |
| Goal | `goal_control/facts/run/continuation.rs`、`facts/settle.rs` | 原则上只验证不改政策；复用 `goal_facade/tests/execution/{refusal,novelty,automatic}.rs`。 |
| ACP 入口 | `crates/code/acp/tests/unattended_e2e.rs`、`tests/long_horizon/` | 复用真实 stdio/local model harness；缺少的压缩场景放 sibling，保留原断言。 |
| CI | `.github/workflows/qualify-code.yml`、`ci.yml` | 仅接入现有检查未包含的新 targeted filters；保留现有 OS matrix，不新增独立 pipeline。 |
| 交付证据 | `docs/evidence/code/2026-11-01-agent-loop-convergence/` | T012 创建 `OUTCOME.md`，引用实际 baseline/red/green/measurement 原文件。 |

`compaction_carry.rs`、`rl_trajectory.rs` 和提示词不属于首轮默认修改范围。不得为“顺便融合”重写旧消息循环、计划生成器或子任务调度器。

## Architecture and invariants

### A. 一个准入 owner，一个事实来源

effect scheduler 继续拥有是否接纳工具执行的决定。Core adapter 继续负责权限、输入钩子、生效参数、原 Run/capability 绑定和 Core 观察封装。原始事实继续是恢复来源，派生统计不是新的授权来源。

Core 的结果封装只在 Core 解码。若机械比较原语需要两个包复用，最小的纯比较部分放在 effect，接收通用类型或宿主规范化身份；不能让 effect 认识 `aster.fact_tool_result`，也不能复制 Core 的封装解析。

### B. 重复拒绝必须保留准备流程的含义

- 在钩子前已经确定的重复拒绝不执行钩子或工具，但仍形成合法的原调用准备/终态证据。
- 已发生的权限拒绝、schema failure、hook denial 和取消按原流程保留；重复诊断不能覆盖原因或效果。
- 钩子可能已执行时，保持原已观察或不确定效果，禁止强制 `Complete([])`。
- 原 `prepare` 与两个公开 request/result 结构体的现有调用保持兼容。先核对全部 `impl ToolRunner` 和构造点，再定接线。
- 如确需扩展 trait，只增加有默认实现的必要方法；不得新增必实现方法、必填公开字段或第二个 policy。默认 adapter 与 Core override 都必须经 T004 验证。方法名和参数以实际实现的最小 diff 为准。

### C. 去重统计不依赖摘要

统计只在现有 scheduler/fact fold 内维护，由原 `tool.prepared` / `tool.result` 重建。`compaction.done` 只改变模型可见消息，不刷新次数。不得增加 per-step 全日志扫描、后台检测任务或第二份持久化状态。

若需在事实 payload 传递规范化观察身份，必须由可信宿主从绑定回执生成；工具自报 metadata 不作依据。新增字段保持可选，旧日志由现有兼容路径处理；缺少/未知值不产生进展。T004/T005 同时验证连续执行、历史重放和旧日志。

### D. 一致的观察含义，不合并不同责任

- 重复依据：工具 + 生效参数 + 原观察 + outcome/退出状态。
- fresh observation 可以延续预算内工作；相同结果和 A/B 旧状态回访不重复赚取进展。
- 真实文件变化后的复验继续允许；同字节重写不算变化。
- 拒绝循环、已观察工作区状态循环、Goal slice 停滞继续各司其职；它们的作用域不同，不能仅因都叫“循环”删除。
- 权限、执行结果 Unknown、不完整观察及宿主等待继续走既有类型化路径。恢复动作仍由模型选择，不根据命令文本选路。
- 只改善现有恢复机会的判断；不新增一次恢复调用、额外 Goal slice 或隐含模型升级。

## Delivery sequence

| 阶段 | Tasks | 输入与产物 | 阶段出口 |
| --- | --- | --- | --- |
| 0：冻结与复现 | T001–T002 | 当前源快照、adapter 清单、原测试结果、R1–R3 red/反证、基线输入 | 每项有实际结果；不能把编译失败当产品 red。 |
| 1：回执和兼容接线 | T003–T004 | 修正重复拒绝顺序与效果保真，验证所有 adapter | C01–C05 及旧调用兼容通过；无拒绝/副作用回归。 |
| 2：统计与观察融合 | T005–T007 | 跨压缩统计、单一观察判据、父/子路径接通 | C06–C17 通过；原 state/refusal tests 保持。 |
| 3：自主循环与真实入口 | T008–T009 | Goal 原额度、旧日志/恢复、ACP stdio | C18–C22 通过；无额外恢复调用或伪成功。 |
| 4：消耗验证与工程检查 | T010–T011 | 固定受控测量、平台/feature 测试、fmt/clippy/依赖检查 | 正确性保持，逐项披露失败/未测，不以少执行代替完成。 |
| 5：复核与证据 | T012 | source/diff/callers 复开核对及 OUTCOME | 未修复项有精确后续；计划未完成项不伪勾选。 |

任务默认串行。T003–T007 共享 scheduler/adapter，不能按文件名拆成同时修改的独立任务。

## Acceptance matrix

以下为 22 个场景，不等于要求新建 22 个测试函数。已覆盖的场景运行原测试；只有缺口新增测试。

| ID | 场景与必要断言 | Owner / task |
| --- | --- | --- |
| C01 | 钩子前确定重复：hook=0、tool=0、准备/拒绝身份一致。 | effect + Core；T003 |
| C02 | hook 写 canary 后拒绝：canary 真实存在，回执保留效果或 Incomplete，完成门不能放行。 | Core 经过 effect 调度；T003 |
| C03 | 重复候选同时遇到 policy deny/schema failure：保留原原因，无执行。 | Core；T003 |
| C04 | hook 改写参数后需要重新批准：不继承旧批准，也不把不同生效参数错算成同一动作。 | Core；T004 |
| C05 | 准备/执行边界取消：保留原取消与效果，不留额外模型请求。 | effect + Core；T004 |
| C06 | 阈值 3、稳定观察、无压缩：保留原有界执行/终态。 | effect + Core；T005 |
| C07 | 同场景强制重复压缩、只保留一个工具对：证明真实压缩且统计不重置。 | effect + Core；T005 |
| C08 | JSON 与图像结果：展示提示缺失不能使重复漏检；不同图像算不同观察。 | Core；T006 |
| C09 | 日志重放/重新打开当前会话：同一工作区间得到同一统计；旧事实可读。 | effect + Core；T005/T007 |
| C10 | 相同参数返回 7 个新外部观察：无本地变更，7 次全部执行并正确交付。 | Core；T006 |
| C11 | 仅调用 ID、宿主提示、耗时或工具自报摘要改变：不刷新信息进展。 | Core；T006 |
| C12 | outcome/退出状态不同：按现有身份规则区分；取消/拒绝不能算成功观察。 | Core；T006 |
| C13 | 真正修改文件后再次验证：保留第 4 次必要复验；同字节重写不刷新。 | 原 revalidation；T006 |
| C14 | A/B 状态往返、一次外部编辑后再循环：保留 state-cycle 停止。 | 原 refusal/mutation；T007 |
| C15 | 首次拒绝后新方法产生新观察：原有恢复机会继续工作，不加额外回合。 | 原 refusal/productive；T007 |
| C16 | 同批次重复、串行和可并行工具：统一计数/拒绝，原 batch 顺序与确认语义保持。 | effect + Core；T004/T007 |
| C17 | ChildRun：同一事实机制、父预算记账、不借用父日志给子任务制造进展。 | 原 child tests；T007 |
| C18 | 新 Run 重复旧观察，Goal information_delta 不增加，重开后仍停滞。 | 原 novelty；T008 |
| C19 | 真正被接受的步骤伴随相同观察：仍支持 Goal 推进；重复 prose 不算。 | 原 novelty/productive；T008 |
| C20 | 停滞后已接受的新计划只获原有一次续跑；后续无进展仍停止。 | 原 automatic；T008 |
| C21 | Unknown、不完整观察、明确宿主等待、硬预算和 deadline：不重放、不扩权、不伪报完成。 | Core + ACP；T008/T009 |
| C22 | 真实 ACP stdio 的压缩重复/新观察对照：正确终态、实际调用数、无额外 End/Goal completed。 | ACP unattended；T009 |

涉及路径的用例使用 portable tempdir，覆盖空格与非 ASCII 名称。仅 hook contract 测试可用有记录的测试钩子；不复制生产检测算法，不新增 test-only 产品绕过。

## Measurement plan

1. T001 在改生产代码前冻结 baseline 源内容、features、构建 profile、输入和现有模型脚本。当前 checkout 有未提交改动，必须记录相关文件内容哈希及 diff，不能只写 HEAD。
2. T010 在同一 checkout 使用基线留存二进制/结果与修改后结果配对，不创建第二个 checkout。产物路径和身份写入证据，拒绝复用身份不明的旧二进制。
3. 受控 workload：稳定重复并强制压缩；持续新观察；修改/复验；A/B 往返；跨 Run 停滞。正向长轨迹使用 017 的 50/200/500 次交互档，沿用其测试总期限，不增加产品上限。
4. 记录每个 case 的完成/失败/未知、模型请求、实际工具执行、压缩次数、provider usage、输入重复内容、总耗时和测量环境。首次结果与诊断重试分开。
5. 受控用量只证明记账/执行行为。假模型返回的 token 不能当作生产节约；工具结果字节下降不能直接换算账单。
6. 无循环正向任务必须产物一致，不能以提早失败换取调用或耗时下降。报告逐任务差异，失败不能从消耗分母删除。
7. 真实模型配对沿用 `tests/test_durable_engineering_paired_real_llm.rs` 的流程与固定任务。另建压缩 cohort 前先冻结输入，不修改原测试中的 `auto_compact(false)` 或 grader。真实模型不作为本轮规划动作；无实际结果时明确 NOT_RUN，不宣称性能改善。
8. 首轮不更换 allocator/hasher、不加 ring buffer/后台服务；如投影扫描已是实测瓶颈，再在原 owner 内优化，并保留等价性证据。

## Verification Plan

从仓库根运行，使用 `rust-toolchain.toml` 的 1.97.1。Cargo 前确认 `apps/studio` 已初始化。命令按依赖顺序执行，不为争抢 build lock 终止其它任务或创建新 worktree。

```sh
cargo test --locked -p aster-effect --lib
cargo test --locked -p aster-core --lib fact_control::receipts::tests -- --test-threads=1
cargo test --locked -p aster-core --lib agent_api::tests::fact_log -- --test-threads=1
cargo test --locked -p aster-core --lib agent_api::goal_facade::tests::execution -- --test-threads=1
cargo test --locked -p aster-core --lib goal_control:: -- --test-threads=1
cargo test --locked -p aster-core --lib agent_api::conversation_runtime::child_run -- --test-threads=1
cargo test --locked -p aster-core --no-default-features --features minimal --lib fact_control::receipts::tests -- --test-threads=1
cargo check --locked -p aster-core --features advanced-harness
cargo build --locked -p aster -p aster-acp
cargo test --locked -p aster-acp --test unattended_e2e -- --test-threads=1
cargo test --locked -p aster-acp --test acp_conformance_stdio -- --test-threads=1
cargo fmt --all -- --check
cargo clippy --locked -p aster-effect --all-targets --no-deps
cargo clippy --locked -p aster-core --all-targets --no-deps
cargo clippy --locked -p aster-core --no-default-features --features minimal --all-targets --no-deps
cargo clippy --locked -p aster-acp --all-targets --no-deps
python3 tests/workspace_dependencies.py
```

- T011 先以 `--list` 确认新/旧 filters 实际命中；0 tests 不算 PASS。若相关 CI 的 feature 不同，补该支持组合的 clippy。
- 复用 `.github/workflows/qualify-code.yml` 的 macOS/Linux/Windows jobs；新增通用 effect tests 和相关 Core filters，保持原平台准备和依赖流程。
- 本机只提供 macOS aarch64 原生证据；Linux/Windows 原生结果由对应 runner 提供，缺失就记录未测。跨编译不当作运行证据。
- 先单独复现每类失败，再判断产品/基线/环境原因。不扩大 lint allowlist，不改超时掩盖失败。
- 构建锁等待或中止不是通过或产品失败；每个命令以其最终退出结果和原始日志为准。

## Risks and stop conditions

| 风险 / 停止条件 | 处理 |
| --- | --- |
| R1–R3 与最新在途改动不再一致 | T001/T002 重读/复现；保留反证并移除无效修复。 |
| 为接线需要破坏公开 trait/struct 调用 | 不实施破坏方案；缩小到兼容扩展并先通过 T004，禁止暗改 API。 |
| 新判据允许持续返回噪声逃避所有预算 | 保留现有全局时间/token/Goal 预算；字节新颖不等于任务成功，不升级完成判定。 |
| 钩子/外部操作真实效果未知 | 保持 Unknown/Incomplete 与原完成门，禁止猜测未执行或自动重放。 |
| 既有 fixtures 或模型环境不能完成真实执行 | 如实记 BLOCKED/NOT_RUN；不换 grader、权限或原模型以获得通过。 |
| 统计状态随原始动作增长 | 沿用日志与既有资源边界，在 50/200/500 档测时间/内存；不预先增加另一套 retention 或淘汰语义。 |

## Deferred work and deviations

首轮不实施 RL 导出、DPO、额外裁判、常驻动作账本、恢复控制器、自动模型切换或 UI。它们不作为本规格完成条件。若后续训练立项，先证明主事实路径捕获的是模型实际输入并符合既有隐私约束，再单独规划。

相对模板的调整：增加复现门、22 场景矩阵、兼容接口约束和消耗测量；省略无用途的独立 contracts/data-model 文件。所有接受证据仍归 `docs/evidence/`。


## 用户授权的失败优化续篇

当前源重新冻结于`docs/evidence/code/2026-11-01-agent-loop-verification-feedback/`。复用既有`model_verification_context`，将fact路径动态状态从system移到最后一个真实ToolResult的模型投影；保留图像、工具身份、stderr边界、trust和原事实。没有工具尾部时system fallback，不创建虚构调用或用户事实。新单测须通过真实LiveCompletion证明系统前缀稳定、缺失绑定可见、验证后消失、再编辑重新出现以及压缩保留。

原native relay的Messages usage显式未资格化，先按原协议事件语义有界读取完整usage/stop reason；OpenAI路径保持原义。原冻结测试输入和grader不变。保留此次优化前P2/P3各自已冻结pair，先跑本地自测和Core/ACP，再以用户选定GLM跑同任务。比较完整通过率和失败在内的请求/耗时/usage；未知账单不计算金额。

Constitution复核：I/II/III/IV/V/VI/VII/VIII/IX/X/XI/XII/XIII/XIV/XV均PASS（设计）；无新增store/配置/恢复额度/裁判/词表/外部依赖；XVI/XVII无UI不适用。已存在的只读verifier默认仍不启用；最终不完整回执不直接转成功。

## 实测续篇 T018/T019

2404 的 4K 截断被原完成门债务遮住；共享 gate_surface_result 同时保留 typed ForcedStop 与原 typed gate debt，公开 API/事件结构不变。单Run feed 实际6次压缩，5次摘要耗尽4K。复用原 compaction_system_prompt 明确只整理观察与既有中间值，禁止摘要阶段代做尚未完成的任务；两个摘要入口共用非空/截断验证，长度截断进入原失败处理而非可靠摘要。不改变原输入、验收器、输出cap、调用额度；本轮新增压缩范围由实际失败证据驱动，不重写调度器。

T019 的实际会话 red 另确认：自动压缩只收到调度器折叠行，遗漏原 ToolCall.text 的公开工作结果。正常模型调用已用原事实重建它；本轮让 SessionCompact 复用同一 recorded_tool_calls_in 与记录索引，从既有 pinned FileLog 读回公开说明，不包含 reasoning。原日志16MiB界限保留，读取移到 spawn_blocking；取消后有限读取可自行结束，不准入模型/工具动作。批次相同说明只保留一次，原记录/输出不改，原完成门/额度不改。
