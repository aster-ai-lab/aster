# Read-only runner errors no longer freeze the turn

Date: 2026-10-20. Local source evidence. Not a published release.

A read-only tool call that returned a runner error, with no result envelope and no child process, was recorded as `ToolExecutionOutcome::Unknown`. That booked one identity-bound unresolved obligation. The completion gate then refused to close the turn, and the terminal sentence appended `Try sending again.`

A read-only runner error now settles as a failed call with a complete empty effect. The turn can close when that call is the only failure and the other calls are ordinary successful reads. A batch whose members are all read-only, and a program whose allowed tools are all read-only, are read-only: they do not start a workspace-mutation watch, and a runner error on that call does not freeze a later read. A call that may have written the workspace, and whose result is unknown, still keeps exactly that call's identity. The gate still refuses. A later read still runs. A later mutating call stays denied until that identity is reconciled. The pager sentence for this refusal is the kernel text. A typed `tool_effect_unresolved` kind does not replace it, and a trailing `Try sending again` already wrapped around it is removed. The ACP error display uses that same kernel sentence.

`receipts.rs` and `receipts/tests.rs` also contain an earlier background-observation-gap edit. This record does not claim that edit.

## Verification

Rust toolchain: workspace pin `1.97.1` (`rustc 1.97.1 (8bab26f4f 2026-07-14)`).
Commands from the repository root. `SCRATCH` was unset in this shell. Logs were written to `/var/folders/3d/0pn3wn612t7cs79b5ml89x440000gn/T/grok-goal-64590bc105aa/implementer/`.

| Check | Result |
| --- | --- |
| `cargo test --locked --offline -p a3s-code-core --lib fact_control::receipts::tests::readonly_effect::read_only_execution_error_settles_failed_and_the_turn_can_close` | 1 passed, 0 failed, 5252 filtered out, 0.02s. Exit 0. `read-only-receipt.log` |
| `cargo test --locked --offline -p a3s-code-core --lib fact_control::receipts::tests::readonly_effect::all_read_batch_and_program_do_not_watch_or_freeze_a_later_read` | 1 passed, 0 failed, 5253 filtered out, 0.06s. Exit 0. The same test also runs the all-read program runner error. `batch-readonly.log` |
| `cargo test --locked --offline -p a3s-code-core --lib fact_control::receipts::tests::readonly_effect::unknown_mutating_call_keeps_one_identity_and_blocks_later_writes` | 1 passed, 0 failed, 5253 filtered out, 0.13s. Exit 0. `mutating-unknown.log` |
| `cargo test --locked --offline -p a3s-code-pager --lib keeps_the_kernel_sentence` | 3 passed, 0 failed, 10195 filtered out, 0.00s. Exit 0. Covers the untyped kernel sentence, the typed kind, the wrapped retry, and `format_acp_error`. `pager-sentence.log` |
| `cargo test --locked --offline -p a3s-code-core --lib fact_control::receipts::` | 66 passed, 0 failed, 5188 filtered out, 8.32s. Exit 0 |
| `cargo test --locked --offline -p a3s-code-pager --lib app::error_display::tests` | 26 passed, 0 failed, 10172 filtered out, 0.00s. Exit 0 |
| `cargo test --locked --offline -p a3s-code-pager --lib reconciliation_wire_kind` | 2 passed, 0 failed, 10196 filtered out, 0.00s. Exit 0. These three suite lines are in `focused-suite.log` |

`rustfmt --edition 2021 --check` is clean on `harness_loop.rs`, `receipts.rs`, `receipts/tests.rs`, `readonly_effect.rs`, `batch.rs`, and `program_tool.rs`. `rustfmt --edition 2024 --check` is clean on `error_display.rs` and `effects/tests.rs`. Package-wide `cargo fmt` was not run.

## Explicitly not claimed

No live Aster TUI turn and no rebuilt `just code` / `aster` binary. A process that is already running still has the old receipt classifier. An unresolved key already stored in a session ledger is not rewritten. `apps/cloud` has its own gate and receipt copy and was not edited. The 4096 stamp cap and the 65536 git-status record cap were not changed. The refusal banner does not print the unresolved call's tool name. `./tests/T01` was not analyzed. A read-only runner `Err` has no `command_started` flag, so that path settles as a failed empty effect even if a process had started before the error. A mutating command that started still stays Unknown. The pre-existing background-observation-gap hunks in `receipts.rs` and `receipts/tests.rs` are outside this fix.
