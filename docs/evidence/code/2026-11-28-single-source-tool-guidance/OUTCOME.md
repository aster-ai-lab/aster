# 工具操作约束只描述一次

上一轮按首请求分项实测，Aster 默认目录的工具部分比独立 DSH 多 684 个输入 token。检查实际工具 JSON 后，选取六种能力中同一份定义内已由其他字段或工具摘要表达的文字。工具路由、枚举、属性名、默认值、范围、必填字段及原校验都沿用原实现。

- `edit` 的两种参数形式、互斥、原子性、唯一匹配和换行约束由一份工具摘要表达。字段保留类型、嵌套形状及预览、替换计数语义。
- `program` 的脚本入口在摘要中表达，模式由枚举表达，模板名称字段保留其条件必填说明。`ask_user_form` 的文档结构和 Markdown 限制在现有摘要中表达，字段保留 JSON 字符串编码要求，组件目录沿用共享源。
- `bash` 的 `command` 类型保留，摘要已有 workspace 命令与执行目标。全部权限、升级及连接授权用法沿用原字段。`search` 与 `code_symbols` 的共享上限由原范围表达，说明保留默认值、适用操作和排名检索更小的上限。

## 首请求的逐字核对

先在同一条真实 ACP 输入上隔离编辑说明，再正反各比较一次。提交官方 DeepSeek Flash，保持原函数列表、用户消息、系统消息和 Schema 结构。四份 HTTP 200 回执齐全。两次基线均为 7,318 输入 token，两次候选均为 7,204，逐次减少 114。[格式探针](transport-probe.json)

再从真实 ACP 两种运行顺序捕获第一份主请求。两组中每对的系统、环境、用户消息与目录之外的请求字段一致，26 个工具的名称和顺序一致。工具 JSON 只有上述六项的 13 处 `description` 有变化。DeepSeek 两组的实际输入再次得到 7,318 → 7,204。DSH 实际首请求为 5,864，差距由 1,454 降至 1,340。[逐字段变更与摘要](wire-invariants.json)

## 默认目录的冻结两轮任务

保留任务、受保护文件、判分、模型、输出上限、权限和超时。两个源码构建使用默认工具呈现，独立 DSH 为 `0.2.2-rc.5`，正反顺序各两次，收到 32 份完整 DeepSeek 用量。

| 版本，各两次 | 首请求输入 token | 总 token 中位数 | 未命中输入中位数 | 验收 |
| --- | ---: | ---: | ---: | ---: |
| Aster 前一源码版 | 7,318 | 31,531 | 3,865 | 2/2 |
| Aster 当前源码版 | 7,204 | 31,171 | 4,052.5 | 2/2 |
| 独立 DSH | 5,864 | 25,194.5 | 1,027.5 | 2/2 |

当前版比前一源码版少 360 总 token（1.14%），比 DSH 高 23.72%。两版每次各四个主请求、两次后台提取，DSH 每次四个主请求。缓存温度和模型输出影响端到端用量。[冻结判分](short/trials.json) · [逐请求用量](short/provider-usage.jsonl) · [汇总](short-summary.json)

## 独立能力

- 精确编辑与原样 Bash 验证。两版各 2/2 得到精确文件字节、原始成功工具回执和同一绑定核验摘要。实际路线均为 `read`、`edit`、`bash`、`read`。通用 `task_pass` 要求原文件不变，带写入任务在该字段上两版均为假，按原运行器的精确落盘和摘要断言记录。`qualified_pass` 另要求候选用了按需目录，本组按原设置为默认目录。[编辑判分](verified_edit_exact_count/trials.json)
- 真实 `program` 运行。两版各 2/2 执行指定源码，后续正确引用计算结果和新读值。每次各有一个成功的 `program` 和 `read` 回执。新版总量中位数 30,710，前版 31,279.5。[脚本判分](program_approved/trials.json)
- 真实 `search`。两版各 2/2 正确返回目标路径和值并保留源文件。候选有一次执行了三次 `search`，其余三次试验各一次。保留这次额外调用。[搜索判分](search/trials.json)
- Kimi 原生 TUI 表单。计量配套通过原冻结的 `kimi_catalog_native_viewport_and_typed_values_match_source`。覆盖共享目录 13 种组件、源图表值和八项有类型的真实输入，源表单字节保持一致。首次启动时终端子进程未获得 Kimi 配置所引用的环境变量，故在创建会话时失败，使用已有进程启动包装器注入后按相同模型、输入和判分复跑成功。[原生表单核验](native-form.txt)

## 按需目录的冻结六轮任务

两版均开启原生按需目录，沿用冻结任务、严格工具判分、权限、输出上限和两种运行顺序。与独立 DSH `0.2.2-rc.5` 比较，155 份 HTTP 200 DeepSeek 回执的用量均完整。

| 版本，各两次 | 总 token 中位数 | 未命中输入中位数 | 正确产物与保护 | 绑定核验 | 严格工具通过 |
| --- | ---: | ---: | ---: | ---: | ---: |
| Aster 前一源码版 | 173,168 | 15,985 | 12/12 | 12/12 | 0/2 |
| Aster 当前源码版 | 167,342 | 16,894.5 | 12/12 | 12/12 | 1/2 |
| 独立 DSH | 254,728.5 | 7,882 | 12/12 | — | 2/2 |

当前版总量比 DSH 低 34.31%，比前一源码版低 3.36%。前版两次和当前版一次多调用 `ls`。全程用量涵盖模型路线、输出和缓存差异。114 个输入 token 的直接因果证据来自固定首请求。[正序判分](long-a/trials.json) · [反序判分](long-b/trials.json) · [汇总](long-summary.json)

