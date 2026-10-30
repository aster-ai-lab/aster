---
description: "Task list for Studio single runtime"
---

# Tasks: Studio single runtime and authority convergence

**Input**: Design documents from `specs/019-studio-single-runtime/`

**Prerequisites**: plan.md and spec.md

**House rule**: every behavior change carries its owning-package hermetic test in the same task.

## Phase 1: Contracts and small vertical slices

- [x] T001 [US1] Create spec/plan/research/data-model/contracts with Constitution Check and single-writer cutover rules.
- [x] T002 [P] [US1] Add `tests/studio_runtime_authority.py` and a shrink-only baseline for raw ACP requests, independent admitted-Run loops, Plan rawInput consumers, permission pre-decisions and alternate ledgers.
- [x] T003 [US3] Add a Core read-only plan snapshot, publish the same ACP Plan on live/load, migrate Studio Plan rendering and add Core/ACP/Studio tests; remove new TodoWrite rawInput dependency.
- [x] T004 [US1] Extract one admitted-Run attempt driver for interactive/background ACP entry points and share cancel/EOF/worker-panic/terminal-missing tests.

## Phase 2: Studio facade and permission authority

- [x] T005 [US1] Add `agent_plane/runtime_client.rs`; migrate initialize/new/prompt/cancel/model/mode/interject/Goal/wiki/memory commands and add facade/no-state tests.
- [x] T006 [US1] Remove Studio permission pre-decision and mode mutation during confirmation; add default/plan/auto/yolo, cancel, timeout and deck regression tests.
- [x] T007 [US3] Replace static A3S execution model/effort/mode/command/capability lists with current ACP catalogs and closed-set UI validation.

## Phase 3: Session and execution convergence

- [x] T008 [US2] Add missing Core typed transcript/plan/boundary APIs and ACP replay/page/fork contracts with hermetic boundary tests.
- [ ] T009 [US2] Migrate Studio session/list/load/create/delete/fork to Core with a single-writer migration receipt; delete Studio transcript/execution authority and fallback.
- [ ] T010 [US1] Parameterize interactive/background attendance over the shared driver; retire the independent unattended event loop and prove identical Run facts apart from confirmation brokerage.

## Phase 4: Goal, Schedule, Review and document convergence

- [ ] T011 [US1] Feed calendar and Goal UTC wake into one idempotent execution scheduler; remove Studio TaskRun/turn firing and message-based recovery.
- [ ] T012 [US1] Migrate Review execution/evidence to Core Finding and Core model side paths; delete Studio HTTP reviewer and second proof/repair authority.
- [ ] T013 [US2] Route all Core text reads through one document overlay/version path; retire duplicate client_fs/document bridge behavior and test unsaved/conflict/timeout cases.
- [ ] T014 [US4] Remove migration adapters, legacy schemas and retired paths; shrink every architecture ceiling to its final value without deleting evidence.

## Phase 5: Evidence and qualification

- [ ] T015 [US1] Run focused Core/ACP/Studio/renderer tests, actual ACP conformance, `cargo fmt --all -- --check` and architecture ratchets; record all counts and failures.
- [ ] T016 [US4] Write `docs/evidence/code/2026-10-25-studio-single-runtime/OUTCOME.md` with Result, What landed, commands, evidence and Explicitly not claimed.
- [ ] T017 Stage only user-requested files if a commit is explicitly requested; verify `git diff --cached --name-only` first. No commit is assumed.

## Final Verification Gate

- [ ] Owning-package focused tests pass
- [ ] `cargo fmt --all -- --check` is clean for Rust edits
- [ ] No new crates.io `a3s-*` dependency
- [ ] No native `<select>` added on a product surface
- [ ] Product configuration, if added, is ACL
- [ ] No dual-write, old-store fallback or permanent legacy runtime remains in a completed cutover
