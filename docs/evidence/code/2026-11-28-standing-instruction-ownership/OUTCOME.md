# 常驻指令的责任归并

Core 的操作循环保留变更、差异、权限和检查顺序，工具输出、退出码和可观测效果由同一运行时证据段描述。会话权限、升权限、取消、工具精确参数、错误恢复和完成条件仍由原宿主契约执行。新交互会话采用压缩的 `engineering-v4`。审查分工、只读回退、具备适用性的检查、禁止弱化验证以及延迟事项边界仍在策略中。已有 `engineering-v1` 至 `engineering-v3` 的名称、版本和原文可按保存的身份重开。无人值守默认仍解析 `engineering-v1`。 [策略正文](../../../../crates/code/acp/prompts/engineering_v4.md)

初始文案试稿在同一份首请求上节省 61 个输入 token。最终纳入本机构建的版本保留了工作区相对路径、仅实施请求可改文件和原有停止条件等更明确的约束。真实 ACP 捕获的两对首请求只有系统正文不同，均包含同一份 26 个定义，参数 Schema、工具说明、顺序、用户消息和其他请求字段逐字相同。系统正文从 7,367 降到 7,226 字节，DeepSeek 输入从 7,203 降到 7,162。 [试稿计量](draft-usage.json) · [实际请求不变量](wire-invariants.json) · [厂商回执](measured/short/provider-usage.jsonl)

## DeepSeek 对照

使用真实 DeepSeek Flash Chat Completions、temperature 0 和关闭思考。冻结两轮和单项能力的输出上限为 1,024，六轮为 4,096。独立 DSH 为 `0.2.2-rc.5`。两组 Aster 都用原生完整工具目录。沿用已有输入、权限、超时和判分，正反各一次。构建对照共 36 次任务和 358 条完整 HTTP 200 用量。输入包含缓存命中，缓存未命中另记。

| 总 token 中位数，三方各两次 | Aster 前版 | Aster 当前版 | 独立 DSH |
| --- | ---: | ---: | ---: |
| 两轮读文件与命令 | 31,021 | 30,871 | 25,216 |
| 六轮文件修订 | 216,600.5 | 214,224 | 246,327 |
| 两轮任务通过 | 2/2 | 2/2 | 2/2 |
| 六轮严格工具通过 | 0/2 | 0/2 | 2/2 |
| 六轮正确产物与受保护字节 | 12/12 | 12/12 | 12/12 |
| 六轮效果绑定核验 | 12/12 | 12/12 | — |

当前版短任务比前版低 0.48%，比 DSH 高 22.43%。六轮比前版低 1.10%，比 DSH 低 13.03%。两次 Aster 六轮均调用了冻结任务额外的 `ls`。前版每次一次，当前版一次两次、另一次一次。未命中输入中位数在两轮分别为 4,055.5、4,728.5 和 1,034.5，在六轮分别为 19,566.5、19,318.5 和 7,908。 [两轮判分](measured/short/trials.json) · [六轮正序](measured/long-forward/trials.json) · [六轮反序](measured/long-reverse/trials.json)

精确分页、真实搜索、MCP、脚本和精确编辑绑定核验两版均通过 2/2。混合批次前版 0/2、当前版 2/2。当前版搜索总量中位数为 44,593.5，对照为 40,460，一次额外调用 `search`。精确编辑当前版为 50,907，对照为 46,842.5，一次额外调用 `ls`。精确编辑使用原运行器的实际文件字节、成功核验回执、绑定摘要和结束状态判据。 [分页](measured/read_window/trials.json) · [搜索](measured/search/trials.json) · [混合批次](measured/batch_mixed/trials.json) · [精确核验](measured/verified_edit_exact_count/trials.json)

单独试验在同一前版 ACP 的中继中标注已装入的 `AGENTS.md` 来源与完整性。两种合成夹具共八次任务、48 条 HTTP 200 用量。原生设置夹具两边各完成 2/2，实际调用均为两次读取 `service.cfg`、一次编辑和一次核验。逐字合同夹具两边均额外尝试 Bash，获准范围只覆盖指定核验命令，因而各为 0/2。两夹具均未观测到候选或对照重读已装入的根指令文件。来源标签没有接入运行时。 [原生设置夹具](instruction-native-probe/proof/trials.json) · [逐字合同夹具](instruction-byte-probe/proof/trials.json)

## 构建与验证

macOS 27.2.1 arm64、Rust 1.97.1。构建命令从已初始化 `apps/studio` 的根工作区运行。

