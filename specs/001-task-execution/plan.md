# Implementation Plan: AsterDream task execution

**Branch**: none created | **Spec**: [spec.md](spec.md) | **Input**: [spec.md](spec.md)
**Gate context**: Work task service, ACP and the Core SDK in the root workspace.

## Summary

Audit the existing execution path against the complete requirements, fix
confirmed causes in their owning modules, and close evidence gaps with current
tests and real pipeline integration. Reuse native Office, fact logs, capability
admission, compaction, budgets and existing loopback-provider fixtures.

## Constitution Check

| Principle | Status | Evidence in the plan |
| --- | --- | --- |
| I. Fail closed | PASS | Typed refusals, terminal/effect checks, denied-path and cancellation tests |
| II. Evidence retained | PASS | Versioned OUTCOME and acceptance matrix; previous records retained |
| III. Hermetic tests | PASS | Owning tests plus loopback provider/real ACP process |
| IV. Closed sets | PASS | Typed tool operations and error classifications; no guessed aliases |
| V. One workspace / published APIs | PASS | Native Office and Core public APIs; no desktop dependency in Core |
| VI. Local schema changes | N-A | No store migration planned |
| VII. Shrink-only ratchets | PASS | No allowlist expansion |
| VIII. Replaceable transports | PASS | Existing provider/ACP boundaries retained |
| IX. One configuration language | PASS | Existing ACL setup only |
| X. Host commands/tests | PASS | Existing commands; new tests in sibling modules |
| XI–XII. Judgment/review | PASS | Independent audit, reproduce-before-fix, adversarial tests |
| XIII. One repository | PASS | All source/evidence remains here |
| XIV. Names not guessed | PASS | Author-specified paths, typed IDs, model-supplied values |
| XV. Meaning not matched | PASS | Model/typed operations, no intent regex |
| XVI–XVII. Pickers/dialogs | N-A | No new product picker or motion |

These are planned invariants, not a claim that all final code has passed review.

## Project Structure

Documentation: `spec.md`, `plan.md`, `research.md`, `tasks.md` here;
acceptance records in `docs/evidence/work/2026-10-18-execution-audit/`.

| Layer | Existing source / pattern | Planned change |
| --- | --- | --- |
| Core model/control | `crates/code/core/src/fact_control/model_call.rs`, `retry.rs`, `budget.rs` | Confirm retry/cancel/fallback behavior and repair proven defects |
| Core context | `compaction.rs`, `context/assembler.rs`, `agent_api/session_context_report.rs` | Audit retention/bounds/measurements and test proven gaps |
| ACP control | `crates/code/acp/src/agent/turn.rs`, sibling terminal tests | Effective cancellation while servicing client requests |
| ACP capabilities | `office_file.rs`, `output_check.rs` | Preserve underlying useful Office operations with typed bounded inputs |
| Native Office | `crates/office/crates/core/src/editor*` | Reuse public mutation APIs; extend only proven missing layout contract |
| Work integration | `apps/work/src-tauri/src/agent_plane/local_acp_tests.rs` | Real ACP/Core + controlled provider delivery/denial tests |
| Work final/capture | `turns.rs`, `workbench_artifacts.rs` | Verify final text, actual saved artifact and terminal consistency |

## Research

[research.md](research.md) maps primary industry guidance to current source and
required evidence. The existing comprehensive product test plan stays the
feature-coverage denominator; this execution audit does not replace it.

## Verification Plan

- Core: focused model governance, run control, budgets, compaction, capability,
  delegation, memory retention, resource settlement and checkpoint recovery.
- ACP: tool discovery/admission, document creation roundtrips, final reply and
  cancellation while the peer is waiting for permission/question input.
- Work: real ACP/Core loopback tests, capture, restart/reconnect and final rows.
- Efficiency: record actual calls, tokens, repeated output bytes and elapsed
  time where observable; compare the same fixtures and source states.
- Corpus: replay the original task suite with its independent acceptance data
  when the provider is available. Keep missing prerequisites as open gates.
- Build from root: paired Aster/ACP/Office and `just build-work`; run
  `cargo fmt --all -- --check` and workspace boundary checks without waivers.
- GUI acceptance remains a separate actual-runtime gate; simulated callbacks
  and a controlled provider cannot certify a full desktop journey.

## 2026-10-20：执行速度优化路径

