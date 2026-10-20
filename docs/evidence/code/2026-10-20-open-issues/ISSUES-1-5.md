# Issues #1 / #5 修复与验证

范围：[issue #1](https://github.com/aster-ai-lab/aster/issues/1)、[issue #5](https://github.com/aster-ai-lab/aster/issues/5)。本报告只覆盖 goal continuation 与同进程多会话 durable memory。完整自有源码、测试、两份现有手册的 **23 个路径及 SHA-256** 见 [source manifest](issue-1-5-source-ready.json)。ACP `agent.rs` 的测试模块声明由 composer 代理维护；未覆盖其他线程的 receipts 修改。

机器结果：[最终 validation JSON](issue-1-5-final-validation.json)。所有运行均在本地使用合成配置、私有临时目录和 mock；没有真实模型服务或用户会话。

## #1 根因与最终行为

当前基线已经限制只读 `StepAccepted` 不刷新停滞计数，issue 中“每次 accepted 都重置”不完全对应当前树。实际仍存在两个缺口：`until_deadline` 把无进展上限设为 1000；已有进展只在 accepted step 上重置，真实修改但 goal judgment 未 achieved 的 slice 无法刷新它。

修复后，缺省 unattended policy 保留既有 1000-slice 总上限和运行截止时间，无实际修改的连续 settled slices 上限为 **3**。显式 host ACL 数值、禁用策略、手动运行和原有准入检查保持。没有新增全局 tool/turn cap。

实际进展从原始 receipt fold 计算：窗口从本 slice 的 `SliceAdmitted` 到下个 slice；foreground admission 必须同 Run、同 session、同 capability binding。完整 observation 中的已知 before/after 身份变化才算进展，排除 harness 路径。纯工具成功、声明 write、只读结果、同内容重写、未知 before、未完成 observation、未结清 invocation、外来 Run/session/capability 都不能刷新。已结清 background mutation 还须精确 child/terminal 绑定和本 Run 的 `source_run`。真实变更不依赖工具成功状态，也不依赖 goal judgment accepted。

Gate verified 不是 goal achieved。未完成目标达到停滞边界后停止准入；ACP 的 typed `GoalContinuationStopped` 写出既有 v1 `failed / execution_failed`，`lastError` 明确 `goal.continuation_stopped` 与 `next=needs_input`。可恢复的旧工具错误不会遮住这条终态原因。没有新 TerminalReason 值、结果字段、自动 Completed 或手动 `/goal` 成功捷径。

### 历史事实兼容

深审复现了首轮修复可能引入的历史回放问题：真实 identical-byte write 可携带非空 receipt ledger、通过独立 ByteAttestor 和 achieved 判断。旧 small-cap policy 随后合法准入的下一 slice，在直接改旧 progress 含义后会被回放拒绝。维护回归在兼容修复前按预期失败，见 [replay before](issue-1-replay-before.log)。

最终实现只在私有 `ContinuationSource` 加闭集 `progress_basis`：旧记录缺省 None，序列化省略，保持原 command digest；新 host source 显式 `observed_workspace_mutation_v1`。两个计数均从同一原始事实投影得到：旧 accepted-receipt 计数只用于旧源的原准入校验；新源使用实际 settled-mutation 计数。没有跳过历史 admission 校验。未知语义拒绝。相同维护测试在 memory/file 两种 session store 下验证旧事实重开及新机会严格拒绝，[replay after](issue-1-replay-after.log) 通过；也包含在最终 execution 51 项内。

### 默认真实 ACP binary 复现

修复前的 10 秒有界运行确实先完成 `answer.txt=42\n` 的 missing→file 完整 mutation、gate verified，再进行只读/narrative 重复：23 slices admitted、22 settled、46 durable model.turn，无 StepAccepted/Completed、无 result。见 [前置条件 JSON](issue-1-acp-before-preconditions.json)、[原始合成事实](issue-1-acp-before-facts.jsonl)、[before log](issue-1-acp-before.log)。因此超时不是初始化或 mock 无响应。

最终同任务、默认 goal continuation、60 秒 run deadline：**4 slices，1464 ms，14 次本地 model call**，终态 result 已落盘，`failed / execution_failed / next=needs_input`，无 deadline、无假 Completed。维护 e2e 在解释 timeout 前先断言真实 mutation、verified、后续成功只读 receipt、无 accepted/completed，并记录结构化计数；见 [最终 e2e log](issue-1-acp-e2e-after-final.log)。`RunProgress.turns` 不是 slice 数，本报告不把二者混用。

## #5 根因与最终行为

ACP 每个 host session 创建独立 Lazy adapter；原先每个 adapter 都尝试独占 `VecMemoryStore::open` 同一 canonical 目录。第一个 handle 活跃时，后续会话的 recall/write 反复失败。`get_or_try_init` 的错误本来就可重试，不能描述为永久缓存失败。

共享只增加在 Lazy 层：canonical 路径索引、Weak registry、OnceCell 持有 Arc。首次 open 与 legacy import 在同一同步 registry 锁内原子发布；同步 guard 不跨 await。失败不发布成功 handle，下次可重试。正常 legacy import 只由该 handle 的首个 initializer 执行；原有部分导入失败保留 legacy 文件语义不变。

最后 Lazy owned Arc 在同一 registry 锁内析构，闭合 strong=0 但 engine/lease 尚未释放时新 opener 误报 already-open 的并发窗口。Weak 不固定存活集合，最后 handle 释放后可重新独占打开。底层直接 `VecMemoryStore::open` 的同进程独占及引擎跨进程锁契约未放宽。

回归覆盖 canonical alias、共享读写、并发首次使用、失败后重试、legacy import、最后释放/新首次使用竞争、直接 open 拒绝和真实两个 ACP host sessions 双向记忆/召回。ACP 修复前实际命中 `durable memory collection is already open`，见 [before](issue-5-acp-before.log)；最终 [ACP sessions](issue-5-acp-after-final.log) 与 [core memory](issue-5-core-after-final.log) 通过。

## 最终验证

| 互不重叠的最终组 | 通过数 | 日志 |
|---|---:|---|
| Core goal control | 40 | [log](issue-1-control-after-final.log) |
| Core goal execution | 51 | [log](issue-1-execution-after-final.log) |
| Core mutation provenance | 4 | [log](issue-1-receipts-after-final.log) |
| Core durable memory | 17 | [log](issue-5-core-after-final.log) |
| ACP 两个 host sessions | 1 | [log](issue-5-acp-after-final.log) |
| ACP unattended units | 29 | [log](issue-1-acp-unit-after-final.log) |
| ACP 实际构建二进制 unattended e2e | 8 | [log](issue-1-acp-e2e-after-final.log) |
| **合计** | **150** | **0 failed** |

Cargo 均 `--locked`。最终 core control Cargo 编译后，直接运行同一维护测试 harness 的 execution/receipts/memory selectors；二进制前后 SHA-256 一致且全自有 source hash 匹配，见 [core 收据](issue-1-5-core-final-validation.json)。ACP session 维护测试复用最终 unattended unit Cargo 编译的 harness，同样前后 SHA 一致，见 [ACP 收据](issue-5-acp-final-validation.json)。

可复现的 Cargo 形式为 `cargo test -p a3s-code-core --lib --locked <selector> -- --nocapture`，selectors 是 `goal_control::`、`agent_api::goal_facade::tests::execution`、`fact_control::goal_evidence::tests`、`durable_memory_vec::tests`。ACP unit selectors 用 `cargo test -p a3s-code-acp --lib --locked <selector>`，分别为 `unattended::` 和 `agent::memory_sessions_tests`；binary e2e 用 `cargo test -p a3s-code-acp --test unattended_e2e --locked -- --nocapture`。

源码 scoped rustfmt check 与 `git diff --check` 均 exit 0，见 [静态检查](issue-1-5-static-checks.json)。根代理另执行根 workspace fmt、paired binary build 和整合验证，不计入上表。before、interim、精确重跑、编译失败和中断尝试均不叠加通过数；早期 compile-error/interrupted 日志仅保留诊断，非行为复现证据。

没有运行完整 workspace tests 或真实远端模型评测；本报告不把 focused hermetic 结果扩大成全仓或真实服务验收。本代理未执行提交、暂存或推送；自有验证子进程均已完成。
