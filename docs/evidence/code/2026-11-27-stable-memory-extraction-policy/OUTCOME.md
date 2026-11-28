# 记忆提取的固定规则与可变输入

上一轮的 159 份 DeepSeek 用量回执显示，Aster 两次六轮运行分别有六次后台记忆提取，贡献了 3,888 和 3,054 个未命中缓存输入 token。两次 Aster 的未命中中位数为 14,668，独立 DSH 为 8,042.5。提取约占这组差额的一半。主请求在跨轮移除旧召回和当轮核验说明时也有前缀变化。[上一轮回执](../2026-11-27-covered-read-recovery/OUTCOME.md)

Core 把固定的 JSON 形状、分类规则、证据排除规则、置信度门槛及演化字段约束放到同一条 system 消息。每次的条数上限、用户请求、回答、相关记录和转录由现有路径组装。有相关记录时才随附关系字段的用法，沿用原始措辞、候选 ID 和校验逻辑。提取通过原有授权端口逐次调用，使用相同的召回和裁剪预算，并按显式记忆规则、脱敏和写入契约处理。

## 改变记忆提取上限

`compare_limits.py` 用现有的 ACP 运行器、中文替换和英文冲突冻结任务、相同的四条种子记忆、独立落盘判分和 DeepSeek Flash。每个构建分别在两个配置上运行。下表比较在同一构建刚运行低上限后首次执行高上限的中文替换。第二组反转构建顺序。同一组每对请求里的四条相关记录和顺序完全相同。16 份提取请求均收到完整厂商用量。[对照清单、原判分及原始计量一](limits-a/) · [对照二](limits-b/) · [逐请求摘要](limits-summary.json)

| 上限切换 | 前版总输入 | 新版总输入 | 前版未命中 | 新版未命中 | 减少未命中 |
| --- | ---: | ---: | ---: | ---: | ---: |
| 3 → 10 | 750 | 750 | 750 | 494 | 256 |
| 4 → 9 | 750 | 750 | 750 | 494 | 256 |

共八次重启后关系判分，前版通过 6/8，新版 8/8。前版两次均在英文冲突任务中缺少所需的冲突关系。首次低上限请求的缓存温度不同，包括前版的一次 512 token 命中和新版的一次零命中。表格保留实际厂商计数，只比较两组高上限切换。

用既有同上限夹具测试空召回与四条召回切换，前版和新版各 4/4 正确落盘。空召回的输入均为 462，四条召回均为 750。不同组的实际缓存命中有差异，包含前版四条召回的 0 和 512，新版两次均为 384。中文偏好与英文流程各跑一次，两版均通过 2/2。提取出的来源与类型符合原判分。[同上限切换](same-limit-cross-recall/trials.json) · [偏好与流程](learning-types/trials.json)

## 六轮端到端 DeepSeek 对照

同一冻结六轮任务、工具范围、权限、超时、40 次上游调用上限和 4,096 输出 token 上限。均用官方 DeepSeek Flash Chat Completions、temperature 0、关闭思考，Aster 两版均使用按需工具定义。DSH 为独立的 `0.2.2-rc.5`。两次对照反转运行顺序，共 161 个 HTTP 200 响应，均取得完整用量。[正序判分](long-a/trials.json) · [反序判分](long-b/trials.json) · [逐阶段计量](long-summary.json)

| 六轮任务，各两次 | 总 token 中位数 | 未命中输入中位数 | 提取总 token 中位数 | 提取未命中输入中位数 | 正确产物 | 严格工具通过 |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| Aster 前一源码版 | 181,540.5 | 14,837.5 | 7,490.5 | 3,489 | 12/12 | 0/2 |
| Aster 当前源码版 | 172,076.5 | 14,080 | 6,902.5 | 3,163 | 12/12 | 0/2 |
| 独立 DSH | 254,332 | 8,109 | 0 次提取 | 0 次提取 | 12/12 | 1/2 |

所有受保护输入的字节符合独立判分，Aster 两版的 24 轮都获得宿主绑定核验。两版各六次提取和一次意图分类。每次 Aster 试验都多调用了一次 `ls`，DSH 一次多调用了 `glob`。新版总量比本轮前版低 5.21%，比 DSH 低 32.34%。普通运行的用量差异还包含模型工具选择和提取输出差异。固定规则的单次缓存收益由上面的受控切换直接测量。[正序原始用量](long-a/provider-usage.jsonl) · [反序原始用量](long-b/provider-usage.jsonl)