这是一条 US3 / FR-009、012、013 的有界开发路径，复用当前 epic，
不替换原来的任务质量、控制与跨平台验收。先压低每次执行成本，再减少
模型往返；检索装配作为独立能力修复，不混入主性能实验。

当前状态：同源码 runtime 对照已有 76 个命令样本和 6 项真实 VM 治理
检查；软件 SHA 与硬件 SHA 两组均值分别为 10.180 秒和 3.438 秒，仅证明
命令启动路径的收益。累计八次真实 Core T01 尝试均已结束，失败样本全部
保留。前四次暴露的 program 失败 receipt、嵌套 Bash 正式检查证明及
post-read 紧循环结算缺口，已完成生产修复、owning tests 和独立审查。
配套 Aster/ACP/Office/Work 与矩阵 runner 已构建；测试 provider 的
accepted socket 缺口已 RED→GREEN，同一冻结 ACP 的最终五场景已通过
5/5（`real-acp/final-binary-v4/qualified.json`），原 4/5 失败记录保留。

修复后同源 v3 四格任务耗时为 HC 343.584、SA 456.712、HA 280.916、
SC 523.432 秒，原提交均为 8/9；SA 达到固定 token 预算停止，HC 有一次
执行前 schema 拒绝且原指标完整性为 false，缺失计数保持 unknown。
H/S 表示硬件/软件 SHA，C/A 表示 code/adaptive profile。
四格均未取得晋级资格，不启动额外 pair 或 held-out，也不更改默认值。
这些各一例的样本不能证明 PTC 或整任务的稳定收益。

独立开发 app 已旁置，实际进程身份及旧 app 重新打开的回退已验证；
用户要求的最终 CUA 是第九次 T01，已作为失败评测结束：原生记录等待
审批，界面无审批/暂停入口；正常退出独立 app 后重开，显示运行已中断。
中断时保存的原始产物独立验收 8/9，已有结果与冻结脚本重跑字节一致。
这不是模型正常完成或矩阵胜出配置的部署。grep 无 embedding、真实 Rust LSP 单次冷启动
导航已验证；真实 ONNX 缺少资产，Windows/Linux 资格仍未完成。
详见 [T013 证据](../../docs/evidence/sandbox/2026-10-19-command-latency/T013-OUTCOME.md)
、[四格筛选结果](../../docs/evidence/code/2026-10-20-task-orchestration/matrix/screen-outcome-v3.json)
和 [优化验收记录](../../docs/evidence/code/2026-10-20-task-orchestration/OUTCOME.md)。

### 本轮执行卡

按以下五个交付批次执行，不再增加编排框架。程序化工具调用是第二条
性能主线：目标是减少模型往返；命令运行时优化降低每次执行的固定成本。
两者分别验证和晋级。

| 顺序 / 状态 | 具体交付与负责层 | 退出条件 / 下一步 |
| --- | --- | --- |
| 1．修复 PTC 生命周期，owning 验收已过 | Core `tools/program_tool.rs`；异常/取消保留已执行子调用，完整性未知不得写成零；VM 内处理 deadline/cancel，排空 native futures 并真实 join | 39 个工具、2 个原始 fact、8 个治理、8 个指标检查通过，含十个有界子进程；独立代码审查无阻断。正常返回等待实际 worker，不能宣称任意 native tool 可在 500ms 内强制停止；合并下批后再绑最终配套 binary |
| 2．接通嵌套检查证明，owning 验收已过 | Core `tool_invoker.rs`、`harness_loop.rs`、`tool_completion_runtime.rs`；复用原生证明入口和已有正式检查 parser | 14 个新增 owning cases 及 65 harness、5 native-artifact、8 governance、39 program 检查通过；父失败/不完整、后续或并发写入、同名异目录、伪造证明均拒绝，独立审查无阻断 |
| 3．确认实际宿主，构建及五场景已过 | 根配套程序和 runner 已构建；测试 helper 修复后，对同一冻结 ACP 完整重跑成功、失败、拒绝、取消、修复五场景 | 最终 5/5，实际映射身份一致、拥有的 ACP 已退出；保留原 4/5。ACL→ACP→Core 与 grep/LSP 的入口范围分别记账；ONNX 无资产仍 unavailable，计时固定 retrieval=disabled |
| 4．同源整任务筛选，已完成失败决策 | 修复后 v3 四格均已结束，累计 Core 八次；原提交均 8/9，SA 预算停止，HC 原指标 unknown 保留 | 四格未全部合格，停止追加 pair/held-out 和配置晋级；保留全部原提交、耗时及失败。不再增加付费重跑，不将微基准或单次最快结果当作产品收益 |
| 5．独立开发版能力评测，失败结果已保存 | 配套 app 旁置、实际身份及旧 app 回退已验证；第九次 CUA 因审批界面不可恢复而正常退出，中断产物为 8/9 | 已保存可见错误终态、实际路径、冻结原始产物及九例独立验收；12 输入未变。不称模型完成或提速，下一批优先修 T029 审批恢复/取消可达性；其他平台与 ONNX 仍单列 |

