# 连续重复读取的恢复与用量

已有的读取引用可以压缩重复字节。受控续接显示，即使输入已经说明读取范围完全可复用，模型仍可能换一个偏移继续读取同一文件。本次在原有历史投影里按完整工具轮次判断进展。只有连续两轮的整批已完成结果都能追溯到可见的原生文本读证据，而且分页或文件结束信息相同，才在第二轮附一次恢复提示。报告新的字节、读到不同的分页信息、混合执行其他工具或收到新用户输入时重新计数。模型仍能读取缺失的数据、复查文件改动或执行用户要求的再次读取。工具执行、权限、留存回执和核验均走原有路径。

## 重复读取后的受控续接

回放在同一冻结任务的首轮向两个配套提供同样的三轮工具动作，由真实原生 `read` 执行，共生成五份工具回执。两版发给 DeepSeek 的第一份续接主请求均引用两个已覆盖的读取范围。新版附有一条恢复提示，前版为零。其后的工具选择和输出均来自真实 DeepSeek Flash，继续使用原始权限与判分。脚本注入的三轮工具动作没有厂商 token，表格从真实请求开始计量。[回放清单](replay-a/manifest.json) · [实际工具序列与统计](summary.json)

| 顺序 | 配套 | 后续对 `order.json` 的读取 | 首轮实际 token | 累计实际 token | 正确产物 / 计划轮数 | 严格工具判定 |
| --- | --- | ---: | ---: | ---: | ---: | --- |
| 新版先跑 | 前一源码版 | 39 次 | 340,513 | 340,513 | 0/6，首轮触及上限 | 未过 |
| 新版先跑 | 当前源码版 | 0 次 | 19,217 | 158,820 | 6/6 | 通过 |
| 前版先跑 | 前一源码版 | 39 次 | 344,793 | 344,793 | 0/6，首轮触及上限 | 未过 |
| 前版先跑 | 当前源码版 | 0 次 | 25,767 | 168,963 | 6/6 | 通过 |

前版两次各用完 40 次上游请求，随后中转站拒绝 11 次请求。表中保留两次中途终止前已发生的厂商用量。新版两次分别收到 25 份和 26 份完整的厂商用量，均完成六轮宿主绑定核验。两组第一份续接主请求的输入用量均从 5,987 变为 6,028，相差 41 token。此回放检验从既定状态恢复，常规用量见下方自由运行。[顺序一判分与计量](replay-a/trials.json) · [顺序二判分与计量](replay-b/trials.json)

初版原型 `0592a2b7eeebff475fde51b2f9e3a6c78fc0c079` 两次均跨过订单重读。对调顺序的原型试验中前一源码版额外重读一次后也完成六轮，原型则因一次 `ls` 未过严格判分。最终提示明确沿用用户的工具限制，触发条件保持原样。最终受控回放两次均符合严格工具判定。[原型顺序一](prototype-replay-a/trials.json) · [原型顺序二](prototype-replay-b/trials.json)

## 自由运行对比

同一冻结六轮任务、独立文件判分、任务工具范围、宿主权限、超时、40 次上游调用上限和 4,096 输出 token 上限。三组均使用 DeepSeek Flash Chat Completions、关闭思考、temperature 0。Aster 两个版本均启用按需工具定义。独立 DSH 版本为 `0.2.2-rc.5`。两次对照反转了执行顺序，合计 159 份完整厂商用量回执。[冻结输入](../2026-11-27-six-turn-dsh-comparison/fixture.json) · [顺序一](long-a/trials.json) · [顺序二](long-b/trials.json)

| 六轮工作流，各两次 | 正确产物 | 宿主绑定核验 | 严格工具通过 | 总 token 中位数 | 未命中缓存输入中位数 | 主请求中位数 |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| Aster 前一源码版 | 12/12 | 12/12 | 0/2 | 181,783.5 | 14,729 | 21 |
| Aster 当前源码版 | 12/12 | 12/12 | 0/2 | 172,344 | 14,668 | 20 |
| 独立 DSH | 12/12 | — | 2/2 | 251,127 | 8,042.5 | 24.5 |

