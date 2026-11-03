# Local main landing

Prepared on the existing local `main`, based on `260f292d4`. No branch switch,
additional worktree, remote push, or merge commit is needed.

The commit contains the Cursor native MCP/stream repair, its focused regressions,
the Auto/Composer acceptance fixtures, and both retained evidence directories.
The staged files were inspected before committing.

Shared files are staged selectively. The pre-existing Gemini-state refusal in
`cursor_account.rs` remains uncommitted because its separate Gemini module is
absent from HEAD. The TUI file contributes only the Auto/Composer live test and
its shared turn helper; concurrent workspace-guard and binary-resolution changes
remain uncommitted. The working files themselves were not reverted or replaced.

The selected TUI snapshot is reconstructed from HEAD plus those test/helper
hunks; it is not claimed byte-identical to the historical tested snapshot.
The recorded live release acceptance was performed in the existing dirty
checkout, not in a clean build of this prospective commit. Earlier Core
regressions, lint and installed-artifact acceptance remain the recorded results.
The Rust integration-test rebuild and ACP lint limits in `OUTCOME.md` remain.

Landing checks: selected Rust snapshots pass pinned `rustfmt --check`; staged
source/document whitespace and file-scope checks pass. Full staged whitespace
inspection reports trailing spaces or blank EOF lines only in original captured
logs. Those evidence bytes are retained unchanged under the artifact-fidelity
contract. Unrelated workspace and submodule changes are excluded and preserved.
