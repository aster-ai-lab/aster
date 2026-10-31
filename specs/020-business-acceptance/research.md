# Research: 任务内业务验收

**Date**: 2026-10-25。只读检查当前源码；不是发布版本或功能效果测评。

## Existing mechanisms and decisions

| 需求 | 现有实现 | 规划决定 |
| --- | --- | --- |
| 条件与计划 | `crates/code/core/src/goal_control.rs`、`planning/mod.rs` | 继续使用原成功条件与 Goal/Plan 修订；不另存一份规则正文 |
| 确认与单写者 | `goal_control/facts/plan.rs`、`agent_api/goal_facade.rs` | 复用 proposal/revision/sequence 校验和非执行 admission；确认不启动 Run |
| 检查合同 | `verification.rs`、`config/verification_contract.rs` | 选择宿主 ACL 已声明合同；条件关联不能扩大覆盖/权限 |
| 执行与证据 | `agent_api/session_verification.rs`、`agent/tool_completion_runtime.rs`、`fact_control/goal_evidence.rs` | 复用实际工具回执与 effect digest；业务证据另校验条件和检查修订 |
| 完成与豁免 | `harness_loop.rs` | 不改写原 Verified/Waived 区分；业务验收汇总不成为第二个完成权威 |
| 审查与复验 | `session_review/finding.rs`、`agent_api/session_review_facade.rs`、`acp/src/engineering.rs` | 复用 Finding 生命周期；只有可信复验能产生当前证据 |
| 卡循环 | `fact_control/receipts/refusal.rs`、`goal_control/continuation.rs` | 沿用重复/无进展限制；不增加自主修复后台进程 |
| 状态传输 | `acp/src/agent/goal_ext.rs`、`host_methods.rs`、`turn.rs` | 扩展原 Goal 类型化边界，不从模型文字推断状态 |
| CLI 结果 | `crates/terminal/crates/codegen/aster-pager/src/app/turn_completion.rs`、`app/acp_handler/session_notification.rs` | 扩展原 turn outcome 解析与呈现；不另加 CLI 执行或判定 |
| 桌面运行 | `apps/studio/src-tauri/src/agent_plane/runtime_client.rs`、`goal_snapshot.rs` | 遵循 019 的单运行时边界；不直接调用模型或 Shell |
| 桌面位置 | `renderer/pages/agents/WorkbenchProjectTray.tsx`、`WorkbenchHomeComposerExtras.tsx`、`hooks/useStudioGoal.ts` | 在现有 Goal 区域增加业务验收分组；不挂载退休详情面板、不添加右侧第八页签 |
| 重启与后续任务 | `session_checkpoint.rs`、`agent_api/session_persistence.rs`、`acp/src/goal/import.rs` | 原事实恢复；后续任务显式导入规则、重新绑定，不导入通过 |
| 测试方法 | `core/src/verification/tests.rs`、`agent_api/goal_facade/tests/execution/acceptance.rs`、`acp/tests/long_horizon_contracts.rs` | 沿用现有 hermetic/stdio/独立断言方法；不另建测试框架 |

## Gaps that require bounded additions

1. `VerificationContractV1` 当前只有 id、command、covers。路径声明与 exit 0 不证明某条业务规则被充分检查；需要可确认的条件关联及检查输入身份。
2. 当前 turn outcome 主要是 terminal、status 和计数，缺少逐条件的来源、当前性和未覆盖原因。需要 Core 只读投影及 ACP 输出。
3. 当前 Goal/Plan 确认机制可复用，但还没有完整的业务验收用户旅程；需要在原确认/执行边界内补充操作。
4. 当前历史导入不应被假定已经支持任意上一任务的规则引用；后续复用须显式补齐并独立测试。

## Alternatives and scope decisions

- 单靠提示词提醒测试：不能提供修订绑定与用户可检查的事实，保留其辅助作用。
- 新建独立验收引擎/状态库：拒绝；会与 Core 的 Run、完成判定和恢复竞争。
- 让 Agent 改写宿主 ACL 来注册检查：拒绝；标准确认不能变成权限升级。
- 首期自动生成测试：后续可在普通受控 Run 中生成草稿；本期先验证已存在、可阅读的检查与闭环，不增加自动批准通道。
- 要求 019 全部完成才开始：不需要；Core 工作可以先做，Studio 集成只等待其实际消费的 RuntimeClient、Goal/Review 权威和会话恢复边界合格。

## Assumptions and remaining execution risks

- 当前工作区包含大量他人改动，且 `.github/actions/checkout-studio/action.yml` 存在未解决冲突。集成前需要确定一个可构建基线；本计划不解决该冲突。
- macOS 原生 Shell 因 `native_process_lifetime_unqualified` 受限。必须预检合格 Guest 或其他合格执行后端；不放宽资格门、权限或取消语义。
- `apps/studio` 是私有子模块。它的实现和测试留在子模块内；父仓库只记录计划、共享证据和后续经授权的 gitlink。
- 用户确认检查映射不等于形式化证明测试充分性；界面始终说明结果只覆盖已绑定检查。

## Clarification record

本轮采用已知上下文中的最小范围：任务内业务验收，桌面优先，共享 Core/ACP 判定；不新增配置格式、不操作生产系统。没有待用户回答才能编写计划的标记。工期为估算，平台资格和 019 集成等待不计入净开发人日。
