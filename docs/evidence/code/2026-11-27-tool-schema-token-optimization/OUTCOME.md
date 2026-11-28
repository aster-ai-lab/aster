# 工具 Schema 与常驻上下文的重复信息

Core 按 JSON Schema 的节点类型投影工具定义。属性名、定义名、常量、默认值、枚举数组及扩展值保持原样。去除 Schema 注解的旧规则仅处理确实承载 Schema 的节点，不再递归改写普通 JSON 数据。DeepSeek Chat Completions 的工具声明省去可由全字符串枚举或字符串常量推出的 `type: string`。异构枚举和非字符串类型保留显式类型。类型约束与枚举的交集由原始 Schema 在宿主执行前验证。参见 [JSON Schema 的枚举和常量](https://json-schema.org/understanding-json-schema/reference/generic) 和 [DeepSeek Chat Completions 工具接口](https://api-docs.deepseek.com/api/create-chat-completion/)。

ACP 由实时 search Schema 报告允许的搜索模式，常驻提示保留索引降级和语义检索停用的说明。完成条件归入会话操作指令，工具调用与思考预算各自报告数值约束。工作策略、能力授权、工具入口和验证仍由原有宿主路径执行。

## 真实模型用量

macOS 27.2.1 arm64，Rust 1.97.1。同一 DeepSeek Flash Chat Completions 端点，关闭思考，temperature=0，输出上限 1024。原 [冻结任务和判分](../2026-11-27-deepseek-harness-token-comparison/compare.py) 与 [独立的重复观测任务和判分](../2026-11-27-repeated-observation-token-optimization/repeat-holdout.py) 没有改动。每个参与组执行 3 次，各列为各自的中位数。用量记录中的 release 是上一轮本机构建。

| 场景 | 指标 | Aster 优化前 | Aster 优化后 | DSH 0.2.2-rc.5 |
| --- | --- | ---: | ---: | ---: |
| 原冻结任务 | 首个主请求输入 token | 7,911 | 7,749 | 5,802 |
| 原冻结任务 | 全程总 token | 34,156 | 33,502 | 25,008 |
| 原冻结任务 | 全程缓存未命中输入 token | 1,330 | 1,331 | 821 |
| 重复观测 | 首个主请求输入 token | 7,893 | 7,731 | — |
| 重复观测 | 全程总 token | 38,766 | 38,117 | — |
| 重复观测 | 全程缓存未命中输入 token | 1,239 | 1,219 | — |

冻结任务 9/9 通过，有 48 份完整用量。首请求减少 162 token（2.05%），全程减少 654 token（1.91%）。重复观测 6/6 通过，有 36 份完整用量，全程减少 649 token（1.67%）。12 次工具执行的事实回执字节与只读 JSON 夹具完全相同。新版在原冻结任务上仍比同期 DSH 多用 33.97% 的总 token。Aster 和 DSH 均有 4 次主请求，Aster 每次任务另有 2 次辅助记忆请求。

捕获的同组请求中，26 个工具的名称、顺序和文字说明一致。工具参数只有 19 处隐含字符串类型被省去。两组用户消息完全相同。首请求消融中，单独省去重复类型减少 114 输入 token，全部去掉字段说明可降低 1,368 token。字段说明包含调用所需的业务含义，因此在生产实现中完整保留。全部去掉说明、移动说明到工具摘要和移除空默认值仅用于诊断。见 [请求不变量](wire-invariants.json) 和 [消融厂商回执](ablation.json)。

- 冻结任务的 [参数](frozen/manifest.json)、[任务判分](frozen/trials.json)、[厂商回执](frozen/provider-usage.jsonl)。
- 重复观测的 [参数](repeated/manifest.json)、[任务判分](repeated/trials.json)、[厂商回执](repeated/provider-usage.jsonl)。
- [中位数](medians.json)。
- 实测 ACP SHA-256，优化前 `2d5949b74df10443f27ea531dc48e80fecda93cec61667ab0361f1ae8f488332`，优化后 `c09c399bfbde3733e0c419c267a77ed1cba6e675848c9e2fcd8c624316ba6aac`。

## 本地验证

实际用量对照使用单独保存的 ACP。随后更新了两项宿主状态测试，并从当前源码重建配对 CLI。最终配对二进制重新跑过 Kimi 原生 PTY 表单验证。

| 命令 | 结果 |
| --- | --- |
| `cargo build --locked -p aster-core -p aster-acp --lib --bin aster-acp` | 通过。 |
| `cargo build --locked -p aster -p aster-acp --bin aster --bin aster-acp` | 通过。CLI 与 ACP 的构建 ID 均为 `aster-cli-pair-v1:4af34850a6195cbfda66b261f672c8d8fd47986f:aarch64-apple-darwin`。 |
| `cargo test --locked -p aster-core --lib -- llm:: prompts:: tools::builtin::search fact_control::tool_history_tests --test-threads=4` | 817 项通过、8 项忽略。新测试验证了属性、默认对象、枚举、常量、引用定义名及不透明扩展。原始和紧凑 Schema 通过同一组可接受及应拒绝实例。 |
| `cargo test --locked -p aster-acp --lib -- engineering prompt_boundary retrieval_seam count_limit session_setup prompt_contracts stop_interrupts_unanswered_permissions_questions_and_forms user_facing --test-threads=4` | 40 项通过、3 项忽略。初跑有两项仍读取常驻文本中的旧搜索列表，更新后验证宿主检索状态。检索能力用例继续对实际工具 Schema 的全部模式和索引降级做精确断言。另单跑 `empty_registry_still_states_the_product_boundary`，1 项通过。 |
| `cargo test --locked -p aster-core --test test_harness_capabilities_live_e2e -- deepseek_flash_search_finds_the_workspace_token deepseek_flash_glob_finds_the_nested_fixture deepseek_flash_git_diff_returns_the_changed_token deepseek_flash_program_returns_the_script_marker --ignored --nocapture --test-threads=1`，`ASTER_TEST_MODEL=deepseek/deepseek-flash` | 4 项通过。对实际搜索、glob、Git diff 与 Program 工具结果进行断言。 |
| `cargo test --locked -p aster-core --test test_control_loop_deepseek_e2e -- r1_deepseek_verified_write_completes r3_deepseek_write_goal_across_reopen --ignored --nocapture --test-threads=1` | 2 项通过。真实 DeepSeek 完成核验写入，持久目标在重开后仍已完成。 |
| `cargo test --locked -p aster-acp --test live_interactive_dialogue_real_llm -- real_agent_recovers_from_an_outside_workspace_refusal_in_the_same_turn --ignored --exact --nocapture --test-threads=1`，`ASTER_LIVE_MODEL=deepseek/deepseek-flash` | 1 项通过。实测候选 ACP 在同一轮记录了一次越界拒绝，随后成功读取获准文件。 |
| `cargo test --locked -p aster-acp --test aster_tui_live_e2e -- forms_catalog::kimi_catalog_native_viewport_and_typed_values_match_source --ignored --exact --nocapture --test-threads=1` | 1 项通过，实际 Kimi。13 种表单组件和 8 个带类型提交字段正确，原输入文件字节一致。最终配对产物由同一个测试二进制复跑通过。[PTY 断言](native-form.txt) |
| `cargo clippy --locked -p aster-core --all-targets --no-deps --features ci-all,deep-live` | 通过。本轮 Core 改动位置没有新增诊断，保留原有告警。 |
| `cargo clippy --locked -p aster-acp --all-targets --no-deps --features headless-search` | 通过。保留既有告警。 |
| `cargo check --locked --workspace --all-targets` | 通过。保留既有告警，`block` 0.1.6 和 `proc-macro-error2` 2.0.1 报告未来兼容性诊断。 |
| `cargo fmt --all -- --check` 和 `git diff --check` | 通过。 |

## Explicitly not claimed

- 测得的 token 为厂商用量回执。缓存未命中在两组中方向不同，账单金额没有纳入比较。
- 实测范围等于列出的模型、固定任务、独立用例及 macOS 本机构建。其他模型、Windows/Linux 原生运行和 Studio GUI 没有纳入本轮验证。
