# PTC projection, governance and measurements

This evidence concerns T014–T017. It does not establish model or T01 speedup.
The owning plan is `specs/001-task-execution/plan.md`; the starting source
manifest is `docs/evidence/sandbox/2026-10-19-command-latency/optimization-source-start.json`.

## Audit result

The repository already had the versioned presentation profiles, canonical
definition validation, permission filtering, bounded code catalog, frozen
Session identity, persistence and child narrowing. `tools/selector.rs` exposes
the normal task tool set; `prompts/common/repository_tool_contract.md` already
explains `batch` and `program`. No production prompt change was justified.

The missing connection was the real fact-driven coding turn. Its parent path
called `prepare_fact_model_call` with the auxiliary default, while its child
path used auxiliary evidence. Neither main path applied the frozen profile.
The new real-Run provider fixture reproduced a code profile receiving 19
definitions rather than the single `program` definition.

The common `fact_control/model_call.rs::model_response` now projects admitted
definitions for both main paths. Parent preparation explicitly records
`Profiled`; the child main evidence helper does the same. The original
auxiliary preparation remains auxiliary. Evidence capture errors propagate;
typed presentation/evidence errors and main owner/binding preparation refusals
are terminal `ActorError::Defect`, so permanent errors cannot consume repeated
model requests. Provider transient retry behavior is unchanged.

The only caller of `present_fact_turn_tools` is `model_response`. The only
caller of `prepare_profiled_fact_model_call` in production is
`governed_model_response`. The only caller of `fact_profiled_model_evidence` in
production is `read_model_response`. Tests exercise these explicit main seams
and retain the original auxiliary path.

## Output and timing receipts

`program` child responses preserve their old fields and add `truncated` and
`originalOutputBytes`. Retained child receipts add the original byte count,
truncation flag, monotonic start offset and duration. Timing is scoped to one
program's monotonic clock; it is not a global Run or VM clock. The final output
bound also applies when the byte limit is shorter than the truncation marker.
An expected nonzero child result can be checked and recovered; both receipts
remain available. There is no blanket failure policy based only on exit code.

The real Run retains nested calls in the parent program metadata. Its public
Run-store event projection currently lacks corresponding nested ToolEnd
events. Controlled Run tests observed the actual child side effect, budget
admission and cancellation settlement, but not those public child events.
The fixture therefore consumes original program receipts rather than claiming
that absent events prove zero children. No new public event protocol was added.
Cancellation can currently return a program error without complete child
receipts; metrics explicitly mark those child counts/bytes unavailable.

`tests/extensibility/metrics.rs` separates parent transcript IDs, public child
events and program child receipts. It merges overlapping intervals rather than
adding nested/parallel durations. Missing starts, incomplete receipts, or
unmatchable public/receipt identities remain unavailable. Program unions remain
per-parent. Positive normalized token usage is retained as reported; missing or
zero-only reports are not replaced with estimates. VM counts require an
independent runtime observation, and are never inferred from Bash calls.

The existing ignored provider fixture distinguishes a forced mechanism case
from a fixed natural-language task across adaptive/code sessions. Neither live
fixture was run in this slice. The natural task contains no tool-choice prompt.

## Initial RED observations

Observed tool output before the main-path repair (no separate raw file was
captured for these initial runs):

- `cargo test --locked --offline -p a3s-code-core --lib tools::program_tool::tests -- --nocapture`:
  31 tests, 30 passed, one new dependent write/read fixture failed because its
  ToolContext lacked the required session ID. The fixture was corrected; this
  was not a production write failure. A later 32-test run passed 31; the same
  fixture compared normalized line output without its final newline. It now
  writes and compares an explicit newline, then passed isolated verification.
- `cargo test --locked --offline -p a3s-code-core --lib agent_api::tests::fact_log::nested_governance -- --nocapture`:
  five tests, two passed. The actual code-profile provider projection failed
  with 19 definitions versus one. Budget/cancellation tests proved their
  effects and settlement, then failed assertions expecting public child
  ToolEnd events. Their authoritative receipt assertions were corrected.

The integration target requires the pre-existing `advanced-harness` feature.
The first metrics command without it was refused by Cargo before any test ran.
With the feature, five metrics tests passed and one synthetic fixture failed:
it modeled a program parent without child receipts while expecting a
complete child count. That fixture now exercises the complete public-event
path as `batch`; missing program receipts retain their own null-data fixture.
The corrected test passed alone, then the final metrics group passed.

GREEN commands, raw retained logs, counts and source digests are recorded in
`CHECKS.json` and `source-final.json`. The seven groups contain 72 distinct
passing tests. Additional isolated reruns are not added to that total. No
provider or VM test was selected by these commands.

## Existing independent artifact proof

`docs/evidence/code/2026-10-19-nested-native-artifact-proof/OUTCOME.md` records
the existing Core/ACP native artifact cases. Core's `native_artifact` group is
an additional regression check; the ACP owner runs its actual harness group.
The old mutation-ledger-only case is not presented as a fresh full Run proof.

## Not established

No live provider batch, VM batch, optimized T01, model experiment, production
deployment, cross-platform result, or end-to-end speedup was run or established
by this PTC slice. The public child-event projection gap remains a separate
follow-up. Full-workspace formatting and CI are owned by the parent task.
