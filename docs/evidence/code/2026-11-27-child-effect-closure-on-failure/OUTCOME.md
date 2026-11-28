# 子任务失败后的效果结算

用户报告的长回合有七条后台子任务启动回执，父日志只保存了一条匹配的后台结算。其余五条子日志在收到空模型答复之后记录了自动续写输入，未出现后续模型决定。另一条子日志有七条文件变更记录且缺少结算。五次空答复对应的最后一份模型输入介于 47,805 和 256,441 token。这里仅汇总现有记录。

再次核对用户报告的 `8h48m`。本机父回合从 14:49:07 持续到 23:37:51，终态事件的 6 个未结算身份均为 `workspace-child`，逐个匹配原后台子日志。五条空答复分支共留下 407 条完整的工具回执，零文件变更。另一条子日志的 70 条工具回执中有一条保存了 7 条文件变更。父日志在该回合共保存 558 条模型决定事实和 739 条工具结果事实。最后一条子日志于 15:44 停止增长，父回合继续运行至结束。检查时，报错窗口的 TUI 和 ACP 仍持有 14:18 加载的 `a66ae4d44a4d7e3de7c76c9d4e9391fa7e96ab8a` 配套。只读检查了日志与进程路径。

在隔离工作区复现了已完成工具调用后收到空答复、再遇到续写拒绝的路径。修改前，原始 `read` 已留下完整回执，子任务已退出并报告失败，父回合仍因一笔 `tool_effect_unresolved` 拒绝交付独立读取的结果。

原始 ChildRun 现在在退出仍持有日志所有权时处理受控失败。只有现有完成门允许无变更终结，已提交用户输入的日志前缀完整，而且原始事件折叠后的全部观察完整、零变更、零未结算身份，才落一条带 `child_execution_failed` 的效果结算事实。随后按原失败返回。后台宿主仍通过原有路径绑定父调用、子任务 ID、子日志切点及实际观测。若已发生变更、记录不完整、执行身份未结清或原进程直接崩溃，原核验契约负责拦截。

## 独立能力复验

原有 `full_catalog.py` 以 DeepSeek Flash、按需目录和相同判分分别测试 MCP 精确参数、真实编辑和原样 Bash 核验。每组各运行两次。两版均通过 MCP 2/2 和编辑核验 2/2。MCP 总 token 中位数从 29,004 到 29,129，编辑核验从 35,354.5 到 35,584.5。两项对照记录了小幅增加。两组请求均收到完整用量。[MCP 判分](mcp/trials.json) · [编辑核验判分](exact-verification/trials.json)

## 六轮 DeepSeek 对照

使用原 `revisions.py`、同一冻结夹具、工具范围、超时、权限和判分。按需目录下 Aster 两版与独立 DSH `0.2.2-rc.5` 同用 DeepSeek Flash、temperature 0、关闭思考、4,096 个输出 token 上限和 40 次上游请求上限。两个顺序共收到 160 条完整 HTTP 200 用量。[正序判分](long-a/trials.json) · [反序判分](long-b/trials.json) · [逐组用量汇总](long-summary.json)

| 六轮任务，各两次 | 正确产物 | 绑定核验 | 严格工具通过 | 总 token 中位数 | 未命中输入中位数 |
| --- | ---: | ---: | ---: | ---: | ---: |
| Aster 前一源码版 | 12/12 | 12/12 | 0/2 | 172,187.5 | 17,417.5 |
| Aster 当前源码版 | 12/12 | 12/12 | 0/2 | 176,972 | 17,238.5 |
| 独立 DSH | 12/12 | — | 1/2 | 256,230.5 | 8,247 |

六次均保留受保护输入。当前构建总量比对照 Aster 高 2.78%，比 DSH 低 30.93%。Aster 各次多调用一次 `ls`，DSH 一次多调用 `glob`。此六轮工作流没有启动后台子任务，衡量的是常规流程复验。故障路径的直接证据来自隔离的子任务失败用例。[正序原始回执](long-a/provider-usage.jsonl) · [反序原始回执](long-b/provider-usage.jsonl)