调度约束：前两批可按文件所有权并行，配套 binary 构建/替换串行；进入
计时后停止并行构建、测试与配置修改。生产源码改变时，重建受影响程序
并重新冻结；仅 cfg(test) fixture 改动时重建和冻结测试 harness，另记
as-built 快照之后的测试源码变更，复核生产 binary 哈希，不重写旧构建证据。
不得把前后不同生产源码的样本合并成同源结果。工作区或后端缺失先准确拒绝，
不让模型反复检索 session 日志或消耗请求预算来猜输入。

每批只修其验收确认的根因。筛选不合格的候选停止晋级，不无上限重跑；
不因缺少可选 ONNX 或异平台 runner 扩大 macOS 评测的前置条件。这些
缺口继续保留在各自任务与最终验收表中。

从仓库根目录使用 pinned toolchain，按串行队列执行：

```sh
cargo test --locked --offline -p a3s-code-core --lib tools::program_tool:: -- --nocapture
cargo test --locked --offline -p a3s-code-core --features advanced-harness --test test_extensibility_real_llm metrics:: -- --nocapture
cargo build --locked --offline -p aster -p a3s-code-acp -p a3s-office-cli
just build-work
cargo fmt --all -- --check
```

上述命令之外，嵌套证明 owning tests 与真实 Work→ACP 五场景使用各自
证据记录中的精确过滤器。紧循环必须通过有超时和 reap 的自有子进程
supervisor 验证；不能把无界回归直接挂进整个测试进程。最终构建完成后
才生成新的实验清单和冻结 runner；真实模型测试须显式绑定该清单，不能
用默认 `cargo test --ignored` 启动不受控的其他模型测试。

### 顺序、依赖与交付

| 步骤 | 开发内容与既有实现 | 依赖 / 完成标准 | 任务 |
| --- | --- | --- | --- |
| M0 固定基线 | 复用历史计时与资产摘要；开始开发前冻结源码、构建特性、资产、模型和预算；后续各 Run 核对实际身份，复用现有 tracing 与证据 JSON | T011 已有初步证据；T013/T014 开始前核对冻结清单。准备、任务、独立验收分列；敏感内容不进入性能日志 | T011；T013/T014 的起始检查 |
| M1 降低命令固定成本 | Core `sandbox/oci.rs`、`oci/tests.rs` 的逐阶段计时；根 workspace 构建 Host/shim，复用已有 SHA 硬件后端与生命周期实现 | 先通过身份/篡改拒绝、EOF/父死亡、取消/超时、setsid 后代回收，再扩大基准。每种构建每轮 19 次顺序命令、至少两轮，轮次交替；首次样本单列，报告 p50/p95 和全部样本 | T012、T013 |
| M2 减少模型往返 | Core `tools/program_tool.rs`、`tools/presentation.rs`、既有提示和真实模型 fixture；复用 QuickJS、`batch`、`ToolPresentationProfileV1::code()`，补齐受信 ACL→ACP profile 接线 | 先观察实际 schema/prompt 是否到模型；读操作有界并行、依赖操作串行，返回必要摘要和可恢复来源。结构化子结果、预算、取消与产物证据全部保留 | T014–T018、T020 |
| M3 修复能力装配 | Work `agent_plane/launcher.rs` → ACP `config.rs` / `agent.rs` → Core `SessionOptions::with_workspace_retrieval`；本地模型复用 `crates/bge/src/embedding.rs` | 只传递受信 ACL 中明确启用的能力；模型资源缺失时准确报告 disabled/degraded。真实 ONNX 取消/结算需单独证明；grep 无 embedding 仍可用，LSP 延迟启动 | T018、T019、T021 |
| M4 整任务对照 | 复用 Work 真实 ACP controlled-provider fixture、Core real-LLM fixture 与既有 T01 独立 oracle；真实 host 核对资产/profile 并预检选定的工作区、输入和执行后端 | T020 是本阶段前置资格，不是 M0 依赖。M1/M2 治理检查通过后做 2×2；正确性不退化，统计模型轮数、嵌套调用、VM、输出量、可得 token 和任务 wall；不得从微基准外推整任务 | T020、T022、T023 |
| M5 接入开发版 | 根构建、配套 ACP/Office/runtime；应用启动后核验进程路径和摘要，保留上一组已验证资产作为回退入口 | M4 通过才晋级配置；本轮未通过。用户明确要求的独立开发 app CUA 能力评测单列，不更改默认或替换发行版；实际 host smoke、取消/超时、产物打开及各平台资格分别验收 | T024–T027 |

