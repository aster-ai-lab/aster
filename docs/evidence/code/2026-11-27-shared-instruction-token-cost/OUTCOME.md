# 共有指令与常驻输入的 token 成本

从当前工作树构建独立基线。对同一合成任务在 DeepSeek Flash 上捕获原生请求。Aster 和 DSH 均发出 4 次主请求，Aster 另有 2 次后台记忆提取。首轮分项诊断固定用户消息与接口参数，仅保留表中列出的输入。下表含两边相同的用户输入与协议开销。[分项诊断](first-request-ablation.json)

| 首请求分项 | Aster 基线输入 token | DSH 输入 token | 差值 |
| --- | ---: | ---: | ---: |
| 用户消息 | 43 | 43 | 0 |
| 用户消息与系统／环境说明 | 1,778 | 792 | 986 |
| 用户消息与工具定义 | 6,018 | 5,059 | 959 |
| 完整请求 | 7,751 | 5,806 | 1,945 |

Core 的运行契约集中描述权限、会话延续、证据与停止规则。工具工作流保留分页字段、检索与写入用法，工具执行和校验沿用原始 Schema。ACP 新会话使用较短的 `engineering-v3`，继续覆盖代码审查分工、可逆变更、验证、延后事项扫描和停止边界。恢复会话保留原策略内容及摘要，无人值守默认仍为 `engineering-v1`。记忆提取精简多语言与演进说明，原证据、置信阈值、结构、持久写入和回读路径均保留。

复核时发现旧的自定义回复样式替换逻辑依赖两份相同正文。默认正文改写后，旧模板匹配会失效。默认组装和自定义替换现从同一份模板读取，测试断言自定义样式只出现一次。重新拼接后的默认提示与直接参与评测的版本逐字节相同，SHA-256 为 `6a6ff5b7d874fc8e027687cb19deffb035d3e32b543b47c07dd02b07c27c0ecc`。

## DeepSeek Flash 实测

macOS 27.2.1 arm64，Rust 1.97.1。固定 Chat Completions、`deepseek-flash`、关闭思考、temperature=0 和最大输出 1024。冻结主评测交错运行基线、候选及独立 DSH 0.2.2-rc.5，各 3 次。独立重复观测各 3 次。下列评测输入、文件、参数、时限和判分沿用旧版本。统计每列自身的中位数。[原测量驱动](../2026-11-27-deepseek-harness-token-comparison/compare.py) · [重复观测驱动](../2026-11-27-repeated-observation-token-optimization/repeat-holdout.py)

| 场景 | 指标 | Aster 基线 | Aster 新版 | DSH |
| --- | --- | ---: | ---: | ---: |
| 冻结任务 | 首请求输入 token | 7,752 | 7,533 | 5,808 |
| 冻结任务 | 全程输入 token | 33,059 | 32,265 | 24,693 |
| 冻结任务 | 全程总 token | 33,328 | 32,606（−2.17%） | 25,087 |
| 冻结任务 | 全程缓存未命中输入 token | 1,190 | 1,289 | 791 |
| 冻结任务 | 主请求／全部请求 | 4／6 | 4／6 | 4／4 |
| 重复观测 | 全程总 token | 38,066 | 37,150（−2.41%） | — |
| 重复观测 | 全程缓存未命中输入 token | 1,212 | 1,264 | — |

主评测通过 9/9，重复观测通过 6/6。24 份 Aster 主请求的 26 项工具名称、顺序、说明和参数 Schema 全部一致。成对首请求的用户消息相同，system 消息从 8,271 降至 7,157 UTF-8 字节，3 组各节省 219 输入 token。新版在这项冻结任务上仍比独立 DSH 多 29.97% 的总 token。两项评测均保留缓存命中与未命中数据。[主评测判分](frozen/trials.json) · [用量](frozen/provider-usage.jsonl) · [参数](frozen/manifest.json) · [重复观测判分](repeated/trials.json) · [用量](repeated/provider-usage.jsonl) · [参数](repeated/manifest.json) · [请求不变量](wire-summary.json)

保持同一模型消息、输出 Schema 和原始问答，只替换记忆提取规则。从 558 降至 535 输入 token。实际会话的辅助输入还包括模型最终回复，主评测中的辅助输入中位数从 1,047 到 1,073。中文长期偏好的独立验证中，两版均完成回复，关停并重启 ACP 后各读回 1 条原始偏好。该场景各运行一次，全程 8,455 到 8,219 token。三组任务合计 17 次、88 份完整厂商用量。[辅助请求消融](memory-ablation.json) · [中文回读](memory/results.json) · [用量](memory/provider-usage.jsonl) · [汇总](summary.json)

