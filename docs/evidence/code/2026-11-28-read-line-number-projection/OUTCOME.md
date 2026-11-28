# 文件行号的模型侧显示

对已报告的长会话做只读汇总。在 7 个后台子任务中，208 份可识别的单文件文本页共展示 40,596 行，行号左侧补齐宽度的空格占 133,705 字节。它们在之后的模型请求里随历史重发。这个计数按原始文件读取回执计算。

模型历史仅在成功的原生 `read` 回执提供可校验的布局时去掉行号左侧的 ASCII 空格。每行仍保留完整数字、分隔制表符及其后的原文。窗口和分页沿用回执上的原始字节预算、行号及游标。原始工具回执、界面显示、权限和效果判断均使用原来的记录。单文件重叠窗口和批量完整区段继续用原始内容判断重复。部分批量区段只提供显示范围，始终缺少可供引用的完整区段身份。

## DeepSeek 格式隔离测量

同一合成内容及任务在 DeepSeek Flash 上各发送两种仅行号补空不同的格式。四次请求都答对了确切行号和值，配置题还答对了正文缩进。四次用量均来自 HTTP 200 提供商回执。[原始用量和判分](transport-probe.json)

| 内容 | 行数 | 原输入 token | 去补齐后输入 token | 节省 |
| --- | ---: | ---: | ---: | ---: |
| Rust 源码 | 1,250 | 23,387 | 22,137 | 1,250 |
| YAML 配置 | 1,400 | 16,080 | 14,680 | 1,400 |

## 真实 ACP 大窗口先导试验

先导比较使用 16,384 字节默认工具回执上限。原计划的两个窗口都超过上限，8 次模型回答都正确，但每次都补调了一次 `search`，因此严格通过 0/8。保留原窗口、原判分及正反两个顺序。单文件原生回执显示 341 行，候选每回合总量从 27,839 降到 27,157，节省的 682 等于 341 行在两次上游请求中各少一个 token。多文件第一段被截为 468 行，候选仍缺少这段的布局证明，两版均为 26,731 token。这一观察确定了部分区段的修复范围。[正序试验](pilot-numbered-a/trials.json) · [反序试验](pilot-numbered-b/trials.json)

## 最终配套的真实 ACP 回执

新增独立的限长窗口和续读题。环回夹具只触发一次原生 `read` 并单独处理会话结束后的记忆提取。任务回答由实际 DeepSeek Flash 生成。每项各跑两版和两个顺序，12 次都收到一份完整的上游任务用量，无额外模型工具调用，文件字节保持原样。相同窗口的两版原始工具回执摘要完全一致。将旧版模型侧行号去掉补空后，其输入摘要逐项等于新版实际发送的摘要。[正序试验](numbered-a/trials.json) · [反序试验](numbered-b/trials.json) · [正序用量](numbered-a/provider-usage.jsonl) · [反序用量](numbered-b/provider-usage.jsonl)

| 读取方式 | 保留行数 | 前版输入 token | 新版输入 token | 节省 | 严格通过，前版 / 新版 |
| --- | ---: | ---: | ---: | ---: | ---: |
| 单文件指定窗口 | 320 | 13,829 | 13,509 | 320 | 2/2、2/2 |
| 两文件完整窗口 | 300 | 12,254 | 11,954 | 300 | 2/2、2/2 |
| 批量读取中途分页 | 468 | 13,097 | 12,629 | 468 | 0/2、0/2 |

中途分页的四次回答均得到确切值 `青石-73` 和下一页偏移 `468`。它们都在 `config_line` 返回源码行字符串，冻结判分期待行号整数 `439`。按原判分保留四个失败。游标完整原文和行 `439` 都包含在字节摘要校验中。

已有两轮 `read_window` 能力任务的四个试验均准确读取 `42,3` 和 `69,2` 两个偏移与长度，答案、保留字节及工具回执均通过。中位数由 31,105 降到 31,094 token。该运行器将 `compact_requests > 0` 纳入候选方 `qualified_pass`，默认目录的候选两次因这个条件记为未通过。[冻结判分](read-window/trials.json) · [运行配置](read-window/manifest.json)

已有两轮 `read_batch_repeat` 在原生按需目录下复测。两版都严格通过 2/2，每次依次发出两个真实的 `read.files` 调用，每次分别读取两个文件。重复文件与新文件的值均准确返回，全部输入字节保持原样。前版总量中位数 27,656.5，新版 27,432.5，减少 224 token。[冻结判分](batch-repeat/trials.json) · [实际工具参数](batch-repeat/read-calls.json) · [逐请求用量](batch-repeat/provider-usage.jsonl)

## 六轮独立 DSH 对照

正反两组沿用冻结任务、工具判分、权限与 DeepSeek Flash 参数。两个 Aster 源码构建均使用原生按需目录，DSH 为独立的 `0.2.2-rc.5`。全部 152 个上游响应为 HTTP 200，并包含用量。[顺序一](long-a/trials.json) · [顺序二](long-b/trials.json) · [汇总](long-summary.json)

| 版本，各两次 | 正确产物 | 绑定核验 | 严格工具通过 | 总 token 中位数 | 未命中输入中位数 |
| --- | ---: | ---: | ---: | ---: | ---: |
| Aster 前一源码版 | 12/12 | 12/12 | 2/2 | 156,803 | 14,497 |
| Aster 当前源码版 | 12/12 | 12/12 | 0/2 | 167,453 | 14,024.5 |
| 独立 DSH | 12/12 | — | 2/2 | 263,230.5 | 7,998.5 |