## 本地验证

环境为 macOS 27.2.1 arm64、Rust 1.97.1，根工作区 `apps/studio` 已初始化。

- `cargo test --locked -p aster-core --lib background_continuation_failure_settles_only_the_effect_free_child -- --nocapture` 在实现前 1 项失败。父回合返回一笔未结算的子任务身份。
- 首轮和最终 `cargo test --locked -p aster-core --lib -- background_subagent child_run:: fact_control::receipts:: receipt_restore_tests::cache_coverage --test-threads=4` 各通过 183 项。最终 `background_child_failure_settles_only_proven_effect_free_calls` 同时验证首次推理失败、读后续写失败、写后续写失败三个原始子任务。前两例保留失败状态而允许父任务交付，第三例保留一处实际写入及其待核验状态。测试对子日志原始切点与父日志摘要逐项核对。
- 追加 `target/debug/deps/aster_core-7265b12a14b39627 tools::task::tests tools::builtin::task_control agent_api::tests::background_settlement --test-threads=1`，175 项通过，1 项忽略，`task_child_run_step_budget_enforced` 1 项失败，单独运行也失败。旧测试期待拒绝超限工具后整个子任务失败。用本轮前版和新版 ACP 配套分别在同一隔离工作区复跑两步上限场景。两边均只准备两个 `read`，后两次模型调用的工具定义为零，经过文本纠正后都返回答案。16:21 编译的更早 Core 单测二进制在同一旧断言通过。[相邻配套的实际步数对照](step-cap-diagnostic.json)
- `cargo build --locked -p aster -p aster-acp --bin aster --bin aster-acp` 成功，配套 ID 为 `aster-cli-pair-v1:04ad02027f70b3ae4ff4e7b294bd2398fa1528e7:aarch64-apple-darwin`。`target/debug/aster` SHA-256 为 `b4d3d6e3b3f0e05de895305eda9b6683cd8f50290567d77c0907318711799ff4`，ACP 为 `09f7e2990d66897819e9382aa72c935b42c0efea187b03a01e7d0b7d42680c60`。前版 ACP SHA-256 为 `ccc870bb2ec0ce2cf0e831fbc53507bdaf8baef7f11b988c04330be261588cf5`。
- 用上述配套运行 `boundary_e2e --skip e_bash --nocapture`，9 项通过。`e_bash` 为已有的本机沙箱失败。[边界基线](../2026-11-27-host-denial-and-pending-verification/OUTCOME.md)
- `cargo check --locked --workspace --all-targets`、`cargo clippy --locked -p aster-core --all-targets --no-deps --features ci-all,deep-live`、定向 `rustfmt`、`git diff --check` 通过。Clippy 有已有警告。`cargo fmt --all -- --check` 仍报告 38 个已有格式差异文件。`/opt/homebrew/bin/python3.12 tests/workspace_dependencies.py` 通过 7 项。
- 六轮按原先的 `revisions.py --reference-deferred` 对照复跑，使用 `--order candidate release dsh` 和 `--order dsh release candidate`。[冻结运行器与完整参数](../2026-11-27-six-turn-dsh-comparison/OUTCOME.md)
- 独立能力使用 `full_catalog.py --native-catalog --reference-acp "$REFERENCE_ACP" --acp "$ACP" --embedding "$EMBEDDING" --out "$OUT"`。两个场景分别是 `--scenario mcp_schema` 和 `--scenario verified_edit_exact_count --disk-content-attestation`。既有 DeepSeek 凭据由环境提供。[原能力运行器](../2026-11-27-model-selected-tool-catalog/full_catalog.py)

## Explicitly not claimed

用户报告对应的是现有旧会话，其历史日志仍保留原来的未结算状态。实际五次续写的底层拒绝原因缺少独立诊断记录。运行时变化通过隔离夹具验证，未声称恢复了缺少原始终态的历史调用或已验证真实工作区改动。
