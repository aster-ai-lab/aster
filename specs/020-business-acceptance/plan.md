# Implementation Plan: 任务内业务验收

**Priority update (2026-10-25)**: 本计划延期；用户当前希望优先获得 DeepWiki 式项目/任务工作区理解。当前实施计划为 [021-workspace-wiki/plan.md](../021-workspace-wiki/plan.md)。下述工期和阶段仅保留作历史规划。

**Branch**: 当前工作区；不创建分支、不提交 | **Spec**: [spec.md](spec.md)

**Gate context**: Product regime；Core/ACP 与私有 Studio 子模块。本文是实施计划，不是已完成清单。

## Summary

把现有 Goal、Verification、Review 组织为用户可操作的业务验收流程。首版选择宿主已声明、可阅读的检查，先交付一条订阅权益修改旅程；再支持同项目后续任务显式复用规则。保持原执行、事实、完成和恢复权威，不建设第二套验收平台。

## Constitution Check

下表 PASS 表示计划符合约束；实现及验证仍由 tasks.md 的未完成任务承担。

| Principle | Status | Evidence in the plan |
| --- | --- | --- |
| I. Fail closed | PASS | 无绑定、无合格后端、旧修订、伪造/缺失证据不通过 |
| II. Evidence retained | PASS | T013 写实际实施日期的 OUTCOME，保留失败与未验范围 |
| III. Hermetic tests | PASS | 每个 T002–T011 行为任务携带 owning-package 正反例 |
| IV. Closed sets | PASS | 条件引用、操作、状态和错误在 owning boundary 明确解析 |
| V. Workspace / APIs | PASS | 根工作区、本地依赖、原公开 API；缺失边界以增量 API 补齐 |
| VI. Local schema | PASS | 原 Core 事实链内版本化关联，不新增数据库 |
| VII. Ratchets | PASS | 不增原始 ACP 调用、平行驱动或退休面板，不放宽基线 |
| VIII. Transports | PASS | ACP/RuntimeClient 封装传输；UI 消费类型化视图 |
| IX. Configuration | PASS | 宿主策略继续用 ACL；任务关联不能配置权限/模型/后端 |
| XIII. Repository | PASS | 新规划在本仓库，Studio 实现仅在既有私有子模块 |
| XIV. Names | PASS | 条件、合同、来源引用均取已存精确身份 |
| XV. Meaning | PASS | 模型提案与用户明确选择；禁止关键词猜测业务规则或授权 |
| XVI. Pickers | PASS | 复用现有设计系统控件，无原生 select |
| XVII. Dialogs | PASS | 优先任务内展开；需要确认框则遵循 DESIGN 原地动效 |

## Project Structure

```text
specs/020-business-acceptance/
├── spec.md
├── plan.md
├── research.md
├── data-model.md
├── contracts/acceptance-flow.md
└── tasks.md
```

### Source Code

| Layer | Existing paths to extend | Bounded additions / callers |
| --- | --- | --- |
| Core 条件与关联 | `crates/code/core/src/goal_control.rs`、`goal_control/facts/`、`agent_api/goal_facade.rs` | 在原事实链增量补充验收关联及只读 API；新逻辑拆小 sibling module/tests，不替换公开结构 |
| Core 证据 | `verification.rs`、`agent_api/session_verification.rs`、`agent/tool_completion_runtime.rs`、`fact_control/goal_evidence.rs`、`harness_loop.rs` | 补充条件/检查身份与当前性校验；保留原完成门与豁免 |
| Core 修复/恢复 | `session_review/`、`agent_api/session_review_facade.rs`、`agent_api/session_persistence.rs`、`session_checkpoint.rs` | 沿用 Finding 与原存储；仅补缺失关联和恢复测试 |
| ACP | `crates/code/acp/src/agent/goal_ext.rs`、`host_methods.rs`、`turn.rs`、`goal/import.rs` | 类型化操作、快照、显式历史导入；更新相关 parser、catalog、stdio tests |
| CLI 呈现 | `crates/terminal/crates/codegen/a3s-code-pager/src/app/turn_completion.rs`、`app/acp_handler/session_notification.rs`、`scrollback/blocks/session_event.rs` | 复用原状态解析和摘要，不先增加 slash command |
| Studio 原生 | `apps/studio/src-tauri/src/agent_plane/runtime_client.rs`、`goal_snapshot.rs`、`controllers.rs`、现有 Tauri 注册/permissions/capabilities | 类型化转发、只读投影与最小主窗口授权；不得增加模型/Shell 调用 |
| Studio UI | `apps/studio/src/shared/types/studioGoal.ts`、`renderer/hooks/useStudioGoal.ts`、`renderer/data/studioAgentPlane.ts`、`renderer/pages/agents/WorkbenchProjectTray.tsx`、`WorkbenchHomeComposerExtras.tsx` | 小型验收分组和 sibling tests；同步中英文文案、a11y 与根 DESIGN |
| 验收夹具 | Core/ACP 原测试树及 `crates/code/acp/tests/long_horizon/` | 小型隔离夹具、独立状态断言；不创建新工作区、锁文件或测试框架 |

