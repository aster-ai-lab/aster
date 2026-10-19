# Rejected tool arguments settle the announced tool row

## Result

The fact runner's argument-schema refusal returned a failed receipt without
emitting the terminal tool event normally supplied by the executor. An already
announced or approved call could consequently remain pending in the desktop
stream even though the model had received the failure and moved to another call.

The shared preparation branch now emits one `ToolEnd` for that rejection, with
the same call ID, name, effective arguments, and failure text. Its nonzero status
and typed `InvalidArgument` distinguish refusal from successful execution. No
execution-start event is emitted. The strict schema, permission-denial branch,
failed receipt, and effect observations retain their existing behavior.

## Validation

The existing hermetic invalid-input test now captures events and requires exactly
one matching terminal event. It also verifies that both hook and tool execution
counts remain zero, no files are written, and the complete effect receipt has no
changes or unresolved operations.

Before the production change:

```text
cargo test -p a3s-code-core --lib invalid_original_arguments_fail_before_hook_or_tool_execution -- --nocapture
test result: FAILED. 0 passed; 1 failed
assertion: a rejected schema must settle the same tool row
```

After the production change:

```text
cargo test -p a3s-code-core --lib fact_control::receipts::tests::argument_validation -- --nocapture
test result: ok. 4 passed; 0 failed
```

The four tests cover invalid original arguments, permission denial taking
precedence over invalid arguments, a hook rewriting valid input into invalid
input while retaining effect uncertainty, and valid input executing once.
Both Cargo invocations built and ran the owning Core test binary. The linker
reported its existing oversized `__eh_frame` warning.

Scoped `git diff --check` passed. Scoped rustfmt passed with
`skip_children=true,reorder_imports=false`, preserving the production file's
pre-existing import order.

Read-only desktop evidence also identified two rejected approval retries using
an already consumed earlier approval ID. The later corrected call had not been
approved. This is consistent with the missing terminal leaving an earlier
permission request in the replayed stream; neither retry is counted as an
accepted decision. No session transcript, source document, or runtime credentials
were copied into this evidence.

## Explicitly not claimed

- No live session was modified or resumed by this slice.
- No frontend or approval-route implementation was changed.
- No release build, desktop rollout, task-corpus pass, or workspace-wide test
  pass is claimed.
- This failure event does not authorize a tool, relax its schema, or manufacture
  successful completion after a rejected invocation.
