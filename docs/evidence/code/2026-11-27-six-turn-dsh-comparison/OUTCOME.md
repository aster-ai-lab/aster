# 六轮文件修订的独立 DSH 对照

用仓库的 `NativeFixture` 导出六轮指令、输入文件原字节和独立 JSON 判分。两边走各自的实际 ACP 进程，在同一个隔离工作区路径逐轮更换订单和政策。每次答复后检验七字段报告、全部四个受保护输入的字节及工具终态。Aster 另外要求自己的绑定核验。严格工具判定基于实际厂商回执，仅接收原生读、写、改及 Batch，不让搜索或目录列举代替任务约定的工具。独立 DSH 版本为 `0.2.2-rc.5`。

## 独立基线

每组运行一次，同一 DeepSeek Flash Chat Completions，关闭思考，temperature 0，每次最多输出 4,096，每组最多 40 次上游调用。完整目录和按需目录用同一份改动前 ACP，SHA-256 `0fc992f694d5b6be2052288ea5a5627e6ceef4a13a203c4cbbfa9891edbd3436`。三边共 102 份完整厂商回执。

| 首轮对照 | 文件与保护验证 | 严格工具判定 | 总 token | 未命中缓存输入 | 主请求 / 全部 |
| --- | ---: | ---: | ---: | ---: | ---: |
| Aster 完整目录 | 6/6 | 未通过，额外 `ls` 1 次 | 283,002 | 28,937 | 26 / 38 |
| Aster 按需目录 | 6/6 | 未通过，额外 `ls` 2 次、`search` 1 次 | 242,693 | 29,261 | 27 / 39 |
| 独立 DSH | 6/6 | 通过 | 269,129 | 7,715 | 25 / 25 |

Aster 每组都有 6 次宿主确认及 6 次绑定核验。DSH 在该隔离工作区直接提交了原生文件写入，ACP 没有发出权限询问。按需 Aster 用量低 9.82%，该样本额外调用只读工具，记录为严格判定未通过。用量按所有实际请求计入。 [原始判分](baseline/trials.json) · [102 份厂商回执](baseline/provider-usage.jsonl) · [冻结夹具](fixture.json)

## 稳定工具结果前缀

原实现遇到重复的完整工具结果时，把早期全文替换成指向后面结果的说明。前缀扩展的测试先生成只有第一次结果的真实模型输入，再追加多次观测。旧实现改变了已经发送的文本，断言失败。

Core 现在留存这一段相同观测中的第一份全文。新的成功观测仍带有本次调用的工具结果身份，保留命令退出码，并明确引用同参数的前一份完整结果。文本、参数或成功状态变化时发出全文。错误、图像和跨信任边界的观测保持原有表示。每次从实际保留的消息构建引用，压缩移除原目标后，第一次剩余观测重新发送全文。事实回执和验证只使用原始观测。

两组各两次重复读取 3,938 字节的 JSON。四次都实际执行两次命令，报告了所要求的精确值，8 个事实回执都保留原始文件字节和退出码 0。比较第二次读取前后两个实际主请求，旧版有 3/6 条历史消息逐字相同，新版有 6/6 条。该次验证输入的未命中缓存中位数由 2,129.5 降至 216.5，整项任务总量中位数由 36,120 变为 36,070。该次请求的原始输入 token 中位数由 9,297.5 变为 9,304.5。[四次判分](repeat/trials.json) · [前缀与原始回执校验](repeat-verified-prefix.json)

原两轮冻结任务再次 9/9 通过，Aster 新版按需、中间版本按需和独立 DSH 的总 token 中位数分别为 23,226、23,238 和 24,984。新版比独立 DSH 低 7.04%。[两轮判分与计量](frozen/trials.json)

## 六轮复跑

旧版和新版 Aster 都用按需目录，两次把测试顺序对调。每一组都包含实际 DSH。模型、输出上限、文件、保护断言和严格工具判定沿用首轮。新版评测 ACP SHA-256 为 `d45cc8e2c65bd93f2c9572e993ca7d6af3ce32836d2d4f4953526a2b49da94f1`。六次试验全数取得完整用量，合计 203 个上游响应。

| 六轮复跑，每组各两次 | 文件与保护验证 | 严格工具判定 | 总 token 中位数 | 未命中缓存输入中位数 | 主请求中位数 |
| --- | ---: | ---: | ---: | ---: | ---: |
| 旧版 Aster 按需目录 | 12/12 | 0/2 | 221,867 | 25,769 | 24.5 |
| 新版 Aster 按需目录 | 12/12 | 0/2 | 240,388 | 16,886.5 | 27 |
| 独立 DSH | 12/12 | 0/2 | 263,788.5 | 7,491 | 26 |

新版 Aster 总量比 DSH 低 8.87%。相对旧版 Aster，未命中缓存的输入少 34.47%，总量高 8.35%，同时主请求中位数多 2.5 次。新版 Aster 与旧版 Aster 均完成每轮绑定核验。两种 Aster 各两次提交额外的 2 次 `ls` 和 1 次 `search`。DSH 两次各提交额外的 1 次 `glob`。保留这些路线的严格失败，文件和保护判分继续独立核对。[顺序一](long-a/trials.json) · [顺序二](long-b/trials.json) · [汇总](long-summary.json)

## 验证命令

- `ASTER_LH_FIXTURE_EXPORT="$FIXTURE" cargo test --locked -p aster-acp --test long_horizon_contracts export_native_file_workflow_fixture -- --ignored`，导出测试成功。相同模块的两个隔离 oracle 测试通过。
- `python3 docs/evidence/code/2026-11-27-six-turn-dsh-comparison/revisions.py --fixture "$FIXTURE" --acp "$ACP" --reference-acp "$BASELINE_ACP" --dsh "$DSH" --out "$OUT" --order candidate release dsh --reference-deferred`，用 `DEEPSEEK_API_KEY` 提供凭据。首轮完整目录对照依照[首轮清单](baseline/manifest.json)使用未设置 `--reference-deferred` 的形式，复跑顺序二依照[清单](long-b/manifest.json)。
- `cargo test --locked -p aster-core --lib repeated_results_preserve_the_original_context_prefix`，修改前在上下文前缀相等的断言失败。
- `cargo test --locked -p aster-core --lib -- tool_history_tests cross_turn_repeat_seeding deferred_catalog --test-threads=2`，最终 35 项通过。
- `cargo build --locked -p aster -p aster-acp --bin aster --bin aster-acp`，完成本机构建。新配套用于 `boundary_e2e --skip e_bash --nocapture` 并通过 9 项。排除的 `e_bash` 在[已有证据](../2026-11-27-host-denial-and-pending-verification/OUTCOME.md)记录。
- `cargo check --locked --workspace --all-targets`、`cargo clippy --locked -p aster-core -p aster-acp --all-targets --no-deps` 和 `git diff --check` 通过。全工作区格式检查仍有 38 个已有差异文件。

## Explicitly not claimed

六轮的语义交付用量有独立 DSH 对照。严格工具判定在 Aster 两版和 DSH 的复跑中仍有失分。当前结果没有证明严格要求全部通过、长期总量相对 Aster 旧版下降、实际费用下降或其他模型与系统上的能力等价。按需工具目录仍为显式选择。
