# T032 `max_turn_requests`：工具预算终止分析

本次 R2 的直接停止原因是工具预算用完，而不是已经证明模型请求次数用完。
原始评测已经终态；没有再次调用付费模型，也没有修改产物、运行环境或预算。

## 本次事实

[最终工具元数据](t032-tool-final.json)记录：

| 项目 | 数量 |
| --- | ---: |
| 持久化 `model.turn` | 50，全部 `kind=tool` |
| 请求的工具调用 | 51 |
| 已准备调用 / 工具结果 | 50 / 50 |
| 成功 | 35 |
| 拒绝 / 失败 | 9 / 6 |
| `budget.denied` | 1 |

没有持久化的空白或纯文本模型决策解释这次预算耗尽。
50 个模型决策可以包含 51 个工具调用，因为一次决策可带多个工具调用。
这些记录数不能代替真实 provider 请求数，也不能用来计算纯模型时间。

在根代理授权的精确原始会话中，额外只读取了已知工具准备记录的身份与摘要、
`budget.denied` 的工具 ID、`turn.complete` 的枚举字段：

- 序号 186：`budget.denied`，工具调用 ID 为 `leaked-0`，该调用没有执行结果。
- 序号 187：`turn.complete`，`terminal=verified`，`forced_stop=tool_budget`。
- 7 个已知拒绝调用的原始与有效参数摘要全部相同；准备记录序号为
  141、144、151、158、165、172、183。没有读取参数正文或模型推理。

`verified` 是已有完成证据的分类，并不能覆盖同一终态的 `tool_budget` 强制停止，
也不代表隐藏用例全部通过。[独立原始产物检查](t032-oracle.json)实际为 8/9，
未通过项是负零 `-0.00` 的保留。[评测记录](t032-evaluation.json)的任务时间为
1892.931 秒，即 31 分 32.931 秒；它包含 provider 工作、运行时错误、修复与自检、
审批等待。纯模型时间不可用，不能把本次总时间减成一个有证据支持的“模型外耗时”。

## 计数和停止链

1. Core 的基础默认 `MAX_TOOL_ROUNDS=50`，见
   [agent.rs:72](/Users/roylin/code/aster/crates/code/core/src/agent.rs:72)；默认配置采用它，见
   [agent.rs:396](/Users/roylin/code/aster/crates/code/core/src/agent.rs:396)。
   会话显式值可以覆盖基础值，见
   [session_config.rs:444](/Users/roylin/code/aster/crates/code/core/src/agent_api/session_config.rs:444)。
2. `session_harness_config_with` 把这个值同时变成工具拒绝预算与工具目录关闭阈值，
   默认执行步数为 `max(rounds * 4, 32)`，每次模型 effect 默认最多尝试 2 次，见
   [fact_control.rs:872](/Users/roylin/code/aster/crates/code/core/src/fact_control.rs:872)。
   这三个量不同：工具预算、actor 步数、模型失败重试次数不能混称模型请求上限。
3. Effect 在有效工具结果落盘时增加 `tool_runs`，见
   [coding.rs:1110](/Users/roylin/code/aster/crates/effect/src/coding.rs:1110)。
   成功按 `ok=true` 计数；typed receipt 的非成功结果明确写入 `countRun=true`，见
   [coding.rs:1437](/Users/roylin/code/aster/crates/effect/src/coding.rs:1437)。
   因此本次 15 个失败或拒绝也占预算，不能只用 35 次成功去对照阈值。
4. 到达工具目录关闭阈值后，下一次 completion 的工具目录为空，见
   [coding.rs:1233](/Users/roylin/code/aster/crates/effect/src/coding.rs:1233)。
   如果模型仍返回工具决策，则在已有 `tool_runs >= budget` 时进入 Deny，见
   [coding.rs:721](/Users/roylin/code/aster/crates/effect/src/coding.rs:721)，随后生成
   `budget.denied`，不执行该工具，见
   [coding.rs:1368](/Users/roylin/code/aster/crates/effect/src/coding.rs:1368)。
   该事实把 fold 收到 Done，见
   [coding.rs:1161](/Users/roylin/code/aster/crates/effect/src/coding.rs:1161)。
5. Core 根据本用户回合内精确的 `budget.denied` 事实产生 `ForcedStop::ToolBudget`，见
   [turn_outcome.rs:123](/Users/roylin/code/aster/crates/code/core/src/agent_api/conversation_runtime/turn_outcome.rs:123)。
   ACP 把这个枚举映射为协议 `StopReason::MaxTurnRequests`，见
   [turn.rs:225](/Users/roylin/code/aster/crates/code/acp/src/agent/turn.rs:225)。
   Work 把该协议停止理由投影为错误，见
   [turns.rs:250](/Users/roylin/code/aster/apps/work/src-tauri/src/agent_plane/turns.rs:250)。

本次 50 次计预算的结果之后拒绝第 51 个请求，与该 50 次有效工具预算吻合；
不是看到“50 条 model.turn”后猜了一个模型 API 限额。

## 配置来源与空响应

