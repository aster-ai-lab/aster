# Nested formal Bash verification

The frozen paid HC sample completed its real nested checks but could not bind
them to the parent effect digest. This slice restores the trusted proof path;
it does not weaken the completion gate or install an automatic attestor.

## Diagnosis and scope

The read-only diagnosis is
`../../matrix/failure-diagnostics/t023-2m-hc-01-completion.json`. Projecting the
28 original typed receipts produces 37 final paths, including 34 Missing paths,
and no remaining failed-effect path. The source-generated completion message
matches the recorded error exactly: 631 bytes and SHA-256
`7022e3531312ba1a7d02c56e2c21d40b11f38be89bed23e42a8ea562ed0ed7ca`.
Successful nested Bash checks existed at seq81 and seq87; no accepted report
reached the parent. The original sample and submitted files were not repaired
or relabeled.

Production changes are limited to `harness_loop.rs`, `agent/tool_invoker.rs`
and `agent/tool_completion_runtime.rs`. `production.diff` is the complete
slice diff against the source captured immediately before this slice, rather
than a diff that mixes earlier native-proof and failed-effect work.

The existing private prepared-call ledger now carries a bounded, nonserialized
shell candidate. Only actual governed native Bash metadata, the existing closed
existence-check parser, and descriptor-anchored file observations can create it.
Epoch and active-mutator fences invalidate earlier candidates on later or
concurrent mutations, including a different path restored to its old bytes.
Drop settles the fence without retaining a mutex guard across an await.

Cross-wrapper transfer requires the specific exclusive epoch to remain current
while holding the destination ledger lock. A complete successful original
parent receipt must cover the child's exact after-states. The final report
also checks the exact recorded file identity and current workspace identities.
Native artifact proof behavior and the existing direct Bash report constructor
are reused. A later formal check can recover an earlier failed after-state while
retaining its original Failed receipt.

The diagnostic next-step helper now chooses an exact recorded File with a
content digest. A known Missing suffix alias cannot be suggested. Legacy
byte-backed advice remains available; the global suffix matching API is unchanged.

## Verification

Exact commands, exit codes and logs are in `checks.json` and `qualified.json`.
The source list and SHA-256 values are in `source-final.json`.

- `red-program.log`: direct check passed; equivalent actual program check lost
  proof and refused completion.
- `red-diagnostic.log`: the next-step suggestion selected a deleted suffix alias.
- `red-wrapper.log`: the actual nested wrapper accepted a concurrent ancestor
  mutation. Serial wrapper behavior passed in the same fixture.
- `green-shell-locked-window.log`: 41 passed, one preexisting ignored test.
  This includes nine new fact/program cases and five new ledger/diagnostic cases.
- `harness-final.log`: 65 passed.
- `native-final.log`: five passed.
- `nested-governance-final.log`: eight passed.
- `program-final.log`: 39 passed, including the separately owned worker
  settlement regressions.
- Six owned source/test files pass pinned rustfmt with edition 2021 and
  `skip_children=true`.

Independent source review found and closed a second transfer race between
fence finish and reacquiring the parent lock. The final code rechecks the
specific epoch and active count under that lock. A regression restores the
same digest after a sibling write and still refuses the stale window. No
separate RED execution is claimed for this second race.

`runtime_evidence` independently reviewed the final three production hashes in
`production-freeze.json` and reported no remaining blocker. That review did not
run Cargo, a model or a VM.

## Integration handoff

The full `fact_control::` regression run has 161 passes and one failure;
`fact-yolo-isolated.log` reproduces that failure alone. Root traced it to a
fixture using different permission sources for LiveCompletion and its mounted
history Surface. The empty catalog on the ungoverned Surface is correct
fail-closed behavior. Root owns the fixture correction, retaining its nonempty
catalog, Done and actual receipt assertions. This is not established evidence
of a production Yolo permission regression.

Cargo was released after the governance and program checks. Clippy was not
started because root owns the coordinated fixture integration, final Clippy
and paired build. All Rust changes in this slice are frozen.

## Explicitly not claimed

No real model, VM or paid attempt was started. The governed Bash fixtures use
a closed controlled backend; they do not qualify shell or VM execution.
Descriptor-anchored proof fixtures are Unix-scoped. No new non-Unix observer
fallback, public API, product configuration, profile, budget or attestation
default was introduced. Original negative evidence and completion guards remain.

No current ACP, Work or Office binary is qualified by these Core tests. Fresh
artifact checks, controlled Work-to-ACP cases and CUA remain root-owned. These
tests do not establish task speed or task completion-rate gains. Full workspace
formatting and a workspace audit are not claimed.
