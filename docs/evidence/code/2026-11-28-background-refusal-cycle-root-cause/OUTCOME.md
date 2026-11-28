# `background` 拒绝循环（`tool 'background' repeated a refusal without fresh original progress`）根因修复与端到端验证

## 根因分析

结合真实会话记录（`id_MmM1Mzc5ODItMzIyOC00YmQ3LTgzOTMtZDU0NWI2ZDg1Y2Mz.json` 中 `acp-tick-331237ae-7eee-4345-a142-afee5f60476a` `ev 226`）及 Core 运行时链路，发现 `background` 工具在已有防误杀基础机制（`aster-effect` 与 `refusal.rs`）之外存在四个独立根因：

1. **`/yolo` / `yolo` 姿态下 `HostAccessSandbox` 丢失了后端声明的 `background` 能力（真实会话经 `ManifestWorkspaceBackend` 触发）**：
   - 会话初始化时默认挂接 `NativeBashSandbox`（`supports_background_execution() == true`），使 `ToolRegistry::sync_background_registration` 将 `background` 暴露给模型。
   - 当会话运行在 YOLO 姿态时，`ToolContext::apply_execution_policy`（`crates/code/core/src/tools/types.rs`）将 `ctx.sandbox` 替换为 `HostAccessSandbox { runner }`。真实会话中 `runner` 为 `ManifestWorkspaceBackend` 包装 `LocalWorkspaceBackend`（`crates/code/core/src/agent_api/capabilities.rs`）。
   - 但 `WorkspaceCommandRunner`、`LocalWorkspaceBackend`、`ManifestWorkspaceBackend` 与 `HostAccessSandbox` 均未实现/转发 `supports_background_execution()`，全部继承默认 `false`。尽管 `LocalWorkspaceBackend::run_command` 已完整具备流式 `output_observer`、`on_output_complete(&summary)` 捕获计数与 `ProcessGroupGuard` / `kill_on_drop` 进程组回收，每次 `background(action="start", ...)` 在 YOLO 下仍在执行前被 `validate_execution_available` 拒绝（`this sandbox backend does not support background execution`），重试即撞上 `run.tool_refusal_cycle`。
2. **`background(action="tail")` 未传 `wait_for` 时忽略 `timeout_ms` 并零等待返回**：
   - `crates/code/core/src/tools/builtin/background.rs` 原仅在 `wait_for.is_some()` 时进入等待循环；模型调用 `background(action="tail", name=..., timeout_ms=5000)` 轮询输出时会在 0ms 内连续返回未变化状态并触发重复调用拒绝循环。
   - 现支持未传 `wait_for` 且显式给出 `timeout_ms > 0` 时，先订阅 `task.notify.notified().enable()` 再比对 `task.emitted` 与 `task.status()`，等待新行输出、退出结算或超时截止。
3. **`InteractiveToolGuardrail` 未纳管 `background`**：
   - `tail` / `stop` 回落为 `High/Ask`（`Plan` 模式更是 `Deny`，无 HITL 通道时拒绝为 `confirmation_required`），而 `start` 的 `command` 参数也未走 `shell_path_crosses_symlink` 与 `classify_bash` 的硬拒绝检查。
   - 现对 `tail` / `stop` 放行 `Allow`，对 `start` 复用 `shell_path_crosses_symlink` 与 `classify_bash`（拦截 `catastrophic_operation` / `symlink_boundary_escape`，其余正常走 `Ask`/`Force`/`Plan`），并在 `assessment.rs` 中将其归为 `ToolRiskType::CommandExecution`。
4. **不支持 `background` 沙箱的拒绝提示缺少替代行动指引**：
   - 保持前缀 `"this sandbox backend does not support background execution"`，补充引导模型改用前台 `bash` 且勿重复调用 `background`。

## 验证结果

运行环境：`/Volumes/GXF-Q-8T/aster`（`apps/studio` 已初始化），macOS `Darwin 27.2.0 arm64`，`rustc 1.97.1 (8bab26f4f 2026-07-14)`。

| 命令 | 结果 |
| --- | --- |
| `cargo test --locked -p aster-effect --lib -- --skip long_fresh_observation_trajectories_deliver_every_observation` | **101 passed; 0 failed; 1 ignored; 1 filtered out** |
| `cargo test --locked -p aster-core --lib -- background host_access permission_ shell::tests governed_tests::runtime_permission_preserves_the_winning_denial_reason refusal_cycle convergence argument_validation agent_api::goal_facade::tests::execution::refusal sandbox_availability --test-threads=4` | **234 passed; 0 failed**（5873 filtered out；含新增 `yolo_host_access_sandbox`/manifest 检查、`host_access_sandbox_runs_and_settles_background_tasks_and_tail_waits_for_new_output`、`background_tool_allows_tail_and_stop_while_guarding_start_commands` 与 `yolo_background_start_and_tail_complete_without_refusal_cycle`） |
| `cargo build --locked -p aster -p aster-acp --bin aster --bin aster-acp` | 通过；配对 `--build-id` 同为 `aster-cli-pair-v1:1041ca366fe148fd83b3e8d54daec39fd4dac11a:aarch64-apple-darwin`；SHA-256：`aster` = `177da52f9b2bc7ea7ebfeb806e9109f2a81d4b03ead03143100df943a4bc6d6e`，`aster-acp` = `2336765180cc0793ef35dfcfb4aa1ea0d08fc509a6562e28f64b15ebb0f01565` |
| `ASTER_TUI_BIN="$PWD/target/debug/aster" ASTER_ACP_BIN="$PWD/target/debug/aster-acp" cargo test --locked -p aster-acp --test shipped_tui_e2e -- recovery_e2e --nocapture` | **3 passed; 0 failed**（43 filtered out；覆盖 `read`、`bash` 与真实 `--yolo` PTY 下 `background` 启动/等待输出完整收敛） |
| `ASTER_TUI_BIN="$PWD/target/debug/aster" ASTER_ACP_BIN="$PWD/target/debug/aster-acp" cargo test --locked -p aster-acp --test unattended_e2e -- refusal --nocapture` | **3 passed; 0 failed**（32 filtered out） |
| `cargo clippy --locked -p aster-effect -p aster-core -p aster-acp --all-targets --no-deps` | 退出码 0；本次修改代码行零新增警告 |
| 本次修改的 12 个 `.rs` 文件 `rustfmt --edition 2021 --config skip_children=true --check` 与 `git diff --check` | 退出码 0 |

## Explicitly not claimed

- Windows 与 Linux 目标未在本机（macOS arm64）执行运行时验证；`LocalWorkspaceBackend::run_command` 的 `#[cfg(windows)]` `maybe_execute_simple_windows_http_command` 分支捕获统计仅经代码核对与 macOS 构建，未在 Windows 主机实测。
- 非 `LocalWorkspaceBackend`/`ManifestWorkspaceBackend` 的自定义远程 `WorkspaceCommandRunner` 默认 `supports_background_execution() == false`，未对其开放后台运行权限。
- `/Users/roylin/.local/bin/aster` 与 `aster-acp` 仍保留 11 月 27 日旧构建产物（`aster-cli-pair-v1:12e4e37eca612e08288babd81f83300e20bfbae7:aarch64-apple-darwin`），尚未覆盖安装；全工作区 `cargo fmt --all -- --check` 存在预存脏树格式差异文件未纳入本次提交。
