# `read` 误触发 `run.tool_refusal_cycle` 根因修复与端到端验证

## 根因分析（第一性原理）

`Tool progress blocked: ... run.tool_refusal_cycle` 会在以下四条独立路径上被只读工具 `read` 误触发：

1. **有进展的只读长回合累计撞穿重复阈值（`crates/effect/src/coding.rs`）**：`step()` 仅在新增 `user.message` 或工作区文件变更时清空 `state.duplicate_calls`；收到新的 `informationIdentity` 时只执行 `state.duplicate_calls.remove(&action)`，导致多文件探索回合里反复回看同一核心文件（超过 `duplicate_tool_call_threshold = 3`）被 `duplicate_nudge` 拒绝为 `ToolExecutionOutcome::Denied` 并触发熔断。现改为当 `state.seen_information.insert(identity)` 证明取得真正新信息时，保留已耗尽阈值的条目（`count >= threshold`），同时清理未耗尽阈值的陈旧计数（`count < threshold`）。
2. **同批并发相同拒绝在首轮即熔断（`crates/code/core/src/fact_control/receipts/refusal.rs`）**：模型在单次并行响应中发出多次相同 `read` 调用时，所有回执共享 `(turn, cycle)` 且拥有不同 `batch_index`，模型此时尚未看到第一次拒绝结果。`fold_execution_control` 现记录 `(round, batch_index)`，仅在不同轮次重复或同一合成身份重放时立即触发 `ExecutionControl::Refused`，使模型在首轮同批拒绝后仍保有一次有界恢复决策机会。
3. **工作区内符号链接目录下的缺失文件被误判为 `symlink_boundary_escape`（`crates/code/core/src/permissions/interactive.rs`）**：`read_path_crosses_boundary` 原先直接对 `resolve_path(&root, path).is_err()` 返回 `true`，把工作区内合法符号链接目录下的 `NotFound` 也判成越界拒绝。现区分 `OutsideWorkspace` 与 `NotFound`：后者沿工作区相对路径逐段检查现有前缀符号链接，仅当符号链接指向工作区外、悬空或不可解析时才 fail closed 返回 `true`；若所有现有符号链接前缀均解析到工作区内且缺失的是普通叶路径，则放行给 `ReadTool` 返回精确的缺失文件错误。
4. **结构错误的批量 `read` 参数被误报为 `policy_denied`（`crates/code/core/src/permissions/read_batch.rs`、`crates/code/core/src/permissions/mod.rs`、`crates/code/core/src/fact_control/receipts.rs`）**：`PermissionPolicy::check` 为 fail-closed 会拒绝畸形批量 `read` 参数（例如模型误写 `{"files":[{"file_path": "..."}]}` 而非 `{"files":[{"path": "..."}]}`）。在 `prepare_tools_with_refusal` 已有 `argument_error` 且策略/检查器并非禁止 `read` 本身或越界路径时，优先结算为 `Invalid arguments for tool 'read'`（`ToolExecutionOutcome::Failed` + `ToolEnd`），不再产生误导性的 `PermissionDenied` 与 `[host tool recovery]`。

## 变更文件

- 生产代码：[coding.rs](../../../../crates/effect/src/coding.rs)、[refusal.rs](../../../../crates/code/core/src/fact_control/receipts/refusal.rs)、[interactive.rs](../../../../crates/code/core/src/permissions/interactive.rs)、[read_batch.rs](../../../../crates/code/core/src/permissions/read_batch.rs)、[mod.rs](../../../../crates/code/core/src/permissions/mod.rs)、[receipts.rs](../../../../crates/code/core/src/fact_control/receipts.rs)
- 测试代码：[revalidation_tests.rs](../../../../crates/effect/src/coding/revalidation_tests.rs)、[refusal_cycle.rs](../../../../crates/code/core/src/fact_control/receipts/tests/refusal_cycle.rs)、[tests.rs](../../../../crates/code/core/src/permissions/tests.rs)、[argument_validation.rs](../../../../crates/code/core/src/fact_control/receipts/tests/argument_validation.rs)、[convergence.rs](../../../../crates/code/core/src/agent_api/tests/fact_log/convergence.rs)、[shipped_tui_e2e.rs](../../../../crates/code/acp/tests/shipped_tui_e2e.rs)

## 验证结果

运行环境：`Darwin 27.2.0 arm64`（macOS）、`rustc 1.97.1 (8bab26f4f 2026-07-14)`、根工作区已初始化 `apps/studio`。

