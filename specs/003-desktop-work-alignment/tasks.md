<a id="tasks-desktop--work-alignment"></a>

# Tasks: AsterDream / Work alignment

**Input**: [spec.md](spec.md), [plan.md](plan.md)

## Phase 1 — Baseline and behavior

- [x] T001 [US4] Save baseline/status and build AsterDream host/renderer with root workspace ACP+Office; identify actual build/tenant prerequisites and preserve unrelated edits.
- [x] T002 [P] [US1] Adapt `components/composer`, QuickPanel and useAgentSessionParts with actual IME/Tiptap/SWR before/after and neighboring regressions; retain AsterDream timer/focus/dockContent.
- [x] T003 [P] [US2] Add independent plan-first submission/review/progress/compact goals to `pages/agents`, projections/types/i18n, with stale review/draft/queue/session/terminal tests.
- [x] T004 [P] [US2/US3] Adapt native approval identities/snapshots/stop in `src-tauri/src/agent_plane`, coordinate renderer contract, test wrong binding/reconnect/cancel/real local ACP behavior. Supply a session/revision-bound typed goal snapshot for current state/tick and terminal cleanup, with stale/foreign/unknown refusal tests; do not infer state from assistant text or historical commands.
- [x] T005 [US3] Synchronize model defaults in `providers.rs` / `preferences.rs` with rollback, persistence/reopen and organization eligibility regressions; wire existing UI callers.
- [ ] T006 [US4] Align AsterDream updater state, typed errors, retry and complete-product replacement in `app_updater.rs`; isolate old app upgrade and reject incomplete/foreign/mixed-generation archives.
- [x] T007 [US4] Adapt AsterDream Linux input module/launcher/install packaging from qualified Work mechanism; test modern/legacy protocol, resource paths, candidate Enter/cancel and required-resource refusals.

## Phase 2 — Wiring and integration

- [ ] T008 [US1–4] Build AsterDream renderer/host/ACP/Office and verify actual selected/bundled identities. Run native interactive Chinese/quick reply, plan→review→approval→execute→cleanup, reconnect/stop, multi-session memory, model persistence and isolated upgrade flows. Preserve own-process cleanup receipts.
- [x] T009 [US1–4] Independent adversarial review of all changed caller/contract/config/export paths; rerun distinct failures alone; resolve all in-scope findings and execute relevant UI/dependency/format gates.

## Phase 3 — Evidence

- [ ] T010 [US1–4] Record source hashes, commands/counts, package identity, interactive artifacts and requirement-by-requirement completion audit in `docs/evidence/desktop/2026-10-21-work-alignment/OUTCOME.md`; update this task list from evidence.
- [x] T011 No commit/staging/push/public release unless separately requested; preserve existing dirty changes.

## Final Verification Gate

- [x] Owning focused native/renderer regressions pass, new behavior has no unresolved failure
- [x] `cargo fmt --all -- --check` executed and owning changed-source formatting passes
- [x] Root local-dependency/workspace guards and relevant AsterDream UI ratchets executed, no allowlist expansion
- [ ] `just build-studio`, renderer production and applicable package/upgrade gates succeed
- [ ] Interactive AsterDream evidence matches FR-001–FR-012; missing evidence remains incomplete
- [x] No new crates.io Aster dependency, second Cargo workspace, native select or non-ACL product configuration
- [x] Exact product/tenant/data and execution ownership retained; cloud/other lanes not falsely covered by local ACP evidence
- [x] Final OUTCOME includes Explicitly not claimed and links to retained failures

## 当前验收状态（2026-10-21）

实现与拥有者回归的勾选不代表所有真实交付/平台场景通过。后续用户范围由
[spec 004](../004-desktop-default-aster/spec.md) 规定：共享 `apps/studio`，本地构建无账号，组织构建保持认证。

- T004：协议/严格解包、确切审批绑定、停止/重连、目标期限/顺序及真实 ACP 回归已验；最新整合窗口的剩余交互属于 T008。
- T005：三项默认值、重开读取、失败补偿与组织准入拥有者回归已验；最终组织 R4 窗口持久化仍列入 T008。
- T006：两产品 updater 各 35 pass；目标清单前向兼容、混代/缺资源拒绝已验。隔离旧版本合成升级未完成，所以本项未勾选。
- T007：移植后的协议/模块/launcher 合同和真实 GTK/private-bus 机制已验；当前 11 个输入机制通过。最终 Linux 真实产品 stage 在 T008；实体麒麟、原生 Wayland、严格 Flatpak export 没有宣称通过。
- T008：R4 双 renderer、根双宿主及 ACP/Office 构建和 F 本地真实 UI 全流程/后续中文回复通过。组织 R4 窗口、最终 macOS 两包、Linux stage、合成旧版升级、尚缺交互及自有进程清理待验。
- T009：独立源码审查、R4 Rich/IME 回归、根 fmt、迁移后根 guards 及 UI 门禁均有记录；全树 TSC 和全根 Cargo check 的既有失败保留，不冒称全绿。
- T010：两份 OUTCOME 已更新需求核对与限制，最终包/整合证据尚未齐全，保留未完成。

收据与准确计数见 [本任务 OUTCOME](../../docs/evidence/desktop/2026-10-21-work-alignment/OUTCOME.md)
和 [后续合并 OUTCOME](../../docs/evidence/desktop/2026-10-21-default-aster/OUTCOME.md)。
