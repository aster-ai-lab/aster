# DeepSeek Messages 的原生延迟定义与并行回执

Aster 的 Messages 适配器将同一轮并行调用的各条回执分别发为 `user` 消息。首次复现的模型响应发出三条调用，ACP 执行工具并保存三个回执。DeepSeek 收到分开的结果后拒绝续接，真实 ACP 的第二份模型请求返回 HTTP 400。使用两个合成读取回执复放时，分开发送得到同样的缺失回执错误，合在一条消息中返回 HTTP 200，模型准确报告两个标记。[首次流式 ACP 回执](native-streaming/short/provider-usage.jsonl) · [同内容的协议复放](parallel-wire-repro.json)

Core 现在只在 Messages 的传输边界把相邻的纯工具结果放入同一条消息，保留结果块顺序、原始 ID、正文、图片、失败标记和已有缓存控制。普通用户消息保留独立边界。两家 Messages 协议都要求同一批的结果放在紧接工具调用的一条用户消息内。[DeepSeek Messages](https://api-docs.deepseek.com/guides/anthropic_api/#returning-tool-results) · [Anthropic 并行工具文档](https://platform.claude.com/docs/en/agents-and-tools/tool-use/parallel-tool-use)

## 延迟定义的厂商测量

使用相同 Aster ACP 构建 `d4793d0b2f1ab1c781b76cc3d4e32f9e91fa22b1` 和相同权限判分。实验中继仅将 Aster 的带工具请求转换为 Messages，候选通过 `defer_loading` 声明原定义并在需要时用精确的 `tool_reference` 展开。两边普通辅助调用和独立 DSH `0.2.2-rc.5` 仍走 Chat Completions。中继缓冲厂商响应，工具实际由原 ACP 执行。每次上游查阅都单独计入厂商用量。没有将这个实验中继接入产品。[DeepSeek 原生延迟协议](https://api-docs.deepseek.com/guides/tool_calls/#deferred-tool-loading)

各组都用同一 DeepSeek Flash、temperature 0、关闭思考，冻结短任务输出上限 1,024，六轮输出上限 4,096、上游请求上限 40。45 项运行配置、184 次试验，共获得 1,448 条完整 HTTP 200 厂商回执。表内均为两次的总 token 中位数。[实验明细](summary.json)

| 短任务 | Aster 试验 | Aster 完整目录 | 独立 DSH | 候选相对 DSH |
| --- | ---: | ---: | ---: | ---: |
| 参数签名及摘要首行 | 16,680 | 30,980 | 25,206 | −33.83% |
| 参数说明及摘要首行 | 20,548 | 31,138 | 25,232.5 | −18.57% |
| 参数说明及完整摘要 | 21,490 | 30,961 | 25,155 | −14.57% |
| 保留复合参数和长说明工具的原定义 | 26,429 | 30,971 | 25,285.5 | +4.52% |

四组短任务三方均各通过 2/2。较长任务用首行方案为 127,490.5，完整目录为 213,276，DSH 为 258,040。候选低 50.59%。完整摘要方案为 157,385、202,592.5、262,714，候选低 40.09%。两个方案各自的候选与完整组均为 12/12 正确产物、输入保全和绑定核验，严格工具判分均为 0/2。额外工具及每轮资料见[首行正序](matrix/first-line-long-forward/trials.json)、[首行反序](matrix/first-line-long-reverse/trials.json)、[完整摘要正序](matrix/annotated-long-forward/trials.json)和[完整摘要反序](matrix/annotated-long-reverse/trials.json)。

独立能力的严格判分，首行方案 15/18 对完整 17/18，参数说明加首行方案 16/18 对 16/18，完整摘要方案 14/18 对 15/18，保留复合原定义方案 13/20 对 15/20。相同通过总数也可掩盖单项退化。仅首行时，精确分页在两次试验都把零基偏移用错；保留参数说明后两次通过。只加参数说明的混合批次两次都被拆为独立调用，完整目录两次都实际用了批次。保留复合原定义能恢复混合批次，但两次搜索严格判分都失败。目录选择规则只使用 Schema 结构和文字边界。[分页](matrix/first-line-read_window/trials.json) · [加参数说明后的分页](matrix/parameter-notes-read_window/trials.json) · [混合批次](matrix/parameter-notes-batch_mixed/trials.json) · [复合原定义搜索](matrix/composite-eager-search/trials.json)

原始调用 ID 改为短别名的四对复放没有节省厂商输入 token。缩短回执标签省 17、24、19 和 26 个。两种字面变化只在探针运行。中继的历史匹配检查确认，只有同一历史前缀、同一已准入工具定义才重放查询的 `tool_reference`。撤下工具、变更定义和不同历史三个案例均无引用。[标签和 ID 的复放](framing/results.json)

## 本地构建与复验

macOS 27.2.1 arm64，固定 Rust 1.97.1，根工作区的 `apps/studio` 已初始化。原 ACP SHA-256 为 `dddd5bc546688f1ed5a2b42e1946599dc46e9e9f5927570ba32a999dc37dfe5a`。当前配套 ID 为 `aster-cli-pair-v1:a81ad09feec6ccf622e4c256b6f482d2a3159eaf:aarch64-apple-darwin`。`target/debug/aster`、`target/debug/aster-acp` 和 `target/debug/libaster_core.rlib` 的 SHA-256 分别为 `51e8b27f4a5807a35fde91ffb1ff42444da442b642b9cdd7fd3e34abcae35523`、`b646fb7bdfd3b06c3a088c9a9f12cae19af9ed2e652a28b75a31855151acebdc`、`ea03bb2268750ea013a2750b864ff7f63ec22ef3c7f9829ea825ad6710a45fef`。

- `cargo test --locked -p aster-core --lib -- llm::anthropic fact_control::batch_history --test-threads=4` 通过 31 项。新增用例检查工具 ID、顺序、文字、图片、失败标记和两个已有缓存标记，追加普通用户消息后结果仍成组。
- `cargo build --locked -p aster-core --lib` 和 `cargo build --locked -p aster -p aster-acp --bin aster --bin aster-acp` 通过。用上述配套执行 `boundary_e2e --skip e_bash --nocapture`，9 项通过，排除 1 项[已知本机失败](../2026-11-27-host-denial-and-pending-verification/OUTCOME.md)。
- `cargo check --locked --workspace --all-targets` 和 `cargo clippy --locked -p aster-core --all-targets --no-deps --features ci-all,deep-live` 通过，保留已有警告。
- 实际 Messages SSE 中继保持 Aster 发出的消息结构，原样转发厂商 SSE。原版短任务有一份 HTTP 400，新版把三份结果发为一组，两个回合完整，6 份请求全部 HTTP 200，总 token 为 32,839。混合批次两版各 4/4 完成并调用实际批次。精确编辑两版各四次皆生成同一核验摘要，精确输出完整。完整目录在预期前提下复用原判分，矩阵中候选必须携带延迟目录的附加条件单独保留。最新 Chat Completions 短任务三方各通过 2/2，总 token 中位数分别为当前 Aster 31,002、前版 30,969 和 DSH 25,217。[流式传输前后与 Chat 对照](adapter-verification/summary.json)
- `cargo fmt --all -- --check` 报告并行修改的 `crates/code/core/src/sse.rs` 和 Studio `reasoning_transport_tests.rs` 仍有格式差异。修改的两个 Rust 文件单独 `rustfmt --edition 2021 --config skip_children=true --check`，加上 `git diff --check` 和三个 Python 文件的语法检查均通过。`/opt/homebrew/bin/python3.12 tests/workspace_dependencies.py` 通过 7 项。
- 设置现有 `DEEPSEEK_API_KEY` 后，执行 `python3 messages_stream.py --scenario short --acp "$ACP" --embedding "$EMBEDDING" --out "$OUT"` 可复验真正的流式适配器，另两项用 `--scenario batch_mixed` 和 `--scenario verified_edit_exact_count`。实验矩阵用 `python3 audit.py --summary first-line --scenario short --acp "$ACP" --dsh "$DSH" --embedding "$EMBEDDING" --out "$OUT"`。六轮增加 `--scenario long --order forward` 或 `--scenario long --order reverse`。四种摘要模式与独立场景见运行器。[审计驱动](audit.py) · [流式运行器](messages_stream.py)

## Explicitly not claimed

厂商原生延迟在实验中继测量。产品当前的协议选择与默认工具目录配置不变。独立夹具发现的能力差距仍在。原生延迟的试验结果不足以支持默认启用，也未测实际账单。Windows 和 Linux 未做原生界面复验。
