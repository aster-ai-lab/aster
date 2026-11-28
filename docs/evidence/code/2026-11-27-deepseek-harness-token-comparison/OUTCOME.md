# DeepSeek Flash 下的 Harness token 对比

macOS 27.2.1 arm64，Rust 1.97.1。三组各运行三次同一项两轮任务，9/9 通过。48 个模型 API 请求全部返回 HTTP 200 和完整的 DeepSeek `usage`，合计 297,816 token。

## 每项两轮任务的中位数

| 指标 | Aster 已发布版 | Aster 当前源码版 | DSH `0.2.2-rc.5` |
| --- | ---: | ---: | ---: |
| 任务通过 | 3/3 | 3/3 | 3/3 |
| 总 token | 37,218 | 37,166 | 24,829 |
| 输入 token | 36,912 | 36,886 | 24,510 |
| 输出 token | 306 | 280 | 319 |
| 命中缓存的输入 token | 35,836 | 35,712 | 23,680 |
| 未命中缓存的输入 token | 1,143 | 1,265 | 840 |
| 首次模型请求的输入 token | 8,665 | 8,665 | 5,806 |
| 真实模型请求 | 6 | 6 | 4 |
| 携带工具定义的请求 | 4 | 4 | 4 |
| 工具调用 | 3 | 3 | 4 |
| 两轮 ACP 请求耗时之和 | 9.13 s | 9.07 s | 7.42 s |
| 三次总 token 逐次值 | 37,210 / 37,218 / 37,302 | 37,290 / 37,166 / 37,160 | 24,794 / 24,829 / 24,847 |
| 三次总 token 合计 | 111,730 | 111,616 | 74,470 |

与发布版相比，Aster 当前源码版的任务总 token 中位数少 52，差异为 0.14%。DSH 比当前源码版少 12,337，差异为 33.19%。三组每次都向模型发了四个携带工具定义的请求。第一次请求上，Aster 两版都是 8,665 输入 token 和 26 个工具定义，DSH 是 5,806 和 23 个工具定义。Aster 另有两个未携带工具定义的请求，当前源码版每次共用 1,228 至 1,265 token。逐项中位数分别计算，缓存列和输入列可能有取样差异。

## 构建与验证

| 命令 | 结果 |
| --- | --- |
| `cargo build --locked -p aster-core -p aster-acp -p aster` | 本地 debug 构建通过，生成 `target/debug/libaster_core.rlib`、`target/debug/aster-acp`、`target/debug/aster`。Core 和 ACP 仍使用 Cargo 版本 `9.3.2`，TUI 为 `0.4.2`。 |
| `target/debug/aster --build-id` 和 `target/debug/aster-acp --build-id` | 相同，均为 `aster-cli-pair-v1:3459c8228f499bfa917f17864b2c57802d767d02:aarch64-apple-darwin`。候选基线为 `1be48a0e2115` 加本地恢复修复。 |
| `~/.local/bin/aster --version`、`~/.local/bin/aster-acp --version` 和两者的 `--build-id` | 发布对照为 TUI `0.4.2`（`61b8abbb845e`）和 ACP `9.3.2`。两者标识均为 `aster-cli-pair-v1:a66ae4d44a4d7e3de7c76c9d4e9391fa7e96ab8a:aarch64-apple-darwin`。 |
| `npm install --prefix <隔离目录> --registry https://registry.npmjs.org --no-audit --no-fund --save-exact @deepseek-ai/dsh@0.2.2-rc.5` | 安装及 `dsh --version` 通过。NPM 完整性为 `sha512-cNbp12tLYLUTXaVli7zwwMjzmmEVCD1PRldg4oKtXM4+lFYckcaliMS2PU/FoThB4iAN/fKOSID0nrtRZU4Gjw==`。 |
| `ASTER_CONFIG_FILE="$PWD/../config.acl" ASTER_PINNED_MODEL=deepseek/deepseek-flash ASTER_TUI_BIN="$PWD/target/debug/aster" ASTER_ACP_BIN="$PWD/target/debug/aster-acp" cargo test --locked -p aster --test tui_913_real_llm -- real_llm_tui_turn_shows_assistant_text --ignored --exact --nocapture --test-threads=1` | 1/1 通过。新 TUI 在真实 PTY 由新 ACP 承接 DeepSeek 实际请求，收到 `end_turn` 且显示了最终回复。 |

