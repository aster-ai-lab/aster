# Tasks: Prompt ownership and convergent execution

**Input**: [spec.md](spec.md), [plan.md](plan.md)

## Phase 1: Ownership and baseline

- [x] T001 [US1–US3] Inspect active Core/ACP/TUI/Studio assembly and built-in
  optimization source; freeze current worktree prompt sources in evidence.
- [x] T002 [US1] Add and run Core/ACP prompt-boundary regressions before edits;
  fix generic execution, engineering child scope, effort and continuation prompts.
  Test actual assembly and preserve existing runtime permission/completion checks.
  ACP's attempted pre-change run never acquired the build lock; it is retained
  as a coverage deviation, not claimed as red-test evidence.

## Phase 2: Host and domain wiring

- [x] T003 [US2] Route Office/browser/terminal guidance through existing admitted
  ACP host capabilities; test no-capability, Studio, terminal and restricted hosts.
  Preserve source fidelity, unavailable-tool behavior, review and completion gates.
- [x] T004 [US3] Apply Studio optimizer methods to generation/polish and delivery
  text; test selected-method behavior and prompt type/language/literal preservation.
  Run relevant Studio and TUI integration checks without copying Studio source
  into the parent repository.

## Phase 3: Verification and evidence

- [x] T005 [US1–US3] Run fixed live-model cases, focused regression suites,
  formatting, owning-package Clippy and applicable workspace checks. Record actual
  results, failures, prompt ownership inventory and coverage limits in
  `docs/evidence/code/2026-11-02-prompt-boundaries/OUTCOME.md`.
- [x] T006 Review final task diffs against the baseline, audit every FR/SC against
  its evidence and update this checklist. No commit or staging requested.

## Final Verification Gate

- [x] Core and ACP focused prompt/permission/completion tests pass
- [x] TUI presentation and Studio prompt/delivery tests pass
- [x] Fixed real-model inputs have inspected outputs and actual effect evidence
- [x] `cargo fmt --all -- --check` and owning-package Clippy results retained
- [x] Existing dependency/governance checks pass or existing failures are explicit
- [x] No new dependencies, native select, product configuration or workspace
- [x] Final requirement-by-requirement audit proves the claimed scope
