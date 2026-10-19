# Tasks: AsterDream task execution

**Input**: [spec.md](spec.md), [plan.md](plan.md), [research.md](research.md)
**Status**: Active; unchecked acceptance work keeps the original goal open.

## Phase 1: Behavior and tests

- [x] T001 [US1–US3] Inspect current rules/source/prior evidence, record primary
  practice references and define the full acceptance requirements here.
- [x] T002 [US1] Extend confirmed missing Office operations in
  `crates/code/acp/src/office_file*` through native public APIs; add real
  save/reopen tests for documents/tables, explicit formulas/display formats and
  presentation chart images, retaining literal text and path/permission bounds.
- [x] T003 [US1] Repair proven numeric/input-contract issues in
  `crates/code/acp/src/output_check*`; cover finite edge cases and schema parity.
- [x] T004 [US2] Repair confirmed fallback/retry/cancel defects in Core
  `fact_control/model_call.rs` and ACP `agent/turn.rs`; each branch gets a small
  controlled failure/wait test preserving ownership and no-effect guarantees.
- [ ] T005 [US3] Audit and verify context/compaction, delegation, budget, memory,
  plugin and scheduler mechanisms through their owning tests. Fix confirmed
  defects with sibling regressions and record measured resource behavior.

## Phase 2: Wiring

- [x] T006 [US1–US2] Add and run real Work→ACP/Core→controlled-provider tests in
  `apps/work/src-tauri/src/agent_plane`, verifying discovery, tool outcomes,
  saved files, artifact capture and final user text; denied calls cannot pass.
- [ ] T007 [US1–US3] Exercise current task and GUI journeys through actual host
  entry points; replay the original task corpus with real provider and
  independent oracle data. Missing prerequisites remain open, not substituted.
- [x] T008 [US1–US3] Independently review changed callers, permissions, input
  schemas, exported APIs, bounded reads/writes and cancellation/resource edges.

## Phase 3: Evidence

- [x] T009 [US1–US3] Maintain requirement-by-requirement current evidence in
  `docs/evidence/work/2026-10-18-execution-audit/OUTCOME.md`; record commands,
  counts, failures, measurements, remaining work and Explicitly not claimed.
- [x] T010 [US1–US3] Audit completion against every FR/SC and leave the goal
  active if any is incomplete, unknown, indirect or blocked.

No commit task is activated: the user has not requested a commit.

Historical checkpoint (2026-10-18): T005 has 68 delegation/budget/compaction and 63
context/memory/scheduler tests; optional plugin/background/recovery combinations
remain conditional. T006's final exact three-case rerun and four ordinary host
tests passed; its ACP digest is recorded in the evidence. T007 has
only a read-only observation of the existing desktop, not an updated GUI or
live-corpus pass. The latest `just build-work` is blocked by the concurrently
introduced, incomplete Core OCI sandbox module; it was not reverted or waived.

Earlier 2026-10-20 checkpoint (superseded by the optimization records below):
a later `just build-work` and actual T01 run supersede the
old build blocker, not the full corpus/GUI gate. T01 completed in 482.856 s
and passed 8/9 local scenarios. The command-latency candidate has root builds,
39 focused runtime checks and 3 real-VM governance cases; no full optimized
T01 or candidate application deployment is claimed. Whole-workspace formatting
and dependency checks still have recorded failures. See the linked evidence
in [research.md](research.md).

## Phase 4: Optimization behavior and owning tests (M0–M3)

**9 tasks; 8 done, 1 open.** Each new behavior carries its sibling hermetic
test; hardware/live evaluation supplements those tests.

- [x] T011 [US3] Record the historical T01 timing split and metadata-only OCI
  phases in `crates/code/core/src/sandbox/oci.rs` / `oci/tests.rs`, with raw
  samples under `docs/evidence/sandbox/2026-10-19-command-latency/` and sanitized
  T01 timings under `docs/evidence/studio/2026-10-19-t01-deepseek-flash/runtime/`.
- [x] T012 [US2–US3] Build the candidate runtime from the root workspace;
  retain SHA identity checks and test EOF, owner death, console/report identity,
  refusal, cancellation and deadline settlement. Exact changed paths and
  39 focused checks are in the latency evidence's `integration-checks.json`;
  Core `sandbox/oci/tests.rs` additionally passed 3 real-VM governance cases.
