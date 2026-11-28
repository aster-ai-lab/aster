# 对已在上下文中的工具结果去重

Core 在构造模型请求时比较工具名、完整参数、工具状态与实际投影文本。成功的大段结果与前一完整观测完全一致时，保留新的调用及其工具结果身份，引用本请求内较早的完整结果。规则适用于文本和 JSON。变化了的结果、不同参数、错误状态和当前附有图像的结果保留完整内容。短文本仍能携带非阻塞的重复提示。每次都从实际保留的上下文确定引用，压缩移走原引用目标后，保留段内第一次出现的结果重新以全文发送。

执行、授权和验证仍读取实际结果。事实日志、检查点和 UI 保存原始的完整观测，`/cost` 的结果字节数按真实留存内容计量。优化同时合并了记忆抽取提示的重复表述，保留抽取频率、评分阈值、语言示例、关系字段和持久化规则。

## 真实 DeepSeek 结果

macOS 27.2.1 arm64，Rust 1.97.1，官方 DeepSeek Flash Chat Completions，关闭思考，temperature=0，输出上限 1024。使用同一 [冻结驱动](../2026-11-27-deepseek-harness-token-comparison/compare.py) 和独立的 [重复观测驱动](repeat-holdout.py)。每个参与组均运行 3 次。表中各列取各自中位数。

| 场景 | 指标 | Aster 优化前 | Aster 优化后 | DSH 0.2.2-rc.5 |
| --- | --- | ---: | ---: | ---: |
| 原冻结任务 | 首个主请求输入 token | 7,912 | 7,912 | 5,804 |
| 原冻结任务 | 全程总 token | 34,142 | 34,076 | 25,032 |
| 原冻结任务 | 辅助记忆请求输入 token | 1,212 | 1,136 | 0 |
| 原冻结任务 | 缓存未命中输入 token | 1,244 | 1,304 | 731 |
| 重复观测 | 首个主请求输入 token | 7,895 | 7,895 | — |
| 重复观测 | 第二次工具执行后的主请求输入 token | 11,783 | 9,968 | — |
| 重复观测 | 全程总 token | 40,916 | 38,797 | — |
| 重复观测 | 缓存未命中输入 token | 1,034 | 1,248 | — |

原冻结任务 9/9 通过，48 次请求的厂商用量齐全。Aster 全程下降 66 token（0.19%），同期 DSH 总量较低。重复观测 6/6 通过，36 次请求的厂商用量齐全。每次都真实执行两次 `cat inventory.json` 并在不同回合报告不同位置的精确值，授权与只读文件校验通过。12 个退出码为 0 的事实回执均保留了完整的 3,938 字节 JSON。发生去重的请求少 1,815 输入 token（15.40%），全程少 2,119 token（5.18%）。首次试跑的只读命令在旧驱动白名单外，主机按设计拒绝。独立驱动绑定了新的冻结命令，正式样本均使用同一授权条件。

- 重复观测的 [参数](repeated/manifest.json)、[任务判分](repeated/trials.json)、[厂商用量](repeated/provider-usage.jsonl) 和 [事实回执校验](repeated/receipts.json)。
- 原冻结任务的 [参数](frozen/manifest.json)、[任务判分](frozen/trials.json) 和 [厂商用量](frozen/provider-usage.jsonl)。
- [各组中位数](medians.json)。
- 实测 ACP SHA-256，优化前 `102f752f6fadd833e475b2566e5926a2a0dcb913e6d9f6d3cf316a3aa833833c`，优化后 `2d5949b74df10443f27ea531dc48e80fecda93cec61667ab0361f1ae8f488332`。

## 本地验证

| 命令 | 结果 |
| --- | --- |
| `cargo build --locked -p aster-core -p aster-acp --lib --bin aster-acp` | 通过。 |
| `cargo build --locked -p aster -p aster-acp --bin aster --bin aster-acp` | 通过，两件产物报告同一个构建 ID `aster-cli-pair-v1:7a0c6d663345aecdc4972fb02a9fdf2661fd36cf:aarch64-apple-darwin`。 |
| `cargo test --locked -p aster-core --lib -- fact_control:: agent::memory_extraction_runtime:: agent_api::tests::fact_log::context_report_and_cost agent_api::tests::fact_log::fact_log_identical_call --test-threads=4` | 329 项通过。覆盖 JSON、观测变化、参数和状态差异、检查点、压缩前后与只读能力。 |
| `cargo test --locked -p aster-acp --lib -- engineering prompt_boundary stop_interrupts_unanswered_permissions_questions_and_forms --test-threads=4` | 15 项通过，3 项忽略。 |
| `cargo test --locked -p aster-acp --test live_interactive_dialogue_real_llm -- real_agent_recovers_from_an_outside_workspace_refusal_in_the_same_turn --ignored --exact --nocapture --test-threads=1` | 1 项通过。使用本轮 ACP 和实际 DeepSeek，拒绝越界后同一个 Run 读取获准文件。 |
| `cargo test --locked -p aster-acp --test shipped_tui_e2e -- shipped_native_form_submit_cancel_and_reject --exact --nocapture --test-threads=1` | 1 项通过，使用本次构建的配对产物。验证原生表单显示、拒绝无效值、提交和取消。见 [PTY 断言](native-form.txt)。 |
| `cargo clippy --locked -p aster-core --all-targets --no-deps --features ci-all,deep-live` | 通过。上下文复核未发现本轮改动代码的新 lint，保留既有告警。 |
| `cargo check --locked --workspace --all-targets` | 通过。保留现有警告和依赖未来兼容性诊断。 |
| `cargo fmt --all -- --check` 与 `git diff --check` | 通过。 |

用真实 DeepSeek 经 ACP 提出中文长期记忆请求，关闭并重开两个进程后，分别通过 `_aster/memory/manage` 查到 1 条持久记录。每侧 2 次厂商请求，用量齐全。仓库原有两项记忆 Live 测试需要 `boyue/deepseek-v4-flash`，现有 ACL 缺少该路由，测试在选择模型阶段退出。另一个冻结离线记忆评测在解析 prompt 时要求存在允许省略的 `Compressed turn transcript` 标题，未读取到判分结果。原有 fixture 和 grader 保持原样。见 [ACP 实测](memory-reopen.json)。

真实压缩用例 `cargo test --locked -p aster-core --test test_tool_history_compaction_real_llm -- original_tool_identity_and_assistant_role_survive_actual_compaction --ignored --exact --nocapture --test-threads=1` 使用现有 DeepSeek 路由。夹具要求 1,024 token 上下文，低于现有宿主 12,288 token 门槛，测试在模型调用前被拒绝。保留该门槛及冻结夹具，压缩后的完整结果回退由上表的密闭用例验证。

## Explicitly not claimed

- 现有会话中旧版已经写入的引用仍按旧记录读取。本轮保护新写入的完整结果和新投影的引用关系。
- 两组的缓存未命中中位数均有上升。这里报告的是实际 token，样本没有证明账单金额下降。
- 覆盖 macOS 本机、列出的固定任务、场景和模型。Windows/Linux 原生运行、Studio GUI 和其他任务类型没有纳入本轮比较。
