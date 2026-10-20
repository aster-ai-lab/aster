# Task execution optimization and end-to-end qualification

Status: ten live T01 attempts are terminal; none qualified for promotion.
The final same-source four-cell screen and ninth development-app assessment
remain unchanged. The tenth, newly authorized CUA assessment reached
`max_turn_requests` after 1892.931 seconds; its frozen original submission scored
8/9, failing negative-zero formatting. It did not complete normally. See the
[new assessment](goal-resumed-20261020/T032-OUTCOME.md) and
[tool-budget analysis](goal-resumed-20261020/MAX-TURN-REQUESTS-ANALYSIS.md).
No end-to-end speed improvement is claimed.

Follow-up control repair (T029) passed its controlled regression gate under
the reopened user goal: renderer recovery/Stop and connected regressions pass
182 tests (one existing conditional skip); the connected native groups pass
29 unique tests (two explicit external-evidence skips) on one frozen compiler
artifact. See the [current native summary](goal-resumed-20261020/t029-summary.json)
and [frontend revalidation](approval-recovery/frontend-fix/revalidation-20261020/OUTCOME.md).
The [native repair](approval-recovery/native-fix/OUTCOME.md) includes synchronous
cached-SSE revocation. Original RED, concurrent Flow build failures and the
terminated inconsistent form fixture remain recorded separately. The corrected
fixture uses the current execution id and a bounded status receive.
T028 now passes 17 distinct controlled cases, including the actual fact-driven
model path and portable Windows guidance checks; this is not a Windows runtime
qualification. The two reproduced T005 gaps also pass their owning regressions:
Run-bound confirmation authority and revision-aware verifier repetition, including
the real bounded preimage producer. See [T005 evidence](goal-resumed-20261020/t005/OUTCOME.md)
and [T028 evidence](execution-environment/t028/OUTCOME.md).
The final renderer builds successfully with unchanged inputs; all 43 scoped Rust
files pass formatting. Whole-workspace failures remain separately recorded in
[current workspace checks](goal-resumed-20261020/WORKSPACE-CHECKS.md).
The new CUA assessment is terminal. Its R2 runtime was preserved; follow-up
repairs were qualified separately and never substituted into the measured run.
The OCI carrier now uses the existing recursive canonical JSON digest, with
the same 29 attachment tests passing under both serde map feature modes.
The [approval deadline and stale-answer repair](goal-resumed-20261020/approval-timeout/OUTCOME.md)
passes 10 distinct controlled regressions, retaining legacy confirmations,
owner boundaries and fail-closed timeout behavior. Duplicate prompts remain
separate; their authorization paths were not merged.

The final terminal/ACP/Office/Work paired build passed in 71.089 seconds with
52 scoped source hashes stable. The R4 signed development bundle is prepared
without launching a new desktop or paid task. Its exact signed ACP passed the
real Work-to-Linux-guest Bash fixture in 23.256 seconds, with no remaining owned
processes. See [final repair qualification](goal-resumed-20261020/R4-QUALIFICATION.md).
Final scoped formatting passes 51 files; the retained 52-file check still finds
pre-existing formatting in `agent_tests.rs`. Whole-workspace CI remains failing
as documented; no parallel Flow migration changes were reverted.

## Scope and acceptance

Follow `specs/001-task-execution/plan.md` and T013–T027 in order. Finish runtime,
PTC and configuration qualification before using visible, non-activating CUA
to launch the development AsterDream, select DeepSeek Flash and evaluate a fresh
copy of `../tasks/T01`. Preserve the original task inputs and frozen submission.
Report task execution, environment preparation and independent acceptance times
separately. Existing original-corpus and platform gates remain separate.

## Current implementation

- Strict host-owned ACL declarations for `tool_presentation` and
  `workspace_retrieval`, including duplicate/incomplete-shape refusals,
  per-layer precedence and execution-section rendering. Untrusted workspace
  layers cannot override these settings.
- Work's explicit ACP launch config now passes only those two declarations
  from the trusted host ACL. Bounded reads and source-preserving projection
  avoid copying unrelated providers or silently erasing duplicate fields.
- ACP passes explicit presentation through the public Core SessionOptions
  API. Retrieval preparation is asynchronous, opt-in and local-only; missing
  assets produce an explicit unavailable note, without blocking other tools
  or silently starting a remote service. Native-backend readiness remains
  unqualified until actual backend tests pass.
- PTC and local embedding have owning-module checks; actual host and native
  backend qualification remain separate gates.