- [x] T013 [US3] After T012, extend the ignored timing probe in
  `crates/code/core/src/sandbox/oci/tests.rs` for the bounded 19-command rounds
  in M1. Check every exit/output/deadline; compare same-source SHA variants,
  record first-run and subsequent timings, sample distributions, binary hashes
  and build differences in the existing command-latency evidence directory.
- [x] T014 [US3] After T011, audit actual prompt assembly and tool projection
  in Core `tools/presentation.rs`, `tools/selector.rs`, `context/assembler.rs`
  and `prompts/common/repository_tool_contract.md`. Extend sibling projection
  tests to prove the admitted schemas reach the provider with stable profile
  identity; denied tools remain absent. Change only a demonstrated gap.
- [x] T015 [US2–US3] After T014, extend
  `crates/code/core/src/tools/program_tool/tests.rs` using the existing executor:
  bounded parallel reads, dependent write→check, structured nonzero failure,
  denied child, call/output/time/Run budget exhaustion, cancellation settlement
  and truncation/source retention. Reuse the existing nested native-artifact
  proof regression; an unrecovered required check failure cannot certify
  completion, while expected failure followed by verified recovery stays valid
  with both receipts. Verify inherited permission and Run budgets through
  `agent_api/tests/fact_log/nested_governance.rs` and actual Run fixtures;
  bare-executor tests alone are insufficient.
- [x] T016 [US3] After T015, extend the existing
  `crates/code/core/tests/test_extensibility_real_llm.rs` fixture and its small
  hermetic metric checks for fixed adaptive/code sessions. Preserve parent and
  nested call counts, VM count, interval union, available tokens and output
  bytes. Separate explicitly forced mechanism tests from unforced task prompts;
  missing token data stays null/unavailable.
- [x] T017 [US3] After T014–T016, make only measured prompt/presentation or
  orchestration corrections in the existing Core modules. Reuse `program`,
  `batch`, bounded output and artifact references; add a regression for every
  actual correction. If no gap is found, record that decision instead of adding
  a framework, task-name router, new setting or unconditional PTC requirement.
- [x] T018 [US3] Audit host-trusted retrieval settings and extend the owning
  ACL parser tests in `crates/code/core/src/config/loader_tests.rs` and ACP
  `config.rs` / `config/tests/` for enabled/disabled/local_cpu, invalid/duplicate
  values and explicit config precedence. After T014, add the minimal trusted
  ACL presentation field in Core `config/mod.rs` / `config/loader.rs`, using
  the existing typed modes, default adaptive and unknown-value refusal. Test
  parse/render parity and define the narrow Work handoff; workspace settings
  cannot supply host providers, paths or credentials.
- [ ] T019 [P] [US3] After T011 and a captured initial source/asset manifest,
  implement the minimal ACP
  `EmbeddingProvider` adapter in a sibling module under
  `crates/code/acp/src/agent/`, reusing public `crates/bge/src/embedding.rs`
  APIs. Add hermetic fake-backend tests for dimensions, cancellation, bounded
  batches, missing/corrupt assets and explicit disabled/degraded behavior.
  Fake cancellation is not real ONNX settlement: qualify the actual backend,
  adding a minimal public cancellation/settlement seam with sibling tests in
  `crates/bge/src/embedding.rs` only if needed, preserving existing APIs and
  keeping resources owned until settlement. No ready claim before that proof;
  do not download assets or start a remote provider implicitly. This can run
  beside T013–T017 on separate files; agent.rs wiring waits for Phase 5.

## Phase 5: Optimization callers and configuration (M0, M2–M3)

**3 tasks; 1 done, 2 open.** Work/ACP wiring is serialized to avoid shared-file
conflicts; production defaults change only after the evidence decision.

- [ ] T020 [US3] After T011, T014, T016 and T018, wire the trusted ACL profile
  through Work `agent_plane/launcher.rs` and ACP session setup into
  `SessionOptions::with_tool_presentation_profile`. Extend real-ACP test setup
  in `apps/work/src-tauri/src/agent_plane/local_acp_tests.rs` and ACP session
  tests to record the actual companion identities, effective capability/profile
  and bounded run metrics. Keep omitted configuration adaptive; verify new-session
  selection and resume mismatch refusal. Cover missing selected workspace/input,
  unavailable backend and exhausted request budget with existing typed refusal /
  stop reasons, preserving verified progress and artifact references without
  retry loops. Keep test variant control out of the product UI and do not change
  defaults before T023.