## 本地验证

在 macOS 27.2.1 arm64、Rust 1.97.1 的根工作区验证。`apps/studio` 已初始化。

- `cargo test --locked -p aster-core --lib extraction_policy_is_a_stable_system_message_across_limits_and_recall` 在改动前失败，固定的 JSON 形状跟在可变条数之后。改动后的同名测试核对固定规则、限额、空召回、相关记忆和转录的消息边界。
- `cargo test --locked -p aster-core --lib -- memory_extraction same_remember_bytes_require_valid_model_authority_in_both_stores --test-threads=4`，68 项通过。覆盖脱敏、有效模型授权、异步记忆提取、关系、回写和双存储。
- `cargo build --locked -p aster -p aster-acp --bin aster --bin aster-acp`，配套构建通过。以该配套运行 `boundary_e2e --skip e_bash --nocapture`，9 项通过。排除项为已有原生沙箱用例失败。[边界基线](../2026-11-27-host-denial-and-pending-verification/OUTCOME.md)
- `cargo check --locked --workspace --all-targets`、`cargo clippy --locked -p aster-core --all-targets --no-deps --features ci-all,deep-live`、定向 `rustfmt` 和 `git diff --check` 通过。Clippy 保留已有告警，未在本轮改动点报告新告警。全工作区格式检查报告已有的 38 个差异文件。`/opt/homebrew/bin/python3.12 tests/workspace_dependencies.py` 通过 7 项。
- 当前配套 ID 为 `aster-cli-pair-v1:ca0f36e9165fe9242fe195cb592ef84625047c7b:aarch64-apple-darwin`。DeepSeek 对照使用的 ACP SHA-256 为 `ccc870bb2ec0ce2cf0e831fbc53507bdaf8baef7f11b988c04330be261588cf5`。Kimi 之后重新执行配套构建，`target/debug/aster` 的 SHA-256 为 `43cf5f34cd9eb41cc4a6f4cee7259b2a2d508d1d62e095d774d5de864550dbdf`，ACP SHA-256 与 DeepSeek 对照一致。前版配套 `b68a4669d07fc66fc06390813896eafe4c0b7cf9` 的 ACP SHA-256 为 `4ca227c45d431fe84dee59ecf414a9a38212f2803b999a563bf9406534775c37`。
- 用 Python 3.12 从根工作区运行 `docs/evidence/code/2026-11-27-stable-memory-extraction-policy/compare_limits.py --reference "$REFERENCE_ACP" --candidate target/debug/aster-acp --embedding "$EMBEDDING" --seed-directory "$SYNTHETIC_SEED" --out "$OUT" --low 3 --high 10 --order reference candidate`。第二组为 `--low 4 --high 9 --order candidate reference`。两组均通过环境变量接收已有 DeepSeek 凭据。`$SYNTHETIC_SEED` 为[先前合成记忆评测](../2026-11-27-memory-relation-references/OUTCOME.md) 的 `seed` 输出目录。
- 同上限切换和独立记忆来源判分分别用原 `measure_cache.py --first-limit 5` 与 `measure_sources.py`。[已有运行器与参数](../2026-11-27-extractor-prefix-and-tool-scope/OUTCOME.md)
- 自由运行使用原 `revisions.py`、原 `fixture.json` 和 `--reference-deferred`，依次用 `--order candidate release dsh` 和 `--order dsh release candidate`。[冻结运行器与参数](../2026-11-27-six-turn-dsh-comparison/OUTCOME.md)
- `cargo test --locked -p aster-acp --test long_horizon_real_llm -- real_native_file_workflow_multiple_user_turns --ignored --exact --nocapture`，选择 `kimi-01/k3.1-pro`，通过 1 项整程测试。六轮的产物、输入保护及宿主字节核验均通过。24 次上游 HTTP 200 取得可对账的 168,017 token。该运行使用同一配套源码 ID 的 Cargo 测试构建，冻结 ACP SHA-256 为 `4e6538db15cb993339725b75c9f4b4e643cdf769c7583707375f9c9bfaff5e3f`。[Kimi 原始结果](kimi-result.json)

## Explicitly not claimed

这里只改变了后台提取的提示构造。主请求的跨轮缓存和严格工具约束仍需单独验证。未比较账单费用，也未据此推断所有模型、平台及任务的能力等价。
