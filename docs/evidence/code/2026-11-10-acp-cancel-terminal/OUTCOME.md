# ACP 取消响应等待前台工具终态

日期：2026-11-10。改动未提交。

## 问题与修复

在 kbox MicroVM 的真实 ACP 验收中，Bash 已启动父子进程；取消后 Aster 在约 0.368 秒返回 `cancelled`，但该工具仍为 `in_progress`。旧路径只收集 150 ms 事件，后续观察器仅处理子 agent 事件，普通工具的迟到终态会丢失。

修复在 Harness 内完成：复用 `AgentSession::cancel_and_settle` 确认原始 Run 清理，跟踪实际开始执行的前台工具，收齐真实终态后返回。等待权限或用户输入的交互不冒充已执行工具，也不因此阻塞 Stop。独立子 agent 的观察与清理机制保持原有语义。

依据 [ACP prompt turn cancellation](https://agentclientprotocol.com/protocol/v1/prompt-turn#cancellation)：完成当前操作的取消并发送待发更新后，才返回取消完成响应。

## 实际检查

```bash
cargo test --locked -p aster-acp --lib --no-default-features --features svg-png,account-import cancelled_prompt_waits_for_foreground_tool_terminal
cargo test --locked -p aster-acp --lib --no-default-features --features svg-png,account-import agent::turn_terminal_tests
cargo clippy --locked -p aster-acp --all-targets --no-default-features --features svg-png,account-import --no-deps
cargo fmt --all -- --check
```

- 新增回归在修复前：1 FAIL；修复后：1 PASS。
- 相关终态、交互和子 agent 测试最终：16 PASS、0 FAIL。
- Clippy：PASS，存在已有警告。
- 全工作区格式检查：FAIL，存在工作区格式差异；未批量格式化无关文件。
- `git diff --check`：PASS。

原始日志位于同机 kbox 工作区的 `docs/evidence/box/20261110-acp-harnesses/aster-cancel-*.log`。

## Explicitly not claimed

已从含修复的独立快照在 kbox 中重建 Linux CLI 0.3.4／ACP 9.2.2 配套包，并验证共同 bookworm 底座上的 ACP 初始化与 EOF 退出。配套 BUILD_ID 为 `aster-cli-pair-v1:b61531cd75633b0262d192e06a0a9feef2666413:aarch64-unknown-linux-gnu`，版本包位于独立 KRun 仓库的 `harness/aster/acp-0.1.0.zip`。

尚未使用修复后的 Linux 包重新执行真实模型长程任务，尚未完成完整工作区 CI。用户当前优先要求先完成 KRun 架构规划，后续原生验收保留为未完成项。