## Observations that changed implementation

The actual fact-driven model path did not apply the frozen tool-presentation
profile: a new controlled-provider Run with a code profile still received the
ordinary tool list. The fix distinguishes primary coding turns (including
children) from genuine auxiliary model calls. RED/GREEN evidence is in `ptc/`;
merely registering `program` is insufficient.

Program child-output clipping also lacked an explicit truncation marker and
original byte count. Required validation will consume structured results and
retain the original failure, truncation and artifact evidence; an expected
failure followed by verified recovery remains valid.

## Checks performed so far

From the repository root with the pinned toolchain:

- `cargo test --locked --offline -p a3s-code-core --lib config:: -- --nocapture`:
  **113 passed, 0 failed**, 0.03 s test time after 1m02s compilation. The group
  includes four new capability parse/render/precedence/trust tests and existing
  configuration/session/security checks. This is configuration evidence, not
  PTC or desktop qualification. No raw log file was captured for this first
  run; the tool output and count were observed directly.
- Scoped rustfmt has been run on the new configuration and Work launcher
  files. The full workspace check still reports differences, including some
  optimization-owned files that must be corrected before final qualification;
  see `workspace-fmt.log`. Other dirty-tree differences are retained.
- After adversarial review, strict assignment/literal/delimiter validation
  rejects mixed colon/equal duplicates, bare mode identifiers, stray tokens
  and mismatched/truncated braces. Core config group rerun: **113/113**, 0.04 s;
  raw output is `core-config-final.log`.
- Core ACL profile bootstrap: **4/4**, 0.01 s (`core-bootstrap.log`).
- ACP actual trusted/workspace/explicit layer loading: **3/3**, 0.01 s
  (`acp-capability-config.log`).
- ACP host session setup: **8/8**, 0.21 s (`acp-session-setup.log`). Core
  persisted presentation inheritance/mismatch refusal: **1/1**
  (`core-profile-resume.log`). An earlier short-name `--exact` invocation
  selected zero tests and is not counted.
- Work launcher final group: **19/19**, 0.01 s (`work-launcher-final.log`),
  including bounded UTF-8 reads, source-preserving duplicate projection,
  delimiter refusal, Unicode offsets and the existing ACP symlink refusal.
- PTC: **72 distinct focused tests** across seven groups, including actual
  parent and delegated-child Runs. Exact commands, source identities and
  RED/GREEN limits are in `ptc/CHECKS.json` and `ptc/AUDIT.md`.
- Local embedding: **16 hermetic tests**, plus focused Clippy; see
  `local-embedding/checks.json`. These tests do not load actual model assets.
- Runtime: **76/76** timed command samples and **6/6** preflight VM governance
  cases. Same-source hardware SHA reduced the sample mean from 10.180 s to
  3.438 s; this is only the OCI microbenchmark. See the runtime directory's
  `T013-OUTCOME.md` and complete raw samples.
- Root paired `aster`/ACP/Office build passed in 1m44s
  (`build-companions.log`). The Work build and actual integration remain
  separate checks; a build does not qualify the model experiment.
- `just build-work` also passed (`build-work.log`), before the subsequent
  LSP readiness change. The final source will require a matching rebuild.
- Workspace dependency checks with Python 3.13: **4 passed, 2 failed**
  (`workspace-dependencies.log`). Failures remain the pre-existing nested
  `apps/work/.git` and `a3s-gateway` publish metadata. The system Python lacked
  `tomllib`; that invocation was a preparation failure, not a test result.
- The independent evaluator now includes the previously recorded signed-zero
  case. Its oracle-wrapper self-check passed **9/9**; the unchanged frozen
  model submission reproduced **8/9**, with only the two expected signed-zero
  string differences. Reports are `oracle-selfcheck-nine.json` (tool-only)
  and `oracle-baseline-nine.json` (original submission). Historical reports
  and original task inputs were preserved.
- Failed write effects now remain part of completion and checkpoint evidence.
  A failed required check cannot be certified solely from unchanged file bytes;
  a successful repair can certify new bytes while retaining the failed receipt.
  The cache additionally requires the exact failed-effect projection from the
  selected original receipt prefix. Focused checks: **14 completion-attestor,
  60 ledger and 11 receipt-cache tests passed**; commands and log hashes are in
  `real-acp/core-checks.json`. These are 85 distinct checks, not a full desktop
  or model-task pass.
