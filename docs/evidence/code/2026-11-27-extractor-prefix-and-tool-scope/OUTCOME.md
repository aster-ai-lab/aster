# 记忆提取前缀与类型规则

此前当轮召回有无记忆会在提取 JSON 示例中提前插入关系字段，导致后续相同的提取规则失去前缀复用。现在稳定的示例和规则先出现，只在有相关记录时附上关系字段说明。原模型调用角色、用户输入、回答、记忆召回和写入校验沿用现有路径。

旧规则要求任何语言的显式记忆请求都给出 `preference`，同时又让模型把可复用流程标为 `procedural`。真实提议同时用了这两个值，原校验器拒绝了整条记录。新版仍保留显式记忆请求的重要性、置信度和语言规则，用既有写入契约明确可用组合。事实和偏好使用 `semantic`，流程和失败经验使用 `procedural`，决定可选两者。两种本地存储的回归确认错误的类型组合仍在写入前被拒绝。

## 真实 DeepSeek 前缀复测

`measure_cache.py` 从全新隔离工作区发起实际 ACP 提取。每组使用同一个合法的最大条数设置和相同的用户请求及普通答复，一边是空召回，另一边复用同一份四条合成记忆存储。设置 6–9 用于首次试验，最终构建复验使用 1–4。设置在同一组的两请求一致，分组选择未在其他组使用的设置，避免提前热好待测的形态。普通答复由夹具提供，提取发给官方 DeepSeek Flash。温度 0，thinking 关闭，输出上限 1,024。通过 ACP 读取实际持久记录，四条召回的组同时要求原始 ID 的替换、归档和反向关系都正确。

| 最终构建的切换方向 | 前版输入／缓存命中 | 新版输入／缓存命中 | 减少的未缓存输入 |
| --- | ---: | ---: | ---: |
| 空召回至四条 | 755／0 | 750／384 | 389 |
| 四条至空召回 | 465／0 | 462／256 | 259 |

最终验证的四组首请求均为零命中。新版实际写入 4/4，前版 2/4。前版两条未写入的相关提议均为 `procedural` 加 `preference`。首次只重排前缀的构建，其两种切换各少 386 和 256 个未缓存输入 token，确认缓存收益在改类型规则前已出现。[首次请求与提议](cache/trials.json) · [最终请求与提议](final-cache/trials.json)

## 真实持久关系与工具范围

再用相同合成种子复跑中文替换与英文冲突，关闭进程后通过全新 ACP 查询，两版均 2/2 通过，原记录、结构化关系和无关记录均按冻结判分器验证。[两个关系任务](final-relations/trials.json)

`measure_sources.py` 单独验证中文个人偏好和英文可复用流程。两次运行中，两版都把中文偏好写为 `semantic` 加 `preference`。新构建两次把英文流程写为 `procedural` 加 `workflow`，并保留 `skill` 演化信息。旧构建两次给出 `procedural` 加 `preference`，重启后均无记录。合计新版 4/4，前版 2/4。[类型任务](final-sources/trials.json) · [独立复跑](final-sources-b/trials.json)

目录发现和已验证编辑使用同一组官方厂商调用，并复用 `full_catalog.py`。新目录用例要求第一轮实际按 `ls`、`read` 运行，第二轮仅 `read`。两个版本各通过 2/2。冻结的已验证编辑也各通过 2/2，保留原样 Bash 命令、文件字节和宿主绑定核验。[目录发现](directory-discovery/trials.json) · [已验证编辑](verified-edit/trials.json)

同时试过把用户工具限制更明确地写进通用仓库工具提示。这个原型的六轮成品与输入完整性为 12/12，严格工具限制 0/2，前版 1/2。原型两次均调用了额外的 `ls`，因此撤回工具提示改动，只保留记忆相关改动。[工具提示原型正向](long-a/trials.json) · [工具提示原型反向](long-b/trials.json)

## 最终构建的六轮任务

用原有 `revisions.py`、`fixture.json`、判分器、权限和超时。两版均使用推迟目录，独立 DSH 为 `0.2.2-rc.5`。两种 harness 使用官方 DeepSeek Flash，温度 0，thinking 关闭，输出上限 4,096。两组共有 159 条完整的 HTTP 200 用量记录。

| 全部三方完成的正序六轮 | 新版 Aster | 前版 Aster | 独立 DSH |
| --- | ---: | ---: | ---: |
| 总 token | 180,718 | 176,567 | 259,084 |
| 未命中缓存的输入 token | 17,663 | 12,489 | 7,318 |
| 主请求未命中输入 | 13,341 | 8,551 | 7,318 |
| 记忆提取未命中输入 | 4,188 | 3,804 | 0 次请求 |
| 成品和源文件完整性 | 6/6 | 6/6 | 6/6 |
| 严格任务通过 | 否 | 否 | 是 |

正序的新版总量比 DSH 低 30.25%，比前版高 2.35%。两版 Aster 都有一次额外 `ls`，六轮都获得宿主绑定核验。反序新版在第一轮停在拒绝循环，使用 152,046 token。前版和 DSH 在反序仍完成六轮，但各自有额外发现工具。反序未完成的新版数据留在原始结果里，与完成六轮的用量分开报告。[完整用量](long-summary.json) · [最终正序](final-long-a/trials.json) · [最终反序](final-long-b/trials.json)

