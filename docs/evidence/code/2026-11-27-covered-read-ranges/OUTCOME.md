# 已覆盖的读取范围

[上一轮的停滞回执](../2026-11-27-extractor-prefix-and-tool-scope/read-stall.json) 显示，在完整读取一个短文件之后，模型不断改变行内偏移。同文件随后进行了 20 次调用。16 次成功返回的文本逐字匹配首个完整结果中对应的后缀，余下 4 次失败或被拒绝。首轮累计 23 次模型请求和 152,046 token，最终进入拒绝循环。

内置文本读取现在记录工作区规范路径、返回的行范围、首行字节偏移以及原始展示文本摘要。模型历史构造只在当前返回文本逐字匹配同路径的较早可见范围时复用。复用保留行号、字节起点、当前文件尾或分页提示。小结果保留实际文本，并说明这一范围没有增加新文字。读取仍实际执行，较早的完整结果仍可见，原回执保留原始文本。发生变化、行截断、旧结果已经离开上下文、摘要不匹配或旧版回执缺少布局时，向模型交付完整的当前结果。

## 受控 DeepSeek 对照

`compare_reads.py` 通过真实 ACP 发出两个确定性的内置 `read` 调用，然后让官方 DeepSeek Flash 实际作答。固定调用把待比较的证据投影带到真实请求里。厂商每次试验收到一次预分析和一次带工具目录的继续请求，24 条用量均完整，两组都无追加模型工具调用。两版普通记忆提取使用相同的空结果夹具。固定温度 0、关闭 thinking、输出上限 1,024。两次均先检验已有范围，然后检验文件中途变更和长行继续。各组在隔离目录运行，文件名包含中文和空格。

| 场景 | 两版严格答复 | 实测结果 |
| --- | --- | --- |
| 重读已覆盖的后续行 | 各 2/2 | 第二条工具消息从 2,209 字节降到 175 字节，每次主请求少 512 个输入 token |
| 文件末尾在两次阅读之间改变 | 各 0/2 | 新值完整交付，两版都识别了新值，答复也提到了旧值，因此未通过要求仅给当前值的判分 |
| 续读首次被截断的长行 | 各 2/2 | 模型收到新范围并抄出准确的首尾值 |

第二组复跑逐条比较了文件变更和截断续读这两个用例送给模型的工具文本 SHA-256，两版相同。对已覆盖用例，第一条工具文本的 SHA-256 相同，第二条仅新版使用范围引用。单次总用量在已覆盖用例的两次对照中分别从 9,147 降到 8,562、从 9,018 降到 8,611。[配对汇总](controlled-summary.json) · [第一次判分](controlled/trials.json) · [复跑与实际输入指纹](controlled-b/trials.json)

## 六轮冻结任务与独立 DSH

沿用原来的 `revisions.py`、`fixture.json`、权限、时限和判分器。正序为新版、前版、DSH。反序为 DSH、前版、新版。两版 Aster 同用推迟工具定义，独立 DSH 为 0.2.2-rc.5。官方 DeepSeek Flash 固定温度 0，关闭 thinking，输出上限 4,096 token。156 条用量全部完整。

| 六轮中位数 | 新版 Aster | 前版 Aster | 独立 DSH |
| --- | ---: | ---: | ---: |
| 总 token | 163,718 | 175,963.5 | 251,495 |
| 未命中缓存的输入 token | 14,224.5 | 15,097 | 7,913 |
| 成品与输入源完整性 | 12/12 | 12/12 | 12/12 |
| 严格任务通过 | 1/2 | 0/2 | 2/2 |

本轮新版总用量较 DSH 低 34.90%，较前版低 6.96%。两版 Aster 各取得 12/12 宿主绑定核验。新版的正序使用了六次原生 `read.files` 批量读取。其他相同任务的 Aster 运行使用单文件读取，且各多执行一次 `ls`。这部分工具选择变化会影响总用量，覆盖范围复用本身的直接收益来自受控对照。[六轮汇总](long-summary.json) · [正序判分](long-a/trials.json) · [反序判分](long-b/trials.json)

