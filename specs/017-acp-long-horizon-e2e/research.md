# Research / 当前事实与选型

日期：2026-10-24。本文是规划依据，不是新的测试通过记录。

## 已确认的现状

- 根 Cargo 固定 `agent-client-protocol = 0.10.4` 且启用 unstable feature；ACP 当前 `initialize` 返回 V1、广告 `loadSession` 与 embedded context。新上游文档的 V2 draft 或更多 V1 可选字段不能直接当作本产品承诺。
- ACP `src/agent.rs` 负责真实 new/load/prompt/cancel，`src/agent/resume.rs` 在模型、MCP、文档和文件激活前校验恢复身份与工作区。
- Core `src/run.rs` 的 RunStatus：Created/Planning/Executing/Verifying/Completed/Failed/Cancelled。waiting-for-permission 是外部观测等待，不是另加协议状态。
- `acp/src/agent/count_limit_tests.rs` 说明 interactive 轮数 0 为无限，各 effort 不可暗加上限；`approval_deadline_tests.rs` 固定现有人审批默认/0/大值语义。这些兼容契约必须保留。
- `manual/HOST_VERIFICATION_CONTRACT.md` 要求 host 明确 command/covers/workspace，绑定原始执行回执和当前 effect digest；命令文字、追加 shell 语句或旧校验不能获取验证权。
- SDK 清理轮的记录为 ACP 库 374 pass/8 ignored（4 线程、package cwd），5 项 stdio 边界通过。这不是行/分支覆盖率，也不能作为新用例 ID 已执行的证据。Core web-search 48/51 项的分母不能混入 ACP。
- 旧 `tui_session_function_coverage_is_at_least_95_percent` 只有 19 项 TUI 适配行为断言，不代表整体 95%。
- 上轮全 workspace all-targets check 在未修改的 `core/examples/workspace_retrieval_benchmark.rs` 有 E0382；执行本规划前必须核对当前实际状态，不能假设仍在/已修。
- 此前 GUI 背景捕获有 `cgWindowNotFound` 和绑定失效记录；新的 U cohort 必须重新核验实际控制能力，不延续退休候选或使用前台兜底。

## 复用，不另建测试平台

| 现有路径 | 可复用内容 | 本规划补的缺口 |
| --- | --- | --- |
| `crates/code/acp/tests/acp_conformance_stdio.rs` | 真二进制、实际 JSON-RPC、受控 provider | 长轨迹、分片、背压、身份与竞态组合 |
| `crates/code/acp/tests/boundary_e2e.rs` | 工具表、读写/bash/Office 的真实模型协议 fixture | 业务验收、阶段依赖、当前字节交付 |
| `crates/code/acp/tests/unattended_e2e.rs` | bind/prove/result、默认停滞、续跑期限 | 更完整的预算/停滞、恢复与外部副作用账本 |
| `crates/code/acp/tests/client_fs_stdio.rs` | 客户端未保存文件读取 | 多版本冲突、关闭和 lease 清理 |
| `crates/code/acp/tests/workspace_layer_trust_stdio.rs` | 未信任 MCP 与显式 host 配置 | 注入内容、切模型/恢复后边界 |
| `crates/code/acp/tests/approval_rules_stdio.rs` | 真实审批规则 wire | 迟到审批、参数/身份替换、取消输入等待 |
| `crates/code/acp/tests/shipped_tui_e2e.rs` | 真 pager/ACP 与终端行为 | 长任务、恢复、产物当前可用性 |
| `crates/code/acp/tests/tui_913_real_llm.rs` | 真实模型回合/修复/长任务门 | 分离必要因果与人为固定序列；允许等价修复 |
| `crates/code/acp/tests/live_shared_prompt_real_llm.rs` | 实际 provider 配置与短回合 | 明确预算/隔离/构建身份/长任务结果，不把 pong 当长程通过 |
| `crates/code/core/tests/run_control_runtime.rs` | Run steer/interrupt 的内核契约 | 通过 ACP/宿主贯通状态与用户操作 |
| `crates/code/core/tests/agent_exact_checkpoint_recovery_v1.rs` | checkpoint 身份/恢复 | ACP 进程崩溃窗口、资源重注入与用户恢复 |
| `crates/code/core/tests/test_session_close_lifecycle.rs` | queue/close/任务结算 | wire cancel/EOF/owner 的端到端清理 |
| `crates/code/core/tests/durable_memory_restart.rs` | 记忆持久边界 | 长任务中源变更和跨租户当前证据 |

以上“复用”表示已存在代码，不代表本计划中的组合用例已通过。新增 helper 仅在实际重复需要时最小提取；不把现有两个完整 harness 一次性重构。

## 外部协议依据

只使用 ACP 官方一手文档，并以锁定 Rust SDK 和当前广告能力约束适用范围：