- [ ] T021 [US3] After T018–T020, wire only the trusted retrieval declaration
  through Work `agent_plane/launcher.rs`, ACP `config.rs`,
  `agent/session_open.rs` and `agent.rs` into existing Core session options.
  Extend launcher/config and `agent/retrieval_seam_tests.rs` plus real-ACP
  fixtures for opt-out, ready, missing-assets degradation and cancellation.
  A fake ready fixture does not qualify the installed BGE backend: real assets,
  cancellation/timeout settlement and resource ownership require separate proof.
  Independently verify `search(mode="grep")` without embeddings and lazy code
  tools on a code fixture; model access cannot exceed workspace permissions.
- [x] T022 [US1–US3] After T013, T015–T017 and T020, run the Work→real ACP/Core
  controlled-provider fixture with program-nested reads/write/validation.
  Extend `local_acp_delivery_tests.rs` to verify saved bytes, nested artifact
  capture, final reply and refused/failed/cancelled terminals, plus successful
  repair with the original failed receipt retained and new bytes certified.
  These are five explicit scenarios. Retrieval state
  is fixed for the performance experiment, independent of T021's readiness.
  The previous binary passed all five, but the first 2M live attempt exposed
  missing failed-program child receipts and nested formal Bash proof binding;
  a new post-read tightloop reproduction also failed to settle. These owning
  regressions and the exact final-binary five-case rerun are now qualified. Preserve
  Failed/Incomplete refusals, late/concurrent-write invalidation and native
  worker ownership; current qualification is `real-acp/final-binary-v4/`,
  not the historical five-case pass.

Current T022 checkpoint: the three production repairs and independent reviews
have passed their owning checks, and paired Aster/ACP/Office/Work plus the frozen
matrix runner built successfully (`final-build-v3/builds.json`). The exact ACP
five-case rerun was **4 passed / 1 failed** (`real-acp/final-binary-v3/`); the
refusal reached its expected terminal but failed the controlled provider's
background extraction drain. A delayed-first-byte socket regression reproduced
the Darwin fixture defect, and the test-only correction passed **2/2** with
independent review (`real-acp/provider-socket/`). The subsequent unchanged
five-case suite passed **5/5** against the same frozen
ACP (`real-acp/final-binary-v4/qualified.json`): all five actual mapped identities
match, all owned ACP PIDs exited, and all paired binaries stayed unchanged.
The pre-model `matrix/admission-v3.json` preserves the old failure and replaces
only its gate pointer. T022 is closed for this tested macOS configuration.

## Phase 6: Optimization evidence and delivery (M4–M5)

**5 tasks; 2 done, 3 open.** T023 completed the bounded screening and failure
decision; no configuration was promoted. T024 still requires a passing decision.
The user-requested T027 independent development-app assessment ended with a
host interruption and 8/9 saved artifacts; it does not waive that gate.
Missing assets or platform runners remain explicit.

- [x] T023 [US1–US3] After T022, execute the bounded 2×2 matrix in [plan.md](plan.md)
  with new sessions/workspaces and fixed inputs/model/permissions/budgets.
  Retain the four terminal attempts. Refreeze after the confirmed production
  fixes, then at most one sample per cell; only if all four qualify may the
  baseline/candidate pair receive one additional sample each. Including the
  final T027 CUA, at most eleven total T01 attempts and eight eligible held-out
  Runs; do not reset counts after source changes or increase the 2M token cap.
  Reuse the independent T01 oracle outside model access, preserve the known
  signed-zero failure. Held-out multi-file analysis/code tasks are conditional
  on all four screening cells qualifying; this condition failed, so none start.
  Record all samples/failures and per-factor decisions in
  `docs/evidence/code/2026-10-20-task-orchestration/OUTCOME.md`; no predicted
  end-to-end result from microbenchmark subtraction.
  Completed as a failed screening decision (`matrix/screen-outcome-v3.json`):
  HC 343.584 s, SA 456.712 s, HA 280.916 s, SC 523.432 s; every original
  submission passed 8/9, so neither runtime nor PTC qualified for promotion.
  SA stopped at the fixed token budget. HC's pre-dispatch schema refusal has
  no program receipt; preserve `metrics_complete=false` and unknown counts,
  rather than inserting zero or reconstructing a receipt. All eight Core
  attempts are terminal; no additional pair, held-out or paid retry is admitted.
  This check marks the experiment and stop decision complete, not T01 quality,
  stable speed improvement or product qualification.
