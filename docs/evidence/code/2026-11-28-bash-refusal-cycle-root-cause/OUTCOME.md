# `bash` 拒绝循环（`tool 'bash' repeated a refusal without fresh original progress`）根因修复与端到端验证

## 根因分析

结合真实会话记录（`id_NTRkNTI5NTYtZDY4OS00N2JkLWEwZjktZmFlMmM0YzkwODkz.json` 中 `run-abe1981c-2cf6-47de-ad69-5efddbbd9fd1`）与权限/恢复代码，除此前已在 `aster-effect`（新原初信息只清理未耗尽重复计数）与 `refusal.rs`（同批次 `(turn, cycle)` 并行拒绝）修复的共性机制外，`bash` 场景存在三个独立根因：

1. **`bash` 对工作区内合法符号链接一律硬拒绝**（`crates/code/core/src/permissions/interactive.rs`）：
   - `read` 已通过 `read_path_crosses_boundary` 放行解析目标仍在 `canonical_root` 内的工作区符号链接，而 `bash` 的 `shell_token_path_crosses_symlink` 仍调用 `path_crosses_symlink(..., true)`，只要路径组件中出现任何符号链接（如 `build/wasi -> build/wasi-deps/...`、`bin/clang -> clang-23`、`AGENTS.md -> CLAUDE.md`），即便所有解析目标完全位于工作区内，也一律在任何模式（含 HITL 与 `--yolo` / `force`）下返回 `ToolRiskLevel::Critical -> PermissionDecision::Deny (symlink_boundary_escape)`。
   - 统一收敛为共享组件遍历函数 `path_crosses_symlink_boundary(root, path, stop_at_shell_glob)`：先经过快速无符号链接路径检查；存在符号链接时逐级 `canonicalize()` 每一段符号链接，仅当所有解析目标均落在 `canonical_root` 内才继续并更新 `current = resolved`，越界符号链接、悬空符号链接、循环链接及其他不可读取状态仍保持 `Deny (symlink_boundary_escape)`。
2. **超长 shell token / 经普通文件子路径触发 `ENAMETOOLONG` / `NotADirectory` 误判为 `symlink_boundary_escape`**（`crates/code/core/src/permissions/interactive.rs`）：
   - `path_crosses_symlink` 与 `path_crosses_symlink_boundary` 对 `std::fs::symlink_metadata` 失败仅豁免 `ErrorKind::NotFound`，遇到超过文件系统单段文件名上限（macOS/APFS 255 字节，返回 `InvalidFilename` / `ENAMETOOLONG`）或把工作区内现有普通文件当作前缀的 shell token（返回 `NotADirectory` / `ENOTDIR`）时走 `Err(_) => return true`，在所有模式下直接触发非可审批硬拒绝 `symlink_boundary_escape`。
   - 新增 `is_absent_path_component_error`，对 `NotFound | NotADirectory | InvalidFilename` 返回 `false`（此前逐级遍历已检查所有实际存在的前缀组件，越界前缀如 `escape/<300字符>` 仍会在前置组件处直接拦截为 `symlink_boundary_escape`）。
3. **`catastrophic_operation`（及策略层 `ShellRisk::Catastrophic` `policy_denied`）拒绝诊断未说明触发规则**（`crates/code/core/src/fact_control/receipts.rs`）：
   - `permission_refusal_hint` 原先没有 `"catastrophic_operation"` 分支，落入仅检查分号/管道/盘符/长度 `> 200` 的 `actionable_denial_hint`，导致诸如 `rm -rf "$work"`、shell 函数定义、`sudo`、`curl ... | sh`、`rm -rf /` 要么无 hint，要么误报 `refused command shape: ... chars; use one short command`，诱导模型改短命令或加 `require_escalated` 重试，最终撞上 `run.tool_refusal_cycle`。
   - 现使用与权限器一致的确定性语法分析器 `crate::permissions::shell::analyze(command)` 生成精确原因提示（明确指出 `FunctionDefinition`、`RemoteCode`、`Escalating`、`Destructive` 或 `Catastrophic`，并声明聊天内请求审批无法解除硬拒绝）。

同步将预存脏树中两条原先依赖工作区内符号链接触发 `bash` `symlink_boundary_escape` 的测试夹具更新为真实越界符号链接（保留 `in-workspace-link` 合法放行断言）：
- `crates/code/core/src/agent_api/direct_tools/governed_tests.rs`（`runtime_permission_preserves_the_winning_denial_reason`）
- `crates/code/acp/tests/boundary_e2e.rs`（`t_prior_verification_pending`）