- Final projection integration exposed a visibility mismatch: the fact path
  could present tools excluded by the ordinary permission-derived catalog.
  The fix reuses the existing filter before applying the profile. The focused
  groups passed **8 nested-governance, 10 fact-governance and 26 evidence tests**;
  four existing protocol integration targets passed **59/59**. The earlier
  unknown-`Skill` failures remain in `core-protocol-*.log`. This does not change
  the existing explicitly configured Ask-without-confirmation execution policy.
- Final real Work→ACP/Core program group passed **5/5**, with one fresh ACP
  identity throughout: `8e4d312fb2dc6b4e634e472ea87f48e8a8f749050d5a34f774ef8ee36f1ffdaf`.
  Success, required-check failure, refusal before execution, nested cancellation
  and successful repair each retained the expected original facts, artifact
  state and visible terminal. Raw output is `real-acp/integration-qualified.log`.
  Its 92.80 s test-group wall includes identity hashing, preparation and cleanup;
  it is not a live model-task performance sample.
- Core Clippy with `--locked --offline --lib --tests --message-format=json`
  exited 0. The final report retains 14 warnings outside the visibility change's
  three files; those three files and the owned code-intelligence files have no
  warnings. See `ptc/visibility-regression.json`, `ptc/visibility-clippy.json`
  and `code-intelligence/clippy.json`. This is not a clean whole-workspace CI claim.

## Current execution path

The updated plan's immediate sequence is T020/T021 entry qualification →
T022 actual Work/ACP program cases → T023 frozen single-run experiment cells →
T024/T025 development staging and per-platform checks → T026/T027 evidence and
visible non-activating CUA evaluation. It keeps the runtime and presentation
decisions independent and leaves disabled retrieval fixed during timing runs.

Independent review found the existing live extensibility fixture cannot stand
in for T023: it pins other Flash routes, measures a two-file task, and does not
retain full failure metrics. The test-only single-Run matrix fixture now binds
the declared DeepSeek route/endpoint, verifies frozen identities and saves
failure receipts before asserting. Its **9 hermetic checks passed**, with the
live test still ignored in the ordinary test command. See
`task-matrix-runner/OUTCOME.md`. A final paired build and actual test executable
were subsequently frozen, and the first explicit live attempt ran as follows.

`t023-sa-01` (software SHA, adaptive) retained its original submission and failed
ordinary completion at the frozen reported-token soft cap: ledger 1,004,236
against 1,000,000; task wall 408.528 s, outer process wall 442.325 s. It produced
both declared files, kept inputs and frozen identities unchanged, and observed
15 complete VM command lifecycles. The 9.516 s backend preflight is separate.
However, the Run emitted 45,684 events while the test runner retained only
16,384. Its window-derived model/tool/token summaries are incomplete and must
not support a speed comparison. The original failed report, executable,
manifest and bytes are preserved; this attempt counts against the overall
12-attempt T01 ceiling. See `matrix/initial-live-checkpoint.json`.

Before further model access, the test-only runner will increase its still
bounded event-count capacity and require a complete event window for a
complete-metrics claim. It will be rebuilt/refrozen with a new identity; the
model budget stays fixed. This is an evaluation-recording correction, not a
production optimization or repair of the model's answer. Independent nine-case
evaluation of the unchanged submitted pipeline passed **8/9** in 1.775 s;
only the two negative-zero reconciliation strings differed, as in the historical
baseline. The original submission and input hashes were unchanged. See
`matrix/process/t023-sa-01-oracle.json`; this artifact score does not change the
failed Run terminal into success.

The test-only recording correction passed **10 hermetic checks** (plus one
live ignored test), including a RED/GREEN regression for apparently complete
suffix metrics after earlier model events were dropped. Version 2 retains at
most 262,144 events / 64 MiB, with the original 1,000,000-token soft cap. Its
source differs only in `tests/extensibility/task_matrix.rs`; production source
and all fixed task settings are unchanged. New executable/manifest identities
are in `matrix/plan-v2.json`. It permits at most eight v2 attempts, plus the
retained earlier failure; incomplete v1 metrics are not pooled with v2.

The first v2 cell, `t023-v2-hc-01` (hardware SHA, code), finished with a failed
budget terminal: **276.034 s task wall**, **1,028,497 reported tokens**. All
33,334 events were retained; metrics cover 28 model calls, 28 executed parent
calls and 17 complete VM lifecycles. The profiled snapshots and model-input
snapshots consistently describe one projected Tool, but the resulting model
decisions used direct tools and no `program`. A code presentation by itself
therefore did not establish PTC adoption. The unchanged submitted pipeline
independently passed **9/9** in **1.828 s**; artifact correctness and the failed
Run terminal are separate. See `matrix/process/t023-v2-hc-01-checkpoint.json`
and its oracle report. No completed-task speedup or profile promotion is claimed.

