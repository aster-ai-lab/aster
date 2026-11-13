# 委派子任务工具事件桥接

状态：原生修复、focused 回归、core／ACP Clippy、配对 release 构建及格式检查通过。
真实模型的严格 ACP 委派探针已由 KRun 独立复验通过；KRun 三套 Harness 的三项完整
长程检查现已全部通过，Aster 最终 3/3 PASS。第 19 轮余额阻断及后续恢复超时的
历史失败副本原样保留，超时根因未定位。

KRun 中 aster-acp `9.2.2` 归档 `acp-0.1.4.zip` 的第 22 轮长程任务暴露了 10 个只有开始而没有终态的子任务工具。
实际 bash 已完成，但 `fact_control/executor_tools.rs::host_bridge_event` 过滤了子任务的
ToolExecutionStart、ToolOutputDelta、ToolEnd 和 PermissionDenied。修复在共享桥接
过滤入口补齐这四类事件，保留原生 worker、权限与沙箱，不改 ACP 客户端的终态要求。

新增 `test_stream_settles_delegated_tool_calls_before_the_parent`：通过真实 SDK stream、
task 委派和 read 工具覆盖文件存在／不存在两种结果；核对子工具实际 ID、退出码与输出，
以及子工具和子任务均先于父工具结束。重用现有 scripted LLM fixture，不复制生产逻辑。
已有 `test_stream_bridges_subagent_lifecycle_events` 同时通过。

构建在 KBox Linux/aarch64，宿主 macOS/arm64 HVF，Rust/Cargo 均为 pinned 1.97.1。
使用已有冻结源码 checkout，基于 `6a8bf40f3d10101165e18cf0f60fbd17e6213f81` 的 dirty
快照；此次两个 core 文件和先前五个 ACP 文件均与指定本地原仓库相同。没有创建新仓库
或 worktree。[源码完整性](checks/source-integrity.json)核对 14,008 个文件／链接条目，
10 个相关源码／夹具路径与原仓库一致；快照摘要为
`sha256:c4e974fa9100db04c2c8d66f5b179aea7cfe21d9e206bedbb8d84c9303be4340`。

实际命令和挂载保留于[构建命令](checks/compile-command.json)、[完整脚本](checks/build.sh)：

```sh
cargo test --locked --offline --profile release-dist -p aster-core --no-default-features --features local-code --lib test_stream_settles_delegated_tool_calls_before_the_parent -- --nocapture
cargo test --locked --offline --profile release-dist -p aster-core --no-default-features --features local-code --lib test_stream_bridges_subagent_lifecycle_events -- --nocapture
cargo clippy --locked --offline -p aster-core --no-default-features --features local-code --all-targets --no-deps
cargo clippy --locked --offline -p aster-acp --no-default-features --features svg-png,account-import --all-targets --no-deps
cargo build --locked --offline --profile release-dist -p aster
cargo build --locked --offline --profile release-dist -p aster-acp --no-default-features --features svg-png,account-import
bash scripts/package-aster-cli.sh linux-arm64 /target/release-dist /out/aster-cli-linux-arm64.tar.gz
cargo fmt -p aster-core -p aster-acp -- --check
```

两项 focused 测试各 1 passed、0 failed。Clippy 通过，包含原有警告，没有扩大 lint
豁免。最终一轮[配对构建](checks/compile-exit.json)耗时 1,434.56 秒，退出 0。
[归档校验](checks/archive-validation.json)核对两套 Linux/aarch64 ELF 和 SHA256SUMS，
最高所需 GLIBC 均为 2.30，保持清单的最低 2.31 声明。配对 BUILD_ID：
`aster-cli-pair-v1:028b037742b6a8745a8e3e8a855697a7b6599600:aarch64-unknown-linux-gnu`。
配对归档 SHA256：`2fc52be325b0631af7261e9a3ccec9bf4f7ab6a554b6b6fde07ff6079fa25464`。
原有 embedding 配套资源保留，没有下载模型或执行推理服务。

两次中间失败保留：第一次回归 fixture 没有实际触发子工具，补充显式 worker 注册与
委派选项后重跑通过；第二次 all-targets Clippy 缺少冻结目录中未复制的原仓库
`packages/intelligent-ui/fixtures/form.json`，按原文件补入后完整重跑通过。没有删测试
或跳过 all-targets。相关日志以 `attempt-1-*`／`attempt-2-*` 保留。
构建后私有 KBox 执行为零，见[清理观测](checks/compile-cleanup-ps.json)。

KRun 新包 `aster/acp-0.1.5.zip` 的 SHA256：
`7f6f226f1d87a82db0a952819056ea7154ec889b8dc1615fe13b818a508d88a6`。
[补丁及构建来源](checks/adapter-source.json)保留两处源码修复与先前 ACL overlay 补丁；
旧版安装包保持原字节。

