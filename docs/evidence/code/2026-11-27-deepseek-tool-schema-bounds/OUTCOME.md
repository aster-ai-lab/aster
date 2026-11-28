# DeepSeek 工具参数与重复观测的输入成本

对同一份合成输入，Chat Completions、Messages 和 Responses 的完整工具目录都计为 7,533 输入 token。Messages 的值是当次输入与缓存读取之和。去掉所有参数说明、工具说明或数值边界只用于诊断。保留边界值并精简表示的组合消融，从 7,533 降至 7,212。该临时变换用于找到开销来源。生产使用下面的受限变换。[分项用量](ablation.json) · [组合消融](compact-diagnostic.json) · [DeepSeek 工具调用文档](https://api-docs.deepseek.com/guides/tool_calls/)

普通 DeepSeek Chat Completions 工具请求保留工具名、说明、参数名、必填字段、各参数类型、嵌套类型、默认值及枚举。根节点存在命名字段时省略重复的对象标记。整数表示的上下限合并到字段说明，继续携带数值和字符、项数单位。含小数的范围保留为机器可读字段。结构化输出保留原有投影，所有工具实际调用继续由宿主的原始 Schema 执行校验。

初版的冻结任务 9/9 通过。独立重复观测中，候选 2 次在重新成功执行命令后又尝试调用工具，额外调用按冻结规则拒绝，最终缺少应交付值。原结果和判分已保留。成功会话的四种投影复放均完成，因此复放本身无法证明某句引用文字造成了最初失败。实现把同工具、同参数、相同字节的成功观测的完整正文留在最后一次，较早副本引用下方同一调用的全文。当前模型决策直接收到最新正文，重复字节只出现一次。事实日志、状态差异、变更内容、不同参数和图像仍沿用原路径。[初版独立判分](initial-repeated/trials.json) · [初版用量](initial-repeated/provider-usage.jsonl) · [隔离复放](observation-replay.json)

## 正式评测

macOS 27.2.1 arm64，Rust 1.97.1。官方 DeepSeek Flash Chat Completions，关闭思考，temperature=0，输出上限 1024。沿用冻结的文件、提示、授权、超时和判分。原任务的本地基线、最终候选与独立 DSH 0.2.2-rc.5 各交错执行 3 次，重复观测的两版各执行 3 次。各列取独立中位数。[冻结驱动](../2026-11-27-deepseek-harness-token-comparison/compare.py) · [重复观测驱动](../2026-11-27-repeated-observation-token-optimization/repeat-holdout.py)

| 场景 | 指标 | Aster 基线 | Aster 最终版 | DSH |
| --- | --- | ---: | ---: | ---: |
| 冻结任务 | 首请求输入 token | 7,533 | 7,258（−3.65%） | 5,808 |
| 冻结任务 | 全程总 token | 32,460 | 31,546（−2.82%） | 25,097 |
| 冻结任务 | 缓存未命中输入 token | 1,200 | 1,199 | 765 |
| 冻结任务 | 主请求／全部请求 | 4／6 | 4／6 | 4／4 |
| 重复观测 | 全程总 token | 37,144 | 36,775（−0.99%） | — |
| 重复观测 | 缓存未命中输入 token | 1,261 | 3,687 | — |

冻结主评测 9/9，重复观测 6/6，共 84 份完整厂商用量。12 个执行回执原样保留 3,938 字节的 JSON。新旧两版均只向模型投影一份全文。新版本的全文位于最新工具结果。早期副本替换后，原历史前缀与上一请求不同，这与重复观测中的缓存未命中增长相符。总 token 中位数在主评测仍比 DSH 高 25.70%。[主评测判分](frozen/trials.json) · [用量](frozen/provider-usage.jsonl) · [参数](frozen/manifest.json) · [重复观测判分](repeated/trials.json) · [用量](repeated/provider-usage.jsonl) · [参数](repeated/manifest.json) · [请求及回执校验](repeated/observations.json) · [中位数](summary.json)

捕获的 3 个首请求对中，消息、工具名和顺序完全一致。每份目录有 26 个根对象标记和 55 个范围节点得到精简。并行开发中已有的 `kill_task` 说明在基线二进制与当前树间存在差异，唯一不同的工具摘要是这项说明。固定为当前说明、只替换 Schema 的隔离复放得到 7,579 到 7,258 输入 token。直接比较两份完整构建时净减少 275。另有 5 个独立参数案例各对两版执行一次，10/10 通过。覆盖分页、嵌套列表参数、数值上限和 BM25 的模式特例。探针检查模型生成参数，真实工具执行由正式评测和下面的能力测试覆盖。[工具目录校验](wire-summary.json) · [实际 Schema 隔离](actual-wire-schema-replay.json) · [参数探针](tool-argument-probes.json)

- 上轮本地基线 ACP SHA-256 `b86f76d91307b76d0254f4b74f291f6cc76b32ae2cf54a39fd793d4b8a86f58b`。
- 首轮只改 Schema 的候选 ACP SHA-256 `1acf98c77f7eff5daa45f6be478f31d019d778e85081439e564dd20d55b6745c`。
- 正式最终候选 ACP SHA-256 `e627e974b2286e66f7b32df88daf434f64d54acb9fee1374c64e371789dccc22`。

在已授权测试环境提供 `DEEPSEEK_API_KEY` 来复跑。诊断复制现有驱动，只在临时目录记录合成请求形状，正式判分使用原有逻辑。

```sh
python3 docs/evidence/code/2026-11-27-deepseek-harness-token-comparison/compare.py \
  --release /path/to/baseline-acp --candidate /path/to/candidate-acp \
  --dsh /path/to/dsh --embedding "$HOME/.local/bin/embedding" \
  --out /tmp/aster-compact-schema-frozen \
  --order release,candidate,dsh candidate,dsh,release dsh,release,candidate
python3 docs/evidence/code/2026-11-27-repeated-observation-token-optimization/repeat-holdout.py \
  --release /path/to/baseline-acp --candidate /path/to/candidate-acp \
  --embedding "$HOME/.local/bin/embedding" --out /tmp/aster-compact-schema-repeat \
  --order release,candidate candidate,release release,candidate
```

## 本地验证

| 命令或场景 | 结果 |
| --- | --- |
| `cargo build --locked -p aster-core -p aster-acp --lib --bin aster-acp` | 最终源通过。 |
| `cargo build --locked -p aster -p aster-acp --bin aster --bin aster-acp` | 最终 CLI 和 ACP 构建 ID 均为 `aster-cli-pair-v1:7c25e0de18a78686842d42cdeff637f592373b56:aarch64-apple-darwin`。 |
| `cargo test --locked -p aster-core --lib -- llm:: prompts:: fact_control:: tool_history_tests --test-threads=4` | 最终 1,030 项通过，8 项按既有标记忽略。覆盖普通与结构化调用的投影、嵌套元数据、最新正文、不同内容和状态、保留后缀。初跑的两个新测试夹具漏填 `validation_schema` 字段，补上真实结构化契约后通过。 |
| `cargo clippy --locked -p aster-core --all-targets --no-deps --features ci-all,deep-live` | 最终通过。读取了改动路径的候选告警，未见本轮新增项。 |
| `cargo check --locked --workspace --all-targets` | 最终通过。保留已存在的警告。 |
| `cargo test --locked -p aster-core --test test_harness_capabilities_live_e2e -- deepseek_flash_search_finds_the_workspace_token deepseek_flash_glob_finds_the_nested_fixture deepseek_flash_git_diff_returns_the_changed_token deepseek_flash_program_returns_the_script_marker deepseek_flash_ask_user_returns_the_host_answer deepseek_flash_batch_reads_two_fixture_files --ignored --nocapture --test-threads=1` | Schema 优化已启用的版本上通过 6 项真实 DeepSeek Flash 能力。 |
| `cargo test --locked -p aster-core --test test_control_loop_deepseek_e2e -- r1_deepseek_verified_write_completes r3_deepseek_write_goal_across_reopen --ignored --nocapture --test-threads=1` | Schema 优化已启用的版本上通过 2 项真实 DeepSeek Flash 核验与恢复。 |
| `cargo test --locked -p aster-acp --test live_interactive_dialogue_real_llm -- real_agent_recovers_from_an_outside_workspace_refusal_in_the_same_turn --ignored --exact --nocapture --test-threads=1` | 实际 DeepSeek Flash 和最终 ACP，1 项通过。先拒绝越界读取，再在同一轮完成已授权读取。 |
| `cargo test --locked -p aster-acp --test aster_tui_live_e2e -- forms_catalog::kimi_catalog_native_viewport_and_typed_values_match_source --ignored --exact --nocapture --test-threads=1` | 最终配对通过 1 项真实 Kimi 原生 PTY 验证。13 种组件、8 个带类型字段和输入文件字节均通过。[PTY 断言](native-kimi.txt) |
| `/opt/homebrew/bin/python3.12 tests/workspace_dependencies.py` 和 `git diff --check` | 7 项依赖检查通过，diff 检查通过。 |
| `cargo fmt --all -- --check` | 报告工作区其他 38 个文件的格式差异。此次编辑的五个 Rust 文件单独检查通过。 |

## Explicitly not claimed

- 总 token 包含缓存命中。重复观测的缓存未命中中位数上升。未测账单金额。
- 兼容性结果限于列出的测试、模型和 macOS arm64。Windows、Linux 原生以及 Studio GUI 没有运行。
