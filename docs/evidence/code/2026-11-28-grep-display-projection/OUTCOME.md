# 搜索上下文路径去重

每条匹配都保留原有 `>路径:行号: 正文`，可直接引用或核对。Core 从原生搜索的文件身份和输出范围取得上下文位置，并把范围绑定到原文摘要。发送给模型时，只有同一匹配块内的上下文行借用匹配行的路径。上下文的行号和正文、块分隔、结果计数、续页游标及后续诊断逐字保留。搜索没有上下文、摘要或块结构不符、或者结果无法缩短时发送原文。工具正文、执行回执、分页和权限继续沿用现有接口。

历史只读抽样在报告的主会话中找到 138 份内容搜索回执，合计 1,294,781 字节。按最终上下文格式估计，76 份可缩短，净省 230,367 字节，占 17.79%。同机可用的子会话样本里，119 份回执共 1,249,620 字节，36 份估计净省 98,561 字节，占 7.89%。估算已扣除格式说明，原会话内容只在本机计数。

整文件分组在主会话上估计可省 52.14% 的搜索回执字节。初版 Unicode 路径用例中，旧版直接答对，候选尝试用 Bash 核对，严格判分失败。隔离中继给候选补充通用格式说明后双方都答对。随后编入 ACP 的真实二进制复测中，整文件分组两次仍尝试 Bash，严格通过 0/2，旧版 2/2。该格式撤回。 [初版判分](initial-controlled/trials.json) · [中继试验](legend-relay-probe/trials.json) · [二进制正序复测](rejected-file-format/forward/trials.json) · [二进制反序复测](rejected-file-format/reverse/trials.json)

## 最终构建的 DeepSeek 实测

使用 DeepSeek Flash、关闭思考、temperature 0、单次最多输出 1,024 token。独立 DeepSeek Harness Code 为 `0.2.2-rc.5`。Aster 使用默认完整工具目录。总 token 为输入与输出之和，缓存命中计入输入。每组两次，调换候选和对照的执行顺序。最终各组共 221 条完整的 HTTP 200 厂商回执。逐笔核对了输入、输出、缓存命中和未命中的合计关系。

| 受控搜索，单次首份观测 | 前版 Aster | 当前 Aster |
| --- | ---: | ---: |
| 分页场景首份工具正文 | 7,510 字节 | 4,037 字节 |
| 分页场景首个真实推理请求输入中位数 | 9,438 | 8,705 |
| 分页场景全部真实请求总 token 中位数 | 20,380.5 | 18,485.5 |
| Unicode 无上下文场景工具正文 | 5,447 字节 | 5,447 字节 |
| Unicode 无上下文严格通过 | 2/2 | 2/2 |

分页首份工具正文缩短 46.25%，主请求少 733 输入 token，降幅 7.77%。8 份首份观测都能逐字恢复，所有命中行本身也逐字相同。分页场景的路径、行号、字段和游标在两边 4 份答案中全部正确。严格格式及工具预算两边均为 0/2。前版两次继续搜索，当前版两次额外读取文件，双方都附了 JSON 之外的说明。该驱动固定首个搜索调用，脚本批准范围和严格判分沿用各版原试验。 [固定搜索脚本](measure_grep.py) · [正序判分](measured/controlled/trials.json) · [反序判分](measured/controlled-reverse/trials.json)

| 两轮真实任务，总 token 中位数 | Aster 对照 | 当前 Aster | DSH | 三方通过 |
| --- | ---: | ---: | ---: | --- |
| 读文件与命令，对照已发布版 | 37,327 | 30,975 | 25,269.5 | 各 2/2 |
| 读文件与命令，对照前一源码版 | 30,981 | 31,122 | 25,223 | 各 2/2 |
| 查阅发布与回滚记录，对照前一源码版 | 58,215.5 | 52,624 | 40,512.5 | 各 2/2 |

对照已发布版本的固定两轮任务，当前总 token 低 17.02%，比 DSH 高 22.58%。未命中输入中位数分别为 4,587.5、4,087 和 1,080.5。当前 Aster 和 DSH 都发出四次主请求。Aster 另有两次记忆提取，中位数 920 token。单个主请求的系统消息中位数为 7,483 字节对 2,802，工具定义为 18,852 字节对 20,471。主请求累计输入的中位数为 29,801 对 24,952.5。 [发布版对照](measured/published-short/trials.json) · [发布版厂商回执](measured/published-short/provider-usage.jsonl) · [前一源码版对照](measured/short-dsh/trials.json)

发布与回滚查阅任务中，当前 Aster 比前一源码版低 9.60%，比 DSH 高 29.90%。当前和前版每次均提交两次搜索和两次读取，DSH 每次提交两次搜索和四次读取。三边每次均发出六次主请求。Aster 每项任务另有四次辅助请求。该对照允许 Harness 自选合法工具参数，执行期间逐字保护了三份输入。 [查阅评测脚本](workflow.py) · [判分](measured/search-dsh/trials.json) · [厂商回执](measured/search-dsh/provider-usage.jsonl)

