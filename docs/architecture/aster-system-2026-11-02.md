# Aster 架构说明（2026-11-02）

原图大体画对了 Core 的执行流程。有几处需要改：Rust SDK 不必经过 ACP，
Meta Harness 应该放在 Actor 之前，运行结束也不一定代表验证通过。
新版补上了这些区别，以及原图没画出的调度、子任务和外部服务。

[新版 PNG](aster-system-2026-11-02.png) ·
[可编辑 SVG](aster-system-2026-11-02.svg) ·
[源码版本与文件哈希](aster-system-2026-11-02.sources.json)

![Aster current architecture](aster-system-2026-11-02.png)

## 看的哪份代码

核对了 Terminal、ACP、Core、Studio Agent Plane、workbench 独立审查的入口和
直接调用的代码，也看了 Cargo 工作区和 feature 配置。主仓库提交为
`453506a2a`，Studio 提交为 `57d3335f2`，读取的工作区还包含其他会话的未提交改动。
JSON 记录了当时的提交号、文件 SHA-256，以及文件是否与 HEAD 一致。

原图标注的是 `8729d7c84a2d…` 和 9.1.6；本次没有检出那份历史代码。
当前 Cargo.toml 中，`aster-core`、`aster-acp` 的版本为 9.1.7，
`aster` CLI 为 0.1.6。这些是包版本，是否已发布要另查 Release。

## 原图与当前源码的差异

| 原图 | 当前实现 |
| --- | --- |
| TUI / Studio / Rust Host 都经过 ACP | Terminal 和 Studio Agent Plane 启动 ACP；Rust 宿主可以直接调用 `AgentSession`。 |
| Actor → Meta Harness | `open_composed_with_hosts` 先组装执行图，再创建 Actor，箭头应反过来。 |
| 固定五个组件 | 五个是默认 stock 组合。Rust 宿主可挂载 `host:<id>`，也可自行组装；权限检查和完成检查始终保留。 |
| 单一事实日志 | 运行决策和恢复以 Fact Log 为准。`RunStore` 用于查状态，`AgentEvent` 用于通知事件；记忆、索引和文件另行存储。 |
| 完成 → 验证后写 turn.complete → End | 无修改的回答可以是 `Narrative`。`Verified` 表示检查通过，`Waived` 表示豁免，`Distinct` 表示各步骤使用不同的 effect digest。还需要画出等待、恢复和中断。 |
| SessionOptions 冻结运行配置 | 运行前还会固定能力列表和版本。Run / Turn / Subtask 分层管理，父级结束时清理子级；显式后台任务归 Run 管理。 |
| budget 一个方块 | 这只是默认组件之一。任务并发、模型生成并发、工具轮数、单工具超时、上下文长度和运行时长分别在不同位置处理。 |
| 整个 Studio 都走 Core 循环 | workbench 独立审查通过 Provider / ModelControl 调用模型，不使用工具，并自行检查证据和返回结果。 |
| 整体 Aster | 原图主要画了 Core。新版另列 Office、Browser、Capture、模型服务、sandbox companion 和可选控制面。 |

## 当前设计是否合理

这样分工是合理的：ACP 处理协议，Core 执行请求，宿主提供界面和工具。
宿主能调整执行图，但权限检查和完成检查仍由 Core 负责。
默认流程也不会自动修改或评估自己的 Harness。

文档有两处已经跟不上代码：

1. `core-authorities.md` 仍称源码相对 `apps/code/core/src/`，实际目录已经是
   `crates/code/core/src/`。原图中的包名和版本也需要更新。
2. 该文档的预算表称 FactRun 的 `tool_timeout_ms` 尚未验证。当前调用链是
   `ExecutorTools → execute_tool_queued_or_direct → execute_tool_timed / ToolCommand`，
   已传递 `self.config.tool_timeout_ms`。因此，不能仅凭这张旧表就说工具没有超时限制。
   本次只查了调用代码，没有逐一测试所有工具的排队、嵌套和取消行为。

后续建议：

- 更新现有预算表，写清每种限制的单位、在哪里生效、由哪个测试检查。
- 分别说明 Core 和 Studio 单独调用模型时，如何处理并发、超时、取消和用量。
  Studio 这类调用已有自己的超时和结果检查；Core 的检查不会自动替它完成。
- 按任务选择检查方法。`Verified` 只表示声明的检查对指定 effect digest 通过，
  文件内容、事实和视觉效果是否正确，还要有相应的检查。
- 在恢复文档里区分 Fact Log、checkpoint、会话快照、状态查询和事件通知。
  执行过程中可直接发送进度事件，并非每条事件都先写入 Fact Log。

## 源码依据

| 结论 | 实际入口 |
| --- | --- |
| Terminal 启动配套 ACP | [aster_spawn.rs](../../crates/terminal/crates/codegen/aster-pager/src/acp/aster_spawn.rs#L57) |
| Studio Agent Plane 启动 ACP | [turns.rs](../../apps/studio/src-tauri/src/agent_plane/turns.rs#L4213) |
| Rust 宿主直接 send / stream | [session_facade.rs](../../crates/code/core/src/agent_api/session_facade.rs#L270) |
| 同一会话一次只接受一个请求，复用同一份日志 | [run_admission.rs](../../crates/code/core/src/agent_api/run_admission.rs#L1) |
| 先组装图，再打开 FactRun | [fact_control.rs](../../crates/code/core/src/fact_control.rs#L1010) |
| 默认组件、宿主 assembler 和固定的 KernelPolicy | [meta_harness.rs](../../crates/code/core/src/meta_harness.rs#L124) |
| Run / Turn / Subtask 生命周期 | [agent_execution.rs](../../crates/code/core/src/capability/agent_execution.rs#L13) |
| 上下文去重、排序与预算 | [assembler.rs](../../crates/code/core/src/context/assembler.rs#L153) |
| 模型生成并发限制 | [admission.rs](../../crates/code/core/src/llm/admission.rs#L229) |
| 工具权限、执行、运行时长与取消 | [executor_tools.rs](../../crates/code/core/src/fact_control/executor_tools.rs#L347) |
| 单工具超时实际传递 | [tool_execution_runtime.rs](../../crates/code/core/src/agent/tool_execution_runtime.rs#L216) |
| 完成状态及判断函数 | [harness_loop.rs](../../crates/code/core/src/harness_loop.rs#L54) |
| 完成检查与 turn.complete 写入 | [turn_outcome.rs](../../crates/code/core/src/agent_api/conversation_runtime/turn_outcome.rs#L205) |
| Studio 无工具独立审查 | [workbench_reviewer.rs](../../apps/studio/src-tauri/src/agent_plane/workbench_reviewer.rs#L129) |
| 运行历史、状态副本和预算说明 | [core-authorities.md](../../crates/code/docs/architecture/core-authorities.md) |
| 默认与高级功能开关 | [Core Cargo.toml](../../crates/code/core/Cargo.toml#L200) |
| 根工作区与排除的控制面工作区 | [Cargo.toml](../../Cargo.toml#L1) |

## 做了哪些检查

PNG 用仓库已有的 Sharp 从 SVG 生成，尺寸为 3840 × 3480。
已打开图片检查文字、连线和裁切，检查了 SVG XML、文档链接和引用行号。
本机缺少 Playwright 所需的 Chromium，因此没有完成浏览器中的几何检查。

本次没有做全仓审计、发行包检查或跨平台测试，也没有遍历所有 feature 组合。
图中列出的可选服务不一定都已启用。以后更新这张图时，需要重新核对代码和构建配置。
