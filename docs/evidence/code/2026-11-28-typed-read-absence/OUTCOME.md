# 保留读取失败的文件不存在类型

本地文件的规范化曾把系统错误转换成字符串。Core 接到现有文件读取的 `NotFound` 时，现在保留工作区相对路径和类型。单文件文本、批量文本和图片读取都能明确报告该路径下未找到文件。路径越界和权限错误仍按原路径处理。批量读取里的其他文件继续各自返回结果。

上一轮四次六轮任务都在首次修改前列出了目录，其中三次是在读取不存在的输出文件后。此次按相同流程复跑，候选的两份缺失诊断都从对照的 208 字节缩到 76 字节，降幅 63.46%。两版的四次运行都在该诊断后使用了 `ls`。精确错误原因得到保留，额外探查依然存在。[实测首轮工具顺序与计数](summary.json)

用同一个前版 ACP 通过中继分别试验了限制额外路径访问、合并已知输入的读取，以及缩小回读条件三种提示表述。三种候选在六轮任务的严格判分中均为 0/2，且每个候选样本仍调用了 `ls`。限定路径访问的同组对照为 1/2。其他两个试验的候选总量中位数下降，严格判分与各自对照相同。三种表述均未编入产品。[只改中继提示的对照统计](summary.json)

## DeepSeek Flash 真实厂商计数

采用原 `compare.py` 和 `revisions.py`，样本、权限、超时和判分沿用冻结夹具。Aster 两边均发送默认完整工具目录。短任务四份 Aster 首请求逐字相同，均有相同的 26 个工具定义。独立 DeepSeek Harness Code 为 `0.2.2-rc.5`。各臂各运行两次并交换顺序。输入包含缓存命中，表内数字为厂商报告的 token 中位数。所有对照及能力复验共 350 条 HTTP 200 用量，逐条核对了总量和缓存计数。[冻结短任务](../2026-11-27-deepseek-harness-token-comparison/compare.py) · [冻结六轮任务](../2026-11-27-six-turn-dsh-comparison/fixture.json) · [执行参数与汇总](summary.json)

| 任务 | 版本 | 总 token | 未命中输入 | 主请求 / 全部请求 | 通过 |
| --- | --- | ---: | ---: | ---: | ---: |
| 两轮读取及命令 | Aster 前一源码版 | 30,961 | 1,265 | 4 / 6 | 2/2 |
| 两轮读取及命令 | Aster 当前版 | 31,358.5 | 4,219.5 | 4 / 6 | 2/2 |
| 两轮读取及命令 | DSH | 25,215 | 974 | 4 / 4 | 2/2 |
| 六轮修订 | Aster 前一源码版 | 213,530 | 16,007.5 | 20 / 27 | 0/2 |
| 六轮修订 | Aster 当前版 | 218,917 | 16,031 | 21 / 28 | 0/2 |
| 六轮修订 | DSH | 270,335 | 8,271.5 | 26 / 26 | 1/2 |

当前版总量比对照 Aster 高 1.28% 和 2.52%。相对 DSH，两轮高 24.36%，六轮低 19.02%。六轮三方各有 12/12 正确产物及受保护输入，Aster 两版各有 12/12 绑定核验。Aster 四次各多一个 `ls`，DSH 一次多一个 `glob`，因此出现表中的严格失败。两轮两次辅助请求来自记忆提取。[两轮判分](short/trials.json) · [两轮用量](short/provider-usage.jsonl) · [六轮正序](long-forward/trials.json) · [六轮反序](long-reverse/trials.json)

目录发现、行范围读取、混合批次和独立搜索的原能力判据，两版分别为 2/2。精确编辑及绑定 Bash 核验首轮为前版 1/2、当前版 0/2，交换顺序的独立复跑为前版 0/2、当前版 1/2。两边合计各 1/4。成功样本有精确文件字节、原样 Bash 成功回执和一个通过的绑定摘要。首轮三个失败样本都在任一读取前选择了 `ls`，随后申请了夹具拒绝的 Bash 命令。复跑结果不足以证明真实模型在该工作流上稳定通过。[目录发现](directory_discovery/trials.json) · [读取范围](read_window/trials.json) · [混合批次](batch_mixed/trials.json) · [独立搜索](search/trials.json) · [精确核验首轮](verified_edit_exact_count/trials.json) · [精确核验复跑](verified-edit-repeat/trials.json)

## 构建及故障复核

环境为 macOS 27.2.1 arm64、Rust 1.97.1，根工作区 `apps/studio` 已初始化。`cargo build --locked -p aster -p aster-acp --bin aster --bin aster-acp` 构建完成，配套 ID 为 `aster-cli-pair-v1:6e049c267f06fc40a755dd77e6d2f91ced0b36f5:aarch64-apple-darwin`。新 ACP SHA-256 为 `3bf222be49703f651d779f0ee93f8ebf90cb3d41cf8546938270335fc1a5eff0`，新 TUI 为 `efe452f6fbe9658ede1143dbf71a152a7eefaefbb624fa353ac4fa7d52edd655`。冻结前版为 `af12f52580916acf8dfe8a5fdc28a5f9ae053643`。[配套校验和](summary.json)

- `cargo test --locked -p aster-core --lib -- tools::builtin::read workspace::local::tests workspace::error::tests fact_control::tool_history_tests --test-threads=4` 通过 125 项。回归覆盖带空格及非 ASCII 字符的路径、明确缺失、目录、工作区外的符号链接，以及部分成功的混合批量读取。
- `cargo test --locked -p aster-common --lib -- tools::tests --test-threads=4` 通过 2 项。
- 用当前工作区配套运行原 `boundary_e2e --skip e_bash --nocapture`，9 项通过。`e_bash` 是此前已记录的本机沙箱失败。使用同一 ACP 和 DeepSeek Flash 运行 `real_agent_recovers_from_an_outside_workspace_refusal_in_the_same_turn --exact --ignored --nocapture`，1 项通过，3 次模型调用，保留真实拒绝并在同回合完成后续操作。[原沙箱证据](../2026-11-27-host-denial-and-pending-verification/OUTCOME.md)
- 针对用户报告的 8 小时 48 分钟回合，`lsof` 再次确认原 Aster 窗口持有配套 `a66ae4d44a4d7e3de7c76c9d4e9391fa7e96ab8a`。PATH 上的配套为 `12e4e37eca612e08288babd81f83300e20bfbae7`，其 ACP 二进制没有 `child_execution_failed` 记录字段，新构建含该字段。当前 Core 测试二进制重跑 `background_child_failure_settles_only_proven_effect_free_calls`，1 项通过，覆盖有完整证据的无改动失败与有实际写入的失败。[原回合核对及子任务修复](../2026-11-27-child-effect-closure-on-failure/OUTCOME.md)
- `cargo check --locked --workspace --all-targets` 和 `cargo check --locked -p aster-core --features ci-all,deep-live` 通过。`python3.12 tests/workspace_dependencies.py` 通过 7 项。定向 `rustfmt --check` 与 `git diff --check` 通过。全工作区 `cargo fmt --all -- --check` 有既有差异，位于 `crates/code/core/src/sse.rs` 与 Studio 的 `reasoning_transport_tests.rs`。

## Explicitly not claimed

这次精确诊断没有实现端到端节省 token。真实模型的精确编辑样本仍有波动，当前覆盖不足以结论所有能力等价。已运行的窗口、PATH 中的旧安装和原故障会话日志均保持原样。历史六笔调用未获补写或销账。Windows 和 Linux 的原生运行未在本机验证。