新版总量低于前一源码版 5.19%、低于独立 DSH 31.37%。六次试验的受保护文件均逐字节一致。Aster 两版的每次普通运行均额外调用 `ls` 一次，严格判分未通过。新版未命中缓存的输入比 DSH 多 82.38%。这是自由运行用量样本，不能将总量变化全部归因于恢复提示。[顺序一厂商回执](long-a/provider-usage.jsonl) · [顺序二厂商回执](long-b/provider-usage.jsonl) · [用量与判分汇总](summary.json)

## 核验命令

从已初始化 `apps/studio` 的根工作区运行。平台为 macOS 27.2.1、arm64，仓库固定 Rust 1.97.1。

- 先给 `read_projection_recognizes_covered_ranges_and_preserves_fresh_text` 加上第二次连续重复后必须有提示的断言，改动前在该断言失败。新用例 `covered_read_recovery_counts_complete_rounds_and_resets_for_new_information` 用真实 ReadTool 回执覆盖多工具一轮、混合的新数据、变化的分页边界、重新提问及只发一次提示。
- `cargo test --locked -p aster-core --lib -- fact_control::tool_history_tests tools::builtin::read:: refusal --skip diagnostic_calls_keep_literal_arguments_and_runtime_refusals_in_model_history --test-threads=4` 通过 137 项。未跳过时复现 1 项已有失败。前版测试二进制也在同一断言失败，测试现有的写入拒绝路径额外发了一条宿主恢复信息。
- 同一 Core 测试二进制过滤 `permission_` 和 `verification_` 分别通过 79 项和 64 项。
- `cargo build --locked -p aster -p aster-acp --bin aster --bin aster-acp` 成功。`target/debug` 的配套标识为 `aster-cli-pair-v1:b68a4669d07fc66fc06390813896eafe4c0b7cf9:aarch64-apple-darwin`。TUI 的 SHA-256 为 `c34e1b66eaaa694ca29057dbd0112f1be143960934c4baa6a6fbd13e56f5934b`，ACP 为 `4ca227c45d431fe84dee59ecf414a9a38212f2803b999a563bf9406534775c37`。前一源码版配套 `6a16b9ea9be83c85b10345b3227badb1c3a63667` 的 ACP SHA-256 为 `10ddb0bdc9c4d3a622425ee1b722123e5b6b35bffda4acf2d5b511076e25afc7`。
- 设置 `ASTER_TUI_BIN` 和 `ASTER_ACP_BIN` 指向当前构建，运行 `target/debug/deps/boundary_e2e-1770b1eb4e0797cc --skip e_bash --nocapture`，9 项通过。独立运行 `e_bash --exact` 仍复现已有沙箱用例失败，`side-effect.txt` 未获绑定核验。[宿主拒绝与终端追问复验](../2026-11-27-host-denial-and-pending-verification/OUTCOME.md)
- `cargo check --locked --workspace --all-targets`、`cargo clippy --locked -p aster-core --all-targets --no-deps --features ci-all,deep-live` 通过，保留已有警告。修改文件的 `rustfmt --edition 2021 --config skip_children=true --check` 和 `git diff --check` 通过。`cargo fmt --all -- --check` 仍报告 38 个已有格式差异文件。
- 受控续接使用 `python3 replay_reads.py --fixture "$FIXTURE" --candidate "$ACP" --reference "$REFERENCE_ACP" --embedding "$EMBEDDING" --out "$OUT" --order candidate release`，反序使用 `--order release candidate`。分别执行原型和最终版本的正、反序对照。
- 自由运行使用 `python3 ../2026-11-27-six-turn-dsh-comparison/revisions.py --fixture "$FIXTURE" --acp "$ACP" --reference-acp "$REFERENCE_ACP" --reference-deferred --dsh "$DSH" --embedding "$EMBEDDING" --out "$OUT" --order candidate release dsh`。反序采用 `--order dsh release candidate`。两条命令均从证据目录执行，使用现有 DeepSeek 凭据的环境变量。

## Explicitly not claimed

新回合仍可能使用任务范围以外的工具，严格工具判分未全面通过。未验证其他平台、其他模型或全部任务类型的能力等价。缓存未命中量高于 DSH，未比较实际费用。评测产物位于根工作区 `target/debug`，本机 PATH 保留已验收宿主拒绝恢复的配套。