另注：本机 `/Users/roylin/.local/bin/aster` 与 `/Users/roylin/.local/bin/aster-acp` 仍为 11 月 27 日构建产物（`aster-cli-pair-v1:12e4e37eca612e08288babd81f83300e20bfbae7:aarch64-apple-darwin`），该版本仍会输出旧文案 `tool 'bash' repeated a refusal without fresh original progress`。本次遵循约束未自动覆盖 `~/.local/bin` 安装文件。

## 验证结果

运行环境：`/Volumes/GXF-Q-8T/aster`（`apps/studio` 已初始化），macOS `Darwin 27.2.0 arm64`，`rustc 1.97.1 (8bab26f4f 2026-07-14)`。

| 命令 | 结果 |
| --- | --- |
| `cargo test --locked -p aster-effect --lib -- --skip long_fresh_observation_trajectories_deliver_every_observation` | **101 passed; 0 failed; 1 ignored; 1 filtered out** |
| `cargo test --locked -p aster-core --lib -- permission_ shell::tests governed_tests::runtime_permission_preserves_the_winning_denial_reason refusal_cycle convergence argument_validation agent_api::goal_facade::tests::execution::refusal --test-threads=4` | **147 passed; 0 failed**（5957 filtered out；含新增 `bash_through_in_workspace_symlinks_is_admitted_while_escapes_and_dangling_remain_denied`、`catastrophic_bash_refusal_explains_exact_hard_denial_cause`、`bash_catastrophic_hint_and_in_workspace_symlink_recover_without_refusal_cycle`） |
| `cargo build --locked -p aster -p aster-acp --bin aster --bin aster-acp` | 通过；配对 `--build-id` 均为 `aster-cli-pair-v1:bf649273c137b985c4b0555dc5a474bbeca0a297:aarch64-apple-darwin`；SHA-256：`aster` = `cfc595393478222226dc09d2ee5853382fd18f201d4798648147f7602ae56da0`，`aster-acp` = `9dbd806530ce20e9e8ce5217311398295437358b5e9bd7efe0a8388956b20cae` |
| `ASTER_TUI_BIN="$PWD/target/debug/aster" ASTER_ACP_BIN="$PWD/target/debug/aster-acp" cargo test --locked -p aster-acp --test shipped_tui_e2e -- recovery_e2e --nocapture` | **2 passed; 0 failed**（`shipped_tui_read_recovery_e2e_completes_productive_rereads_twin_batch_and_bad_batch_schema` 与真实 PTY 下 `shipped_tui_bash_symlink_and_recovery_e2e_recovers_and_completes`；43 filtered out） |
| `ASTER_TUI_BIN="$PWD/target/debug/aster" ASTER_ACP_BIN="$PWD/target/debug/aster-acp" cargo test --locked -p aster-acp --test unattended_e2e -- refusal --nocapture` | **3 passed; 0 failed**（`an_interactive_run_stops_generating_after_repeated_original_refusals`、`a_delegated_child_refusal_returns_to_the_actual_acp_parent`、`provider_failures::exhausted_provider_after_refusal_retains_the_original_denial`；32 filtered out） |
| `ASTER_TUI_BIN="$PWD/target/debug/aster" ASTER_ACP_BIN="$PWD/target/debug/aster-acp" cargo test --locked -p aster-acp --test shipped_tui_e2e -- recovery --nocapture`（前置对照） | **7 passed; 1 failed**（唯一失败为与本次改动无关的预存脏树回归：`recovery::shipped_tui_questions_before_approving_a_batched_write` 因 `DiskContentAttestor` 自动签出 `verified ✓ (1 check)` 而未触发 `completion gate:`） |
| `cargo clippy --locked -p aster-effect -p aster-core -p aster-acp --all-targets --no-deps` | 退出码 0；本次修改的 Core/permissions/receipts 代码行无新增警告 |
| 本次涉及 8 个 `.rs` 文件的 `rustfmt --edition 2021 --config skip_children=true --check` 与 `git diff --check` | 退出码 0 |

## Explicitly not claimed

- 未修改除 `read` 与 `bash` 以外写入/浏览工具（`write`/`edit`/`patch`/`ls`/`search`）的符号链接策略。
- 真实外部 LLM 在原始 Klang 仓库中的 WASI 构建尚未在用户终端重跑（当前端到端测试均使用本地隔离 mock 模型服务器）；`/Users/roylin/.local/bin/aster` 与 `aster-acp` 尚未替换为本次 `target/debug` 构建产物。
- Windows 与 Linux 目标未在本机（macOS arm64）运行；`cargo fmt --all -- --check` 存在既有脏树未格式化文件未纳入本次提交范围。
