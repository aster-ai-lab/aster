# Tasks: Current Core and ACP in Aster TUI

**Input**: [spec.md](spec.md), [plan.md](plan.md)
**House rule**: Each behavior fix includes its hermetic regression in the owning package. Completion is evidence-based, not task-count based.

## Phase 1: Baseline and complete surface

- [x] T001 [US1] Retain source/worktree identity, build `aster` and `aster-acp` together, and inspect actual pair IDs and versions. Current owned source pair: `d2db60c55bd7323babe9f5443f0bcccc045971cf`; exact before/after source records and both binary hashes agree (`local-20261030/final-pair.json`). Rejected source-drift/cache-relocation candidates are retained. Final post-campaign recheck remains T014.
- [ ] T002 [US1] Run existing `apps/aster/tests/workspace.rs`, `tui_one_agent.rs`, runtime-boundary and delivery guards against the source pair; triage each failure independently.
- [ ] T003 [US2] Create the complete capability/feature matrix from Core inventory, ACP commands/host methods and shipped pager/CLI entries. Every row names implementation, activation, success, refusal/lifecycle cases and evidence.
- [ ] T004 [US3] Review connected source and tests for oracle branches, guessed capability meanings, fake assertions, state loss and lifecycle leaks; retain concrete findings.

## Phase 2: Behavior, wiring and hermetic regressions

- [ ] T005 [US1] Fix proven source-pair, configuration, initialization or lifecycle mismatches in the shared owning path with actual-process and focused regressions.
- [ ] T006 [US2] Close current command/capability and tool/result projection gaps in pager/ACP with a hermetic success and rejected/unknown case for each changed contract.
- [ ] T007 [US2] Connect stateful features (sessions, recovery, goals, delegation/background tasks, memory, retrieval and extensions) to their current Core semantics; pair each change with persistence, authority and cancellation cases as relevant.
- [ ] T008 [US2] Under the user's confirmed scope, exercise all default and already configured activations. Verify explicit refusal for each disabled optional/host-owned activation identified in T003; keep all rows and distinguish refusal from successful execution.
- [ ] T009 [US3] Review each fix against callers and negative cases. Reject prompt/model/fixture-name special cases and unmeasured optimizations.

## Phase 3: Full end-to-end campaigns

- [ ] T010 [US3] Drive actual ACP stdio conformance and tool loops with deterministic model fixtures; retain effects and protocol evidence, not only mocks or startup logs.
- [ ] T011 [US2] Drive the actual Aster TUI through the complete matrix in both render modes, including resize, keyboard, streaming, permissions, tools, artifacts and session/restart workflows.
- [ ] T012 [US3] Execute the live-model terminal coding, cancellation and resume campaign using authorized existing credentials without copying them into fixtures or evidence.
- [ ] T013 [US3] Verify every campaign releases its PTY, model fixture and ACP process tree on success, failure and interruption; do not stop unrelated sessions.

## Phase 4: Evidence and completion audit

- [ ] T014 [US1] Rebuild/reidentify the final pair when inputs drift; capture final source and binary hashes.
- [ ] T015 [US3] Record commands, counts, matrix evidence, unresolved findings and Explicitly not claimed in `docs/evidence/code/2026-10-27-tui-core-acp/OUTCOME.md`.
- [ ] T016 [US3] Audit every FR/SC and matrix row against current authoritative evidence. Leave the user goal active until all requirements are proved. No commit or release is authorized by these tasks.

## Final Verification Gate

- [ ] Every required matrix row has direct end-to-end evidence at its claimed scope; no unknown, omitted or merely compatible substitute remains.
- [ ] Owning Core/ACP/pager/CLI tests and relevant existing regression suites pass.
- [ ] `cargo fmt --all -- --check` passes; residual failures are resolved rather than hidden.
- [ ] Workspace dependency and runtime-boundary ratchets pass without increasing ceilings.
- [ ] Source/build identities agree for the actual delivered pair; no upstream binary replacement.
- [ ] Product configuration remains ACL; no user credentials or session state were copied into fixtures.
- [ ] No native `<select>`, speculative framework, oracle branch or permission bypass was added.
- [ ] Live-model and actual TUI workflows pass and all owned test processes are reaped.
- [ ] Final review and requirement-by-requirement completion audit pass.