- [ ] T024 [US1–US3] After a passing T023 decision, build paired terminal/ACP,
  Office and Work from the root, run scoped tests and required CI checks;
  investigate recorded format/dependency failures without widening ratchets.
  Stage the verified candidate beside existing assets, switch only the
  development application's selected assets, verify actual process hashes,
  run host task/cancel/timeout/artifact smoke and retain the previous selection
  as the tested rollback. No installed release replacement is implied.
- [ ] T025 [US2–US3] Qualify changed shared behavior on Windows and Linux with
  owning-package tests and their actual supported sandbox backends; macOS
  results are separate. Do not replace an unqualified platform's runtime or
  introduce a weaker host fallback. Record runners/backends that are missing.
- [ ] T026 [US1–US3] After the T023 decision, update the existing runtime/T01
  evidence and original acceptance matrix with exact source/build identities,
  correctness, all timing boundaries and Explicitly not claimed. Independently
  review authority, output reduction and cancellation. Close this slice only
  against FR-009/012/013 and SC-004/006; T005/T007 and unrelated original gates
  remain open until separately proved. Keep T021's unqualified ONNX backend
  and T025's unavailable platform runners open separately; neither blocks a
  macOS capability evaluation with retrieval explicitly disabled. T024 remains
  unqualified: staging a separate development app is not configuration promotion.
- [x] T027 [US1–US3] Conduct the user-requested independent development-app
  capability assessment, separate from optimization promotion. Use the
  visible, non-activating CUA mode to launch the verified development
  AsterDream and submit the original T01 instructions to DeepSeek Flash in a
  fresh workspace. Observe model identity, actual execution and final visible
  state; independently evaluate the saved submission with all nine local
  oracle scenarios. Record the complete result, including failures, input
  integrity, output paths, timing boundaries and tested binary identities.
  Do not repair the model's submitted pipeline or expose oracle answers during
  the run. This is the explicit 2026-10-20 final evaluation request.
  Record the exact macOS development configuration and capability result;
  this does not close optimization, ONNX or Windows/Linux qualification.
  Current checkpoint: the paired independent app is staged, actual app/ACP
  identities checked, and reopening the unchanged old app verified rollback.
  The ninth total T01 attempt ended as a host approval-recovery failure.
  The original native decision awaited confirmation without visible approval
  or pause controls; normal stage-app exit ended the owned app/ACP processes.
  Reopening the same app showed the explicit interruption error, without a
  new prompt. Saved interrupted artifacts were frozen before the 9-case oracle:
  8/9, negative zero failed, existing results byte-identical to regeneration,
  and all 12 input hashes unchanged. Evidence is under `real-acp/t027-cua/`.
  This closes the assessment, not model completion, T01 quality or promotion.
  CUA wall includes approvals/operator diagnostics and remains separate from
  the fixed-permission Core matrix; no additional paid attempt is admitted.

Optimization checkpoint: T013 has 76/76 command samples, 608 phases and 6/6
preflight VM governance checks in the dedicated `T013-OUTCOME.md`. T014–T017
have 72 distinct focused tests, including real parent/delegated-child provider
projection, recorded in `docs/evidence/code/2026-10-20-task-orchestration/ptc/`.
T018 has Core config 113/113, Core bootstrap 4/4, ACP layer loading 3/3 and
Work launcher 19/19; strict token/delimiter and symlink refusals were added
after independent review. T019's 16 hermetic tests pass, but installed ONNX
qualification is still open; it is intentionally unchecked. Actual ACP final
five-case qualification is 5/5. The model matrix completed without promotion;
the CUA assessment failed its host/quality gates, and affected-platform
qualification remains open.
T020/T021 retain their unverified actual-host/refusal/readiness obligations:
trusted profile/retrieval wiring, embedding-free grep and a real Rust LSP
navigation result do not by themselves close every entry-point requirement.
Next work follows [plan.md](plan.md): T029 approval recovery/cancel, T028
execution target context, existing PTC adoption/rejection analysis and only
reproduced contract repairs, then actual capability entry points. No new paid
model run or automatic default change is part of this closeout.

