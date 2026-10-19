# Stamp-budget overflow no longer freezes later mutating calls

Date: 2026-10-20. Local source evidence. Not a published release.

A git status list longer than the 4096 stamp cap was treated as a missed
status. The first mutating call, including a shell that never started, booked
`unresolved_tool_effects`. Every later mutating call was denied with
`tool_effect_unresolved: mutating calls require reconciliation of earlier uncertain invocations`.

This worktree's `git status --porcelain -z -u` is 1,016,097 bytes and 14,742
records, and finishes in about 0.3s. That list fits the 4MB git output cap and
the new 65,536 status-record cap. It does not fit the 4096 stamp cap. The
status diff stays a complete observation. Only a failed status read, a failed
hidden-index query, a corrupt hidden-index record, or a list past 65,536
records remains a missed read.

A tool result whose metadata says `command_started: false` records an empty
complete effect. That call created no process, so an incomplete watch of a
tree that was already dirty does not freeze the next call. A command that did
start, and whose status could not be read, still stays unresolved.

## Verification

Rust toolchain: workspace pin `1.97.1` (`rustc 1.97.1 (8bab26f4f 2026-07-14)`).
Command from the repository root.

| Check | Result |
| --- | --- |
| `cargo test --locked --offline -p a3s-code-core --lib -- porcelain:: execution_state::tests::a_git_tree_past_the_stamp_cap execution_state::tests::baseline_ fact_control::receipts::tests::pre_spawn::` | 58 passed, 0 failed, 0 ignored, 5143 filtered out. Exit 0. Tests 3.25s after a 44.27s compile. The remaining warning is the pre-existing `__eh_frame` linker note |

The new cases are `a_status_list_past_the_stamp_cap_is_a_complete_empty_delta`,
`a_git_tree_past_the_stamp_cap_stays_a_complete_status_read`,
`a_write_past_the_stamp_cap_does_not_freeze_the_ledger`, and
`a_shell_that_never_starts_on_a_missed_status_does_not_block_a_later_tool`.
`git_parsers_bound_paths_and_do_not_accept_overflow_as_complete` now treats
4097 status records as a complete read and overflow at 65,536.

## Explicitly not claimed

No claim that a running `just code` session has this binary. ACP was not
rebuilt and the TUI was not launched. A run whose ledger already contains the
unresolved key stays frozen until a new run. Search `limit`/`cursor` rejection,
the sandbox inventory mismatch, and the completion-gate sentence were not
changed. Content-only edits of already-dirty paths past the 4096 stamp cap are
still absent from the command delta. `apps/cloud` has its own porcelain copy
and was not edited. The frozen sandbox cache
`~/Library/Caches/aster/sandbox/dev-20261018-core-bash` was not modified.