[真实模型探针](checks/native-probe-result.json)总计 183.67 秒，原生能力任务 18.005 秒、
8 次工具调用；[委派检查](checks/native-probe-delegation-audit.json)另为 12.93 秒、2 次
工具调用，实际子 bash 输出和完成事件先于父 task 完成。所有执行仍在 KBox 客体内。
[运行后审计](checks/native-probe-post-run-audit.json)确认无已知提供方／随机 MCP 凭据
残留，临时源删除，零执行、零卷。

## KRun 长程验收：历史失败与最终结果

历史失败（原样保留，不覆盖）：KRun 首个长测完成 18/24 个持续输入轮次、234 批数据，
19 轮成功会话、139 次工具调用；[原始正式结果](checks/long-results.jsonl)三项 FAIL。
第 19 轮原生 ACP 返回既有 DeepSeek HTTP 402 `Insufficient Balance`，见
[阻断回执](checks/long-provider-blocker.json)。[结束后审计](checks/long-post-run-audit.json)
未发现已知提供方凭据，零执行、零卷。[独立数据复核](checks/long-post-failure-oracle.json)
通过已完成的 18 轮数据与二进制，也保留验证短语出现在原生自动记忆 WAL 的文件检查失败。
KRun 随后用原生 `memory_dir = "/data/.aster/memory"` 保持记忆能力与 grader 不变。

充值后的完整复跑通过 2,451.065 秒持续阶段、24 轮输入、25 轮会话、235 次工具，
但新 VM 的 `session/load` 成功后首个 prompt 超过 300 秒，L4-2/L4-3 FAIL；该失败、
24 轮独立数据复核和四个真实恢复诊断均保留在 KRun 仓库
`docs/evidence/2026-11-12-long-tasks-aster/OUTCOME.md`。四个诊断全部通过，原超时
未复现，根因仍未定位，没有为此修改本仓库的原生生产代码。

最终完整复跑只增加原生 stderr 阶段日志
（`RUST_LOG=warn,perf_probe=info,aster_acp::retrieval_phase=info`）与已有原生
`memory_dir="/data/.aster/memory"`，grader、包、底座、超时、版本 ZIP 均不变，
2026-11-12 10:09 CST 自然结束、退出码 0：
[三项正式结果](checks/long-final-results.jsonl)与[完整结果](checks/long-final-result.json)
为 3/3 PASS。持续阶段 2,527.433 秒、24/24 轮、312 批、25 轮会话、193 次真实工具；
新 VM 执行身份 `a4a028e2…` → `8ae9a6d4…`，`session/load` 后恢复 prompt 32.539 秒／
4 次工具，通过验证短语回忆与新随机外部文件证明；16 MiB 二进制、24 份旧报告、
canary、零执行零卷均通过。[退出回执](checks/long-final-exit.json)为 3,032.722 秒／退出 0，
[轮次日志](checks/long-final-runner.log)、[冻结规格](checks/long-final-run-spec.json)、
[恢复阶段原生日志](checks/long-final-restored.stderr.log)与
[独立审计](checks/long-final-post-run-audit.json)（556 个自有文件、4,874,715,377 字节，
两套已知凭据零残留、临时凭据源已删除、零执行零卷）一并保留；
[来源清单](checks/long-final-provenance.json)记录各副本的字节级 SHA256。
五个 `checks/long-*` 历史失败副本未改动。

恢复阶段日志仍含以下 WARN，本次均未导致失败，但不能据此认定与先前超时存在因果关系：
工作区检索目录部分索引（7 个不存在的点文件读取失败）、恢复后两次
`stale workspace revision`、会话关闭时记忆抽取未在超时前排空并被取消。先前 300 秒
超时的根因仍未定位；KRun 的 `docs/evidence/2026-11-12-aster-restore-timeout-analysis/OUTCOME.md`
把边界缩小到 prompt 发出之后、首个输出之前，仍不能确证内部原因。本次单次通过、
四个诊断和阶段日志都不能作为已修复的证据。

## Explicitly not claimed

不声明先前恢复续跑超时的根因已定位或原生生产代码已修复；不把历史失败、四个诊断
与最终运行拼接为一次通过。不声明整个 KRun 项目、完整 M4、全故障矩阵、性能或发布
验收完成；不声明整个 Aster workspace、GUI、其他 feature 或 Windows/macOS 原生执行
全部通过。先前全仓格式的既有差异和离线 workspace check 的 ab_glyph 0.2.32 缓存缺失
仍由[前次记录](../20261111-acp-native-overlays/OUTCOME.md)保留；本次不改变无关路径。
没有提交或推送源码。
