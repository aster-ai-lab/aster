# 后台记忆提取的字段说明

同一构建分别运行完整目录和按需目录，并用原两轮任务与独立 DSH 比较。一次按需试验的四条主请求共 24,201 个输入 token，DSH 为 24,835。Aster 的两次后台提取增加了 997 个输入 token。主任务和记忆提取的输入、请求数与缓存命中分别从厂商回执计算。[请求分项](attribution/provider-usage.jsonl)

按用户报告的 8 小时 48 分钟运行边界，只读累计主日志里 558 条模型决定的 `prompt_tokens`，合计 228,282,101，中位数 386,154。原日志只有这些请求的输入计数，无法由此推算缓存未命中和费用。本轮优化处理后台提取，长会话的历史重发量须单独处理。[长回合与子任务证据](../2026-11-27-child-effect-closure-on-failure/OUTCOME.md)

Core 将原记忆提取模板中的长 JSON 样例与对应规则合并为逐字段说明。每个封闭类别的允许值仍逐字列出，并带类型与来源的对应关系。对未来会话的价值、语言规则、分数门槛、演化字段和排除规则均保留。提取仍按原路径调用，使用原始当轮证据、限额和关联记忆。原候选解析、脱敏、授权、写入和关联校验未改变。

第一版进一步压缩了类型与来源的合法集合。两条冻结任务的提议给出未授权分类，均正确拒绝写入。另有两条独立任务也用 `source=user`，触发原校验器。保留该版失败，将完整的封闭集合放回字段说明。第一版关系复跑还因为本地驱动传错参数而未取得数据。[第一版冻结提议](prototype/sources/trials.json) · [第一版独立判分](prototype/holdout/trials.json)

## 受控请求

用真实 ACP 捕获的两条短任务提取请求及另构造的五条合成证据。每对请求保持证据、模型和所有其他字段一致，正反交错发送 DeepSeek Flash，temperature 0，关闭思考。14 份 HTTP 200 回执用量齐全，七对的输入各少 40 token。两个普通任务和一条临时交接请求均得到空提取。四个正例在两版均通过类型、来源及内容判分。该探针对提议进行判分，实际持久化见下述 ACP 测试。[用量、摘要和原始提议](final/transport.json)

## 实际 ACP 与独立 DSH

实际 ACP 提取中，前版和当前版各通过 8/8。中文替换和英文冲突通过关闭、重启后读回并核对关系及独立记录。中文偏好与英文流程保留类型和来源。独立夹具覆盖西班牙语项目事实、中文故障预防，以及临时交接和合成秘密两种空提取。两版每项切换的主回答固定，提取由真实 DeepSeek Flash 完成，当前版在八个场景都少 40 个输入 token。实际缓存命中随顺序波动，与受控总输入差异分别记录。[持久关系](final/relations/trials.json) · [来源类型](final/sources/trials.json) · [独立夹具](final/holdout/trials.json)

两轮和六轮用原有冻结任务、工具范围、判分、权限和超时。相同的 DeepSeek Flash 模型、temperature 0、关闭思考，输出上限分别为 1,024 和 4,096。两轮分别比较完整与按需配置，两种配置中两版 Aster 都用相同模式。六轮两版 Aster 都用按需配置。独立 DSH 为 `0.2.2-rc.5`。每组两次，三方交错跑，共 228 条端到端请求，连同上述 ACP 提取测试共 244 条完全可对账的 HTTP 200 用量。[逐组计量与判分](final/summary.json)

| 两次的中位数 | 当前 Aster | 前版 Aster | DSH |
| --- | ---: | ---: | ---: |
| 两轮完整目录总 token | 31,180 | 31,231 | 25,205 |
| 两轮按需目录总 token | 25,379 | 25,465 | 25,175.5 |
| 两轮按需主请求输入 | 24,202 | 24,208 | 24,899 |
| 两轮按需提取输入 | 911.5 | 991.5 | — |
| 两轮按需未命中输入 | 3,609.5 | 1,135.5 | 1,027 |
| 六轮总 token | 191,770.5 | 187,927 | 262,591.5 |
| 六轮提取输入 | 5,282 | 6,123.5 | — |
| 六轮未命中输入 | 15,446 | 15,675.5 | 8,258.5 |
| 六轮严格通过 | 0/2 | 0/2 | 1/2 |