- `cargo build --locked -p aster -p aster-acp --bin aster --bin aster-acp` 成功。两者配套 ID 为 `aster-cli-pair-v1:af12f52580916acf8dfe8a5fdc28a5f9ae053643:aarch64-apple-darwin`。`target/debug/aster` SHA-256 为 `a578eeb899c5b7504bb6d9eb8463fe4b8c6a662243a12aa2a2d7e3b9df55b383`，`target/debug/aster-acp` 为 `a5ac8519a9004d14c58f9abb9c09272ba1c9b32aaa4605c7dcac98c2ef8410a7`。冻结前版 ACP 为 `40758b5399df33c8e2eadce2e72de6ff52798b83`，SHA-256 `3e1852bdff970b47acdcdd268b7cacef4aa0df054aa6a6b31babad20446b50cb`。
- `cargo test --locked -p aster-acp --lib -- engineering prompt_boundary session_setup count_limit reasoning_transport --test-threads=4` 通过 41 项，3 项按既有标记忽略。验证了新会话默认、旧选择重开、审查职责和提示边界。`engineering_v3.md` 的 SHA-256 在修改前后均为 `50027ef40006a227ca759bd02f56417360d9931c5790db52cbc647602ae58c71`。
- `cargo test --locked -p aster-core --lib -- prompts:: context::assembler agent_api::tests::fact_log::tool_call_fidelity agent_api::project_instructions --test-threads=4` 通过 74 项，`diagnostic_calls_keep_literal_arguments_and_runtime_refusals_in_model_history` 失败。该断言期待后续请求只有用户原文。对前版和当前 ACP 分别用固定提供方做隔离复放，同样都保留了用户原文并附加一条逐字相同的 `[host tool recovery]`。两边的拒绝都是零变更的完整回执。保留了原失败断言。 [前后配套的复放](refusal-replay.json)
- 当前 ACP 的原 DeepSeek 越界拒绝恢复用例通过。实际三次模型调用，先拒绝，后读取获准文件。当前配套运行 `boundary_e2e --skip e_bash --nocapture` 通过 9 项，跳过一项已有本机沙箱失败。原 Kimi `kimi_catalog_native_viewport_and_typed_values_match_source` 通过，覆盖 13 种组件和八项有类型的提交。 [DeepSeek 恢复](live-refusal.txt) · [沙箱基线](../2026-11-27-host-denial-and-pending-verification/OUTCOME.md) · [TUI 边界](native-boundary.txt) · [Kimi 表单](native-kimi.txt)
- `cargo check --locked --workspace --all-targets`、`cargo check --locked -p aster-core --features ci-all,deep-live`、`cargo clippy --locked -p aster-core -p aster-acp --all-targets --no-deps --features aster-core/ci-all,aster-core/deep-live,aster-acp/headless-search` 均通过。定向复核未发现本轮 Rust 改动引入的 Clippy 诊断。`cargo build --locked -p aster-core --lib` 通过，`target/debug/libaster_core.rlib` SHA-256 为 `8531ba3cad8f9f8801fbef311158fb07a8d392673ed4799d6ca03e0db6ba12a7`。
- 定向格式检查、`git diff --check` 和 `/opt/homebrew/bin/python3.12 tests/workspace_dependencies.py` 通过，依赖检查 7 项。`cargo fmt --all -- --check` 仅报告原有 `sse.rs` 和 Studio `reasoning_transport_tests.rs` 两处文件差异。
- 按原 `compare.py` 使用 `--release "$REFERENCE_ACP" --candidate "$ACP" --dsh "$DSH" --embedding "$EMBEDDING" --out "$OUT" --order candidate,release,dsh dsh,release,candidate` 复跑两轮。六轮用本目录 `default_revisions.py` 复用原 `revisions.py` 的同一 fixture、上限和判分，参数 `--acp "$ACP" --reference-acp "$REFERENCE_ACP" --dsh "$DSH" --embedding "$EMBEDDING" --fixture "$FIXTURE" --out "$OUT" --order candidate release dsh`，再反序运行。两组通过环境变量 `DEEPSEEK_API_KEY` 供给已获准的密钥。 [两轮脚本](../2026-11-27-deepseek-harness-token-comparison/compare.py) · [六轮脚本](default_revisions.py) · [六轮夹具](../2026-11-27-six-turn-dsh-comparison/fixture.json)

## Explicitly not claimed

短任务默认配置仍高于 DSH，六轮严格工具判分仍有差距。两轮缓存未命中并未下降。实际用量不足以推断费用和跨模型能力等价。构建位于 `target/debug`，本次未替换系统 CLI 和已运行的会话。未运行 Windows、Linux 原生检查或 Studio GUI。
