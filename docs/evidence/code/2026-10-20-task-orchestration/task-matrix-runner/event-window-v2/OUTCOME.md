# T023 bounded event-window correction

The first paid `t023-sa-01` attempt retained 16,384 of 45,684 Run events.
Its tool metrics described a retained suffix while `metrics_complete` remained
true. That sample and its original report, manifest and executable are retained;
this correction does not rewrite them or reinterpret the suffix as a full Run.

Only `crates/code/core/tests/extensibility/task_matrix.rs` changed:

- Raise the test runner's event-count cap to 262,144, keeping the serialized
  event-byte cap at 64 MiB and all model/tool/time/token budgets unchanged.
- Require the exact Run event window to be complete before tool metrics can
  qualify; the saved aggregate also explicitly combines its completeness flag
  with `event_window_complete`.
- Add a hermetic regression with a complete retained tool interval and a missing
  earlier model event. It fails before the guard and passes after it.

`red.json` / `red.log`: the new regression failed on the previous predicate.
`green.json` / `green.log`: 10 passed, 1 live fixture ignored; no model or VM ran.
The focused tests took 0.26 seconds; compilation and orchestration are recorded
separately. Single-file `rustfmt --check` passed. An independent read-only review
confirmed the cap, byte bound, per-Run count comparison and aggregate guard.

`build.json` records the root `--locked --offline --no-run` command, Cargo's
actual artifact features, source digest and copied executable digest. The new
executable is `target/t023-frozen-event-window-v2/t023-real-llm` with SHA-256
`33de605e83617a2e850778a2308832610697b1bcc95c0ffd227ec0ad1d9f130a`.
The source SHA-256 is
`996b60461d683374646d43a3765928576886b1578eb664cfa9f869284fa5c893`.
The source remained unchanged during tests/build. Before/after hashes confirm
the old frozen executable, first sample manifest and `run.json` were unchanged.

This is measurement qualification, not a new task result or a speed claim.
The first paid attempt still counts against the bounded experiment. Future
samples require a new versioned source/executable/manifest freeze. The 1,000,000
reported-token soft cap remains unchanged; if the count or byte bound truncates
another event window, its metrics must remain incomplete.
