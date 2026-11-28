# 记忆提取的分数表示与召回试验

前次六轮试验里，当前 Aster 两次各发出六份记忆提取请求，其中未命中输入为 3,612 和 2,880 token。沿实际 ACP 请求检查发现，相关记忆的 `f32` 分数进入通用 JSON Value 时被拓宽为 `f64`，例如 `0.92` 写成 `0.9200000166893005`。同一个二进制分数的多余十进制位随每份记忆提取输入传送。[前次六轮计量](../2026-11-28-read-line-number-projection/OUTCOME.md#六轮独立-dsh-对照)

Core 将现有相关记忆字段直接用类型化的 JSON 行序列化。分数以能还原原 `f32` 的最短表示输出。选取顺序、条数上限、字符预算、类型、标签、源、关系字段、短引用的授权和记忆写入契约沿用原路径。字符预算仍按实际 JSON 计算，输出中的数字缩短时可以在限额内保留更多正文。

## 固定模型请求的格式对照

使用真实 ACP 捕获中文替换和英文冲突的现有提取请求。对每组四条合成相关记忆，仅替换 `importance` 的十进制写法，逐值验证可往返到相同 `f32`。每组正反交错提交官方 DeepSeek Flash。每对使用相同的测试缓存前缀标记、原证据和工具定义，输出上限 1,024，temperature 0，关闭思考。8 份 HTTP 200 回执均有完整用量。

| 对照组 | 前版输入 token | 最短表示输入 token | 节省 | 前版 / 最短表示关系提议 |
| --- | ---: | ---: | ---: | ---: |
| 一，两个任务 | 766 | 746 | 每次 20 | 2/2、2/2 |
| 二，两个任务 | 767 | 747 | 每次 20 | 1/2、1/2 |

两组每个请求均命中 384 个缓存 token，因此未命中输入也逐次下降 20。第二组的英文题在两种格式都遗漏冲突关系。格式探针只判模型提议，持久化另用原有 ACP 判分。[输入与判分](score-transport/trials.json) · [逐请求用量](score-transport/provider-usage.jsonl)

## 实际 ACP 记忆关系与写入

沿用中文替换、英文冲突的冻结提示和重启后的独立判分。四条合成种子保持一致。每组固定同一个提取条数上限，分别使用 9 和 10，并对调构建顺序。两个上限的四对请求都从 750 降到 730 输入 token，各少 56 字节。合计 16 个分数逐个还原到同样的 `f32` 位模式。每对请求的召回行和数字以外的字段逐项相同。

前一源码版通过 2/4，新版通过 4/4。前版中文替换两次均成功，英文冲突两次均缺少冲突关系。新版两类任务均能从新 ACP 读取正确的更新和结构化关系。当前这组实际顺序中，前版每次缓存命中 512，新版每次命中 384，因此新版未命中输入由 238 变为 346。固定同缓存条件的输入节省由格式探针单独验证。[正序落盘](relations-a/trials.json) · [反序落盘](relations-b/trials.json) · [反序构建映射](relations-b-arm-map.json) · [分数字节核对](relation-summary.json)

独立的中文个人偏好和英文流程题两版均为 2/2。实际两类请求分别有 449、456 个输入 token，两版计数相同。这两个场景使用空召回。[持久类型判分](sources/trials.json)

## 默认配置的两轮 DSH 对照

复用原两轮任务和判分器。同一 DeepSeek Flash，Aster 两版均通过默认工具目录，独立 DSH 为 `0.2.2-rc.5`。正反顺序共 32 个完整上游用量回执。

| 版本，各两次 | 总 token 中位数 | 未命中输入中位数 | 每次上游请求 | 任务通过 |
| --- | ---: | ---: | ---: | ---: |
| Aster 前一源码版 | 31,597 | 1,221.5 | 6 | 2/2 |
| Aster 当前源码版 | 31,684 | 4,167 | 6 | 2/2 |
| 独立 DSH | 25,109.5 | 904 | 4 | 2/2 |

当前版默认目录比 DSH 多 26.18% 总 token。每次 Aster 整程各有两次额外记忆提取，当前两次分别用了 1,057 和 1,053 个提取总 token。这个数量只能解释总量差距的一部分。DSH 两次均有四个主请求。[冻结判分](short/trials.json) · [逐次用量](short/provider-usage.jsonl) · [中位数](short-summary.json)

两边各四个主请求。当前 Aster 的主请求总量中位数 30,629，DSH 为 25,109.5。六次试验里两边的首个输入分别稳定在 7,317 与 5,862 token。另捕获同一冻结任务的真实首请求，分离系统与环境消息、工具目录并保持用户输入逐字相同。用相同官方模型以 `max_tokens=1` 单独计量得到下表。完整重放中两版分别多 1 和 2 个输入 token，分项差额仍与完整重放差额一致。[逐次用量](short/provider-usage.jsonl) · [7 份分项计量](main-input-ablation.json)

| 首请求分项，均含共同用户输入和协议开销 | Aster 输入 | DSH 输入 | 差值 |
| --- | ---: | ---: | ---: |
| 仅用户输入 | 43 | 43 | 0 |
| 系统、环境及用户输入 | 1,620 | 850 | 770 |
| 工具目录及用户输入 | 5,743 | 5,059 | 684 |
| 完整重放 | 7,318 | 5,864 | 1,454 |

## 按需目录的六轮 DSH 对照

沿用原六轮夹具、判分、超时和工具权限。两个 Aster 构建均开启原生按需目录。反转试验顺序，各跑两次。官方 DeepSeek Flash 使用 temperature 0、关闭思考、输出上限 4,096。162 份 HTTP 200 回执均包含可对账用量。

| 版本，各两次 | 总 token 中位数 | 未命中输入中位数 | 正确产物 | 绑定核验 | 严格工具通过 |
| --- | ---: | ---: | ---: | ---: | ---: |
| Aster 前一源码版 | 181,701 | 15,953.5 | 12/12 | 12/12 | 0/2 |
| Aster 当前源码版 | 177,901 | 14,876.5 | 12/12 | 12/12 | 0/2 |
| 独立 DSH | 267,191.5 | 8,132 | 12/12 | — | 1/2 |

当前版六轮总量比 DSH 低 33.42%，比前一源码版低 2.09%。两版 Aster 每次各多一个 `ls`，DSH 反序多一个 `glob`。Aster 两版每次各发六份记忆提取。前版的两次提取总量分别为 7,851 和 7,628，当前版分别为 7,560 和 7,497。长程数据同时含模型路线、缓存及输出差异。对分数表示的直接因果判断来自上面的配对试验。[正序判分](long-a/trials.json) · [反序判分](long-b/trials.json) · [长程用量按请求阶段汇总](long-summary.json)

## 撤回召回前置原型

尝试把相关记忆放到当轮用户证据前面，让输入前缀能跨轮复用。实际两个任务的相关记忆前两条相同，后两条排序交换。先导和另两组隔离探针在这种部分重复上未复现缓存收益。隔离探针固定全部四条后，第二次请求多命中 128 token，两种布局总输入相同。[部分重复探针](recall-first-prototype/isolated-natural-recall/trials.json) · [完全重复探针](recall-first-prototype/isolated-identical-recall/trials.json)

按原有判分通过实际 ACP 重新打开合成记忆，原实现通过 4/4，前置版 3/4。前置版丢失了一个英文冲突关系。前置版四次的实测未命中输入均为 366，原版均为 238。两种类型的独立空召回能力试验两版均为 2/2。原型配套 `b9031e3904461996043c6a8db1ea585b8ceba080` 的 Core 聚焦测试通过 69 项，PTY 通过 9 项。六轮父运行器因测试路径未解析成规范路径而退出，它启动的长任务仅留下部分已判分步骤和用量，完整的两组关系试验随后以规范路径复跑。召回前置的 Core 与评测读取器改动均已撤回。[关系正序](recall-first-prototype/relations-a/trials.json) · [关系反序](recall-first-prototype/relations-b/trials.json) · [反序构建映射](recall-first-prototype/relations-b-arm-map.json) · [原型部分六轮正序](recall-first-prototype/long-a/trials.json) · [原型部分六轮反序](recall-first-prototype/long-b/trials.json)

## 构建与验证

macOS 27.2.1 arm64，仓库 Rust 1.97.1，根工作区 `apps/studio` 已初始化。

- `cargo build --locked -p aster -p aster-acp --bin aster --bin aster-acp` 通过。厂商用量对应的配套 ID 为 `aster-cli-pair-v1:6e6a22152f215031cc75a8c917009656429f3ce9:aarch64-apple-darwin`。该配套的 TUI SHA-256 为 `40aa1cfb2a6b81b80ac2dd84406988288abfdcd3499ea31e5c1f52651661e043`，ACP 为 `181466653c4ee73d8356494edbee39ff6d139348aaaa0c3ab3df4e9aea9d27e0`。前一源码版 ACP SHA-256 为 `c7ee1c1f4bafff375516f52d5e0b6c87e58c1a8fcefec49ac7b00b25d87369a2`。
- 保留工作区随后对渲染辅助函数的并行整理，并再次执行同一构建命令。当前 `target/debug` 的配套为 `aster-cli-pair-v1:2d579114ec4254f41d94e974ef67a0f88c1b7863:aarch64-apple-darwin`，TUI SHA-256 为 `e8b6beb72db8b7d0793e9e9da5cab3536a1970af8af70ace31d0ccf13f1bc78c`，ACP 为 `48c9407ad2d99066258efec824ee1defd516aab749547cb8b973a1d033645984`。以两个实际 ACP 分别复放中文替换和英文冲突，捕获的两对完整模型请求逐项相同。[请求等价回执](local-pair-extraction-equivalence.json)
- `cargo build --locked -p aster-core --lib` 通过。独立 SDK 产物 `target/debug/libaster_core.rlib` 的 SHA-256 为 `ca9390059d87f3c85a97390e4e8777b736c8562cbef945b3e1820256dd0e20c2`。
- `cargo test --locked -p aster-core --lib -- memory_extraction same_remember_bytes_require_valid_model_authority_in_both_stores --test-threads=4` 通过 69 项。新增读取断言验证分数及相邻的可表示值都可按位还原。既有测试覆盖跨存储保留、关系权限、敏感值过滤和带截断的 JSON 行。
- 计量配套与最终配套分别运行 `boundary_e2e --skip e_bash --nocapture`，各通过 9 项。排除项是已有的本机原生沙箱失败。[边界基线](../2026-11-27-host-denial-and-pending-verification/OUTCOME.md)
- `cargo check --locked --workspace --all-targets` 和 `cargo clippy --locked -p aster-core --all-targets --no-deps --features ci-all,deep-live` 通过，保留已有告警，改动文件内无 Clippy 候选。`cargo fmt --all -- --check` 的 42 个差异文件与改动前相同，两处 Rust 修改的定向 `rustfmt --edition 2021 --config skip_children=true --check`、`git diff --check` 通过。依赖边界 `python3.12 tests/workspace_dependencies.py` 通过 7 项。
- 关系评测用原 `compare_relations.py --reference "$REFERENCE_ACP" --candidate "$ACP" --embedding "$EMBEDDING" --seed-directory "$SYNTHETIC_SEED" --out "$OUT"`。使用原 `measure_cache.py` 的 `configure` 设置每组同一个合法条数限额。反序的二进制映射另存。空召回类型用原 `measure_sources.py`。[关系运行器](../2026-11-27-memory-relation-references/compare_relations.py) · [类型运行器](../2026-11-27-extractor-prefix-and-tool-scope/measure_sources.py)
- 两轮对照执行原 `compare.py --release "$REFERENCE_ACP" --candidate "$ACP" --dsh "$DSH" --embedding "$EMBEDDING" --order candidate,release,dsh dsh,release,candidate --out "$OUT"`。六轮执行原 `revisions.py --reference-deferred`，分别用 `--order candidate release dsh` 和 `--order dsh release candidate`。两者通过环境接收 DeepSeek 凭据。[两轮脚本](../2026-11-27-deepseek-harness-token-comparison/compare.py) · [六轮脚本](../2026-11-27-six-turn-dsh-comparison/revisions.py)

## Explicitly not claimed

分数的受控比较衡量已有相关记忆的输入表示。默认目录两轮仍高于 DSH，按需目录六轮的严格工具判分与未命中输入仍有差距。性能对照未证明任意任务、模型和系统上的总体能力等价。本机未验证 Linux、Windows 原生运行，二进制保留在当前源码的 `target/debug`，无 PATH 安装和历史会话写入。
