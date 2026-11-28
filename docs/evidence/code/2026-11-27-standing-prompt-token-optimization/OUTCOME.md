# Aster 常驻提示的 token 优化

首个真实 DeepSeek 请求的诊断显示，优化前的 Aster 和 DSH 工具定义分别为 20,547 B 和 20,433 B，两者接近。Aster 的 system 消息为 10,573 B，DSH 为 2,746 B。优化集中在每次主模型请求都会传入的提示。Core 合并运行规则、操作流程和工具用法之间重复的说明。新交互会话使用精简的 `engineering-v2`，保留工作范围、先检查后修改、复用、验证、审核角色、延后事项扫描及停止规则。此前命名的 `engineering-v1` 原文和摘要保持原样，已有会话恢复其原始绑定。无人值守仍可显式选择任一策略。

## 实测

macOS 27.2.1 arm64 和 Rust 1.97.1。使用原来的 [冻结任务与客户端](../2026-11-27-deepseek-harness-token-comparison/compare.py)，提示、夹具、标记、验收条件、运行顺序和模型参数均沿用原评测。优化前对照是包含跨工具恢复修复的本机 ACP，SHA-256 为 `e25dc9f28ab5c209c70217aae4f4a39fe842128bdf2d48e9218dba6eecb5ae7e`。候选 ACP 的 SHA-256 为 `d65d793690d277246086f58a58e6068eb49fa0a918492909ac3b14c8edf830c8`。独立 DSH 为 `0.2.2-rc.5`。9 次两轮任务全部通过，48 次真实调用都收到完整 `usage`。

| 每项两轮任务的中位数 | 优化前 Aster | 优化后 Aster | DSH |
| --- | ---: | ---: | ---: |
| 总 token | 37,144 | 35,772 | 25,020 |
| 输入 token | 36,874 | 35,500 | 24,641 |
| 输出 token | 270 | 272 | 379 |
| 未命中缓存的输入 token | 1,293 | 1,200 | 841 |
| 首次模型请求的输入 token | 8,664 | 8,322 | 5,804 |
| 模型请求 | 6 | 6 | 4 |
| 工具调用 | 3 | 3 | 4 |
| 任务通过 | 3/3 | 3/3 | 3/3 |

总 token 中位数少 1,372（3.69%），首次请求少 342 输入 token（3.95%）。三次首次请求均观察到相同的差值。另一次成对请求诊断使用相同工作区，Aster system 消息从 10,571 B 降至 9,093 B（13.98%）。26 项工具的名称、顺序、描述和参数 Schema 全部相同，非 system 消息也相同。Aster 两版均为四次带工具定义的请求和两次无工具定义的请求。按四个主模型请求的位置分别算输入 token 中位数，减少量依次为 342、342、345 和 345。

[冻结参数](manifest.json) · [9 次任务结果](trials.json) · [48 次 DeepSeek usage 回执](provider-usage.jsonl) · [首请求一致性检查](first-request.json)

复跑下面的命令时，使用测量前保留的 ACP 及同版本 DSH，并向驱动进程提供 `DEEPSEEK_API_KEY`。

```sh
python3 docs/evidence/code/2026-11-27-deepseek-harness-token-comparison/compare.py \
  --release /path/to/baseline-acp \
  --candidate target/debug/aster-acp \
  --dsh /path/to/dsh/node_modules/.bin/dsh \
  --embedding "$HOME/.local/bin/embedding" \
  --out /tmp/aster-standing-prompt-token-repeat \
  --order release,candidate,dsh candidate,dsh,release dsh,release,candidate
```

## 独立验证

| 检查 | 结果 |
| --- | --- |
| `cargo build --locked -p aster-core -p aster-acp --lib --bin aster-acp` | 成功。 |
| `cargo test --locked -p aster-core --lib -- prompts:: token_optimization_measurements_report cross_turn_repeat_seeding read_only_delivery work_policy refusal_cycle sandbox_availability --test-threads=4 --nocapture` | 95 项通过。Core 常驻提示从 5,247 B 降至 4,936 B。工具 Schema 与上一轮测量相同。 |
| `cargo test --locked -p aster-acp --lib -- engineering prompt_boundary reasoning_transport count_limit --test-threads=4` | 23 项通过，3 项忽略。其中验证了旧策略、新策略和显式不启用策略在重开后仍匹配各自摘要。 |
| `cargo test --locked -p aster-acp --test live_interactive_dialogue_real_llm -- real_agent_recovers_from_an_outside_workspace_refusal_in_the_same_turn real_agent_answers_then_approves_a_single_write_and_reopens --ignored --nocapture --test-threads=1`，DeepSeek Flash | 越界读取用例通过。三个模型回合，先拒绝越界读取，再成功读取获准文件，保留同一个 Run。另一项通过原问答和写入验证后，在重开后的完成门槛处失败。 |
| `cargo test --locked -p aster-acp --test live_interactive_dialogue_real_llm -- recovery::real_agent_answers_then_approves_a_single_write_and_reopens --ignored --exact --nocapture --test-threads=1`，分别通过 `ASTER_ACP_BIN` 指向优化前和优化后 | 优化前后均有一次回答、单次授权写入和重开后的原始读取。两次均在同一个已知完成门槛失败，错误绑定相同效果摘要，要求补上 `Passed` 核验。原用例继续采用冻结条件。 |
| `cargo test --locked -p aster-core --test test_control_loop_deepseek_e2e -- r1_deepseek_verified_write_completes r3_deepseek_write_goal_across_reopen --ignored --nocapture --test-threads=1`，同一份用户 ACL | 2 项通过，覆盖有绑定核验的真实写入，以及已核验 Goal 重开后保持 `Complete`。 |
| `cargo build --locked -p aster -p aster-acp --bin aster --bin aster-acp` | 通过。`target/debug/aster` 和 `target/debug/aster-acp` 的 `--build-id` 均为 `aster-cli-pair-v1:f1a68b675aba14cc6920a8f826e4d51f80cf1e2f:aarch64-apple-darwin`。 |
| 用上述二进制运行 `cargo test --locked -p aster --test tui_913_real_llm -- real_llm_tui_turn_shows_assistant_text --ignored --exact --nocapture --test-threads=1`，显式指定用户 ACL 和 `deepseek/deepseek-flash` | 1 项通过，真实 PTY 收到 `end_turn` 并显示最终回复。 |
| `cargo clippy --locked -p aster-core -p aster-acp --all-targets --no-deps --features aster-core/ci-all,aster-core/deep-live,aster-acp/headless-search` | 通过，保留了已有警告。 |
| `cargo check --locked --workspace --all-targets` | 通过。依赖未来兼容性诊断仍涉及 `block` 0.1.6 和 `proc-macro-error2` 2.0.1。 |
| `cargo fmt --all -- --check` | 通过。 |
| `python3.12 tests/workspace_dependencies.py` 和 `git diff --check` | 7 项依赖检查通过，diff 检查通过。 |

最初较宽的 Core 选择包含一个已有失败。`diagnostic_calls_keep_literal_arguments_and_runtime_refusals_in_model_history` 假设模型会收到唯一一个 `user` 消息。2026-11-26 预构建和本次构建分别单独运行该测试，都观察到原用户消息后有一条真实宿主恢复指令。本次保留了该测试，后续可让它分别验证原始用户内容和宿主恢复指令。

## Explicitly not claimed

- token 样本覆盖两轮读文件与命令任务。完整的长程编码、任意模型、默认思考档和其他平台需要各自的样本。
- 服务端缓存可以跨试跑与样本复用。报告里的总 token 包含缓存命中输入，不能直接当作账单。
