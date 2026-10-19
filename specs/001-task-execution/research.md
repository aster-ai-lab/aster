# Task execution research

Reviewed 2026-10-18. These primary sources guide the audit; they are not proof
that AsterDream implements the practices or achieves their published results.

| Source | Applied audit decision | Existing local pattern / evidence required |
| --- | --- | --- |
| [Anthropic: Building effective agents](https://www.anthropic.com/engineering/building-effective-agents) | Start with a comprehensible tool/feedback loop, explicit stopping rules and verifiable subtasks. Add orchestration when a real task requires it. | Core fact loop, `tools/task`, `task_scheduler`; inspect actual child effects, budgets and completion facts rather than prompt claims. |
| [Anthropic: Writing effective tools](https://www.anthropic.com/engineering/writing-tools-for-agents) | Tools need precise contracts, useful errors and bounded meaningful results. Evaluate end outcomes and actual tool traces; measure calls, errors, tokens and time. Valid equivalent phrasing should not fail solely on punctuation. | ACP `office_file`/`output_check`, Core output artifacts, Work real-ACP fixtures and the original task results; use independent outcome checks. |
| [Anthropic: Effective context engineering](https://www.anthropic.com/engineering/effective-context-engineering-for-ai-agents) | Keep high-value instructions, retrieve source material on demand, retain recoverable observations, and test compaction on long traces. | Core `context/assembler`, `compaction`, durable memory and `artifact_view`; prove bounded context and retained task obligations. |
| [Anthropic: Scaling Managed Agents](https://www.anthropic.com/engineering/managed-agents) | Separate durable session history, harness context and execution resources. Keep secrets outside generated-code access and permit independent recovery. | Core facts/checkpoints and workspace capability ceilings, Work credential boundary and ACP resource lifetime. Architectural similarity alone is insufficient. |
| [MCP cancellation specification, 2025-11-25](https://modelcontextprotocol.io/specification/2025-11-25/basic/utilities/cancellation) | Cancellation correlates to an active request, releases resources and tolerates late messages. Task cancellation has its own protocol. | Existing MCP client plus ACP generation ownership; check waiting human/tool requests, not only receive-loop cancellation. This does not claim ACP and MCP have identical wire semantics. |

The older agent article now directs readers to newer managed-agent material;
both were inspected. The local design remains governed by the repository's
constitution and public Rust APIs. No new framework or transport is required
by these references.

## Current evidence boundaries

- The earlier task-delivery slice built the real Work host and companions and
  verified file/terminal units. It did not replay live task quality.
- Work's comprehensive product test plan already distinguishes reachable,
  conditional and hidden features; source presence is not an acceptance pass.
- Current environment variables do not supply a live-model credential, but a
  read-only desktop observation found a logged-in app with Qwen 3.8 Flash
  selected. Environment variables alone do not establish live unavailability;
  the updated desktop/provider must be exercised separately from the loopback
  fixtures.
- Concurrent changes mean every test result needs its command and covered source
  state. Old binaries cannot certify newly modified behavior.

## 2026-10-20：T01 延迟与程序化工具调用

下列本地实测更新上面的 10 月 18 日环境观察；旧记录保留为历史，
不再用于断言模型不可用或当前构建失败。

| 观察 | 直接依据 | 开发决策 |
| --- | --- | --- |
| 约 38 分 32 秒包含环境修复和验收；有效任务为 482.856 秒、26 次模型调用、26 次工具调用。工具区间并集 270.022 秒，其余 212.834 秒包含模型、网络和调度，无法标为纯推理时间。 | [T01 时间记录](../../docs/evidence/studio/2026-10-19-t01-deepseek-flash/runtime/timing-analysis.json)、[原始验收](../../docs/evidence/studio/2026-10-19-t01-deepseek-flash/OUTCOME.md) | 分开记录准备、任务、独立验收；使用单调计时和调用身份关联，重叠区间不能重复相加。 |
| 19 次 Bash 共 270.012 秒；简单命令三次均值由旧缓存的 13.978 秒降至候选的 3.894 秒。旧样本主要耗在连接和 VM 创建。 | [命令延迟证据](../../docs/evidence/sandbox/2026-10-19-command-latency/OUTCOME.md) | 先扩展独立 VM 基准，再评估整任务；候选还包含源码契约修复，不能将全部改善归因于 SHA。 |
| `program` 已默认注册，已有 QuickJS、调用/输出/时间上限；Core `code()` presentation 已提供紧凑入口，但 ACP/Work 尚无 profile 选择接线。原 T01 未调用 `program` 或 `batch`。 | `crates/code/core/src/tools/{mod.rs,program_tool.rs,presentation.rs}`；Core `tests/test_extensibility_real_llm.rs`；ACP session setup | 复用现有引擎和 typed profile，补最小受信 ACL→ACP 入口。先审查实际 prompt/工具投影，再做自然选择与 code profile 对照，不新建编排框架。 |
| `ctx.bash` 只返回文本；内部 Bash 仍各自启动 OCI 生命周期。默认脚本上限 20 次调用、30 秒；允许 `task` 时默认 600 秒。 | `crates/code/core/src/tools/program_tool.rs` | 必要校验使用结构化 `ctx.tool("bash", args)` 并检查失败；显式设置有界实验预算。PTC 降低模型往返不等于减少 VM。 |
| 通用提示已提及 `program`；强制使用一次 program 的真实模型测试只证明执行机制。嵌套原生产物证明也已有修复。 | `prompts/common/{repository_tool_contract.md,system_default.md}`、`prompts/analysis/pre_analysis_system.md`（均在 Core）；[嵌套产物证据](../../docs/evidence/code/2026-10-19-nested-native-artifact-proof/OUTCOME.md) | 不先堆叠提示词，不重复实现证据链；新增无工具指令的任务评估，观察模型是否选择合适编排。 |
| 用户已启用本地 workspace retrieval；Work 临时 ACL 未传递该声明，ACP 明确配置不合并用户 ACL，production embedding seam 返回 None。 | `apps/work/src-tauri/src/agent_plane/launcher.rs`；`crates/code/acp/src/{config.rs,agent.rs}`；Core `config/loader.rs` | 修复受信配置传递和 provider 装配，禁止简单合并全部用户配置。缺本地资产时显式降级，不能静默切远程。 |
| 现有本地 BGE `embed()` 使用 blocking pool 和 Mutex，没有公开取消参数；丢弃等待不能证明 ONNX 计算结算。 | `crates/bge/src/embedding.rs` | fake backend 只证明适配契约；真实 ready 路径另验取消/超时和资源所有权，必要时扩展 BGE 公开结算 seam，保留既有 API。 |
| `search(mode="grep")` 已注册且不依赖 embedding；代码工具已注册并延迟启动 LSP，但原 T01 未调用。 | Core `tools/mod.rs`；ACP `agent.rs` 的 session capability 与 retrieval seam | 用独立代码任务验证工具可用性；不要求 CSV 任务调用语义检索或 LSP。 |

### 方案边界

- 同源码的软件 SHA / 硬件后端构建用于隔离原因；旧缓存只作历史对照。
- PTC 主实验比较原有 `adaptive` 与现有 `code` profile；`direct` 只作额外
  机制对照，不能冒充原 T01 的配置。每格使用新会话，冻结 profile。
- 独立只读调用可有界并行；依赖写入、执行和验收按顺序处理。
  合并命令必须保留各步骤退出状态，不能通过大量并行 VM 掩盖启动成本。
- 保留完整性、身份复验、DedicatedVm、凭据隔离和后代回收；本轮不引入
  VM 池、Box daemon、主机执行回退或 mtime 代替完整性校验。
- 原提交 8/9 本地场景通过，signed-zero 为已知失败。新旧对照使用同一
  验收集并单列该例，不向模型提供 oracle 答案，不修改冻结提交。