- [V1 Initialization](https://agentclientprotocol.com/protocol/v1/initialization)：初始化与能力协商。
- [V1 Session Setup](https://agentclientprotocol.com/protocol/v1/session-setup)：会话建立、条件 load 与 cwd。
- [V1 Prompt Turn](https://agentclientprotocol.com/protocol/v1/prompt-turn)：流式 session/update、工具/审批及回合结算。
- [V1 Cancellation](https://agentclientprotocol.com/protocol/v1/cancellation)：request cancel 与 session cancel 不能混淆；泛化取消有可选部分，不宣称协议天然保证所有进程已回收。
- [V1 Transports](https://agentclientprotocol.com/protocol/v1/transports)：UTF-8、换行分隔 JSON-RPC、stdout 只放协议；不引入 draft HTTP transport。

进程清理、sandbox、当前字节 Verified、Goal 和租户隔离是本产品契约，不能仅引用 ACP 规范声称已提供。

## 已作的设计决定

1. 同一工作目标复用独立 oracle；H 档证明 harness，R 档证明具体模型/协议组合，U 档证明真实入口。任何一层都不替其它层背书。
2. 三个旧模型的少量不同失败样本不足以证明“都只是模型天花板”；保留 product/provider/test_spec/infra/mixed/undetermined 归因。
3. 未限定过程的多文件一次修复是合法行为。现有 LONG_PROMPT 明确要求先检查、rate→fee、每次复验，因此该门的顺序是有效用户约束，应保留并单独统计；新自由修复档不能替它放行。外部状态依赖、权限和版本提交的因果边也强制顺序。
4. 计划初始 130 case 的分母公开：120 product + 10 test-oracle；LH01–LH12 只是组合编排，不增加 case 基数。矩阵 variants 和重复 seed 单列。
5. 真 LLM 凭证只由既有可信配置/宿主控制；新本地 relay 若确有必要，持有凭证的父进程不序列化秘密，不复制用户状态；ACP 子进程只拿测试 fixture 的无秘密配置。
6. 工具链不为覆盖率换版。已安装 `cargo-llvm-cov` 和 LLVM tools；其 branch flag 是 unstable，因此不能预先保证可测分支百分比。行/函数、分支、功能、执行率和通过率分别呈现。
7. U 层使用 a3s-test 已有协议；TUI 稳定路径观察后再写 ACL，GUI 一 lane 一 owner，绑定失效就结束该 attempt。

## 需要在 T001 实际发现的运行变量

可信 ACL 路径、当前模型精确 ID/支持的 provider 协议、可用 sandbox、背景 GUI 能力、候选源/二进制身份、各平台执行端和有限预算。它们是实施前置，不是在规划文档中编造的现状，也不要求重新确认已授权的测试目标。

## 实施补充：原始用量与基线检查

- [DeepSeek 官方 Chat Completions](https://api-docs.deepseek.com/api/create-chat-completion/)定义 prompt/completion/total 及缓存命中/未命中字段，缓存命中也可出现在 prompt_tokens_details.cached_tokens；流式最后的 usage 位于 DONE 之前。新的测试观测器分别保存实际观察到的数字字段，并核对总数、别名一致性和范围；不把原始字段缺失补为零，不按全部输入为未缓存计价。
- JSON 和 SSE 的用量记录绑定 relay 接纳序号；取消、流不完整、损坏、字段矛盾及溢出保留 unknown。单事件上限 64 KiB，超限只将观测判为无效，不改变/修复下游传输。支持此 OpenAI-compatible 观测档，不据此宣布所有 provider 方言都已覆盖。
- 现有 Reqwest 0.12.24 默认 redirect policy 最多自动跟随 10 次；relay 的 upstream_calls 当前数的是转发调用，而非传输内部每个跳转。严格网络请求预算和后续真实 A/B 前，必须加单跳正反控制并明确原始认证/路由边界。暂不将这部分资格算完成，不为本地 meter 自证消费真实模型。
- 宿主合同已在初始 system 投影，但原提示只明确“验证已修改内容”；真实样本的基线检查仍出现 cd/echo 组合和状态掩盖。现将提示明确到用户请求的基线检查，并告知 bash 自带退出码；不增加未请求的基线步骤、不修改 fixture 的通过标准或命令。
- 发现结果保留器只写工作流 pass，可能遗漏外层身份/计量失败；已增加独立 C0 红/绿对照，整体 pass 必须同时满足工作流与外层资格。旧样本逐份审计，不以新逻辑覆盖原始状态。

## Terminal-Bench 耗时与 LoopX 归因补充

- 只读核验清零停滞计数、跨回合 unknown 长期禁止写入，以及 V1 将债务增长当进展均有历史缺陷证据；新调用身份收据仍可被当作新信息，是待证伪的收敛风险。不能用“没有异常”或一次长任务通过排除这些逻辑问题。
- 工具结果的事实 envelope 含 typed exit_code，但旧 `message_from_folded_tool` 投影只保留 text。OpenAI-compatible 序列化也没有独立 is_error 字段，因而模型可能看不到失败状态。共享投影已加数字状态并保留原始回执；488 项相关 Core 回归通过。实际 H1 还须将模型所见数字与本次真实工具回执绑定，不能硬编码期望 rustc 的 101：OCI bash 当前观察到的码可能是 1。硬编码断言的失败保留，不计产品失败或通过。
- 本仓库最早 `d5398e219` 是 2026-10-18 源码快照边界，不能证明更早 LoopX 机制的引入点。只读检查既有 `../a3s/apps/code` 历史，2026-09-29 `411d19bb` 已存在丢弃 exit_code 的同一投影；2026-10-17 `c3b95614` 是后续文件拆分。因此缺陷早于 Aster 仓库导入，但是否由用户所指 LoopX 内化引入，仍缺内化前的对应版本与原评测轨迹。
- 既有上游 `docs/evidence/code-harness/20261017-terminal-bench-subset/OUTCOME.md` 记录 native runner 固定十任务诊断全部超时、七项有 empty-retry；它不是 ACP cohort，也没有 token 用量。这份历史记录不能与当前 ACP 交互式 R1 的 12 次请求合并统计。
- 上游 `scripts/harbor/run_acp_tb.sh` 明确不构建二进制，并默认将总执行预算设为 28,000,000 ms（约 7.8 小时），高于产品原始 840,000 ms；它记录命名 job 的二进制/ACL SHA、模型和 effort，但未将构建产物与当前源码编码绑定。原 cohort 的实际 launch 不在此仓库，不能断言它一定用了该默认值或最新构建；重放必须同时冻结实际二进制和配置。这是停滞风险的预算放大条件，不是成功样本性能结论。
- 当前 R1 不启用 unattended 自动 Goal，仍有 217,377 输入 / 4,866 输出的 ACP 用量投影，说明高输入并非只能由目标重启导致。主请求每次工具 schema 42,761 字节、system 约 19 KiB，缓存下降仅有相关证据。按 [DeepSeek 缓存文档](https://api-docs.deepseek.com/guides/kv_cache/)，复用依赖匹配前缀且为 best effort；需同任务受控比较，不能把总输入都当作缓存未命中或据此计算账单。
- 下一轮分别验证收敛性、状态回传和精确核验恢复，再控制相同模型/任务/effort 的提示布局与辅助调用；保留权限、沙箱及当前字节验收。安装、构建、VM 资格准备、ACP 工作阶段和独立 oracle 分开计时，不把 macOS 每命令 VM 开销套到 Harbor Linux process-host lane。

## 实施补充：ignored 根与单跳 relay

- 已有两个 SOLO 红例证明完全被父 Git 忽略的工作区是产品缺陷：真实新增/修改/删除得到空 changes，SDK 写入后错误返回 Narrative。修复使用 Git 自己的 check-ignore 结果选择已有的有界文件索引，保留 Git 读取失败与预算边界；不解析 ignore 文件、不改全局环境、不放开权限或沙箱。只处理整个根被忽略；普通 Git 根内单个 ignored 后代的完整观察仍须单独证明。
- 运行级观察保留活跃基线的 ignored 状态；完整文件索引统一提供该根的路径，避免 Git 只列出 force-added 文件时漏掉其它文件。后台子任务保留活跃文件索引，并不把父 Git 的空状态当作丢失文件基线的恢复证据。根的 ignore 边界变化或仅有旧 Git 基线时保持 Incomplete；当前没有声称已实现所有文件索引的持久恢复。
- 169 项观察器/执行状态与 389 项 fact/Goal/完成门回归通过（组有重叠）；新增控制包括同 mtime 修改、新建、删除、force-added 文件、工作区外父目录修改、info/exclude、两种后台槽安装路径及 ignore 状态改变。不是新原始 case 覆盖率。
- relay 重定向红例证明一次接纳实际上到达两个 HTTP 服务。新增显式 ReqwestHttpClient::without_redirects，复用原代理/连接设置并以 Result 拒绝构造错误；共享 builder、默认构造器及 MCP 行为保持。relay 一次创建并复用此单跳 client，JSON/SSE 均停止在 307；不是把所有产品 HTTP 都改成不跟随跳转。66 项 HTTP/MCP 回归和现有 builder 仍跟随跳转的正向控制通过。
- 拒绝原因现为闭集、有界计数，只保留空连接/不完整读取/帧/认证/路径/长度/模型输出预算/请求预算等类型，不保存原始正文、路径或凭证。旧 cohort 的一次本地拒绝没有原因数据，不能补归因。单跳控制统计 HTTP dispatch，不声称统计 TCP 连接尝试，也不替 web/search/MCP 的完整 G10 egress 资格。
