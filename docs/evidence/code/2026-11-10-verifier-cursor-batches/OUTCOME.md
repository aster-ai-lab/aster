# Foreground verification and Cursor tool batches

## Scope

Tested the task changes in the working tree based on `b1be95c5b` on macOS
arm64, Rust `1.97.1`. Existing Studio driver edits, Terminal-Bench manual edits,
and untracked assets were preserved. No commit or push was requested.

The original verification children could not run even a shell probe because
the parent held dirty-file claims. A foreground built-in verifier now borrows
those claims for its lifetime and returns them on completion or cancellation.
Its session and receipts remain independent. The parent and unrelated sessions
cannot overwrite the loaned files. Permissions and sandbox checks still apply.

A second stall was reproduced against the real Cursor account: a batch listed
two requests, one native tool and one already-answered MCP catalog query. The
adapter counted only the native call and waited until the generation deadline.
Catalog replies now count toward batch completion. Completed catalog-only steps
reset their count. Missing native calls still refuse completion.

Known-tool arguments with the wrong JSON shape now enter the existing parse
repair guard without executing the tool. Unknown tools, foreign providers,
foreign servers, and approval-only requests remain refused, with bounded reason
codes instead of one ambiguous error. Diagnostics contain counts and reason
codes, not argument values or credentials.

Integration validation also found two tests reading the obsolete result
projection instead of the canonical receipt. Both now inspect
`payload.receipt.output`, retaining their command, permission, and outcome
assertions. The native sandbox no longer creates workspace `.aster/loops` when
the existing process state override places harness state outside the workspace.
Existing workspace control directories remain write-denied.

## Commands and results

All Cargo commands used the repository's `target` directory and
`CARGO_INCREMENTAL=0`, with one Cargo process at a time.

```sh
export CARGO_TARGET_DIR="$PWD/target" CARGO_INCREMENTAL=0
cargo clippy --locked -p aster-core -p aster-acp --lib --bin aster-acp \
  --test cursor_native_tools_live --test unattended_e2e --no-deps
cargo build --locked -p aster -p aster-acp
cargo test --locked -p aster-core --lib -- \
  external_observation:: tools::task::tests llm::cursor_account:: verification:: \
  harness_loop::receipt_tests harness_loop::unchanged_receipt_tests \
  sandbox::native::tests \
  tools::builtin::bash::tests::bash_does_not_hide_another_sessions_dirty_path \
  tools::builtin::bash::tests::detach_refuses_when_workspace_has_foreign_dirty_path \
  --test-threads=4
cargo test --locked -p aster-acp --test unattended_e2e -- --test-threads=4
cargo test --locked -p aster-acp --test cursor_native_tools_live auto_ -- \
  --ignored --test-threads=1 --nocapture
```

| Check | Actual result |
| --- | --- |
| Focused Clippy | Exit 0; existing warnings remain |
| CLI and ACP build | Exit 0 |
| Core regression | 313 passed, 1 existing opt-in test ignored, 0 failed |
| ACP unattended stdio integration | 33 passed, 0 failed |
| Original three ACP failures, rerun individually | Each passed before the final full suite |
| Real Cursor Auto copy and delegated verification | 2 passed, 0 failed; Composer case not selected |
| `python3.13 tests/workspace_dependencies.py` | 7 passed |
| Driver interactive/unattended argv and outcome assertions | Passed |
| Changed Rust files, `rustfmt --check` | Passed |
| `git diff --check` | Passed |

The mixed-batch regression covers all six orders of catalog, native-call, and
batch-list frames, following a completed catalog-only step. An additional case
retains rejection when a native call is missing. Other coverage checks loan
restoration on cancellation, parent and unrelated-session exclusion, recovery
from malformed arguments, and native sandbox control-directory protection.

## Real unattended task

The current `target/debug/aster-acp` was driven through the existing
`apps/studio/scripts/acp_session_driver.py`, using a private ACL containing only
the Cursor account import and model selection. The fixture and result lived at
`/tmp/aster-verifier-e2e`, outside source and user projects. Debug stderr was
redirected by a launcher so the driver's unread stderr pipe could not fill.
The launcher only used `exec` to run the current ACP binary with unchanged argv.

The prompt, `cursor/auto` model, 420000 ms execution budget, default engineering
policy, and default goal continuation stayed fixed across diagnosis and the
final run. The task copied the exact input bytes, delegated `python3 check.py`
to a synchronous verifier, and read the output after verification.

Final result:

```json
{
  "outcome": "succeeded",
  "reason": "agent_end",
  "terminalEvent": true,
  "exitCode": 0,
  "durationMs": 92179,
  "turns": 9,
  "toolCalls": 10,
  "successfulToolCalls": 10,
  "artifactEvidenceCount": 1,
  "errorCount": 0
}
```

Host checks independently compared the input/output bytes and the original
input and checker SHA-256 values. A successful verification-child receipt was
correlated with that exact child session's successful `python3 check.py` shell
receipt. No workspace `.aster` directory was created. Existing ksandbox
`.kbox` fixture metadata was retained. The earlier timed-out and partial runs
were preserved separately; neither was counted as a successful result.

## Explicitly not claimed

- The original screenshot's invalid request was not retained, so its exact
  rejected field and original long conversation were not replayed.
- Background or custom-role claim sharing was not added; the loan applies to
  synchronous built-in verification, without granting additional permissions.
- No Composer account run, Linux/Windows native campaign, Studio GUI campaign,
  release build, distribution, or performance benchmark was performed.
- Workspace-wide formatting was attempted and fails on existing untouched
  files, including shipped TUI fixtures and tool-acquire sources. Changed Rust
  files pass formatting; unrelated formatting was preserved.
- All-target Clippy was attempted and is blocked by the unchanged
  `test_persisted_schema_roundtrip.rs:211` fixture passing a `String` where
  `Arc<str>` is required. Focused Clippy passed. No full-workspace green claim
  follows from the focused checks.