## Follow-up: effective Bash execution environment

- [x] T028 [US3] After freezing T027's terminal assessment and T029's control
  regression gate, repair the confirmed
  host/guest context ambiguity in Core `agent/turn_context.rs`, the bound
  `BashSandbox` capability seam and Bash tool presentation. Preserve the host
  OS, separately describe the actual Bash execution target using typed backend
  facts, and keep unreported targets (including custom backends without
  metadata) unknown. Preserve public callers
  with the smallest compatible seam; do not infer a distro/tool family from
  names, alter commands or weaken sandbox/permission checks. Add a small
  controlled-provider regression that observes the real fact-path model input
  for a macOS host with Linux OCI target, parent and tool-capable delegated
  child, adaptive/code presentation and optional-context timeout. Keep required
  target information outside truncated descriptions. Verify host execution is
  separately described when authorized. Source-only diagnosis is in
  `docs/evidence/code/2026-10-20-task-orchestration/execution-environment/OUTCOME.md`;
  it proves ambiguity, not the cause of a particular model command or speedup.
  No additional paid T01 run is admitted by this task.
  Current regression evidence is in `execution-environment/t028/`: 17 distinct
  controlled/unit cases pass, including actual fact provider parent/delegated
  requests in adaptive/code and optional timeout. Two portable Windows shell
  guidance tests passed on macOS; the earlier zero-match filter is retained and
  not counted. No actual Windows runner or task-speed claim follows.

- [x] T029 [US2–US3] First repair Work approval recovery and cancellation
  reachability, using `approval-recovery/OUTCOME.md` as source-only diagnosis
  and `real-acp/t027-cua/` as the failed assessment. Bind reconnect state to
  the current native execution and valid pending approval; persisted pending
  alone cannot authorize or revive an expired request. Prevent an old overlay
  from hiding a newer authoritative approval. Keep abort reachable while
  awaiting approval/form input and verify it settles the actual owned turn.
  Sidebar End remains list organization, not cancellation. Reuse controlled
  provider, stream/approval and frontend fixtures for reconnect, stale/closed
  answers, cross-run isolation, orphan pending and cancellation settlement.
  Do not claim the unobserved overlay state caused this specific run, broaden
  permissions, bypass confirmation or use a new prompt as recovery. No real
  model, VM, new orchestration framework or paid T01 retry is needed.
  Current control regression gate passed: frontend connected regressions pass
  182 tests (one existing conditional skip); native approval, response, captured
  repair, status/SSE, form, question, deletion and scheduled-owner groups pass
  on the same frozen native test binary. Counts and source identity are in
  `goal-resumed-20261020/t029-summary.json` and `native-v2.json` in the evidence
  directory. Retain the original RED and Flow build failures. One existing form
  fixture used an empty execution id inconsistent with its Execution; it was
  aligned to the real id and its status receive now has a timeout. The earlier
  exact fixture process was terminated and reaped; the corrected group passes.
  This closes the controlled regression gate, not the subsequent GUI assessment.

## Final Verification Gate

### Reopened evaluation goal (2026-10-20)

The user explicitly requested a new visible, non-mouse-taking CUA assessment
after the outstanding optimization repairs are verified. This authorizes one
new T01 submission; it does not erase the nine retained attempts or reopen the
failed four-cell screening campaign. Optional ONNX is outside this assessment.

- [x] T030 [US1–US3] Finish T029 control recovery/cancellation and T028 effective
  execution context, and investigate the two recorded dispatch refusals with
  owning regressions. Repair only reproduced contract defects. Verify current
  Work→ACP→Core presentation, grep and code-intelligence entry points without
  requiring embeddings. Retain source hashes and current focused results.
  Controlled candidate evidence: `goal-resumed-20261020/T030-OUTCOME.md` and
  `t030-results.json` (13 distinct tests, all five groups exit 0). Actual grep
  executes; code intelligence is registration-only qualification. The frozen
  binaries, not the latest entire live source tree, identify this result.