关键依赖：`M0 → M1 → M4 → M5`；`M0 → M2 → M4`。
M3 的 provider 适配在起始源码冻结后可与 M1/M2 并行；Work/ACP 配置和调用方集成
由同一条串行队列处理。M3 不阻塞隔离的 runtime/PTC 对照，但能力修复
本身必须完成 T021 才可称已生效。

### M2 的开发界限

程序化工具调用（PTC）直接复用现有 `program`，以一个模型调用编排多个
原生子工具：独立的检索/读取有界并行，搜索结果决定的读取及写入→执行
检查按依赖串行；返回必要摘要与来源引用。验收同时统计模型往返、父
program、实际子调用、实际 VM 和输出量，不能把隐藏工具目录或 program
父调用数少当成收益。多个 Bash 子调用仍各自可能启动 VM；减少 VM 次数
须来自合理合并同一依赖链的命令，并保留各步骤退出状态。

1. 首先捕获不含正文/凭据的有效工具目录、profile、提示模板身份及大小，
   对照声明和实际呈现；已有通用提示已经提及 PTC，只修查明的缺口。
2. 用小型 hermetic fixture 覆盖多文件读取、搜索后读取、写入后执行校验。
   只并行没有依赖的调用，不让 JavaScript 重新实现 CSV/金额业务解析。
3. 必要校验调用 `ctx.tool("bash", args)` 并检查结构化失败/退出码。
   未恢复的必需校验失败不能认证完成；允许预期失败后修复并成功重试，
   但保留失败 receipt。摘要不删除原 receipt、截断标志、来源引用或
   现有嵌套原生产物证明，不实现“任意子调用非零就整个脚本失败”。
4. 子调用仍通过原有 Run 治理。沿用权限、总预算、调用数/输出量限制，
   验证取消和脚本超时后子任务已结算；实验显式设置时间与调用上限，
   允许 `task` 不自动获得更大总预算。
5. 比较当前自然选择和已有 `code` profile；不增加按文件名/关键词选择
   编排的路由。PTC 内多次 Bash 仍是多个 VM；合并有依赖的命令是独立
   可测的操作，必须保留各步骤状态和错误传播。
6. Core profile API 与 ACP/Work 受信 ACL 入口已接线；继续验证真实子进程
   接收的 schema。复用既有 typed mode，缺省 adaptive、未知值拒绝；ACP
   传入 `SessionOptions::with_tool_presentation_profile`，Work 临时 ACL 只
   转交明确选择。无新 UI，旧会话拒绝 profile 漂移，默认值在对照前不变。

### M3 的真实后端门槛

BGE 已增加最小公开取消/结算 seam，保留原有 API；真实 worker 的资源
在结算前不能释放或重新使用。适配器和 BGE 共 16 项 hermetic 测试通过，
但本机缺少已安装模型资产，尚未证明真实 ONNX 取消/结算。T019 的 fake
backend 只验证适配契约。T021 的 ready 声明须有实际本地后端资格证据；
没有资产或尚未证明有界结算时只能报告 disabled/degraded，不能用 fake
通过替代，也不为此隐式下载模型。grep 单独验收；代码智能需证明真实
语言服务器完成项目加载及导航，initialize 成功不能单独认证 ready。

### 对照设计与放行条件

| 因素 | 对照 | 候选 |
| --- | --- | --- |
| Runtime | 同源码、软件 SHA 构建 | 同源码、现有硬件 SHA 后端构建 |
| 工具呈现 | 原 T01 使用的普通 `adaptive` profile | 现有 `code` profile / `program` 入口 |

