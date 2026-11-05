# Tasks: Retire Studio Electron dependencies

**Input**: [spec.md](spec.md), [plan.md](plan.md)
**Prerequisites**: Product regime and Constitution Check read. Work in the current checkout; do not commit.

## Phase 1: Behavior and tests

- [ ] T001 [US1/US2] Audit every executable reference in Studio manifests, scripts, tests, renderer, shared contracts and native wiring. Record callers, active/retired decision, existing native owner and test obligations in the evidence record. Establish the complete inventory first, and close each group's audit before modifying it in T002–T005. Preserve license/history mentions and source hashes.
- [ ] T002 [US1] Remove confirmed retired Electron build/ABI/release hooks and their orphaned dependency patches. Update their callers and tests to supported native contracts; test that active scripts and manifests cannot reintroduce the removed packages. Keep native bundle completeness coverage.
- [ ] T003 [US2] Migrate retained `window.electron` callers through existing IPC/capability owners, including quote, tab movement, Python and backup/restore progress. Verify each outcome, cancellation and event cleanup with owning tests. Retire old preboot windows only with proven unreachable entry points; preserve current data migration tests.
- [ ] T004 [US2] Remove Electron guest branches and migrate retained browser/HTML/mini-app behavior through existing native owners. Test navigation, storage isolation, popup/permission boundaries, search/annotations, pane hiding and disposal. Verify actual native guests; no empty placeholder replacement.

## Phase 2: Wiring

- [ ] T005 [US1/US2] Retire the Electron global declaration and launch/CDP test facilities after callers migrate. Preserve equivalent smoke/regression outcomes using supported Tauri test entry points. Strengthen existing import/global/tag regressions and regenerate source-owned files through their generators.

## Phase 3: Evidence

- [ ] T006 [US1/US2] Run affected focused suites, lint/typecheck, dependency checks, renderer/native builds and native package membership/signature checks. Run relevant Rust checks if changed. Record exact failures, diffs, platform coverage and before/after evidence in `docs/evidence/studio/2026-11-01-electron-retirement/OUTCOME.md`.
- [ ] T007 [US2] Perform isolated background native CUA on changed visible entry points, then continue the existing frozen C00–C08 campaign and independent P0 repetitions. Keep actual deadlines, identities and all earlier failures; do not infer full completion from focused cleanup checks.

## Final Verification Gate

- [ ] Each audited executable reference has a verified removal/migration disposition.
- [ ] Owning focused tests, supported installation/dependency checks and actual renderer/native builds pass, or confirmed existing failures have precise evidence and no false completion claim.
- [ ] Required native bundle members remain present; no new crates.io Aster dependency.
- [ ] Changed Rust passes owning tests/Clippy and root fmt check, with baseline failures recorded.
- [ ] No Electron global/tag or native select is introduced; no new configuration, guest manager, workflow or approval engine.
- [ ] Tenant/data contracts and private submodule boundaries remain intact; no credentials or user data copied.
- [ ] Background CUA verifies retained functionality; frozen long-horizon scope remains required.
