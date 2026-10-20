# A captured exit settles the invocation

Date: 2026-10-20. Local source evidence. Not a published release.

A finished tool call whose workspace file list was incomplete was stored
as an invocation identity in `unresolved_tool_effects`. Nothing removes a
key that is not a settled `workspace-child:`. The completion gate checks
that set first and returns Incomplete, so a turn that already has an
answer fails with the existing identity-obligation sentence. A failed
turn writes no turn-complete fact, so the next send folds the same
receipt and the debt returns.

A captured exit now settles the invocation. An incomplete file list stays
a run-level observation, and a later complete scan replaces it.
`observe_tool_receipt` inserts the invocation key only when the outcome
is `Unknown`. Replay of `tool.result` does the same. Each entry in
`unresolved_external_operations` is still debt.

Missing execution evidence still forces Incomplete, clears
`budget_exceeded`, and records the invocation key in
`unresolved_external_operations` when the call is not read-only and a
process was started. Cancelled and Unknown still record that key. A
shell that never starts stays complete and empty. A preparation with no
terminal stays debt. A background receipt that is Incomplete and not a
budget still records `background-receipt-incomplete:`.

This supersedes the sentence in
`docs/evidence/code/2026-10-20-tool-effect-budget-settlement/OUTCOME.md`
that a missed read with the flag clear denies the next mutating call.
That file is unchanged. The budget flag, the caps, and both gate
sentences are unchanged. Old facts that are Incomplete and do not list
the identity under `unresolved_external_operations` are not replayed as
invocation debt. That recovers a stuck session whose only gap was
attribution. Unknown results and preparations without a terminal still
replay. A new missing-execution receipt records the identity, so replay
still blocks.

## Verification

Rust toolchain: workspace pin `1.97.1`. Commands from the repository root.
`cargo fmt -p a3s-effect -p a3s-code-core` ran first, then `--check`.

| Check | Result |
| --- | --- |
| `cargo fmt -p a3s-effect -p a3s-code-core -- --check` | Exit 0 |
| `cargo test --locked --offline -p a3s-code-core --lib -- porcelain:: harness_loop:: fact_control::receipts::` | 180 passed, 0 failed, 0 ignored, 5056 filtered out. Exit 0. Tests 56.56s after a 1m 12s finish |

Cases that hold the split:
`a_captured_attribution_gap_does_not_freeze_and_unknown_still_does`,
`a_captured_incomplete_receipt_is_not_replayed_as_an_invocation_obligation`,
`incomplete_paths_and_external_unknown_are_distinct_from_a_local_scan`,
`exact_original_unknown_and_incomplete_receipts_prove_cached_uncertainty`,
`missing_execution_evidence_overrides_a_path_budget`,
`a_shell_that_never_starts_on_a_missed_status_does_not_block_a_later_tool`,
`preparation_and_unknown_cannot_be_forgiven_by_a_turn_marker_or_scan`,
`a_capture_failure_after_the_child_starts_stays_unknown`.

The effect crate was not edited. `coding::tool_effects` was not re-run.
The linker warning `__eh_frame section too large` and the workspace
profile warnings are pre-existing.

## Explicitly not claimed

No TUI launch. ACP was not rebuilt. `apps/cloud` was not edited. The caps
were not raised. The completion-gate sentences were not changed.
Background incomplete receipts still book debt. The one obligation in the
27m57s turn was not identified in the fact log. If that key is Unknown,
or a preparation without a terminal, this change does not discharge it.
A process already running stays on the previous binary until rebuilt.
The earlier budget OUTCOME file was left as written.
