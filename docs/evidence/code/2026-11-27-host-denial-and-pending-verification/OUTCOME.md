# 宿主拒绝诊断与待核验回合

用户会话的工具回执记录 `execution_started=false`、`arguments_changed=false`。在该工作区里，传给编译命令的 SDK 目录和编译器路径都有符号链接。交互式宿主检查对这类路径返回硬拒绝。当前 `ask_user` 回执标明 `permission_grant=false`。后续询问错误原因时，完成门槛仍持有上一回合的未核验改动。

运行时组合权限检查器现在保留作出拒绝的策略或宿主的稳定原因码。符号链接拒绝提示检查工作区内的实际路径，并从普通宿主审批通道提交新调用。Core 和 ACP 沿用原有拒绝决定，拒绝发生在工具启动之前。恢复提示区分任务补充信息和宿主授权。

普通交互式新回合仅产生文本，且完成门槛报告与回合开始时相同的未核验改动时，Core 发出 `prior_verification_pending`。ACP 保留改动摘要和原诊断，继续用非成功响应传递尚未完成的核验。TUI 显示此前改动仍待核验。状态答复保留现有文本流，非流式答复在状态之前补发一次文本。改动仍须用绑定到该摘要的验证或宿主豁免来关闭。

## 验证

从当前根工作区执行，`apps/studio` 已初始化。环境为 macOS 27.2.1、arm64、仓库固定 Rust 1.97.1。

| 命令 | 结果 |
| --- | --- |
| `cargo test --locked -p aster-core --lib runtime_permission_preserves_the_winning_denial_reason` | 修改前 1 项失败，宿主确实拒绝调用，组合检查器返回的原因却是 `None`。 |
| `cargo test --locked -p aster-core --lib permission_ -- --test-threads=2` | 79 项通过。覆盖符号链接硬拒绝、工作区内真实路径重新接受权限检查及策略拒绝优先。 |
| `cargo test --locked -p aster-acp --test boundary_e2e e_edit -- --exact --nocapture` | 1 项通过。越界文件内容不变，工作区内替代编辑经过宿主确认后成功。原回合被核验门槛拦截，后续纯文本回复返回结构化待核验状态。 |
| `cargo test --locked -p aster-core --test event_protocol_v1 pending_verification_projects_a_status_with_the_original_diagnostic -- --exact` | 1 项通过。确认版本 1 事件名与诊断投影。 |
| `cargo test --locked -p aster-pager --lib viewer_finalize_stop_reason_to_marker_mapping` | 1 项通过。确认会话观察分支识别待核验状态。 |
| `cargo test --locked -p aster-pager --lib prompt_response_displays_outstanding_verification_without_an_error_toast` | 1 项通过。确认发起回合的 TUI 显示一条待核验状态，回合回到 Idle。 |
| `cargo test --locked -p aster-core --lib prior_verification_pending_delivers_reply_once_without_end` | 1 项通过。已有流式回执时不重复补发。非流式路径发送一份答复和一份待核验终态。 |
| Core 单元测试二进制过滤 `refusal_cycle`、`truncated_completion_preserves_the_stop_and_the_original_gate_debt` | 分别 17 项和 1 项通过。 |
| `cargo build --locked -p aster -p aster-acp --bin aster --bin aster-acp` | 通过。从当前源码构建配套 `target/debug/aster` 和 `target/debug/aster-acp`，含 Core 9.3.2 与 TUI。 |
| `cargo test --locked -p aster-acp --test boundary_e2e -- --nocapture`，指定新构建的 `ASTER_ACP_BIN` 和 `ASTER_TUI_BIN` | 8/9。`e_edit` 检查两个回合具有同一个未核验摘要。真实 PTY 的 `t_ask` 通过。`e_bash` 复现下述失败。 |
| `cargo check --locked --workspace --all-targets` | 通过，产生已有警告。 |
| `cargo check --locked -p aster-core --features ci-all,deep-live` | 通过，产生已有警告。 |
| `cargo clippy --locked -p aster-core -p aster-acp -p aster-pager --all-targets --no-deps` | 通过，产生已有警告，包括 `emit_gated` 现有的参数数量告警。 |
| `cargo fmt --all -- --check` | 仍报告 38 个已有格式差异文件。受影响的原有工作树改动已保留。 |
| `/opt/homebrew/bin/python3.12 tests/workspace_dependencies.py` | 7 项通过。 |

构建输出 `aster --version` 为 `0.4.2 (1be48a0e2115)`，`aster-acp --version` 为 `9.3.2`。最终配套 `aster` 和 `aster-acp` 的 SHA-256 分别为 `9d67ce90fca53138766c3da8659be0ca06e2fd44663487cd12479426077a8cb0` 和 `005b23b8c34ddce458a7b701fe5151b2c1f3b5c520947db5864bb09089e0d688`。直接用测试二进制对这对产物复验，`--skip e_bash` 的 8 项均通过，`e_bash --exact` 单独仍复现已有失败。