The same-version software/adaptive pilot then failed at 1,000,946 reported
tokens: **315.839 s task wall**, 28 model calls, 19 complete VM lifecycles,
all 12,976 events retained. Its unchanged submission passed **8/9** in
**1.789 s**, with only the known signed-zero strings differing. Its failed
terminal remains unchanged; see `matrix/process/t023-v2-sa-01-metadata.json`.
A requested one-second CPU sample found the exact process already terminal;
no stack was collected and no CPU-hotspot conclusion is claimed.

All three pilot processes are now terminal. The common 1M limit was introduced
by the experiment, not required by the original task, and stopped both runtime/
profile configurations after producing files (including one 9/9 submission).
An independently reviewed **one-time resource calibration** therefore freezes
2M reported tokens for a new four-cell regime, using the exact same v2 binary,
production source, inputs, prompt and other budgets. The 15-minute/128-round
bounds remain, no further budget increase is allowed, and the three earlier
attempts count toward a maximum of eleven total T01 attempts. Only if every
initial calibrated cell completes with full metrics and a passing oracle may
the reverse second wave run; otherwise the initial outcomes are reported.
Pilot times are not pooled with the calibrated regime. The decision and actual
manifests precede model access in `matrix/calibration-decision.json` and
`matrix/plan-calibrated.json`. Its first attempt is now terminal; no other
builds, tests or model tasks ran during its timing window.

### First calibrated attempt and current blocking repairs

`t023-2m-hc-01` (hardware/code) failed ordinary completion in **311.182 s**,
using **1,631,144 reported tokens**, below the unchanged 2M cap. All 32,008
events were retained. It made 29 model calls and 28 parent program calls;
20 succeeded and eight failed. Successful parents retain 33 observed child
calls; failed parents lacked child arrays, so the original global child totals
remain unknown. Six of those failures were native source/entrypoint rejections
before worker creation; two were actual QuickJS execution errors. They must
not all be guessed to have zero children. The independent frozen submission
passed **8/9** in **1.731 s**, with only the signed-zero case failing. Original
inputs, submission and as-run identities were unchanged.

The 631-byte terminal refusal was reconstructed exactly from the original
typed receipt prefix. Its effect digest is
`fc3a2a92c99bf1e5f3f24cb0d0e82d421d9e38797f9abb40795705df7d3bf8f9`.
The final negative failed-effect projection was empty after the successful
cleanup; the refusal was for missing bound Passed verification. Actual nested
Bash checks were present, but their native proof did not reach the parent
completion seam. Do not weaken the gate, install a byte-only attestor as a
substitute, or repair the model's submission. See the original result,
checkpoint, metadata and oracle under `matrix/process/t023-2m-hc-01-*`.

Three specific repairs now precede another paid attempt:

1. `program_tool.rs` must retain native child receipts on exceptions, timeout,
   cancellation and worker failure. Pre-worker refusals can certify an empty
   array; incomplete dispatch/settlement must retain observed records and mark
   completeness false. The receipt slice has **37 tool, two original-fact and
   eight metrics checks passing**, without relabelling the old live evidence.
2. The original post-read tightloop test must settle. Two bounded child runs
   reproduced the hang; a one-second stack sample shows the nested Tokio VM
   parked after the QuickJS interrupt. Pinned rquickjs swallows the asynchronous
   job exception, leaving the root Promise pending; abort cannot stop an
   already running `spawn_blocking`. Repair the VM's own wait and drain native
   child futures before joining; dropping/detaching the worker is unacceptable.
   The first idle-based candidate exposed a real QuickJS GC assertion and is
   retained as failed evidence. The final context-driven repair passes **39
   program, two original-fact, eight nested-governance and eight metrics
   checks**, including ten bounded child processes and actual settlement beyond
   the grace period. Independent final-source review found no blocker. This
   qualifies the tested normal-return and stop paths, not universal caller-Drop
   governance or a 500 ms hard kill. Evidence and exact source identities are in
   `program-failure-receipts/OUTCOME.md` and `freeze.json`.