两轮的 12 次试验全部通过。按需总量比 DSH 高 0.81%，完整目录高 23.71%。按需各版的四条主请求、两条记忆提取均收到用量。两轮所有 Aster 配对的首条主请求系统、用户和工具目录字节计数一致。按需候选先跑的一次有 6,058 个未命中输入，后跑的一次是 1,161。[完整目录](final/short-default/trials.json) · [按需目录](final/short-deferred/trials.json) · [按需原始请求](final/short-deferred/provider-usage.jsonl)

六轮当前版总量比 DSH 低 26.97%，比前版高 2.05%。三方产物及输入完整性均为 12/12，两版 Aster 的宿主绑定核验也均为 12/12。前版和当前版均在第一轮的记忆提取发生前调用了附加 `ls`。反序当前版还多了一次 `ls` 和一次 `search`，同样发生于首次提取前。反序前版和当前版实际发出的首条模型系统及用户指纹相同，工具目录均为 14,541 字节。独立 DSH 一次因额外 `glob` 未通过。[六轮正序](final/long-a/trials.json) · [六轮反序](final/long-b/trials.json)

## 构建与检查

环境为 macOS 27.2.1 arm64、Rust 1.97.1，根工作区的 `apps/studio` 已初始化。前版 ACP SHA-256 为 `eedae5a4cc2af35121e9e6de5d32882b0af420a1f2a28b68eee542c074762e4e`。当前的配套 ID 为 `aster-cli-pair-v1:d4793d0b2f1ab1c781b76cc3d4e32f9e91fa22b1:aarch64-apple-darwin`。

- `cargo build --locked -p aster-core --lib` 与 `cargo build --locked -p aster -p aster-acp --bin aster --bin aster-acp` 通过。`target/debug/aster`、`target/debug/aster-acp`、`target/debug/libaster_core.rlib` 的 SHA-256 分别为 `be20fca497be09cd63a77de10d0939983ad857b1641d172beab360a0b9af79e7`、`dddd5bc546688f1ed5a2b42e1946599dc46e9e9f5927570ba32a999dc37dfe5a`、`45ef6f75c96ed9c42aa1ec2c34e2c1fd0817370555b01f59037bfbfb84b6f097`。
- `cargo test --locked -p aster-core --lib -- memory_extraction same_remember_bytes_require_valid_model_authority_in_both_stores --test-threads=4` 通过 69 项，覆盖原写入授权、异步提取、召回关系和双存储。用当前配套运行 `boundary_e2e --skip e_bash --nocapture`，9 项通过，排除 1 项[已知本机失败](../2026-11-27-host-denial-and-pending-verification/OUTCOME.md)。
- `cargo check --locked --workspace --all-targets` 与 `cargo clippy --locked -p aster-core --all-targets --no-deps --features ci-all,deep-live` 通过，留有已有警告。`rustfmt --edition 2021 --config skip_children=true --check crates/code/core/src/agent/memory_extraction_runtime.rs` 和 `git diff --check` 通过。`cargo fmt --all -- --check` 报告并行工作树的 `crates/code/core/src/sse.rs` 一处格式差异。该文件由前版构建之后的并行改动引入。`/opt/homebrew/bin/python3.12 tests/workspace_dependencies.py` 通过 7 项。
- 两轮用原 `compare.py` 与原 `native.py --reference-deferred`，两者均传 `--order candidate,release,dsh dsh,release,candidate`。六轮用原 `revisions.py --reference-deferred`，分别传 `--order candidate release dsh` 和 `--order dsh release candidate`。[两轮运行器](../2026-11-27-model-selected-tool-catalog/OUTCOME.md) · [六轮运行器](../2026-11-27-six-turn-dsh-comparison/OUTCOME.md)
- 中文替换和英文冲突复用原 `compare_relations.py --reference "$BASE_ACP" --candidate "$ACP" --seed-directory "$SYNTHETIC_SEED" --embedding "$EMBEDDING" --out "$OUT"`。来源类型用原 `measure_sources.py`，独立夹具为 `python3 measure_retention.py --reference "$BASE_ACP" --candidate "$ACP" --embedding "$EMBEDDING" --out "$OUT"`。[关系与种子](../2026-11-27-memory-relation-references/OUTCOME.md) · [来源判分](../2026-11-27-extractor-prefix-and-tool-scope/OUTCOME.md)

## Explicitly not claimed

厂商缓存命中随运行顺序变化，受控的 40 token 差异衡量总输入。两轮和六轮对照均有有限样本。具体结果不足以证明所有模型、任务和平台的能力等价。未替换本机 PATH 上的程序或已经运行的会话。