## 构建与验证

macOS 27.2.1 arm64，仓库指定 Rust 1.97.1，根工作区 `apps/studio` 已初始化。

- `cargo build --locked -p aster -p aster-acp --bin aster --bin aster-acp` 构建计量配套 `aster-cli-pair-v1:e781c1e49955e93ac0da4e47f961116df5ebc4cb:aarch64-apple-darwin`。TUI SHA-256 `2ef1dc6004e22ef0dfc00979983fae79351574947afc78ef6609003b95223054`，ACP SHA-256 `7992ef92f1e4a4020a78440449f20e5ba88b8e6c8848340d4ac0981a76be16b5`。前一源码版配套 ID `2d579114ec4254f41d94e974ef67a0f88c1b7863`，ACP SHA-256 `48c9407ad2d99066258efec824ee1defd516aab749547cb8b973a1d033645984`。
- 修复 Pager 编译后按同一命令构建最终配套 `aster-cli-pair-v1:53de35f9931bce6824847ad04147713a04d7628e:aarch64-apple-darwin`。TUI SHA-256 `12af47f40dc83830a2925abbc1d0abe46b6ae8ff74ae35d07482e6488605e0cb`，ACP SHA-256 `08c8165da44211983aec533b3ff9cf8f65a714d798b485f99f6a42c3a3ad718d`。最终配套的冻结两轮复跑通过，得到 31,067 总 token。最终与计量配套的首请求工具目录逐字一致，完整请求只随隔离工作区位置变化。[完整请求核对](final-pair-request.json) · [最终配套厂商回执](final-pair-provider-usage.jsonl)
- `cargo build --locked -p aster-core --lib` 通过，`target/debug/libaster_core.rlib` SHA-256 为 `6dc46fd8608690e66c61e6d49ffc50f6d861a397045c4a5e8c03ab3a555c7536`。
- `cargo test --locked -p aster-core --lib -- tools::builtin::edit tools::builtin::bash tools::builtin::search tools::builtin::code_intelligence tools::builtin::ask_user_form tools::program_tool llm::openai --test-threads=4` 通过 273 项，忽略两个显式标记的长期沙箱和外部 rust-analyzer 测试。
- Core `cargo clippy --locked -p aster-core --all-targets --no-deps --features ci-all,deep-live` 通过。改动文件候选告警为 `bash.rs` 已有互斥枚举的两处不可达测试分支，`HEAD` 已有。定向 `rustfmt --edition 2021 --config skip_children=true --check`、`git diff --check` 及 `python3.12 tests/workspace_dependencies.py` 通过。全量格式检查报告 43 个文件，42 个与此前相同，第 43 个为并行改动的 `entry_renderer.rs`。
- `cargo check --locked --workspace --all-targets` 首次在并行编辑的 `scrollback_pane.rs` 编译失败。时间戳覆盖分支使用了另一个函数局部定义的 `ts_reserved`。接入现有宽度判断，继续用与换行相同的条件决定绘制。补齐 `AppearanceConfig` 的借用后，全工作区检查通过。Pager 的 `cargo clippy --locked -p aster-pager --all-targets --no-deps` 通过，新改动的文件无候选警告。该文件的 2024 版定向 `rustfmt` 通过。
- 最终配套运行原 `boundary_e2e --skip e_bash --nocapture`，9 项通过。跳过项在本机有独立沙箱失败记录。最终配套的 Kimi 原生表单复跑也通过。[原生边界基线](../2026-11-27-host-denial-and-pending-verification/OUTCOME.md) · [最终表单](final-native-form.txt)
- `cargo test --locked -p aster-pager --lib -- pushed_sticky_header_lines_are_rebased_onto_visible_rows test_timestamp_not_rendered_when_narrow --test-threads=2` 通过 2 项，覆盖粘性标题选区的可见行转换和窄窗时间戳。
- 原冻结两轮运行 `compare.py --release "$REFERENCE_ACP" --candidate "$ACP" --dsh "$DSH" --embedding "$EMBEDDING" --order candidate,release,dsh dsh,release,candidate --out "$OUT"`。六轮用 `revisions.py --reference-deferred`，分别按 `--order candidate release dsh` 和 `--order dsh release candidate`。均用环境接收既有 DeepSeek 凭据。[两轮脚本](../2026-11-27-deepseek-harness-token-comparison/compare.py) · [六轮脚本](../2026-11-27-six-turn-dsh-comparison/revisions.py)
- 原独立用例使用 `full_catalog.py --default-catalog` 的 `--scenario verified_edit_exact_count --disk-content-attestation`、`--scenario program_approved` 和 `--scenario search`。终端表单使用 `aster_tui_live_e2e --ignored --exact forms_catalog::kimi_catalog_native_viewport_and_typed_values_match_source --nocapture`，由原 Kimi ACL 和已构建的配套进程启动。[能力脚本](../2026-11-27-model-selected-tool-catalog/full_catalog.py)

## Explicitly not claimed

首请求测量支持这组工具说明的成本变化。默认目录的短任务仍高于独立 DSH，按需长任务的严格工具判分和缓存未命中输入仍有差距。未作跨所有任务或模型的能力等价声明。本机验证使用 macOS arm64，未执行原生 Linux 或 Windows 测试。本机产物放在当前工作区的 `target/debug`，原运行进程和历史会话由各自的宿主管理。