首次 ACP 端到端运行发现待核验事件没有登记为 Run 的终态，等待回合超时。在 Run Store 补齐非成功终态映射后，同一用例通过。完整 `event_protocol_v1` 套件中 7 项通过，1 项在既有 `package_binding()` 夹具内失败，原因为 capability snapshot digest 与 Package、generation、Knowledge surface 不匹配。独立的新增事件契约用例通过。

`e_bash` 的原生沙箱回归用例期望工具不写文件。在本机，实验前的已构建 ACP（SHA-256 `e627e974b2286e66f7b32df88daf434f64d54acb9fee1374c64e371789dccc22`）和最终 ACP 都产生 `side-effect.txt`，并因摘要 `dd2e7698ac1f381f81b9a5a6bf32d87e2d22598f7899a37faf3e80460ebc9a09` 缺少绑定验证而被完成门槛拦截。保留该失败与原有测试断言。

## 本机启动路径与真实终端复验

Klang 的原终端进程在 17:48 启动，实际从 `~/.local/bin` 加载了配套标识为 `a66ae4d44a4d7e3de7c76c9d4e9391fa7e96ab8a` 的二进制，两文件的修改时间为 05:49。新增的真 PTY 用例先做一处获准的工作区内编辑，越界编辑被拒，首次回合留下一份未核验改动。第二次回合只回答状态。旧配套复现原现象，第二轮的正常文字下仍出现 `Turn failed in 0.0s: completion gate: workspace mutation`。同一个测试在配套标识为 `7e171ea5a26e9f2b3e2fa482af161e866137d4bd` 的构建产物上通过，第二轮的一份回答后出现 `Previous workspace changes still await verification`。越界文件和已编辑文件的内容各自符合预期。

已更换本机 PATH 下的 `aster` 和 `aster-acp`，并重新核对配套标识和两个 SHA-256。安装的 TUI SHA-256 是 `b94f7bfe7355567cb5bfb9d15d07236c027a5d31f3be588302b0128087b66b19`。安装的 ACP SHA-256 是 `70480ce0f6007392e533fd98b40bab73f364cb342967f03f1efbd7d5d446ac9e`，同一文件用于[最终 DeepSeek 对照](../2026-11-27-native-deferred-tool-catalog/OUTCOME.md)。替换后 `lsof` 显示原 Klang 进程还持有已备份的旧配套文件。可退出旧终端会话，并在原目录用 `aster --continue` 启动新版、恢复会话。

- `cargo test --locked -p aster-acp --test boundary_e2e --no-run`，编译通过。
- `target/debug/deps/boundary_e2e-1770b1eb4e0797cc t_prior_verification_pending --exact --nocapture`，指定更新前的 `ASTER_TUI_BIN` 和 `ASTER_ACP_BIN`。在第二轮等待待核验标签时失败，终端屏幕保留了原错误。
- `target/debug/deps/boundary_e2e-1770b1eb4e0797cc --skip e_bash --nocapture`，对最终安装文件运行，9 项通过，1 项已知原生沙箱用例排除。工作区产物也在新增回归用例后重新成对构建成功。
- `cargo check --locked --workspace --all-targets` 与 `cargo clippy --locked -p aster-core -p aster-acp --all-targets --no-deps` 复验通过。`cargo fmt --all -- --check` 仍有 38 个已有格式差异文件，新增回归文件的定向格式检查通过。`git diff --check` 通过。

## 退出后恢复会话

20:20 的进程检查确认，Klang 的 TUI 和 ACP 仍持有 17:48 启动的 `a66ae4d44a4d7e3de7c76c9d4e9391fa7e96ab8a` 配套。两个实际运行文件均在本机备份目录，PATH 上的两份文件已经是 `7e171ea5a26e9f2b3e2fa482af161e866137d4bd`。原 Klang 进程和会话保持原样。

扩展 `t_prior_verification_pending`，在隔离工作区制造未核验改动并收到一次状态答复后，关闭测试进程，使用相同的隔离配置、会话存储和 `--continue` 打开新进程。恢复后再问一次状态，屏幕显示一份答复和一条待核验状态，越界文件仍是原始字节。

- `cargo test --locked -p aster-acp --test boundary_e2e --no-run` 通过。独立运行 `t_prior_verification_pending --exact --nocapture`，PATH 配套 1/1 通过。
- `cargo build --locked -p aster -p aster-acp --bin aster --bin aster-acp` 通过。本次根工作区构建的配套 ID 为 `aster-cli-pair-v1:eabcfad3d630729b5e8e9d2d539caef9d14bd2fb:aarch64-apple-darwin`。指定这对产物运行 `boundary_e2e --skip e_bash --nocapture`，9/9 通过，含退出再恢复用例。
- `cargo check --locked --workspace --all-targets`、`cargo clippy --locked -p aster-acp --all-targets --no-deps`、`rustfmt --edition 2024 --check crates/code/acp/tests/boundary_e2e.rs` 与 `git diff --check` 通过。全工作区格式检查仍报告 38 个已有文件。