## Research

复用依据、被放弃方案与当前基线风险见 [research.md](research.md)。关系与当前性见 [data-model.md](data-model.md)，用户/协议行为见 [contracts/acceptance-flow.md](contracts/acceptance-flow.md)。

## Delivery phases and estimates

估算假设一名熟悉 Rust/React 的工程师，1 人日约 6 小时有效开发；为净工作量，不含 019/平台资格等待和外部用户招募。先以一条旅程验证，再决定是否扩大范围。

| 阶段 | 交付物与退出条件 | 任务 | 净人日 |
| --- | --- | --- | --- |
| M0 基线与接入边界 | 冻结源码/子模块基线，核验已定位的 CLI 消费链、019 依赖和可执行后端；记录未通过项 | T001 | 1–2 |
| M1 条件与可信证据 | 用户确认修订、绑定现有合同、逐项真实证据；伪造/陈旧/部分通过不成功 | T002–T004 | 5–6 |
| M2 ACP/CLI 接入 | 类型化命令和同源摘要，实际 stdio 与恢复测试通过 | T005 | 2–3 |
| M3 桌面完整旅程 | 原任务区内确认、运行、查看证据和错误恢复；无第二执行权威 | T006–T008 | 3–4 |
| M4 修复与后续复用 | 首次失败可追溯、同标准复验；第二任务导入后重新取证 | T009–T010 | 3–4 |
| M5 资格与试用准备 | 独立夹具、12 类反例、合格后端及原生桌面、配对构建与证据 | T011–T013 | 2–3 |

**合计 16–22 人日，约 4–5 个工作周。** M3 结束可演示一条人工选择既有检查的旅程；M5 才能声明本期资格完成。首轮执行 M0 后按实际 API 缺口重估。

### Dependencies and release gates

- Core 的 M1 可先进行；Studio M3 开始前须核验 019 的 RuntimeClient、Goal/Review 单权威与会话恢复边界已实际可用。未满足只推迟对应集成，不复制旧实现绕过。
- 当前 CI action 冲突和他人工作区改动由原任务处理。T001 记录稳定基线；本期不清理或混入它们。
- 无合格执行后端时可完成 hermetic 开发，但 M5 执行资格保持未通过；不把 macOS native refusal 改成允许。
- 每阶段交付一个包含调用方、配置/权限、测试和证据的完整改动；不单独发布只有 UI 的成功按钮。

## Verification Plan

- Core：`cargo test --locked -p a3s-code-core --lib business_acceptance`；沿用并运行受影响的 `verification`、`harness_loop`、Goal acceptance/recovery 与 Review 测试。新测试的命名使用该过滤前缀；这些是计划命令，尚未执行。
- ACP：新增 `business_acceptance_stdio` owning-package 集成测试，运行 `cargo test --locked -p a3s-code-acp --test business_acceptance_stdio`，并跑现有 `acp_conformance_stdio`。
- Studio：新增测试沿用 `business_acceptance` 过滤；`cargo test --locked -p a3s-studio business_acceptance`。在子模块运行 `pnpm exec vitest run --project renderer <本期精确测试路径>`，同步受影响 shared/parser 测试。
- 集成：独立断言的订阅夹具完整旅程，以及第二任务复用；实际 ACP/合格执行后端与原生桌面分别记录。脚本模型不替代真实模型，renderer 不替代原生窗口。
- 仓库门禁：`cargo fmt --all -- --check`、`python3 tests/workspace_dependencies.py`、`python3 tests/studio_runtime_authority.py`、受影响类型/i18n/架构检查及项目 CI。既有失败独立报告，不删除基线或声称全仓通过。
- 每项结论保留原始失败、重跑原因、当前源码与成对 CLI/ACP 身份；只汇总真正执行且去重的结果。

## Requirement-to-task coverage

| Requirement | Producing tasks |
| --- | --- |
| FR-001 | T002, T007 |
| FR-002 | T003, T005, T007 |
| FR-003 | T004, T005, T006, T009 |
| FR-004 | T003, T004, T009 |
| FR-005 | T004, T005, T008 |
| FR-006 | T004, T005 |
| FR-007 | T009 |
| FR-008 | T010 |
| FR-009 | T005, T007, T008 |
| FR-010 | T005, T006 |
| FR-011 | T002, T004, T010 |
| FR-012 | T008, T011 |
| FR-013 | T011, T012, T013 |

## Follow-up after the first release

只有真实使用显示接入和维护成本可接受，才排下一期：在原受控 Run 中生成业务测试草稿、展示断言和 diff、用户确认后绑定；扩大检查栈和按显式变更触发复验。收集实际人工验收耗时、确认有效的问题比例、下次发布复用情况；不以测试生成数量作为主要价值指标。本轮不提前实现这些能力。