基线 ACP SHA-256 为 `23007dd0c851e949c5c1c78a26b17f672dec3b7dcb651f43d946bc9f10ff7e94`，实测候选为 `f4d5231bd34880f2e6fc31768e14f39140077255cecad45bff4b2488b87c42c9`。复现时以 `DEEPSEEK_API_KEY` 给原驱动提供测试凭据。为记录请求形状，测量在同一驱动的私有临时副本中额外写出发送前的合成请求。这里仅保留计数与摘要。

```sh
python3 docs/evidence/code/2026-11-27-deepseek-harness-token-comparison/compare.py \
  --release /path/to/baseline-acp --candidate /path/to/candidate-acp \
  --dsh /path/to/dsh --embedding "$HOME/.local/bin/embedding" \
  --out /tmp/aster-compact-guidance-frozen \
  --order release,candidate,dsh candidate,dsh,release dsh,release,candidate
python3 docs/evidence/code/2026-11-27-repeated-observation-token-optimization/repeat-holdout.py \
  --release /path/to/baseline-acp --candidate /path/to/candidate-acp \
  --embedding "$HOME/.local/bin/embedding" --out /tmp/aster-compact-guidance-observations \
  --order release,candidate candidate,release release,candidate
```

## 本地验证

| 命令或场景 | 结果 |
| --- | --- |
| `cargo build --locked -p aster-core -p aster-acp --lib --bin aster-acp` | 通过。 |
| `cargo build --locked -p aster -p aster-acp --bin aster --bin aster-acp` | 通过，最终配对构建 ID 为 `aster-cli-pair-v1:b3e3484a4cf6a59cffbe65e4bea526c32dca06b7:aarch64-apple-darwin`。 |
| `cargo test --locked -p aster-core --lib -- prompts:: agent::memory_extraction_runtime memory_retention_tests fact_control:: agent_api::tests::read_only_delivery llm::text_tool_calls --test-threads=4` | 最终源码 409 项通过。初跑发现工具引导缺少 web 搜索和读取的明确入口，以及若干指令文字不符。恢复了入口和必要规则，自定义格式改为验证段落计数后复跑通过。 |
| `cargo test --locked -p aster-acp --lib -- engineering prompt_boundary reasoning_transport count_limit session_setup stop_interrupts_unanswered_permissions_questions_and_forms --test-threads=4` | 42 项通过，3 项按既有标记跳过。覆盖新版默认、v1 和 v2 的摘要回读、显式关闭、审查分工、表单与许可取消。 |
| `cargo clippy --locked -p aster-core -p aster-acp --all-targets --no-deps --features aster-core/ci-all,aster-core/deep-live,aster-acp/headless-search` | 通过。复核了变更文件中的诊断，均指向现有代码的其他位置。 |
| `cargo check --locked --workspace --all-targets` | 通过，保留已有警告。 |
| `real_agent_recovers_from_an_outside_workspace_refusal_in_the_same_turn` | 配置 DeepSeek Flash 并运行候选 ACP，1 项通过。先拒绝越界读取，再在同轮成功读取已授权内容。 |
| `cargo test --locked -p aster-core --test test_control_loop_deepseek_e2e -- r1_deepseek_verified_write_completes r3_deepseek_write_goal_across_reopen --ignored --nocapture --test-threads=1` | 真实 DeepSeek Flash，2 项通过。覆盖完成前的绑定核验和重开后仍为 Complete。 |
| `cargo test --locked -p aster-acp --test aster_tui_live_e2e -- forms_catalog::kimi_catalog_native_viewport_and_typed_values_match_source --ignored --exact --nocapture --test-threads=1` | 最终配对二进制通过 1 项真实 Kimi 原生 PTY 实测。13 种组件、8 个带类型字段、原文件字节均通过。初次启动的测试进程清理了所需的凭据环境，复跑在 TUI 启动包装器中注入同一个模型的既有环境变量。[PTY 断言](native-kimi.txt) |
| `/opt/homebrew/bin/python3.12 tests/workspace_dependencies.py` 和 `git diff --check` | 7 项依赖检查通过，diff 检查通过。 |
| `cargo fmt --all -- --check` | 工作区检查报告 36 个 ACP 文件的既有格式差异。复核本轮编辑位置，保留其他进行中的工作。 |

## Explicitly not claimed

- 厂商总 token 包括缓存命中。两组任务的缓存未命中输入中位数上升。本次未测账单。
- 通过记录限于列出的任务、模型和 macOS arm64。Windows／Linux 原生及 Studio GUI 不在本轮验证范围内。