当前版总量比 DSH 低 36.39%，比前一 Aster 源码版高 6.79%。两次均在第一轮多调用一次 `ls`。前版和当前版的主请求中位数分别是 18.5 和 19.5。第一份输入在每一组的两版之间逐字一致，正序却分别请求一次和四次 `read`，说明前置请求相同也可能走不同路线。[正序逐请求用量](long-a/provider-usage.jsonl) · [反序逐请求用量](long-b/provider-usage.jsonl)

另做工具选择复放。分别从反序两版的实际日志提取第一组四个读调用，交给新 ACP 在原冻结任务的首轮执行。捕获下一次实际请求，把三个带补空的行号按原读取回执恢复，形成两份除此以外逐字一致的请求。每份各运行六次。在这两个复放历史中，24 次下一步均调用 `ls`，紧凑输入每次少三个输入 token。这两组诊断没有复现仅切换补齐就改变工具选择的现象。全流程严格失分仍保留。[候选调用复放](next-action-candidate.json) · [前版调用复放](next-action-reference.json)

按实际请求分类，当前 Aster 六轮两次各发生 6 次记忆提取，未命中输入分别为 3,612 和 2,880 token。两次各有 134 个额外的前置分析未命中 token。DSH 的两次试验均只有主请求。这解释了部分未命中输入差距，实际主请求的差距仍受工具路线和缓存命中影响。[正序逐请求用量](long-a/provider-usage.jsonl) · [反序逐请求用量](long-b/provider-usage.jsonl)

## 构建与验证

本机 macOS 27.2.1 arm64，Rust 1.97.1，根工作区 `apps/studio` 已初始化。

- `cargo build --locked -p aster -p aster-acp --bin aster --bin aster-acp` 通过。`aster` 和 `aster-acp` 配套 ID 均为 `aster-cli-pair-v1:c9892a16fcebdc60c901f7f8c54afe76bf9ead2d:aarch64-apple-darwin`。两者 SHA-256 分别为 `429a75c494e74adad276a9db0c8de594108d3121e5660a80c519bdd714f7d6c2` 和 `c7ee1c1f4bafff375516f52d5e0b6c87e58c1a8fcefec49ac7b00b25d87369a2`。前一源码版配套为 `aster-cli-pair-v1:04ad02027f70b3ae4ff4e7b294bd2398fa1528e7:aarch64-apple-darwin`，ACP SHA-256 为 `09f7e2990d66897819e9382aa72c935b42c0efea187b03a01e7d0b7d42680c60`。大窗口先导的候选配套为 `aster-cli-pair-v1:2c95a7af37e567a913ccec601daa413fa292ab8e:aarch64-apple-darwin`。
- `cargo test --locked -p aster-core --lib -- fact_control::tool_history_tests tools::builtin::read:: background_subagent child_run:: --test-threads=4` 通过 106 项。覆盖 Unicode、空行和源缩进，拒绝摘要失效的布局，保留字节续读边界和后台任务结束规则。
- 对上述配套运行 `boundary_e2e --skip e_bash --nocapture` 通过 9 项。排除的 `e_bash` 有本机独立的沙箱失败基线。[基线记录](../2026-11-27-host-denial-and-pending-verification/OUTCOME.md)
- `cargo check --locked --workspace --all-targets` 通过。`cargo clippy --locked -p aster-core --all-targets --no-deps --features ci-all,deep-live` 通过。Clippy 在受影响文件范围唯一候选警告位于已有验证测试的 `&[contract.clone()]`，在 `HEAD` 已存在。
- `cargo fmt --all -- --check` 仍报告 42 个改动前已存在的差异文件。三个实际修改的 Rust 文件用 `rustfmt --edition 2021 --config skip_children=true --check` 通过。`git diff --check` 通过。`/opt/homebrew/bin/python3.12 tests/workspace_dependencies.py` 通过 7 项。
- 实际回执比较用 `python3 compare_numbered_reads.py --shape bounded --reference "$REFERENCE_ACP" --candidate "$ACP" --embedding "$EMBEDDING" --order reference candidate --out "$OUT"`，反序是 `--order candidate reference`。原先导用 `--shape large` 的固定输入。凭据通过 `DEEPSEEK_API_KEY` 提供。
- 六轮命令沿用 `revisions.py --reference-deferred --order candidate release dsh` 及 `--order dsh release candidate`。两轮能力使用 `full_catalog.py --scenario read_window --default-catalog` 和 `full_catalog.py --scenario read_batch_repeat --native-catalog`，前版和当前版参数对应。[六轮运行器](../2026-11-27-six-turn-dsh-comparison/revisions.py) · [两轮运行器](../2026-11-27-model-selected-tool-catalog/full_catalog.py)

## Explicitly not claimed

本轮的节省对应已观测的模型输入格式。严格工具判分和未命中缓存的输入仍需改善，现有数据无法证明整体能力或调用路线普遍等价。未对运行中的旧进程和历史会话做修改。本机验证使用 macOS arm64，未执行 Windows、Linux 原生运行测试。
