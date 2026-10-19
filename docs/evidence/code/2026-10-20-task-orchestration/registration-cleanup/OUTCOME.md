# ACP native registration cleanup

The ACP session now closes and settles before returning any failure from
Office initialization, Office Tool registration or Output Check registration.
Those operations share the existing plugin-registration error boundary.
The returned error remains the original registration error.

## Regression and checks

The new hermetic regression opens a real Core session without invoking a
model or VM. It publishes a session-owned capability effect whose asynchronous
`close` yields once and increments a counter. It then exercises three failures:

| Trigger | Closed before return | Cancellation requested | Resource settled |
| --- | --- | --- | --- |
| Workspace removed after session open | true | true | exactly once |
| Projected `office_file` name already owned | true | true | exactly once |
| Projected `output_check` name already owned | true | true | exactly once |

Each case retains an extra session `Arc` while capturing these observations.
The fixture subsequently calls `close` again and verifies that the effect is
not closed twice. These assertions therefore test explicit settlement rather
than cleanup caused by dropping the last session reference.

Before the production fix, the workspace-removal case failed with
`closed=false`, `cancelled=false`, `resources_settled=0`. The regression cleans
up its own session even on that failing implementation. Raw output remains in
`red.log` and the command/exit status in `red.json`.

After the fix, all three cases pass. The final focused campaign passed **3
tests, 0 failed**:

- `agent::registration_cleanup_tests::native_registration_failure_closes_and_settles_the_open_session`
- `agent::session_setup_tests::source_review_has_no_tools_even_after_a_yolo_mode_change`
- `agent::document_read_agent_tests::opted_in_agent_registers_reader_on_open_and_replacement_only`

Each ran from the root with:

```sh
cargo test --locked --offline -p a3s-code-acp --lib <exact-test-name> -- --exact --nocapture --test-threads=1
```

Exact commands, exit statuses and log names are in `final-checks.json`. The
source-review case also checks ordinary sessions still register Office and
Output Check. The document-bridge case covers both initial open and replacement.
These execution times include Cargo contention and are not performance data.

The owned production function matches rustfmt output, the new sibling test
passes rustfmt, and focused `git diff --check` passes. `source-checks.json`
records those checks, source hashes and an independent read-only review by
`/root/runtime_speed`, which found no blocking issue. Existing build warnings
remain visible in the logs.

## Scope

- Production: only `finish_plugin_tools` in `agent/session_open.rs`.
- Test module registration: three lines in `agent.rs`.
- New sibling regression: `agent/registration_cleanup_tests.rs`.
- Evidence: this directory only.

## Explicitly not claimed

No model, VM, installed runtime, application selection or release was invoked
or changed. This check does not qualify Windows/Linux execution, native BGE
settlement, caller cancellation during plugin setup, or unrelated session-open
failure paths. No Git commit was created.