另外复跑三项原有能力。真实搜索两边各 2/2 达到任务和工具要求。混合批次含真实 `batch`，当前 2/2，前版 1/2。精确编辑及原样核验两边各 2/2，编辑最后的字节、Bash 成功回执和绑定摘要均完整。原比较工具还输出一个专为按需目录设置的通过字段，当前默认配置的能力统计使用相同夹具中相应的任务、工具和绑定核验判据。 [搜索](measured/search/trials.json) · [混合批次](measured/batch_mixed/trials.json) · [绑定核验](measured/verified_edit_exact_count/trials.json)

## 构建及复验

环境为 macOS 27.2.1 arm64、Rust 1.97.1，根工作区 `apps/studio` 已初始化。当前本机配套 ID 为 `aster-cli-pair-v1:40758b5399df33c8e2eadce2e72de6ff52798b83:aarch64-apple-darwin`。冻结前版 ACP 为 `a81ad09feec6ccf622e4c256b6f482d2a3159eaf`，SHA-256 `b646fb7bdfd3b06c3a088c9a9f12cae19af9ed2e652a28b75a31855151acebdc`。已发布 ACP 为 `a66ae4d44a4d7e3de7c76c9d4e9391fa7e96ab8a`，SHA-256 `1fe12f48d4e4d1f0553a393fee4def28e2f2531a09cb88c94fa15bf385deb25d`。

- `cargo build --locked -p aster -p aster-acp --bin aster --bin aster-acp` 与 `cargo build --locked -p aster-core --lib` 成功。`target/debug/aster` SHA-256 为 `3a8fc880a86694f28322a3c9df4ab7f504519bcb573398a4b74b597ed91408f5`。`target/debug/aster-acp` 为 `3e1852bdff970b47acdcdd268b7cacef4aa0df054aa6a6b31babad20446b50cb`。`target/debug/libaster_core.rlib` 为 `afdeac6bb48057eb26a244bcf46198d2089b4f664cd2b64eb4df0abe16c7382b`。
- `cargo test --locked -p aster-core --lib -- fact_control::tool_history_tests tools::builtin::grep tools::builtin::search workspace::local::tests workspace::manifest::tests --test-threads=4` 通过 177 项。新用例验证带空格和 Unicode 的多文件分页、原匹配位置、上下文、后缀游标、摘要不符及错误回退，还有不带上下文结果原文直传。
- `ASTER_TUI_BIN="$PWD/target/debug/aster" ASTER_ACP_BIN="$PWD/target/debug/aster-acp" target/debug/deps/boundary_e2e-1770b1eb4e0797cc --skip e_bash --nocapture` 通过 9 项。跳过的 1 项是已记录的本机沙箱失败。 [已有边界证据](../2026-11-27-host-denial-and-pending-verification/OUTCOME.md)
- `cargo check --locked --workspace --all-targets`、`cargo check --locked -p aster-core --features ci-all,deep-live` 和 `cargo clippy --locked -p aster-core --all-targets --no-deps --features ci-all,deep-live` 均通过。定向复核了触及的 Rust 路径。Clippy 在测试文件原有的 `&[contract.clone()]` 处建议使用 `std::slice::from_ref`，此行属于原验证夹具。
- 定向 `rustfmt --check`、`git diff --check` 通过。`/opt/homebrew/bin/python3.12 tests/workspace_dependencies.py` 通过 7 项。`cargo fmt --all -- --check` 报告已有两个格式差异，位于 `crates/code/core/src/sse.rs` 和 Studio 的 `reasoning_transport_tests.rs`。
- 固定页使用 `python3 measure_grep.py --projection context --reference "$REFERENCE_ACP" --candidate "$ACP" --embedding "$EMBEDDING" --out "$OUT" --order reference candidate`，再反序复跑。真实搜索对照使用 `python3 workflow.py --release "$REFERENCE_ACP" --candidate "$ACP" --dsh "$DSH" --embedding "$EMBEDDING" --out "$OUT" --order candidate,release,dsh dsh,release,candidate`。固定短任务使用原 `compare.py` 和相同参数。默认能力使用原 `full_catalog.py --default-catalog`，精确核验带 `--disk-content-attestation`。真实厂商凭据由 `DEEPSEEK_API_KEY` 传入评测程序。

## Explicitly not claimed

这些样本尚未证明 Aster 默认配置普遍低于 DSH，也不足以推断所有模型和平台的能力等价。计数包含缓存命中输入，实际账单取决于请求时的缓存状态与计价。本机构建产物位于 `target/debug`，已运行的会话和系统安装版未被本次构建替换。历史会话是回执字节估算，未重放为真实模型用量。