3. Transfer only live native nested formal Bash proof, through the existing
   private verification scope. Bind to exact current file identity and parent
   effects; invalidate for late/concurrent mutation, failed/incomplete parent,
   and attempted serialized/model-metadata forgery. The implementation and
   regression suite are now qualified: **14 new owning cases**, with **65
   harness, five native-artifact, eight governance and 39 program** checks
   passing. Cross-wrapper concurrency and the finish/relock race were each
   reproduced and closed; independent frozen-source review found no remaining
   blocker. The existing shell group passed 41 with one pre-existing ignored
   test. See `real-acp/nested-shell/OUTCOME.md`. No completion authority comes
   from JS text, and controlled Bash backend tests do not qualify actual OCI.

The broader fact-control group exposed a test-only authority mismatch: its
completion used an explicit lane policy while its borrowed model surface had
neither checker nor confirmation. The fail-closed production filter was
correct. Aligning the fixture's two policy inputs retained all original
assertions and passed the exact case plus **162/162** checks. No production
permission logic changed; see `ptc/fixture-authority/OUTCOME.md`. The combined
current source was frozen and paired Aster/ACP/Office/Work plus the matrix
runner built successfully; exact identities are in `final-build-v3/builds.json`.
Scoped format and Core Clippy also passed, with no added warning signature.
This does not close the recorded full-workspace formatting/dependency failures.

The fresh ACP's five-case protocol rerun passed **4/5**. The refusal reached its
expected terminal, then failed the controlled provider's background extraction
drain after a loopback reset. That result remains unchanged in
`real-acp/final-binary-v3/`. A new delayed-first-byte socket regression reproduced
the test helper's inherited nonblocking flag on Darwin. Explicit blocking and
bounded read/write I/O passed **2/2** with independent review; request validation,
refusal assertions and the drain deadline remain unchanged. See
`real-acp/provider-socket/OUTCOME.md`. The original run did not retain its first
server-side errno, so this is a reproduced fixture defect, not proof of that
historical reset's exact errno. Only test sources changed after the paired
as-built snapshot; preserve that snapshot and record the later fixture revision
separately. The updated harness subsequently passed **5/5** against the same
frozen ACP; `real-acp/final-binary-v4/qualified.json` binds all five actual process
identities, owned PID exits and unchanged paired binaries. The old failed record
remains. `matrix/admission-v3.json` was written before the first v3 model access
and replaces only the old protocol gate pointer, retaining all conditions.

T022 is now qualified for the tested macOS configuration. The prepared v3
Core source, runner, input and complete asset manifests were independently
verified before model access. Production changes require affected binaries
to be rebuilt and refrozen; the test-only helper change requires the updated
harness and its own source identity, without rewriting the paired as-built
record. Old source identities remain as-run evidence. The
prospective remaining cap is six Core attempts plus one final CUA, at most
eleven total T01 attempts including the four already retained. First screen
four same-source cells; only all-pass permits one extra baseline/candidate
pair. Do not increase the 2M budget again or pool different source regimes.

The first v3 cell, hardware/code (`t023-v3-hc-01`), has now ended: ordinary task
completion in **343.584 s**, outer process **371.655 s**, **30 model calls**, 30
parent calls, **1,423,101** reported tokens and **20** independently observed VMs.
All 45,016 events were retained, presentation and identities matched, inputs
were unchanged, and session/agent/worker closure settled. Native completion
proof transfer worked in this actual task. However, one parent lacks child
metrics, so total nested counts and interval unions remain unknown; the runner
correctly failed qualification. Its frozen original submission passed **8/9**
independent scenarios in **0.828 s**, failing only signed zero. See
`matrix/process/t023-v3-hc-01-qualification.json`. No rescore, repaired submission,
promotion or end-to-end speed claim follows from this sample. Five paid Core
attempts are now counted; no v3 cell is repeated to rescue the outcome.

Native Rust navigation also exposed a cold-start readiness window: successful
LSP initialize and outline can precede usable definitions. Temporary-fixture
protocol observation confirmed correct project/URI/position assembly and real
project loading, with empty definitions followed by a real location. The
built-in Rust lifecycle now waits for the typed server-status readiness signal
within the existing deadline. On the pinned Rust 1.97.1 analyzer, the final
cold fixture passed a single definition query after readiness (3.63 s test
wall), with the actual child reaped; `code-intelligence/native-qualification.json`
retains the bounded protocol observations. Earlier diagnostic repetitions are
not performance samples, and this is not a blanket language-server or full
workspace readiness claim.

