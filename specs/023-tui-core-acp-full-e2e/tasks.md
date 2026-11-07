# Tasks: Current Core and ACP in Aster TUI

**Input**: [spec.md](spec.md), [plan.md](plan.md)
**House rule**: Each behavior fix includes its hermetic regression in the owning package. Completion is evidence-based, not task-count based.

## Phase 1: Baseline and complete surface

- [x] T001 [US1] Retain source/worktree identity, build `aster` and `aster-acp` together, and inspect actual pair IDs and versions. 2026-11-07 recheck: matching pair stamp `aster-cli-pair-v1:bdf7db9857beee3638b283894dc33d94dd4a24e1:aarch64-apple-darwin`; hashes in `docs/evidence/code/2026-11-07-tui-full-e2e/artifacts/final-pair.txt`. Final post-campaign recheck remains T014 if sources drift.
- [x] T002 [US1] Run `tui_one_agent` and shipped_tui_e2e identity/config-failure cases against the source pair (2026-11-07). `workspace.rs` / delivery guards not re-run in that campaign.
- [x] T003 [US2] Create the complete capability/feature matrix from Core inventory, ACP commands/host methods and shipped pager/CLI entries: `docs/evidence/code/2026-11-07-tui-full-e2e/FEATURE-MATRIX.md`.
- [ ] T004 [US3] Review connected source and tests for oracle branches, guessed capability meanings, fake assertions, state loss and lifecycle leaks; retain concrete findings.

## Phase 2: Behavior, wiring and hermetic regressions

- [ ] T005 [US1] Fix proven source-pair, configuration, initialization or lifecycle mismatches in the shared owning path with actual-process and focused regressions.
- [ ] T006 [US2] Close current command/capability and tool/result projection gaps in pager/ACP with a hermetic success and rejected/unknown case for each changed contract.
- [ ] T007 [US2] Connect stateful features (sessions, recovery, goals, delegation/background tasks, memory, retrieval and extensions) to their current Core semantics; pair each change with persistence, authority and cancellation cases as relevant.
- [ ] T008 [US2] Under the user's confirmed scope, exercise all default and already configured activations. Verify explicit refusal for each disabled optional/host-owned activation identified in T003; keep all rows and distinguish refusal from successful execution.
- [ ] T009 [US3] Review each fix against callers and negative cases. Reject prompt/model/fixture-name special cases and unmeasured optimizations.

## Phase 3: Full end-to-end campaigns

- [x] T010 [US3] Drive actual ACP stdio conformance and tool loops with deterministic model fixtures (2026-11-07 shipped_tui_e2e: 18 pass / 3 ignored). Full optional host-method matrix still open.
- [x] T011 [US2] Drive the actual Aster TUI through the baseline slash/session/permission matrix in fullscreen/minimal (launch_resume ORACLEs). A3S/`aster-test` TUI ACL retired; sandbox-gated multi-turn remain open.
- [x] T012 [US3] Live-model campaign (2026-11-07): one-turn + cancel/recover on `deepseek/deepseek-flash`; kimi quota blocked; dual-defect coding not re-run. Credentials not copied into evidence.
- [x] T013 [US3] Campaign runners completed; temp live ACL files removed. No intentional orphan PTY retained.

## Phase 4: Evidence and completion audit

- [x] T014 [US1] 2026-11-07 pair hashes recorded in `docs/evidence/code/2026-11-07-tui-full-e2e/artifacts/final-pair.txt`. Rebuild again if inputs drift.
- [x] T015 [US3] Record commands, counts, matrix evidence, unresolved findings and Explicitly not claimed in `docs/evidence/code/2026-11-07-tui-full-e2e/OUTCOME.md`.
- [ ] T016 [US3] Audit every FR/SC and matrix row against current authoritative evidence. Leave the user goal active until all requirements are proved. No commit or release is authorized by these tasks. SC-002 still open while gap/blocked rows remain.

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