用同一配套运行现有 `kimi-01/k3.1-pro` 六轮独立验证。成品判分、源文件保留和宿主绑定核验均为 6/6。实测 25 个 HTTP 200 请求均有用量，合计 175,784 token。[Kimi 原始结果](kimi-result.json)

## 本地验证

macOS arm64，仓库固定 Rust 1.97.1，`apps/studio` 已初始化。

- `cargo test --locked -p aster-core --lib read_projection_recognizes_covered_ranges_and_preserves_fresh_text` 修改前失败，返回了覆盖范围的完整重复文本。
- `cargo test --locked -p aster-core --lib -- read_projection batch_read tools::builtin::read:: fact_control::tool_history_tests --test-threads=4` 通过 78 项。覆盖跨行和 UTF-8 字节偏移、不同文件、修改后的文本、摘要不匹配、长行截断续读，以及较早结果离开模型上下文时的完整回退。
- `cargo build --locked -p aster -p aster-acp --bin aster --bin aster-acp` 通过。配套 ID 为 `aster-cli-pair-v1:6a16b9ea9be83c85b10345b3227badb1c3a63667:aarch64-apple-darwin`。TUI SHA-256 为 `78ee054d0e643b2e49a274dea96b771d4d2b13d02833f1272c2ebe59adfbfd55`，ACP 为 `10ddb0bdc9c4d3a622425ee1b722123e5b6b35bffda4acf2d5b511076e25afc7`。对照 ACP 为 `2d3929b1db8222c7b8d9bcdb014ab1543b0a2ec0a825324541ae9bcf94095781`。本机 PATH 仍为此前安装的配套。
- 用本次构建运行 `boundary_e2e --skip e_bash --nocapture`，9 项通过。`e_bash` 为此前单独复现的已有失败。
- `cargo test --locked -p aster-acp --test long_horizon_real_llm --no-run` 通过。成对构建恢复后，指定 `ASTER_LIVE_MODEL=kimi-01/k3.1-pro`、可信 `ASTER_CONFIG_FILE`、已创建的 `ASTER_LH_EVIDENCE_DIR` 和向量模型目录，运行上述测试二进制的 `real_native_file_workflow_multiple_user_turns --ignored --exact --nocapture`，1 项通过。
- `cargo check --locked --workspace --all-targets` 和 `cargo clippy --locked -p aster-core --all-targets --no-deps --features ci-all,deep-live` 通过。修改文件的定向 `rustfmt --edition 2021 --config skip_children=true --check` 与 `git diff --check` 通过。全工作区格式仍报告 38 个已有差异文件。

## 复跑命令

Python 3.12 运行 `compare_reads.py --reference <前版 ACP> --candidate target/debug/aster-acp --embedding <向量模型目录> --out <新目录>`。`DEEPSEEK_API_KEY` 从官方 DeepSeek ACL 提供到子进程环境。再新建一个输出目录运行同一命令可复验摘要和输入差异。

```sh
python3.12 docs/evidence/code/2026-11-27-six-turn-dsh-comparison/revisions.py \
  --fixture docs/evidence/code/2026-11-27-six-turn-dsh-comparison/fixture.json \
  --acp target/debug/aster-acp --reference-acp "$ASTER_TRIAL_REFERENCE_ACP" --reference-deferred \
  --dsh "$ASTER_TRIAL_DSH" --embedding "$HOME/.local/bin/embedding" \
  --out "$ASTER_TRIAL_OUTPUT" --order candidate release dsh
```

反序使用新输出目录和 `--order dsh release candidate`。

## Explicitly not claimed

受控测试直接证明了覆盖范围的交付成本下降，未复现上一轮 23 次请求的自发停滞，尚无相同失败可重复消失的证据。严格工具范围与未命中缓存输入仍有差距。改变文件的两个评测版本均未通过严格答复判分。其他操作系统及全任务能力等价未获验证。