- 旧缓存与候选的历史对比保留，但源码修复和 SHA 后端不同，不能作为
  单一原因的证明。受控构建只允许记录中的 SHA 特性不同；若做不到，
  只能报告整套候选的改善，不能独立归因 SHA。
- 受控构建可复用现有 `sha2/force-soft` 特性覆盖 asm，不改源码：同一
  `target/sandbox-ab-build` 顺序构建 Host/shim，先软件组并快照全部配套
  资产，再去掉该特性构建硬件组并快照。保存 feature tree、工具链、签名
  参数和摘要；相同 target 路径避免编译内嵌 runtime 路径成为额外变量。
- 同机、同镜像、同输入快照、同模型端点/模型 ID、同提示正文、权限、
  Run 总预算及检索状态；每次新建会话和干净工作区。呈现策略与其工具
  目录是实验变量，不要求两种 profile 的 schema 字节完全相同。
- 四格先各跑一次筛选；仅全部机制与正确性通过后，追加原始组合与候选
  组合各一次确认。已发生的四次尝试全部计数；最多再 6 次 Core T01，
  加最终 CUA 共不超过 11 次，不因修源码重置计数。两个保留任务各比较
  原始组合与晋级组合、各两次，最多另加 8 个 Run；无配置晋级则不启动。
  每个 Run 的模型/工具/时间预算在启动前
  冻结；预算耗尽、未完成、失败都保留，不只统计成功样本。
  所有配置报告原始数据和范围；不凭一两次样本声称稳定收益、统计显著性
  或尾延迟。预算固定为一次校准后的 2M token，不再提高。
- 除原 T01，加入未用于调优的通用多文件分析和小型代码修复任务。
  变换路径、数据顺序、规模及合法边界；oracle 置于模型工作区之外。
  signed-zero 第九例固定保留，未过该例不能称 T01 全通过。
- 分别统计父 program 与子调用、真实 VM 数和区间并集，避免重复计时。
  输出必须区分“提交任务到产物可用”和“随后独立验收”的耗时。
- 工作区/后端永久缺失时，用现有明确拒绝结束；达到请求上限时保留真实
  停止原因、已核实进度和实际产物引用。不以盲增 `max_turn_requests`
  或自动重试掩盖无进展循环，也不检索用户历史 session 日志补猜输入。
- Runtime 与 PTC 分别作放行决定：前者需阶段耗时稳定降低，后者需
  减少模型往返且整任务中位数改善；两者均要求相同验收下正确性不降、
  治理测试全过。PTC 无收益则保留已有可选机制，不切换产品默认。

### 当前最近一步与停止条件

以下停止条件保留旧筛选 cohort。2026-10-20 用户重新设定目标，明确授权在
修复并验证后进行一次新的 CUA T01 评测；按 tasks.md 的 T030–T032 单独
记录，不重置原九次计数，不重开四格筛选，不重写已冻结清单。ONNX 不作
本轮评测前置。新评测之前仍先完成下列控制、环境和已证实契约修复：

1. **归档已结束的 CUA（T026/T027）**：保留第九次任务的原生审批
   checkpoint、正常退出/重开后的可见 error、原始产物和九例报告。冻结
   文件为 8/9，仅负零失败；已有结果与公开输入重跑字节一致，12 输入
   未变。提交请求到冻结的 1659.335 秒是含审批、操作和排障的观察上界，
   不是模型或纯任务计时；独立验收为 0.960 秒。无正常模型完成，不重发
   原任务、不人工修题，不因此认证晋级或改写默认。
2. **恢复审批并使取消可达（T029）**：优先修 Work 的 live execution、
   stream overlay 与审批 composer 恢复契约。重新连接须绑定当前 Run
   的有效审批，不能仅凭历史 pending 卡复活权限。等待审批时仍须有
   可达的取消入口；侧栏“结束”只整理列表，不承担取消。先用受控
   provider/前端回归证明旧 overlay 不掩新审批、失效回答拒绝、重连恢复
   和取消后的真实结算，不运行真实模型或 VM。当前 overlay 候选尚未
   捕获本轮内存，不能称唯一根因，详见
   [审批恢复记录](../../docs/evidence/code/2026-10-20-task-orchestration/approval-recovery/OUTCOME.md)。