- [x] T031 [US1–US3] Build terminal/ACP, Office and Work together from the root;
  freeze a new renderer and side-by-side development app with actual executable
  identities. Preserve the prior app and evidence. Verify native approval
  recovery/Stop using controlled fixtures before the paid evaluation. Use CUA
  visual observation and semantic app-bound input without pointer movement.

  - [x] Root paired terminal/ACP/Office/Work build and `just build-work` pass.
  - [x] New renderer and separate signed development stage retain executable
    identities; original app and prior evidence are preserved. Port 4322 refusal
    is retained; staged configuration now uses already allowed port 4319.
  - [x] Controlled native approval recovery/Stop and Work→ACP loopback groups pass.
  - [x] Retain R2 corrected-port visual CUA/semantic-input evidence in
    `goal-resumed-20261020/stage-launch-4319.json`. Its frozen host/ACP identities
    and actual port 4319 launch are recorded separately from the initial 4322
    refusal. The ongoing R2 T01 run is not replaced or modified by R3 staging.
  - [x] Canonical runtime-carrier regression suite passes 29 cases in each of
    the default and preserve-order graphs. The post-canonical root paired
    build passes (97.038 s); its 45-source before/after snapshots match exactly.
    R3 is separately signed and strict-verified, not launched (`stage-r3.json`).
  - [x] Complete R3 actual guest smoke and retain its executable-bound result:
    `t031-r3-default-bash.json` / `.log` record one controlled loopback pass,
    real Linux output with default Bash and no host escalation, 21.74 s test
    time / 23.66289425 s wall time including cleanup, and no owned process
    remaining. Frozen Work libtest and signed-R3 ACP clone identities are
    recorded in `t031-r3-binaries.json`; see `controlled-sandbox-fixture/OUTCOME.md`.
    T031 combines the retained R2 CUA/semantic-input qualification with the
    separate R3 build and controlled guest proof. R3's GUI app was not launched
    and no new paid CUA submission was performed; T032 remains the original R2
    assessment and is not certified by this smoke.
    R2's earlier post-build guard records semantic `porcelain.rs` drift (42/43).
    The new 45-file build snapshot includes that source revision, but neither
    build identity nor SDK canonical tests extend the earlier T005 proof to
    the new porcelain semantics. Keep these evidence scopes separate.
- [x] T032 [US1–US3] In a fresh workspace containing unchanged T01 inputs and
  instructions, select DeepSeek Flash through the app and submit the original
  request once. Observe actual provider/model, execution, approvals and final
  state. Freeze the unmodified submission before running the existing nine-case
  independent oracle. Report paths, correctness, model/tool/approval timing
  boundaries, cancellation/resource settlement and limitations separately.
  No manual answer repair, oracle hints, automatic extra attempt or promotion
  follows from this authorization. A failed model submission remains a measured
  capability result, not a successful task or proof of speed improvement.
  Result: one R2 CUA submission reached a matching native/visible terminal
  `max_turn_requests` error after 1892.931 s. Its frozen original artifacts score
  8/9; `negative_zero` fails. All 12 input/instruction hashes match. No manual
  repair or extra paid retry occurred. See
  `goal-resumed-20261020/T032-OUTCOME.md`; runtime repairs qualified after this
  observation are not substituted into its measured result.

- [ ] Each changed owning package's focused tests pass on current source.
- [ ] Real pipeline tests and the full acceptance matrix prove all FR/SC rows.
- [ ] `cargo fmt --all -- --check` and applicable workspace CI checks pass.
- [ ] No new remote `a3s-*` dependency or product configuration format.
- [ ] No product native select or new authority bypass.
- [x] Paired terminal/companion build and `just build-work` pass.
- [ ] Live corpus and actual GUI gates are verified; environment/missing oracle
  blocks are resolved or remain explicitly open under the active objective.
- [ ] Runtime/PTC task comparisons identify actual binaries and preserve all
  samples, nested failures, receipts, budgets and independent oracle results.
- [ ] Configured retrieval reaches the session or reports its precise unavailable
  state; no implicit download, remote provider or host authority expansion.
- [ ] Each affected platform has its own qualification or remains explicitly
  unshipped; an optimization claim names its tested configuration and scope.
