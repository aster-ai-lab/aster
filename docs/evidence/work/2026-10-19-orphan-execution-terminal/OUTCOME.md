# Lost runtimes settle the original execution as failed

## Result

Recovery previously marked the assistant message and workbench proof as failed
without updating the execution summary. Startup also downgraded persisted live
summaries to `Unknown` in memory while leaving their former live status on disk.
The normal execution writer deliberately refuses changes to an unknown or
already terminal summary. A recovered error could therefore retain a persisted
`awaiting-approval` execution indefinitely.

Work and Desktop now call a dedicated store recovery operation while holding the
existing guard that proves no live turn owns the session. It changes an existing
live or unknown summary to `Error`, preserving its execution ID and assistant
anchor. It creates no summary, preserves existing terminal outcomes, and does not
infer success from transcript text or files. Startup's temporary unknown status
and the normal writer's monotonic terminal rule are unchanged.

The recovery also handles a message already settled by an earlier recovery. A
failed execution-summary write remains unauthoritative and can be retried; it
does not publish an uncommitted terminal as reliable state.

## Validation

Actual red run before production edits:

```text
cargo test -p aster-dream --lib orphan_recovery_fails_the_original_execution_after_restart -- --nocapture
test result: FAILED. 0 passed; 1 failed
assertion: left Unknown, right Error
```

Actual green runs after production edits:

```text
cargo test -p aster-dream --lib execution_status_tests -- --nocapture
test result: ok. 4 passed; 0 failed

cargo test -p a3s-desktop --lib execution_status_tests -- --nocapture
test result: ok. 4 passed; 0 failed

target/debug/deps/a3s_desktop_lib-104c4ea3ed9a9da3 scheduled_recovery_ --nocapture
test result: ok. 8 passed; 0 failed; 1 ignored

target/debug/deps/a3s_desktop_lib-21728e7535d45581 scheduled_recovery_ --nocapture
test result: ok. 8 passed; 0 failed; 1 ignored
```

Each host has the same three new hermetic tests. They cover recovery after a
disk reopen from all four live phases, including an already errored transcript;
preservation of the original execution and anchor; repeat recovery retaining
the first failure timestamp; protection of a current live owner, a completed
execution, and another session; and a failed summary write followed by successful
retry. Both hosts also pass the existing owner-publication test and eight
scheduled-recovery cases. The ignored test requires a signed-in live gateway and
was not enabled. Both owning Cargo test binaries compiled successfully, with
existing unused-code warnings.

Scoped default rustfmt checks passed for both execution-status test files.
Scoped rustfmt checks using `skip_children=true,reorder_imports=false` passed for
the production files, preserving their existing import order. Scoped
`git diff --check` passed. The two store files and two test files are respectively
byte-identical; the recovery call is the same bounded hunk in both hosts.

## Explicitly not claimed

- No real session state, task artifact, or installed application was modified.
- No live UI, model, credential, or runtime-configuration request was made.
- No successful task completion or generated artifact is inferred from this
  failure terminal.
- No release build, deployment, task-corpus pass, or workspace-wide pass is
  claimed by this slice.
- Recovery runs when the host confirms the session has no live turn; this is
  not a blanket conversion of every historical unknown summary at store open.