3. **修复已确认的执行环境说明缺口（T028）**：当前环境块输出宿主
   `macos/aarch64`，实际 Bash 可绑定 Linux OCI，描述未清楚区分两者。
   从已绑定 backend 提供 typed 执行 OS/shell，单列 host 与 Bash target；
   未知后端保持 unknown，不猜发行版或工具家族。用真实 fact provider
   的受控捕获验证父/工具型子任务、adaptive/code 和 context timeout；
   不改命令、不放宽沙箱，也不在当前冻结 CUA 中热修或追加模型重跑。
   源码证据在 [环境契约记录](../../docs/evidence/code/2026-10-20-task-orchestration/execution-environment/OUTCOME.md)。
   此缺口尚不能证明某次模型错误命令的因果或任何已实现提速。
4. **先定位 PTC 的实际采用与失败（M2，现有证据只读分析）**：按源码
   cohort 分开统计 profile/schema 是否到模型、模型轮数、program 父调用、
   原生子调用、VM、输出字节及区间并集。明确“可调用”“实际采用”与
   “减少整任务往返”三层；code profile 不等于模型必然选择 program。
   HC 的执行前 schema 拒绝和原 unknown 指标保持原样，不补零、补 receipt
   或用成功调用推算失败调用。退出条件是每个已观察阻断均能对应拥有层
   和已有/最小可复现回归，不能先写一个新的编排框架。
   当前 CUA 的两个原生拒绝分别为 effective-arguments 未获批准，以及
   同参数 Bash 第四次触发重复调用保护；24 个实际审批回答均为允许。
   先用 T005 的小型受控回归核对参数授权身份及文件更新后的合法复验，
   不能将拒绝写成用户点了拒绝，也不能直接提高阈值或取消无进展保护。
5. **只修已证实的 PTC 契约缺口（M2，Core→ACP→Work）**：复用现有
   `program`、`batch`、typed schema、正式检查证明和治理入口。先使用
   当前回归；仅对新复现缺口补一个小型 hermetic case，覆盖调用参数到
   原生 admission 的准确传递、失败阶段的指标分类或实际 profile 投影。
   通用操作仍是有界并行读取、搜索后读取、写入后执行检查；保留每步
   结果、权限、预算、失败 receipt 和资源结算，不嵌入 T01 业务答案。
   受控 provider 能证明机制与接线，不能证明模型自然采用或性能收益。
6. **补齐实际能力入口（T020/T021）**：在 Work→实际 ACP→Core 上逐项
   对账有效 profile、resume 漂移拒绝、工作区/后端缺失与预算停止原因。
   无 embedding 的 grep 和真实 Rust LSP 已有独立证据，继续明确其实际
   宿主入口及权限范围，不能把 Core fixture 扩写成所有语言/所有宿主
   ready。ONNX 仍因缺资产单列 unavailable，既不隐式下载，也不阻塞
   明确 disabled 的评测。未覆盖条目继续开着，不因 T022 的 5/5 自动关闭。
7. **按门槛结束本轮，而非继续筛选（T023/T024–T026）**：四格筛选已经
   完成失败决策；不追加 pair、held-out 或自动重跑。adaptive 默认保留，
   独立开发 app 与旧 app 回退记录单列，T024 配置晋级保持未通过。
   后续性能实验须针对新的已证实修复，另立同源、固定预算、通用多文件
   分析/代码修复的有界方案，先过正确性与治理再验证采用和收益；本轮
   不执行该实验。Windows/Linux、ONNX 及已有全仓检查缺口分别保留，
   不因 macOS 单次评测补勾。

工作区产物重复计数与跨平台“在文件管理器中打开工作区”属于独立产品
问题，单独验收；它们不进入模型速度收益统计，不能因性能门槛通过而
自动视为已完成。

发现身份漂移、权限扩大、子调用失败被掩盖、取消后继续写入、正确性下降
或测量无法关联实际二进制时，停止该候选晋级，保留样本并修根因。
本轮不规划 VM 复用、删除 hash、主机执行回退、新 PTC 引擎或强制检索。
只有新数据证明剩余瓶颈且现有治理可覆盖，才另立后续切片。

### Constitution Check 增量

既有表继续适用：I/III/XII 由嵌套失败、权限拒绝和资源结算测试约束；
II 由上述两份既有证据目录及新的 PTC 对照证据约束；V/IX/XIII 保持
根 workspace、公开 Core seam、ACL 和本仓库；XIV/XV 禁止 T01 文件名、
关键词或答案缓存驱动策略。无 store migration 或新 UI picker，VI、
XVI–XVII 仍为 N-A。所有 PASS 表示方案可遵守，不表示验收已经通过。