The plan now has a five-batch execution card: close current regressions and the
five real-ACP program cases; qualify the actual capability entry; freeze and
run the bounded matrix; build/stage the qualified development configuration
with rollback; then perform the requested background-visible CUA evaluation.
Missing ONNX assets and absent Windows/Linux runners remain their own open
gates rather than prerequisites for a qualified macOS retrieval-disabled run.

The initial source/asset manifest and runtime microbenchmark results belong in
`docs/evidence/sandbox/2026-10-19-command-latency/`. Later source edits do not
change the frozen test binaries or signed assets used for that comparison.

## Completed v3 screening and independent development assessment

The final same-source screen is complete. Exact immutable per-cell records are
linked by `matrix/screen-outcome-v3.json`:

| Runtime / presentation | Task wall | Model calls | Ordinary completion | Original submission |
| --- | ---: | ---: | --- | --- |
| Hardware / code | 343.584 s | 30 | yes; incomplete metrics | 8/9 |
| Software / adaptive | 456.712 s | 35 | no; fixed token limit | 8/9 |
| Hardware / adaptive | 280.916 s | 34 | yes | 8/9 |
| Software / code | 523.432 s | 41 | yes | 8/9 |

HC's one missing program child array was a native schema rejection before
execution. The original aggregate remains incomplete/unknown; the diagnostic
does not insert zero children or fabricate a receipt. SC's single program
only called `update_plan`, so that count does not establish batching adoption.
All four frozen original pipelines failed the retained signed-zero case. No
cell qualifies, and no extra pair, held-out model task or default change is
admitted. Sparse single-cell observations cannot prove stable task speed or a
causal PTC benefit. The separate 76-command runtime microbenchmark remains
valid within its own scope.

The final Work protocol qualification is 5/5 against the same frozen ACP;
`real-acp/final-binary-v4/qualified.json` retains actual process identities and
settlement. The earlier 4/5 result remains historical evidence. The paired
development app is staged beside the original, signed and identity-checked;
the unchanged old app was actually reopened before the separate app launched.
`real-acp/pre-cua-decision-v3.json` records why the user-requested final CUA
assessment proceeds independently of failed promotion. It does not waive the
quality gate. The ninth task runs DeepSeek Flash with adaptive presentation,
retrieval disabled and native per-operation confirmations in a fresh workspace.
Its observed wall includes approval waiting and must not be pooled with the
fixed-permission Core matrix. Actual app/ACP, effective configuration and an
observed OCI Host/shim pair are retained under `real-acp/t027-cua/`.

That CUA assessment is now terminal. Native facts stopped at a required Bash
approval; the persisted host state was awaiting approval while the UI showed
an idle composer with neither approval nor pause controls. Opening home and
reopening the exact conversation did not restore the controls. Normal exit of
only the staged app ended the original app/ACP and observed OCI processes.
Reopening the same unchanged stage, without a new prompt, produced the visible
error `执行已中断，运行进程已不存在。请重新运行任务。` and host status `error`.
All six saved model-generated scripts/results were frozen before independent
evaluation; no answer was repaired. The pipeline passed 8/9 (negative zero
failed), existing results matched public regeneration byte-for-byte, and all
12 original/copy input hashes matched. Independent oracle wall was 0.960 s.
The 1659.335 s submission-request-to-freeze wall includes approvals, UI recovery
and operator diagnosis; it is not model inference or a completed-task speed
sample. See `real-acp/t027-cua/OUTCOME.md` for the exact boundaries and records.

The executable next path is in `specs/001-task-execution/plan.md`: finish and
archive this assessment; first repair approval recovery/cancel reachability
(T029), then effective host/guest execution context (T028), inspect existing
PTC adoption and failure boundaries and repair only a reproduced contract gap
with a small regression; reconcile actual grep/LSP/configuration entry points.
No further paid model run is part
of this closeout. ONNX assets, Windows/Linux and full-workspace CI gaps remain
separate open qualifications.

## Explicitly not claimed

- No optimized configuration has qualified for promotion. The completed 2×2
  screen did not pass its quality gate; no extra pair or held-out run starts.
- No complete desktop CUA acceptance has passed. Its independent assessment
  ended with host interruption and 8/9 interrupted artifacts, separate from
  configuration promotion or normal model completion.
- No real local ONNX readiness/cancellation proof without installed model
  assets. Fake-backend tests cannot substitute for that qualification.
- No Windows/Linux runtime result or clean full-workspace CI gate is claimed.
- No integrity check, sandbox boundary, required verification, failure receipt
  or resource permit is removed for speed.