反序首个文件读取回执显示 `eof=true`、`next_offset=null`、没有丢弃字节，随后模型改变字节偏移反复读取同一个已完整交付的文件。拒绝循环最终在 23 次模型请求后终止。两个版本首个请求的 system 和用户消息指纹、工具数及工具定义字节数相同，均先调用 `read` 和 `ls`。失败发生在首次记忆提取之前。保留工具序列、范围和拒绝终态供后续追踪。[读取停滞回执](read-stall.json)

在新隔离目录用原夹具和原判分器补做一组 Aster 对照，分别运行新版和前版。两版均完成 6/6 轮并获得 6/6 宿主核验。两版各有一次额外 `ls`，严格判分均失败。总用量新版 157,114、前版 181,544，未命中输入分别为 15,073 和 13,347。补跑收到 54 条完整用量，和前面反序的失败分别留存。[独立补跑](final-long-retry/trials.json)

## 本地验证

macOS arm64，仓库固定 Rust 1.97.1，`apps/studio` 已初始化。前版配套 ID 为 `17bc329d9c74b6af92e9c535b59670c47ebc2638`，ACP SHA-256 为 `c25777c0e669ce13cf2035660cf2a8857a4a41ab035258e9941f31c9402dcee0`。工具提示原型配套 ID 为 `fa9c61e14c728a94d96311a5ae95acdccc46adc6`，ACP SHA-256 为 `b5158c3a18f9e578c8a80cf9fb1cebf09479267dd77b354391f784581ba457da`。

- 前缀稳定性断言和已有语言中立记忆提示的类型组合断言分别在各自修改前失败。
- `cargo test --locked -p aster-core --lib -- memory_extraction prompts::tests same_remember_bytes_require_valid_model_authority_in_both_stores --test-threads=4` 通过 117 项。
- `cargo build --locked -p aster -p aster-acp --bin aster --bin aster-acp` 通过。最终配套 ID 为 `aster-cli-pair-v1:44dcbcde5a042e189a32c02deddb06edea54578d:aarch64-apple-darwin`。`target/debug/aster` SHA-256 为 `1141f14a8a72652f9695bead705f767716d0d6a6d7c904beb4626c98e5844d69`，ACP 为 `2d3929b1db8222c7b8d9bcdb014ab1543b0a2ec0a825324541ae9bcf94095781`。本机 PATH 仍使用此前安装的配套。
- 用最终配套运行 `boundary_e2e --skip e_bash --nocapture`，9 项通过。`e_bash` 为此前独立复现的已有失败。
- `cargo check --locked --workspace --all-targets`、`cargo clippy --locked -p aster-core --all-targets --no-deps --features ci-all,deep-live`、修改文件的定向 `rustfmt --edition 2021 --config skip_children=true --check` 与 `git diff --check` 通过。全工作区格式检查仍报告 38 个已有格式差异文件。

## 复跑命令

用 Python 3.12。`DEEPSEEK_API_KEY` 从官方 DeepSeek ACL 注入子进程环境。`ASTER_TRIAL_SEED` 指向前一轮 [合成记忆评测](../2026-11-27-memory-relation-references/OUTCOME.md) 的 `seed` 输出目录。

```sh
python3.12 docs/evidence/code/2026-11-27-extractor-prefix-and-tool-scope/measure_cache.py \
  --reference "$ASTER_TRIAL_REFERENCE_ACP" --candidate target/debug/aster-acp \
  --embedding "$HOME/.local/bin/embedding" --seed-directory "$ASTER_TRIAL_SEED" \
  --first-limit 1 --out "$ASTER_TRIAL_CACHE_OUTPUT"

python3.12 docs/evidence/code/2026-11-27-extractor-prefix-and-tool-scope/measure_sources.py \
  --reference "$ASTER_TRIAL_REFERENCE_ACP" --candidate target/debug/aster-acp \
  --embedding "$HOME/.local/bin/embedding" --out "$ASTER_TRIAL_TYPE_OUTPUT"

python3.12 docs/evidence/code/2026-11-27-six-turn-dsh-comparison/revisions.py \
  --fixture docs/evidence/code/2026-11-27-six-turn-dsh-comparison/fixture.json \
  --acp target/debug/aster-acp --reference-acp "$ASTER_TRIAL_REFERENCE_ACP" --reference-deferred \
  --dsh "$ASTER_TRIAL_DSH" --embedding "$HOME/.local/bin/embedding" \
  --out "$ASTER_TRIAL_LONG_OUTPUT" --order candidate release dsh
```

反向命令使用不同的输出目录和 `--order dsh release candidate`。Aster 补跑使用 `--order candidate release`。目录发现和已验证编辑用 `full_catalog.py --native-catalog --scenario directory_discovery` 或 `--scenario verified_edit_exact_count --disk-content-attestation`，传入同一候选及对照。

## Explicitly not claimed

受控前缀和类型结果只覆盖列出的合成记忆及官方 DeepSeek Flash。本轮结果不足以证明所有能力等价。严格工具范围、重复读停滞，以及整体未命中输入量仍待解决。未运行其他系统和模型。