SHA-256 在上述终端测试结束后重新计算。

| 本机产物 | SHA-256 |
| --- | --- |
| `target/debug/aster` | `5cbf253cfe64cedb0d840fc2855e8a8adb4b2e178099cd6c322e76bad4d8f6d3` |
| `target/debug/aster-acp` | `e25dc9f28ab5c209c70217aae4f4a39fe842128bdf2d48e9218dba6eecb5ae7e` |
| `target/debug/libaster_core.rlib` | `fa40e37d2357306b6e49c9c5f5489ea159a4cd036889d152e49bf8dbd72816e8` |

## 任务与计数

沿用仓库里已有的两轮 [token 和缓存测试](../../../../crates/code/acp/tests/live_interactive_dialogue_real_llm.rs#L1521) 的提示、三个文件及必须逐字返回的标记。首轮读取两个文件并执行指定 `printf`，第二轮读取第三个文件并引用首轮命令输出。三边都须返回两个 ACP `end_turn`、全部正确标记且三个文件字节一致。Aster 同时检查原有的工具回执 ANSI 条件，六次均通过。

三边使用相同的 ACP 客户端、工作区绝对路径及新会话，执行顺序依次为发布版、当前版、DSH，再是当前版、DSH、发布版，最后是 DSH、发布版、当前版。Aster 两版使用同一套本机 embedding 资产。DSH 使用官方命令行的 `acp` profile，其 `dsh-llm-pi-ai` 自定义路由使用 OpenAI Chat Completions，以便三边经过同一个 DeepSeek API 计量入口。 [DSH CLI 文档](https://github.com/deepseek-ai/deepseek-harness/blob/master/docs/cli-help.md) · [DSH 模型配置](https://deepseek-harness.github.io/deepseek-harness/en/guide/providers)

本地转发器把三边统一到 `https://api.deepseek.com/chat/completions`、`deepseek-flash`、关闭思考、`temperature=0` 和输出上限 1,024，并请求流式 `usage`。所有模型输入、工具目录、轮数由各自的 Harness 生成。已有 ACL 的 DeepSeek 密钥经 `aster-acl` 读取，只用于转发器向官方端点认证。保留的请求统计仅包括计数、模型 ID、状态和 token 等元数据。服务端 `total_tokens` 包含缓存命中输入。逐笔检查了 `输入 + 输出 = 总数` 以及 `命中 + 未命中 = 输入`。 [DeepSeek Chat Completions 的计数定义](https://api-docs.deepseek.com/api/create-chat-completion/)

复跑时向脚本进程提供 `DEEPSEEK_API_KEY`。输出目录应尚未创建。`release` 和 `candidate` 指向对应 ACP 二进制，`dsh` 指向独立安装的 DSH CLI。

```sh
python3 docs/evidence/code/2026-11-27-deepseek-harness-token-comparison/compare.py \
  --release "$HOME/.local/bin/aster-acp" \
  --candidate target/debug/aster-acp \
  --dsh /path/to/dsh/node_modules/.bin/dsh \
  --embedding "$HOME/.local/bin/embedding" \
  --out /tmp/aster-deepseek-repeat \
  --order release,candidate,dsh candidate,dsh,release dsh,release,candidate
```

[固定参数和夹具摘要](manifest.json) · [9 次任务结果](trials.json) · [48 次厂商 usage 回执摘录](provider-usage.jsonl) · [评测脚本](compare.py)

复核了脚本语法、证据文件与实际输出的字节一致性、任务通过数、逐笔 usage 与分组中位数。`git diff --check` 通过。

## Explicitly not claimed

- 这是一项两轮读文件和命令任务的结果。拒绝恢复、长程编码、默认思考档和 DSH 默认 Anthropic Messages 路由需要各自的样本。
- 服务端缓存可能在试跑和样本之间复用。耗时反映这次本机测量，缓存与 token 数不能直接当作账单。
- 构建是本地 debug 版本。没有覆盖系统中的 CLI 安装、正式发布或 Windows／Linux 实机验证。
