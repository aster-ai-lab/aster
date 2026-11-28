# 工具定义按需展开的能力核对

按需目录原先只发送工具说明的第一行，并截到 150 字节。字段说明超过 128 字节或横跨多行时也会从简表消失。真实表单试验暴露了丢失的约束。现在简表发送完整工具说明。当字段说明无法无损写入现有简表时，该工具保留原始定义，其他符合条件的工具仍可使用简表。未知调用仍须由原目录查找后再提交。表单校验为 `Stack.direction`、`Stack.gap`、`Select.options` 和初始值不匹配返回具体的字段诊断。默认使用完整定义，按需模式仍需显式启用。

使用同一份已捕获请求，只替换常驻文本中的相应段并重送给 DeepSeek。两次原始输入都是 7,204 token。工程策略试稿为 7,170，语言段试稿为 7,208，ACP 回合指引试稿为 7,188，组合试稿两次均为 7,158。这四种试稿均未启用。[试验请求身份与用量](prompt-probe/results.json)

## DeepSeek 实测

修复前的实验使用同一个配套 `53de35f9931bce6824847ad04147713a04d7628e`，通过配置切换目录。修复后的实验使用当前配套作为按需组，仍用前一配套作为完整目录对照。表单两组都使用当前配套。对照 ACP 的 SHA-256 是 `08c8165da44211983aec533b3ff9cf8f65a714d798b485f99f6a42c3a3ad718d`。任务沿用固定夹具、超时和判分，独立 DSH 为 `0.2.2-rc.5`。同用 DeepSeek Flash、temperature 0、关闭思考。最终共收到 431 条完整用量。[同配套对照](before/summary.json) · [最终对照](after/summary.json)

| 冻结场景，每组两次 | 当前按需 Aster | 前版完整目录 Aster | DSH |
| --- | ---: | ---: | ---: |
| 两轮总 token 中位数 | 25,659 | 31,072 | 25,141.5 |
| 两轮未命中输入中位数 | 3,312.5 | 3,916.5 | 910 |
| 两轮严格通过 | 2/2 | 2/2 | 2/2 |
| 六轮总 token 中位数 | 194,672.5 | 211,327.5 | 264,702 |
| 六轮未命中输入中位数 | 17,774.5 | 18,756.5 | 7,778 |
| 六轮严格通过 | 0/2 | 0/2 | 2/2 |
| 六轮正确产物与受保护输入 | 12/12 | 12/12 | 12/12 |
| 六轮绑定核验 | 12/12 | 12/12 | — |

两轮中当前按需消耗比完整目录对照低 17.42%，比 DSH 高 2.06%。六轮中分别低 7.88% 和 26.46%。六轮的两次 Aster 实验组和两次对照均因一次额外 `ls` 失去严格通过。完整的每轮产物、核验及工具统计见[正序](after/long-a/trials.json)和[反序](after/long-b/trials.json)。两轮数据见[短任务](after/short/trials.json)。

八项独立能力各运行两次。最终按需通过 16/16，完整目录对照通过 15/16，失分发生于一次精确编辑。分页、原生批读、MCP、精确编辑、获准程序、混合批次、目录发现和搜索的总 token 中位数分别少 18.33%、15.97%、8.42%、1.27%、18.00%、17.03%、17.71% 和 16.40%。[分场景用量及判分](after/summary.json)

首个表单夹具要求 `Select.options` 使用带独立标签和值的对象。共享表单协议接受互异字符串。保留该失败记录，另加一份符合协议的夹具，检查真实 ACP 宿主收到的 schema、字段绑定、图表和提交后的类型和值。旧按需组 0/2，当前按需组 2/2，总 token 中位数从 60,214 降为 20,105。完整目录在修复前后均为 1/2，两次试验组中位数分别为 41,763 和 45,940.5。中位数包含失败尝试消耗。[首个夹具](before/forms-prototype/trials.json) · [有效夹具修复前](before/forms-contract/trials.json) · [修复后](after/forms-contract/trials.json) · [实际拒绝诊断](after/forms-contract/refusals.json)

## 运行与验证

环境为 macOS 27.2.1 arm64、Rust 1.97.1，`apps/studio` 已初始化。DeepSeek 用量来自原 [native.py 与 full_catalog.py](../2026-11-27-model-selected-tool-catalog/OUTCOME.md) 和 [revisions.py](../2026-11-27-six-turn-dsh-comparison/OUTCOME.md)。每项能力使用 `full_catalog.py --default-catalog --scenario SCENARIO`，仅候选 ACP 的启动配置启用按需模式，精确编辑添加 `--disk-content-attestation`。两轮使用 `native.py --order candidate,release,dsh dsh,release,candidate`。六轮通过 `revisions.py --order candidate release dsh` 和 `--order dsh release candidate` 各一次。新增表单命令为 `python3 measure_form.py --fixture contract --acp "$ACP" --embedding "$EMBEDDING" --out "$OUT"`。

- `cargo test --locked -p aster-core --lib -- deferred_catalog tools::builtin::ask_user_form llm::openai --test-threads=4` 通过 80 项。覆盖完整说明保留、单工具定义回退、原始标识精确展开和混合批次拒绝。
- `cargo test --locked -p aster-form-core --lib` 通过 5 项。
- `cargo build --locked -p aster-core --lib` 与 `cargo build --locked -p aster -p aster-acp --bin aster --bin aster-acp` 均通过。根工作区配套 ID 为 `aster-cli-pair-v1:36c4eb37365eda9a6d1df639384c15d1688f189e:aarch64-apple-darwin`。`target/debug/aster`、`target/debug/aster-acp` 和 `target/debug/libaster_core.rlib` 的 SHA-256 分别为 `9c231d16cf3f8a1e91e21a66b851aea71d202d160d02d98edf90db6361ce08b5`、`eedae5a4cc2af35121e9e6de5d32882b0af420a1f2a28b68eee542c074762e4e`、`0977100dfc8e8914136c9b4bf04efb851e2da2bb644c6b9e5c9e6b0f45d2a976`。
- 用上述构建运行 `boundary_e2e --skip e_bash --nocapture`，9 项通过，排除 1 项[已有本机失败](../2026-11-27-host-denial-and-pending-verification/OUTCOME.md)。Kimi 真 PTY 用例 `forms_catalog::kimi_catalog_native_viewport_and_typed_values_match_source` 通过 1 项，覆盖 13 种共享组件、8 个精确类型值及正常退出，记录在 [Kimi 结果](kimi-native-oracles.txt)。
- `cargo fmt --all -- --check`、`cargo check --locked --workspace --all-targets`、`cargo clippy --locked -p aster-core --all-targets --no-deps --features ci-all,deep-live`、`cargo clippy --locked -p aster-form-core --all-targets --no-deps`、定向 `rustfmt` 和 `git diff --check` 通过。工作区检查和 Clippy 保留已有警告。`/opt/homebrew/bin/python3.12 tests/workspace_dependencies.py` 通过 7 项。

## Explicitly not claimed

这批结果没有证明所有能力在所有模型上等价，也没有证明短任务或未命中输入普遍低于 DSH。性能收益属于显式按需配置。六轮的严格工具判分仍有差距。系统 PATH 中的二进制、本机已运行的会话和其他平台均未用此构建替换或复验。