## 同一恢复轮的核验指引

追加的终端回归先保存一处编辑，再提交含工作区内符号链接的 Bash 核验命令。旧配套 `4dc4285a874c8a93d842fa964ce6113fb41a084b` 给模型的同一请求同时包含宿主拒绝和 `Next step: run one bash command`。随后纯文本状态追问依然需要继承原来的改动摘要。

事实运行时把本轮经身份绑定的 Bash 拒绝与结构化未核验状态用于同一条模型指引。指引保留未核验摘要、原工具回执以及宿主提供的核验契约，建议在存在获准替代检查时验证同一改动。权限判定和摘要绑定规则沿用原契约。

- `cargo test --locked -p aster-core --lib verification_ -- --test-threads=2` 通过 64 项。
- `cargo test --locked -p aster-acp --test boundary_e2e --no-run` 通过。此前的配套构建运行 `t_prior_verification_pending --exact --nocapture`，在拒绝与重试指引共存的断言处失败。
- `cargo build --locked -p aster -p aster-acp --bin aster --bin aster-acp` 通过。从当前根工作区构建的配套为 `aster-cli-pair-v1:12e4e37eca612e08288babd81f83300e20bfbae7:aarch64-apple-darwin`。
- 指定该构建的 `ASTER_TUI_BIN` 和 `ASTER_ACP_BIN` 执行 `boundary_e2e --skip e_bash --nocapture`，9 项通过。回归覆盖真实 PTY 中的拒绝、追问和退出后续接，模型响应由隔离 fixture 提供。`e_bash --exact` 独立运行，修复前后的配套均复现 `side-effect.txt` 缺少绑定核验的已有失败。
- 指定 PATH 上实际安装的产物执行 `boundary_e2e t_prior_verification_pending --exact --nocapture`，1 项通过。
- `cargo check --locked --workspace --all-targets`、`cargo clippy --locked -p aster-core --all-targets --no-deps --features ci-all,deep-live` 和 `cargo clippy --locked -p aster-acp --all-targets --no-deps` 均通过。保留已有警告。
- 修改文件的 `rustfmt --edition 2021 --config skip_children=true --check` 与 `git diff --check` 通过。`cargo fmt --all -- --check` 仍报告 38 个已有格式差异文件。

本机 PATH 的 `aster` 与 `aster-acp` 已切换到该配套，安装前后逐字节核对 SHA-256。TUI 为 `b7f806bfdfc56d959401b13dc16b8e1d461df8e22120454b47434afb8e9c35d5`，ACP 为 `e9c3ed3fd87dc254b17fa843798830bd6e3f5da6940cb5fe9c5e7f12b81ab189`。`lsof` 显示 Klang 窗口仍持有 17:48 加载的 `a66ae4d44a4d7e3de7c76c9d4e9391fa7e96ab8a` 文件。它的进程需要退出后才能用 `aster --continue` 加载新版并恢复会话。

## 再次报告后复验

`lsof -n -a -p 61488,61490 -d cwd,txt -Fn` 确认 Klang 窗口的 TUI 和 ACP 仍在该工作区，两个进程分别持有备份目录中的旧文件。对备份、PATH 和当前根工作区产物运行 `aster --build-id` 和 `aster-acp --build-id`，分别得到配套 `a66ae4d44a4d7e3de7c76c9d4e9391fa7e96ab8a`、`12e4e37eca612e08288babd81f83300e20bfbae7` 和 `b68a4669d07fc66fc06390813896eafe4c0b7cf9`。

- 用旧配套运行 `boundary_e2e e_edit --exact --nocapture`，1 项失败。第二轮 ACP 返回原始完成门槛文本，无法解析为 `prior_verification_pending`。用同一配套运行 `boundary_e2e t_prior_verification_pending --exact --nocapture`，1 项失败，首次拒绝缺少具体的 `symlink_boundary_escape` 原因。
- 用 PATH 配套运行 `boundary_e2e t_prior_verification_pending --exact --nocapture`，1 项通过。原回合的拒绝带有具体原因。追问和退出后恢复都显示一份答复及一条待核验状态。
- 用当前根工作区配套运行 `boundary_e2e --skip e_bash --nocapture`，9 项通过，包含上述 PTY 场景及 ACP 摘要一致性检查。
- 检查实际 Klang 文件路径，`build/wasi` 和 `build/wasi/bin/clang` 均为符号链接。后者解析到工作区内的 `build/wasi-deps/wasi-sdk-34.0-arm64-macos/bin/clang-23`。本次只检查路径元数据，原会话进程继续运行。

## Explicitly not claimed

真实会话的 K/WASI 构建仍待验证。退出和恢复用例使用隔离工作区，每次使用同一配套重启。原 Klang 会话尚未退出或用新版重开，其他平台未在本机验证。DeepSeek 定义按需展开试验另有用量证据，本次恢复修复沿用当前生产定义传输策略。[用量证据](../2026-11-27-model-selected-tool-catalog/OUTCOME.md)
