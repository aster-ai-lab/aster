# 以模型输入为边界的 token 优化

用同一条合成用户输入调用 DeepSeek Flash，将系统／环境消息和工具定义分别送入 API。Aster 与 DSH 的工具定义字节数接近，分词成本仍相差 1,267 token。系统／环境消息另相差 1,253 token。两者各占原首请求差距的一半。下表包含两边相同的用户输入及各请求的协议开销。

| 分项诊断的输入 token | 上轮 Aster | DSH | 差值 |
| --- | ---: | ---: | ---: |
| 仅用户输入 | 43 | 43 | 0 |
| 用户与系统／环境消息 | 2,043 | 790 | 1,253 |
| 用户与工具定义 | 6,326 | 5,059 | 1,267 |
| 完整首次请求 | 8,324 | 5,804 | 2,520 |

Core 将参数名、类型、可选值及边界交给现有工具 Schema，常驻提示保留游标、冲突处理、Git 安全和完整验证等操作规则。只读角色使用共用工作流，并保留只读 Bash 指令。写入指导只进入通用角色。显式 SDK 参考仍包含完整参数说明。

模型端表单说明从同一共享目录生成简写。测试可将简写逐项还原成全部 13 种组件、每个允许属性和 `children` 标记。从实际请求比较，26 项工具的名称、顺序及参数 Schema 相同。唯一改写的工具说明是 `ask_user_form`，Schema 和执行路径保持原样。独立分项试验用相同的工作区和用户输入，系统部分省 216 token（2,042 至 1,826），工具部分省 194 token（6,326 至 6,132）。完整请求少 410 token。

旧执行循环的记忆提取把与已单独提供的用户请求和最终回复逐字相同的边界消息改为引用。引用只有在更短且来源完整时才使用，选择历史的预算和证据原文不变。ACP 事实日志路径已只传送这一问一答，分项捕获确认两次额外调用均为记忆提取，未触发历史引用。因此本次默认 ACP 节省的 410 个首请求 token 均来自系统规则与表单目录。

## 冻结任务复跑

macOS 27.2.1 arm64，Rust 1.97.1。沿用原 [两轮评测脚本](../2026-11-27-deepseek-harness-token-comparison/compare.py)，未改动提示、夹具、判分、模型和推理设置。9 次任务通过，48 次真实请求收到完整 DeepSeek usage。优化前 ACP SHA-256 `5cc7a4f4046ffe1023b0244721b380719cfeb4256512f701375afbfa567e6b3b`。优化后测量使用 ACP SHA-256 `102f752f6fadd833e475b2566e5926a2a0dcb913e6d9f6d3cf316a3aa833833c`。

| 每项任务中位数 | 优化前 Aster | 优化后 Aster | DSH `0.2.2-rc.5` |
| --- | ---: | ---: | ---: |
| 总 token | 35,780 | 34,140 | 24,977 |
| 首请求输入 token | 8,321 | 7,911 | 5,802 |
| 输入 token | 35,508 | 33,865 | 24,606 |
| 未命中缓存的输入 token | 1,206 | 1,225 | 951 |
| 输出 token | 275 | 278 | 371 |
| 模型请求 | 6 | 6 | 4 |
| 任务通过 | 3/3 | 3/3 | 3/3 |

本轮总 token 减少 1,640（4.58%），首请求减少 410（4.93%）。相对首轮优化前的 37,144 个 token，这一固定任务的两轮累计降幅约 8.09%。候选三次任务工具调用分别为 4、3、3 次。首个候选样本单独读取了两个文件，另两次使用批量读取。三边每次均完成两个回合，全部输出标记和原文件字节一致。

[冻结参数](manifest.json) · [9 次任务结果](trials.json) · [48 次厂商回执](provider-usage.jsonl) · [Aster 与 DSH 分项](aster-dsh-ablation.json) · [本轮前后分项](before-after-ablation.json) · [Schema 与记忆调用检查](wire-invariants.json)

## 独立验证

| 命令 | 结果 |
| --- | --- |
| `cargo build --locked -p aster-core -p aster-acp --lib --bin aster-acp` | 通过。 |
| `cargo test --locked -p aster-core --lib -- prompts:: token_optimization_measurements_report ask_user_form memory_extraction_runtime memory_retention_tests work_policy refusal_cycle sandbox_availability --test-threads=1 --nocapture` | 163 项通过。Core 常驻模板从 4,936 B 降至 4,326 B。测量中的完整工具目录从 16,458 B 降至 15,899 B，14 个基础工具的参数 Schema 仍为 8,111 B。 |
| `cargo test --locked -p aster-acp --lib -- engineering prompt_boundary stop_interrupts_unanswered_permissions_questions_and_forms --test-threads=4` | 15 项通过，3 项忽略。覆盖旧策略重开、权限问答和表单取消。 |
| `cargo test --locked -p aster-acp --test live_interactive_dialogue_real_llm -- real_agent_recovers_from_an_outside_workspace_refusal_in_the_same_turn --ignored --exact --nocapture --test-threads=1`，原 DeepSeek ACL | 1 项通过。拒绝发生在执行前，随后在同一个 Run 读取获准文件。 |
| `cargo test --locked -p aster-core --test test_control_loop_deepseek_e2e -- r1_deepseek_verified_write_completes r3_deepseek_write_goal_across_reopen --ignored --nocapture --test-threads=1`，原 DeepSeek ACL | 2 项通过，真实模型完成绑定核验写入和完成目标重开。 |
| `cargo build --locked -p aster -p aster-acp --bin aster --bin aster-acp` | 通过。新 TUI／ACP 的 `--build-id` 同为 `aster-cli-pair-v1:62924638a77dac4f7a813ea29e8cf8f347322b44:aarch64-apple-darwin`。 |
| `cargo test --locked -p aster-acp --test aster_tui_live_e2e -- forms_catalog::kimi_catalog_native_viewport_and_typed_values_match_source --ignored --exact --nocapture --test-threads=1`，本次构建的 CLI／ACP，`kimi/k3.1-pro` | 1 项通过。现有冻结夹具在真实 PTY 覆盖 13 种目录组件，提交的八个值及类型与独立断言完全匹配，表单文件字节保持原样。 |
| `cargo test --locked -p aster-acp --test shipped_tui_e2e -- shipped_native_form_submit_cancel_and_reject --exact --nocapture --test-threads=1`，本次构建的 CLI／ACP | 1 项通过。验证必填项、范围、提交和取消。 |
| `cargo clippy --locked -p aster-core --all-targets --no-deps --features ci-all,deep-live` | 通过，保留了已有警告。 |
| `cargo check --locked --workspace --all-targets` | 通过。依赖未来兼容性诊断涉及 `block` 0.1.6 和 `proc-macro-error2` 2.0.1。 |
| `cargo fmt --all -- --check` 和 `git diff --check` | 通过。 |

原生 Kimi 表单第一次启动时，创建会话的进程未取得测试凭据环境变量。复跑在测试 PTY 启动真实 CLI 时从已授权 ACL 注入该变量。保持原模型、提示、夹具、超时和判分，保留 [通过的原生表单断言](native-form.txt)。

## Explicitly not claimed

- 样本证明了表中这项任务及列出的独立场景。其他模型、长程任务、Windows 与 Linux 实机需要各自验证。
- 未命中缓存的输入中位数略有上升，服务端缓存状态受执行顺序和之前的请求影响。本次结论针对总 token，没有做账单估算。
