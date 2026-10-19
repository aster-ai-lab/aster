# Prepared facts keep a status digest, not the dirty list

Date: 2026-10-20. Local source evidence. Not a published release.

Every mutating call stored the whole git snapshot on `ToolPreparedV1.baseline`.
On this worktree that JSON was 2,159,875 bytes. `bound_json` rejects a
prepared fact over 1,048,576 bytes, so the turn failed before the command
started with `tool effect evidence exceeds byte bound`.

The prepared fact now stores HEAD, a status digest, a stamp digest, the
counts, and the existing read flags. The watch still keeps the status lines
in memory and still diffs them. A missed status stays `status_complete:
false` and `stamp_read_failed: true`. A list past the 4096 stamp cap stays a
complete status read.

## Verification

Rust toolchain: workspace pin `1.97.1`. Command from the repository root.

| Check | Result |
| --- | --- |
| `cargo test --locked --offline -p a3s-code-core --lib -- porcelain:: fact_control::receipts::tests::pre_spawn::` | 52 passed, 0 failed, 0 ignored, 5151 filtered out. Exit 0. Tests 2.27s after a 34.48s compile |

New or extended cases: `a_large_status_list_is_a_digest_in_the_prepared_baseline`
(14,840 lines plus 4096 stamps, baseline under 4 KiB, one changed line changes
the digest, a missed status has a null digest),
`a_status_list_past_the_stamp_cap_is_a_complete_empty_delta`, and
`a_write_past_the_stamp_cap_does_not_freeze_the_ledger`.

## Explicitly not claimed

No claim that a running `just code` process has this binary. ACP was not
rebuilt and the TUI was not launched. The full status bytes are not written
to a side blob; nothing in this crate reads `git.porcelain` back out of a
prepared fact. Content-only edits of already-dirty paths past the 4096 stamp
cap are still absent from the command delta. `apps/cloud` has its own
porcelain copy and was not edited. Search pagination and the sandbox
inventory mismatch were not changed.
