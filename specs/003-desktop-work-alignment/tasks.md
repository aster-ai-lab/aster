<a id="tasks-desktop--work-alignment"></a>

# Tasks: AsterDream / Work alignment

**Input**: [spec.md](spec.md), [plan.md](plan.md)

## Phase 1 — Baseline and behavior

- [x] T001 [US4] Save baseline/status and build AsterDream host/renderer with root workspace ACP+Office; identify actual build/tenant prerequisites and preserve unrelated edits.
- [x] T002 [P] [US1] Adapt `components/composer`, QuickPanel and useAgentSessionParts with actual IME/Tiptap/SWR before/after and neighboring regressions; retain AsterDream timer/focus/dockContent.
- [x] T003 [P] [US2] Add independent plan-first submission/review/progress/compact goals to `pages/agents`, projections/types/i18n, with stale review/draft/queue/session/terminal tests.
- [ ] T004 [P] [US2/US3] Adapt native approval identities/snapshots/stop in `src-tauri/src/agent_plane`, coordinate renderer contract, test wrong binding/reconnect/cancel/real local ACP behavior. Supply a session/revision-bound typed goal snapshot for current state/tick and terminal cleanup, with stale/foreign/unknown refusal tests; do not infer state from assistant text or historical commands.
- [ ] T005 [US3] Synchronize model defaults in `providers.rs` / `preferences.rs` with rollback, persistence/reopen and organization eligibility regressions; wire existing UI callers.
- [ ] T006 [US4] Align AsterDream updater state, typed errors, retry and complete-product replacement in `app_updater.rs`; isolate old app upgrade and reject incomplete/foreign/mixed-generation archives.
- [ ] T007 [US4] Adapt AsterDream Linux input module/launcher/install packaging from qualified Work mechanism; test modern/legacy protocol, resource paths, candidate Enter/cancel and required-resource refusals.

## Phase 2 — Wiring and integration

- [ ] T008 [US1–4] Build AsterDream renderer/host/ACP/Office and verify actual selected/bundled identities. Run native interactive Chinese/quick reply, plan→review→approval→execute→cleanup, reconnect/stop, multi-session memory, model persistence and isolated upgrade flows. Preserve own-process cleanup receipts.
- [ ] T009 [US1–4] Independent adversarial review of all changed caller/contract/config/export paths; rerun distinct failures alone; resolve all in-scope findings and execute relevant UI/dependency/format gates.

## Phase 3 — Evidence

- [ ] T010 [US1–4] Record source hashes, commands/counts, package identity, interactive artifacts and requirement-by-requirement completion audit in `docs/evidence/desktop/2026-10-21-work-alignment/OUTCOME.md`; update this task list from evidence.
- [ ] T011 No commit/staging/push/public release unless separately requested; preserve existing dirty changes.

## Final Verification Gate

- [ ] Owning focused native/renderer regressions pass, new behavior has no unresolved failure
- [ ] `cargo fmt --all -- --check` executed and owning changed-source formatting passes
- [ ] Root local-dependency/workspace guards and relevant AsterDream UI ratchets executed, no allowlist expansion
- [ ] `just build-desktop`, renderer production and applicable package/upgrade gates succeed
- [ ] Interactive AsterDream evidence matches FR-001–FR-012; missing evidence remains incomplete
- [ ] No new crates.io A3S dependency, second Cargo workspace, native select or non-ACL product configuration
- [ ] Exact product/tenant/data and execution ownership retained; cloud/other lanes not falsely covered by local ACP evidence
- [ ] Final OUTCOME includes Explicitly not claimed and links to retained failures
