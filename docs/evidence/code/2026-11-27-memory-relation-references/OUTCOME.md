# 记忆关系的请求内短引用

记忆提取先召回相关记录，再让模型判断是否替换或关联冲突。模型需要区分这些记录和原有关系。没有被其他可见证据直接引用的生成式 UUID 可以用请求内短引用表示。自定义 ID 和证据里原样引用的 ID 保持原样。模型提议经本轮引用表还原后，仍由现有的可见记录集合校验。只在历史关系里出现的引用无法成为本轮写入目标。裁剪时保留完整 JSON 行和非空内容，随后登记可见目标。

内存及文件存储的真实归档读取验证了 `supersedes`、`superseded_by` 和源记录字节。单测还覆盖同一引用在记录和历史关系中的一致性、旧 ID 兼容、未知引用、自定义 ID 冲突，以及含多字节字符的预算边界。记忆提取调用数、持久 ID、原始回执和写入校验继续使用现有契约。

## DeepSeek 受控提取

`compare_relations.py` 通过真实 ACP 和实际持久化存储运行。测试先生成四条合成记忆并关闭种子进程，每组对照从同一份合成存储重新启动。对照的用户请求及普通助手答复固定，仅提取请求发给官方 DeepSeek Flash。模型温度 0，thinking 关闭，输出上限 1,024 token。通过再次启动 ACP 读回持久记录，验证关系、原记录和三条无关记录。在每对实际请求中逐项检查四条记录、顺序和非 ID 字段相同。第二组重新生成了种子 UUID。两次固定种子的复跑使用第一组保存的合成存储。

| 单次提取中位数 | 前版 | 短引用构建 |
| --- | ---: | ---: |
| 输入 token | 830 | 755 |
| 输出 token | 273.5 | 250.5 |
| 总 token | 1,103.5 | 1,005.5 |
| 未命中缓存的输入 token | 325 | 243 |
| 重开后关系通过 | 6/8 | 7/8 |

八对请求各少 108 UTF-8 字节和 75–89 输入 token。中位输入下降 9.04%，总量下降 8.88%。每个版本各八次提取。新版中文替换 3/4，英文冲突 4/4。前版分别为 4/4 和 2/4。第一次新版中文替换无新增记录，该次只采集了用量和请求，无法判定提议为何失效。两次前版冲突漏掉了结构化关系，原始提议均显示 `conflicts_with` 为空。固定种子后新版中文替换两次均归档正确。四组合计 16 条 HTTP 200 用量回执。[配对结果](relation-summary.json) · [初次运行](relations-a/trials.json) · [新种子复跑](relations-b/trials.json) · [固定种子一](relations-fixed-1/trials.json) · [固定种子二](relations-fixed-2/trials.json)

## 六轮冻结任务与独立 DSH

沿用原有 `revisions.py`、`fixture.json`、判分器、权限规则和中止时限。正序为新版、前版、DSH。反序为 DSH、前版、新版。两版 Aster 同用推迟工具定义模式。独立 DSH 为 `0.2.2-rc.5`。两种 harness 都用官方 DeepSeek Flash，温度 0，thinking 关闭，输出上限 4,096 token。162 条请求全部得到完整用量。

| 中位数 | 短引用构建 | 前版 Aster | 独立 DSH |
| --- | ---: | ---: | ---: |
| 总 token | 175,002 | 189,575 | 260,026.5 |
| 输入 token | 170,250 | 184,317.5 | 255,920 |
| 未命中缓存的输入 token | 15,308 | 15,359.5 | 7,920 |
| 记忆提取总 token | 8,061 | 8,707.5 | 0 次请求 |
| 成品与源文件完整性 | 12/12 | 12/12 | 12/12 |
| 严格任务通过 | 0/2 | 0/2 | 2/2 |

新版总量中位数比 DSH 低 32.70%，比前版低 7.69%。六轮差异包含模型输出和工具选择，短引用的直接因果数据来自上面的受控对照。两版 Aster 各有 12/12 宿主绑定核验，每次运行均完成六次记忆提取。新版每次各有一次额外 `ls`，前版每次各有两次，原判分器据此判定 Aster 严格失败。新版正序的 `ls` 在首次读取尚未创建的成品之后，反序的 `ls` 出现在第一批工具调用中。[六轮汇总](long-summary.json) · [正向原始判分](long-a/trials.json) · [反向原始判分](long-b/trials.json)

## 本机构建和验证

macOS arm64，仓库固定 Rust 1.97.1。`apps/studio` 已初始化。对照 ACP 的配套 ID 为 `aster-cli-pair-v1:12e4e37eca612e08288babd81f83300e20bfbae7:aarch64-apple-darwin`。

- `cargo test --locked -p aster-core --lib related_memories_are_formatted_as_json_lines` 在修改前失败，默认记录仍直接序列化原始 UUID。
- `cargo test --locked -p aster-core --lib memory_extraction -- --test-threads=2` 通过 66 项。
- `cargo build --locked -p aster -p aster-acp --bin aster --bin aster-acp` 通过。新版配套 ID 为 `aster-cli-pair-v1:17bc329d9c74b6af92e9c535b59670c47ebc2638:aarch64-apple-darwin`。`target/debug/aster` 的 SHA-256 为 `e68a5a006d6cf6ce3850c2a7b78ffdf7520b7bc049620cbb39426bd42fefd3c8`，`target/debug/aster-acp` 为 `c25777c0e669ce13cf2035660cf2a8857a4a41ab035258e9941f31c9402dcee0`。本轮评测使用这份新版 ACP，本机 PATH 保持前一个版本。
- 用该配套运行 `boundary_e2e --skip e_bash --nocapture`，9 项通过，包含宿主拒绝后的追问和重启续接。`e_bash` 为上一轮独立复现的已有失败。
- `cargo check --locked --workspace --all-targets` 和 `cargo clippy --locked -p aster-core --all-targets --no-deps --features ci-all,deep-live` 通过。修改文件的定向 `rustfmt --edition 2021 --config skip_children=true --check` 与 `git diff --check` 通过。全工作区格式检查仍报告 38 个已有文件。

受控任务用 Python 3.12 运行 `compare_relations.py --reference <前版 ACP> --candidate <新版 ACP> --embedding <向量模型目录> --out <新目录>`，其中 `DEEPSEEK_API_KEY` 由官方 DeepSeek ACL 注入子进程环境。固定种子的复跑传入 `--seed-directory <初次输出目录>/seed`。六轮的执行顺序和原 fixture 哈希见 [正向清单](long-a/manifest.json) 和 [反向清单](long-b/manifest.json)。

```sh
python3.12 docs/evidence/code/2026-11-27-six-turn-dsh-comparison/revisions.py \
  --fixture docs/evidence/code/2026-11-27-six-turn-dsh-comparison/fixture.json \
  --acp target/debug/aster-acp --reference-acp "$HOME/.local/bin/aster-acp" \
  --reference-deferred --dsh "$ASTER_TRIAL_DSH" --embedding "$HOME/.local/bin/embedding" \
  --out "$ASTER_TRIAL_OUTPUT" --order candidate release dsh
```

反序使用另一个输出目录和 `--order dsh release candidate`。

## Explicitly not claimed

八对受控任务不足以证明所有记忆判断和全部 Aster 能力等价。中文替换仍保留一次新版失败。六轮严格工具限制仍待解决，未命中缓存的输入量仍高于 DSH。本轮仅在本机 macOS arm64 验证，未测其他系统和模型。
