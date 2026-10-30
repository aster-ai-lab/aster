# Tasks: Rust SDK and explicit capability activation

**Input**: [spec.md](spec.md), [plan.md](plan.md)
**Prerequisites**: Constitution check and read-only consistency analysis pass.

## Phase 1: Behavior and tests

- [x] T001 [US1] Inventory actual SDK source/jobs; clean active guides, API comments and retired publishing material; update manual pointers and add the installer/index guard in `core/tests/manual_index.rs`.
- [x] T002 [US2] Add strict optional `officeTools` host metadata and condition Office registration in ACP; test missing/false/true/invalid and overriding source-review behavior.
- [x] T003 [US3] Replace eager Moli spawning with the existing lazy shared runtime future; test no-runtime construction, deferred resolution and existing search refusal/cancellation contracts.
- [x] T004 [US4] Correct embedding backend instructions and unreachable stale error guidance; verify the existing host-factory and embedded-light tests.

## Phase 2: Wiring

- [x] T005 [US2] Have Studio explicitly declare Office in existing host metadata; extend the actual metadata/ACP fixture tests. Private source stays private.
- [x] T006 [US3] Make ACP headless search opt-in; update all desktop development/build/package callers and their checks; verify feature unification on the actual paired build graph.

## Phase 3: Evidence

- [x] T007 Run focused and connected checks, current-source paired builds, formatting and applicable workspace checks; triage distinct failures alone.
- [x] T008 Record `docs/evidence/code/2026-10-24-sdk-capability-cleanup/OUTCOME.md` with counts, source identities, residual failures and Explicitly not claimed.

## Final Verification Gate

- [x] Owning-package tests for every behavior change pass.
- [x] Formatter and applicable workspace/dependency checks are recorded honestly.
- [x] No retired SDK source/jobs/installers are reintroduced; current manual links resolve.
- [x] Default ACP graph is free of browser search; desktop explicitly enables it.
- [x] Office remains permission-scoped and source-review remains tool-free.
- [x] Persistent configuration remains ACL; no UI control changes.
- [x] Public Rust APIs and unrelated changes are preserved; no stage/commit/push.
