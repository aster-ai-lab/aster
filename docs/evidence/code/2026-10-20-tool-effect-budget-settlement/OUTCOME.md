# A path or byte budget does not freeze the next mutating call

Date: 2026-10-20. Local source evidence. Not a published release.

A command delta over 4096 paths, or over 1,048,576 bytes of canonical JSON,
was stored as an incomplete observation. That booked `unresolved_tool_effects`,
so the next mutating call was denied. The same denial followed a git status
past 65536 records or 4 MiB, and a non-git walk past 262144 nodes. A bad
record, a timeout, a spawn failure, a non-zero exit, and a directory read
error shared that bucket.

Those three caps are now `budget_exceeded` on `ToolEffectObservationV1`.
The fact lists no prefix. Old facts without the field read as false.
A real read failure stays incomplete and still denies the next mutating
call. The same reason text with the flag clear still denies; settlement
does not match on the reason string. The completion-gate sentence is
unchanged. That gate closes for a budget only when the run-level status
or the non-git walk itself is over its cap. A command that changes many
files while status was fully read still records those paths from the
status diff. Missing execution evidence clears a path budget.

## Verification

Rust toolchain: workspace pin `1.97.1`. Commands from the repository root.
Stdout and exit codes are in the goal scratch directory `implementer`.

| Check | Result |
| --- | --- |
| `cargo fmt -p a3s-effect -p a3s-code-core -- --check` | Exit 0 |
| `cargo test --locked --offline -p a3s-effect --lib -- coding::tool_effects` | 7 passed, 0 failed, 0 ignored, 70 filtered out. Exit 0. Tests 0.02s after a 0.58s finish |
| `cargo test --locked --offline -p a3s-code-core --lib -- porcelain:: harness_loop:: fact_control::receipts::tests::pre_spawn::` | 126 passed, 0 failed, 0 ignored, 5109 filtered out. Exit 0. Tests 47.85s after a 0.67s finish |
| `cargo test --locked --offline -p a3s-code-core --lib -- a_fresh_status_record_cap_closes_the_gate a_path_list_over_the_stamp_cap_stays_recorded_and_the_gate_stays_open` | 2 passed, 0 failed, 0 ignored, 5233 filtered out. Exit 0. Tests 8.80s after a 1.16s finish |

Cases that hold the split: `a_budget_observation_validates_and_old_facts_default_the_flag`,
`a_path_budget_does_not_freeze_and_a_missed_read_still_does` (budget, the
budget sentence with the flag clear, and an unknown outcome that also carries
the flag), `a_background_path_budget_does_not_become_unresolved_debt`,
`typed_path_observation_keeps_missing_unknown_and_contract_bounds_distinct`
(path cap, and 700 paths under `NAME_MAX` whose canonical JSON exceeds 1 MiB),
`a_status_record_cap_is_a_budget_and_drops_the_prefix`,
`a_status_byte_cap_is_a_budget_and_a_hidden_index_byte_cap_keeps_the_status`,
`a_status_budget_does_not_diff_a_prefix_or_fail_the_read`,
`a_non_git_walk_past_the_node_cap_is_a_budget_with_no_prefix`,
`a_non_git_read_failure_stays_incomplete`,
`a_run_level_budget_closes_the_gate_and_a_command_path_list_does_not`
(a fresh status cap on the delta still closes; a revision-name flag does not),
`a_fresh_status_record_cap_closes_the_gate` (bind a small git repo, then
`STATUS_RECORD_CAP + 1` files: empty budget, existing gate sentence),
`a_path_list_over_the_stamp_cap_stays_recorded_and_the_gate_stays_open`,
`missing_execution_evidence_overrides_a_path_budget`, and the earlier
stamp-cap and never-started shell cases, including
`a_write_past_the_stamp_cap_does_not_freeze_the_ledger`.

A status cap discovered on the fresh read sets `PathDelta.status_budget`.
The run fold keeps that budget. It still clears a revision-name cap, so
those paths stay on the status diff and the observation gate stays open.

## Explicitly not claimed

No TUI launch. ACP was not rebuilt. `apps/cloud` was not edited; its
porcelain copy still inlines the full snapshot. The caps 4096, 65536,
4 MiB, 262144, and 1 MiB were not raised. The full status list is not
written to a side blob. Content-only edits past the 4096 stamp cap are
still absent from the command delta. A nested shell that drops a proof
over 1 MiB is unchanged. Search pagination and the sandbox inventory
were not changed. A process already running, and a run ledger already
frozen, stay on the previous binary until that process is rebuilt.