| 检查命令 | 结果 |
| --- | --- |
| `cargo test --locked -p aster-effect --lib -- --skip long_fresh_observation_trajectories_deliver_every_observation` | **101 passed; 0 failed**（1 ignored，1 filtered；含新增 `novel_information_clears_prior_duplicate_counts_while_repeats_preserve_them`） |
| `cargo test --locked -p aster-core --lib -- refusal_cycle convergence permission_ argument_validation batch_read sandbox_availability agent_api::goal_facade::tests::execution::refusal --test-threads=4` | **146 passed; 0 failed**（5955 filtered；含原有 `new_original_information_between_refusals_keeps_the_run_eligible` 与 4 个新增 `convergence` 集成用例及各单元测试） |
| `cargo build --locked -p aster -p aster-acp --bin aster --bin aster-acp` | 通过。配套 `--build-id` 均为 `aster-cli-pair-v1:0d481da1ceb9064735a7f6f8570af819723cc7fe:aarch64-apple-darwin` |
| `ASTER_TUI_BIN="$PWD/target/debug/aster" ASTER_ACP_BIN="$PWD/target/debug/aster-acp" cargo test --locked -p aster-acp --test shipped_tui_e2e -- shipped_tui_read_recovery_e2e shipped_tui_shows_recovery_and_an_actionable_stop_in_the_same_session shipped_agent_recovers_from_a_refused_tool_and_bounds_repeated_denials shipped_agent_refuses_repeated_reads_without_new_progress shipped_agent_bash_denials_use_one_recovery_round --test-threads=1` | **5 passed; 0 failed**（39 filtered；真 TUI PTY + 真 ACP 完成畸形批量参数恢复、同批三连读、跨文件重读 `center.txt` 7 次及工作区内符号链接缺失路径处理，且保留反复无进展停机用例） |
| `ASTER_ACP_BIN="$PWD/target/debug/aster-acp" cargo test --locked -p aster-acp --test unattended_e2e -- convergence cycle original_refusals child_refusal --test-threads=1` | **6 passed; 0 failed**（29 filtered） |
| `cargo clippy --locked -p aster-effect -p aster-core -p aster-acp --all-targets --no-deps` | 退出码 0；本轮修改行无任何诊断警告（保留未修改文件中的既有警告） |
| 修改文件 `rustfmt --edition 2021 --config skip_children=true --check` 与 `git diff --check` | 退出码 0，全部通过 |

产物校验和：
- `target/debug/aster` SHA-256：`33ccc7723bc84a6d987de0d8d56c63bf7709f137b4b91ddbdae78a1e9b6c88eb`
- `target/debug/aster-acp` SHA-256：`84d4be506487a9b8bd8f2c5547bf04ecb7f4ba99ac04b6d8c13ddd1059973947`

### 既有工作树冲突记录（未回退、与本轮修复无关）

在早先构建（`aster-cli-pair-v1:78755813f6c4379c1c9c3391fc3988aedfddde80:aarch64-apple-darwin`）上运行 `boundary_e2e --skip e_bash --nocapture` 时：**7 passed; 3 failed**（`tui_readiness::*` 3 项、`e_list`、`e_read`、`e_office`、`t_ask` 通过；`e_edit`、`e_worktree`、`t_prior_verification_pending` 失败）。核对 `git diff -- crates/code/acp/src/agent/session_open.rs` 确认这是当前脏工作树中既有的未提交改动导致：该改动无条件为非 `source_review` 会话挂载 `DiskContentAttestor`，使写编辑在落盘后自动核验通过，从而不再触发测试预期的 `completion gate: workspace mutation`。本轮按要求保留了全部既有未提交改动。全工作区 `cargo fmt --all -- --check` 亦存在未修改文件（如 `crates/code/acp/tests/boundary_e2e.rs`）的既有格式差异。

## Explicitly not claimed

- 未在原生 Windows 或 Linux 上执行编译与运行验证。
- 最终一次编辑（`refusal.rs` 类型别名结构化）后的 `aster-effect` 运行跳过了耗时的 `long_fresh_observation_trajectories_deliver_every_observation`（该用例在上一版等价的 `coding.rs` 逻辑下已通过），未在最终编辑后重跑 `boundary_e2e` 或完整全量 `aster-core` / `aster-acp` 测试套件。
- 未使用真实线上 LLM 进行真实会话重放；端到端测试均使用本地确定性 HTTP mock provider 驱动真实的 `aster` 与 `aster-acp` 二进制。
- 未覆盖安装本机 PATH 中的 `/Users/roylin/.local/bin/aster` 与 `/Users/roylin/.local/bin/aster-acp`（两者仍指向旧构建 `aster-cli-pair-v1:12e4e37eca612e08288babd81f83300e20bfbae7:aarch64-apple-darwin`）。
