# 批量读取中重复文件段的复用

原批量读取按整份工具回执比较。一次批量读取含有一个变化文件时，其他内容没变的文件段也会随工具历史反复发送。先前冻结的六轮样本中，两个 Aster 试验各有 8 个重复文件段，累计 4,320 字节。样本同时出现了模型连续发出独立读取的情况，收益取决于实际工具调用形态。[前轮数据](../2026-11-27-cache-prefix-experiments/OUTCOME.md)

`read.files` 在生成文本时记录已完成文件段的 UTF-8 边界，标记绑定完整输出摘要、解析后的工作区路径和读取范围。Core 仅在当前模型请求能找到先前完整可见的相同段时，保留当前文件标题并将重复正文替换为引用。原回执、实际读取、错误文本、续读参数和变更核验保留。改写过输出的回执、旧回执和压缩后缺少来源的情况按原文发送。运行时依据结构和字节比较，文件文字里的标题或指令无法决定分段。

## 独立批量读取对照

冻结两个只读回合，先读取目录与当前状态，再以相反顺序读取备用状态与目录。两版都使用相同的按需工具目录、DeepSeek Flash、关闭思考、temperature 0，且在两种出场顺序中各运行一次。四次试验均完成两次实际 `read.files`，产出全部预期值，输入文件保持原始字节。每回合完整工具回执的输出 SHA-256 在四次试验间一致。新版两次在第二轮交付请求中都发出一处引用。

| 两轮试验中位数 | 原版 Aster | 新版 Aster |
| --- | ---: | ---: |
| 总 token | 28,603 | 27,479 |
| 第二轮交付请求的输入 token | 8,111 | 6,880 |
| 第二轮交付请求的未命中缓存输入 token | 1,327 | 224 |
| 第二轮交付请求的工具消息字节 | 9,891 | 5,314 |
| 原判分通过 | 2/2 | 2/2 |

这项任务减少 1,124 个总 token，降幅 3.93%。第二轮交付少 1,231 个输入 token。完整回执与厂商请求用量分别复核，工作区文件内容和值均由既有检查确认。[回执摘要与统计](batch-receipts.json) · [冻结合约](read_batch_repeat/manifest.json) · [厂商用量](read_batch_repeat/provider-usage.jsonl)

## 保持冻结输入的六轮对照

沿用原 `revisions.py` 和六轮夹具，按 `candidate release dsh` 和 `dsh release candidate` 顺序执行。两组 Aster 都显式启用按需目录。独立 DSH 为 `0.2.2-rc.5`。183 个厂商请求均返回 HTTP 200 和完整用量。

| 六轮对照 | 总 token 中位数 | 未命中缓存输入中位数 | 产物及输入保护 | 严格工具通过 |
| --- | ---: | ---: | ---: | ---: |
| 原版 Aster | 176,630 | 15,479.5 | 12/12 | 0/2 |
| 新版 Aster | 167,634 | 15,231 | 12/12 | 0/2 |
| 独立 DSH | 280,880.5 | 7,536.5 | 12/12 | 2/2 |

两组 Aster 的每轮产物均取得绑定核验，每次运行仍多发出一次 `ls`。新版总 token 中位数比本次 DSH 样本低 40.32%。实际六轮对照没有在厂商请求中触发批量文件段引用，观测到的总量差异不能归因于本次修改。批量复用的独立对照单独验证该项效果。[正向](long-a/trials.json) · [反向](long-b/trials.json) · [汇总](long-summary.json)

## 本机验证

- 根工作区、macOS arm64、仓库固定 Rust 1.97.1，`apps/studio` 已初始化。`cargo test --locked -p aster-core --lib -- batch_read_projection tools::builtin::read tool_history_tests --test-threads=4`，67 项通过。新增回归使用实际 `ReadTool`，覆盖非 ASCII 路径、空格、同批错误、内容变化、改写和压缩后失去来源。
- `cargo test --locked -p aster-acp --test long_horizon_real_llm --no-run`、`cargo build --locked -p aster -p aster-acp --bin aster --bin aster-acp` 通过。新 `aster` 和 `aster-acp` 的配套 ID 为 `aster-cli-pair-v1:3fe1f16ef297ac4b5c83981bbb81cc9bb8459d30:aarch64-apple-darwin`。以该配套运行 `boundary_e2e --skip e_bash --nocapture`，9 项通过。排除原生沙箱用例的基线证据见[终端边界记录](../2026-11-27-host-denial-and-pending-verification/OUTCOME.md)。
- 候选 ACP 的 SHA-256 为 `ec41951909931c8bf941c495cd09ac6e13001c7d5477d1d166006e5bf17eddb8`。原版 ACP 的 SHA-256 为 `bb14c0113330c13a6c7eb69107739a5ee12d10ba0dcb744715506cf9ed5c6e8c`。
- 独立场景使用 `full_catalog.py --scenario read_batch_repeat --native-catalog --reference-acp … --acp … --embedding … --out …`，无代理改写工具定义。另复跑原分页和精确命令场景。
- `cargo check --locked --workspace --all-targets`、`cargo clippy --locked -p aster-core --all-targets --no-deps --features ci-all,deep-live` 和 `git diff --check` 通过。定向 Rust 格式检查通过，全工作区 `cargo fmt --all -- --check` 仍报告原有的 38 个文件。`/opt/homebrew/bin/python3.12 tests/workspace_dependencies.py`，7 项通过。
- 两组各通过分页 2/2，调用权限与实际范围准确。两组各通过精确命令 2/2，所有试验的 Bash 命令都取得成功回执，文件字节与一次绑定的改动核验吻合。[分页结果](read_window/trials.json) · [精确命令结果](verified_edit_exact_count/trials.json)
- 用这对二进制运行既有 `real_native_file_workflow_multiple_user_turns --ignored --exact --nocapture`。`kimi-01/k3.1-pro` 的 6 轮产物、输入保护和宿主绑定核验均通过。31 次上游 HTTP 200 均取得用量，共 176,057 token。[Kimi 结果](kimi-result.json)

## Explicitly not claimed

收益取决于原生批量 `read.files` 是否观察到可复用的大文件段，独立场景的幅度不外推到所有任务。样本没有覆盖其他平台实机及所有模型。未计算账单费用。
