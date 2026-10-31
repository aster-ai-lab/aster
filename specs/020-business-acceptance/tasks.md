---
description: "Task list for task-scoped business acceptance"
---

# Tasks: 任务内业务验收

**Input**: `specs/020-business-acceptance/`；先阅读 spec.md、plan.md 和 constitution。

**Status**: Deferred；下列实施任务全部未开始。用户当前优先 [021-workspace-wiki](../021-workspace-wiki/tasks.md)；本清单保留作后续备选，不作为当前开发队列。

**House rule**: 每项行为任务与它的 owning-package hermetic 正反例一起交付。任务共享 Core、协议或运行状态，不预标并行。

## M0: 基线与依赖（1 task）

- [ ] T001 [US1-US4] 记录当前 Core/ACP/Studio 源码与 dirty 基线；核验 `crates/terminal/crates/codegen/aster-pager/src/app/turn_completion.rs`、`app/acp_handler/session_notification.rs` 的既有消费链；核实 019 的 RuntimeClient/Goal/Review/恢复依赖和合格后端。运行原有最小基线检查并保留结果；未解决 CI 冲突或后端资格不得记作通过，不改他人文件。

## M1: Core 条件与证据（3 tasks）

- [ ] T002 [US1] 在 `crates/code/core/src/goal_control.rs`、`goal_control/facts/`、`agent_api/goal_facade.rs` 复用条件、修订和确认机制，补齐验收关联操作及原日志恢复。新增 sibling `business_acceptance` 测试覆盖草稿无权限、旧确认、重复 operation、运行中修订拒绝、重启与旧日志兼容；不直接破坏公开 Rust 结构。
- [ ] T003 [US1-US2] 在 `verification.rs`/小型 owning 子模块及 `config/verification_contract.rs` 增加对现有宿主合同和检查/夹具身份的绑定；不新增配置语言或模型可写权限。对应测试覆盖未知合同、越界路径、部分绑定、检查/夹具变化、ACL 权限未升级及幂等。
- [ ] T004 [US2] 在 `agent/tool_completion_runtime.rs`、`fact_control/goal_evidence.rs`、`harness_loop.rs` 和原会话持久化路径投影逐条件结果并接入原完成门；`VerificationReport` 与当前条件/检查/修改及原执行身份一致才可通过。附测试覆盖伪造、旧摘要、部分通过、豁免、迟到回执、取消/超时/拒绝、重启和普通任务兼容。

## M2: ACP / CLI（1 task）

- [ ] T005 [US1-US2] 在 `crates/code/acp/src/agent/goal_ext.rs`、`host_methods.rs`、`turn.rs` 增量接入类型化操作/快照；同步 pager 的 `src/app/turn_completion.rs`、`app/acp_handler/session_notification.rs` 和 `scrollback/blocks/session_event.rs` 显示同源结果，不新增执行驱动。新增 `crates/code/acp/tests/business_acceptance_stdio.rs` 与 pager sibling 测试，证明未知字段拒绝、读取不执行、确认不扩权、取消/恢复、CLI/ACP 判定一致及旧客户端能力协商。

## M3: 桌面旅程（3 tasks）

- [ ] T006 [US1-US2] 依赖 019 的合格边界，在私有 `apps/studio/src-tauri/src/agent_plane/runtime_client.rs`、`goal_snapshot.rs`、`controllers.rs` 补类型化转发；若新增 Tauri 命令，同步 host 注册、permission 和主窗口 capability。更新 shared `studioGoal.ts`、`studioAgentPlane.ts`、`useStudioGoal.ts`。同任务测试覆盖未知值、越权、断连、过期异步响应、仅主窗口允许及没有原始 transport/模型/Shell 旁路。
- [ ] T007 [US1] 先把交互约束补入根 `DESIGN.md`，再在子模块 `WorkbenchProjectTray.tsx`、`WorkbenchHomeComposerExtras.tsx` 的原 Goal 区域增加条件编辑、待回答、检查选择与精确修订确认；需要时拆小 sibling 组件。附 renderer 测试覆盖未配置、冲突保留输入、未确认不能通过、重复提交与键盘操作；同步中英文。
- [ ] T008 [US2] 同一任务区域增加逐项状态、覆盖说明、原始回执、历史失败、执行不可用原因与复验入口；附 renderer/shared 测试覆盖长条件、窄窗口、全部未检查、部分失败、过期、豁免与运行中取消，不新增任务状态存储或右侧页签。

## M4: 修复及后续复用（2 tasks）

- [ ] T009 [US3] 在 Core `session_review/`、`agent_api/session_review_facade.rs`、ACP `engineering.rs` 及 Studio 同一验收入口补齐失败项到原 Run 的修复关联；复验仍使用已确认检查。测试实现修复可通过、主 Agent 自报无效、断言弱化要求新确认、首次失败和所有重试保留、重复拒绝/无进展停机。
- [ ] T010 [US4] 在原 Core Goal import/来源验证边界、`crates/code/acp/src/goal/import.rs` 和 Studio 原历史选择路径补同项目显式复用；导入为新 proposal 后重新绑定。附 Core/ACP/Studio 测试覆盖第二任务未检查、源修订变化、重启、跨工作区/租户拒绝及不复制通过/豁免/权限。

## M5: 验证与证据（3 tasks）

- [ ] T011 [US1-US4] 复用现有 Core/ACP 隔离夹具与 long-horizon harness，完成订阅权益旅程及第二任务复用；增加双用户作用域反例。独立断言检查实际状态，覆盖 SC-002 的 12 类反例；合格后端、实际 ACP、原生桌面和真实模型分别记录，环境阻塞不得冒充成功或忽略。
- [ ] T012 [US1-US4] 运行 plan.md 的 owning-package、ACP conformance、renderer/shared、格式、依赖、类型/i18n 与 runtime-authority 检查；成对构建并核对 CLI/ACP 身份。检查无新增依赖/工作区/生产 unwrap、无原生 select、无并行状态权威；区分既有失败并按仓库规范独立复现，不放宽基线。
- [ ] T013 [US1-US4] 在 `docs/evidence/code/<实施日期>-business-acceptance/OUTCOME.md` 记录 Result、What landed、源码/二进制/后端身份、实际命令/去重计数、失败与 Explicitly not claimed；更新功能文档、ACL 合同接入说明和回退/版本兼容说明。仅依据真实结果勾选任务；不默认提交、发布或联系试用用户。

## Final Verification Gate

- [ ] owning package 的聚焦正反例通过（T002–T011）
- [ ] 当前条件/检查/修改三者过期均不能沿用旧通过（T003、T004、T009）
- [ ] 同一 Core 权威贯穿 CLI、ACP、Studio；无第二模型/执行/验收账本（T005、T006、T012）
- [ ] 第二任务重新取证且隔离/重启正确（T010、T011）
- [ ] 合格后端、实际 ACP 和原生桌面旅程有当前证据（T011）
- [ ] Rust 修改通过 `cargo fmt --all -- --check`，受影响 CI 检查完成（T012）
- [ ] 无新 crates.io `aster-*` 依赖，无原生 `<select>`，产品配置保持 ACL（T012）
- [ ] CLI/ACP 配对身份与保留失败、未覆盖范围写入 OUTCOME（T013）