WorkPolicyV1 只包含 `id`、`revision`、`instructions`，见
[work_policy.rs:13](/Users/roylin/code/aster/crates/code/core/src/work_policy.rs:13)，
它不选择工具预算或模型请求数。预算来自 Agent/Session 配置与 ACP effort 映射。
ACP 的 medium=48、high=128 等明确映射在
[models.rs:204](/Users/roylin/code/aster/crates/code/acp/src/agent/models.rs:204)，经
[session_open.rs:46](/Users/roylin/code/aster/crates/code/acp/src/agent/session_open.rs:46)
写入 `with_max_tool_rounds`。桌面的 `default` 会清掉 effort 覆盖，见
[models.rs:238](/Users/roylin/code/aster/crates/code/acp/src/agent/models.rs:238)。
不能把某个 ACP 默认 effort、用户界面标签或当前源代码的其他档位，反推为本次实际阈值。
本次工具事实证明有效阈值为 50，与 Core 基础默认一致；未据此声称捕获了全部配置选择历史。

纯文本或空文本 `ModelDecision::Text` 本身不增加 `tool_runs`，而是把阶段收为 Done，见
[coding.rs:653](/Users/roylin/code/aster/crates/effect/src/coding.rs:653)。
若实际发生 provider 调用，它仍会消耗真实时间、token 和 host budget 的检查机会；
这与工具预算是两套机制。可选 host `BudgetGuard::check_before_llm` 默认 Allow，见
[budget.rs:81](/Users/roylin/code/aster/crates/code/core/src/budget.rs:81)；入口见
[llm_invoker.rs:1198](/Users/roylin/code/aster/crates/code/core/src/agent/llm_invoker.rs:1198)。
原始 model owner 的 reservation 负责同一 worker、同一 session 和未结清调用生命周期，见
[run_admission.rs:664](/Users/roylin/code/aster/crates/code/core/src/agent_api/run_admission.rs:664)，
它不是一个默认 50 次的请求计数器。

此外，文本或 reasoning 通道泄漏出的工具协议也能被恢复成工具决策，见
[tool_results.rs:363](/Users/roylin/code/aster/crates/code/core/src/fact_control/tool_results.rs:363)。
因此界面上像“思考/文字”的输出未必是一个不含工具的决策。本次末尾 `leaked-0`
是这条恢复路径生成的 ID 形态；这不等于额外读取或分析了其推理内容。

## 已有机制、测试与下一步

已有工具关闭后的收尾机会，不应重复建设同一机制：

- `fact_control_tests::effort_tool_rounds_are_the_fact_budget`：配置的 budget/cap、步数、重试数。
- `fact_control_tests::effort_tool_budget_runs_every_round_before_deny`：预算内都可执行，随后拒绝。
- `fact_control_tests::fact_log_budget_does_not_call_the_tool_body`：被预算拒绝的工具不运行。
- `fact_control_tests::effort_ceiling_asks_for_a_final_answer_before_denying`：两次工具后空目录收尾，
  正常文本收尾不产生 `budget.denied`。
- `fact_control_tests::fact_log_tool_round_cap_uses_an_empty_tool_list` 与
  `effect/tests/tool_round_cap.rs::tool_round_cap_completes_once_with_an_empty_tool_list`：
  目录为空，而不是伪造一条用户输入。
- `agent_api::tests::fact_log::final_delivery`：已有验证不替代用户所需的实际助手答复；
  答复也不能替代缺失的验证。
- ACP `agent::turn_terminal_tests::forced_stop_is_not_a_successful_end_turn`：预算停止不能冒充成功。

这些是现有源码测试证据，本代理本轮没有运行 Cargo，也没有新增测试通过声明。

直接提高预算会放大相同故障路径的审批与模型重试开销，还可能延长已经有产物的自检；
它不能修复此次 15/50（30%）失败/拒绝，不能消除原始 sandbox carrier 不兼容，
也不能让负零隐藏用例变正确。把失败从计数中排除还会削弱无限拒绝循环的资源边界。

最小下一步应先消除无效调用的原因，而不是改预算：

1. 复用已独立通过真实 Linux guest 回归的
   [canonical carrier 修复](runtime-carrier-canonical/OUTCOME.md)，只进入后续新构建，
   不替换本次原始 R2。
2. 完成审批期限与 stale-answer 的独立小回归；进一步调查 7 次相同拒绝类别的确切分支。
   这 7 次占本次预算 14%。摘要相同已排除“这些调用参数被改写”的解释，但当前通用错误
   还合并了 `effective_permission=Deny` 与 `Ask + unconfirmed_ask`，见
   [receipts.rs:241](/Users/roylin/code/aster/crates/code/core/src/fact_control/receipts.rs:241)。
   不据同一错误字符串猜权限、缓存审批或 provider 漂移；由并行只读诊断给出精确分支后，
   在原有调用身份/权限边界内做受控 provider RED，证明未改参数的一次合法授权可执行一次，
   改参、跨调用或迟到回答仍拒绝。不能跳过 Hook 后的有效参数重验。
3. 保留已有空工具目录收尾机制。若未来确要改善目录已关后仍泄漏工具协议的用户体验，
   先加一例真实 provider 输入的有界回归，再决定是否需要一次纯文本交付修复；
   它必须保持工具零执行、保留原强制停止/未完成状态，不能把 `budget.denied` 改成成功。

本文件只记录分析和下一步验证路径。没有增加预算、排除失败计数、修改冻结代码、
重写产物或再发起付费评测；本次 R2 的失败、31 分 32.931 秒总时长和 8/9 原始成绩均保留。
