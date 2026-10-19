# Program failure receipts and VM stop ownership

This slice fixes production error recording and a reproduced embedded-VM stop defect. It does not grade or repair a model submission. The four existing paid attempts, their reports, frozen runners, manifests, and original fact logs were not rewritten. No model or utility VM was started for this slice.

## Confirmed causes

The terminal `t023-2m-hc-01` retained all 32,008 events, but its eight failed `program` results lacked `metadata.program`. Twenty successful results retained 33 child receipts. Six failures were source/entrypoint refusals before worker creation; two were VM script errors after real effects. The native output evidence reported no projection loss. Therefore the missing records were a producer defect, and those two effectful failures cannot safely be filled with zero child calls.

`run_quickjs_script` originally attached its child records only to its success branch. Error, cancellation, deadline, and join-error returns discarded those records. The fixed producer retains observed native records on every worker return. Pre-worker refusals publish an authoritative empty array from control flow. `tool_calls_complete` requires a settled VM and one retained result per actual dispatch. Child/VM panic or other missing terminal records cannot certify zero. The test-only metrics consumer retains partial observations but refuses exact totals when that flag is false; old successful metadata without the new field remains compatible.

The original read-then-tight-loop fixture also reproduced a separate stop defect. A frozen pre-fix test binary exceeded its bound after entering the test; a one-second native sample showed both the outer waiter and the worker's nested Tokio runtime parked. The interrupt had a source-supported path to fire without rejecting the root Promise: pinned QuickJS treats the interrupt as uncatchable, and rquickjs 0.11's context driver discards the pending-job exception. Waiting only on that Promise could then last forever. Aborting a running `spawn_blocking` handle does not stop its worker, so the old grace/abort/await sequence supplied no hard return bound.

The first VM-side Promise/token/deadline race removed that wait, but `AsyncRuntime::idle()` exposed an actual GC assertion in the multithreaded cancellation case. Pinned `idle()` constructs an owning `AsyncContext::from_raw` from the borrowed context returned by `JS_ExecutePendingJob`; dropping that wrapper decrements an unowned reference. The failed candidate and its SIGABRT are retained. No dependency or registry source was changed.

## Final ownership behavior

The final VM uses its existing context driver for both the root Promise race and native-call settlement. A native pending admission is counted synchronously when the host closure creates its future, including calls queued before the scheduler's first poll. A local RAII guard releases admission after the future's actual outer join and receipt recording, or when promise allocation fails before the future can start. Every root terminal path drains these admitted futures before context destruction; this includes normal return and JS throw with unawaited queued or already dispatched calls.

The same child cancellation token drives the interrupt, native Promise race, and host invocations. Dropping the outer program future requests defensive cancellation. The normal governed return path still awaits the actual blocking worker. The grace interval is an observation checkpoint, not permission to detach; a worker that settles after grace retains its actual join result and complete receipts.

The source validator, script/tool/output/memory limits, allow-list checks, actual invoker selection, permission and budget gateways, cancellation forwarding, parent error text and failure outcome, and successful `script_result` layout are preserved. No error string is used to infer a zero child count.

## Verification

- RED: four receipt assertions failed against the original producer; the original tight-loop fixture did not converge and only its owned test process was terminated. `red.json`, `red.log`, and the two `tightloop-red-child*.json` records preserve this history. The first short stack mainly sampled loader startup and is explicitly inconclusive; `tightloop-red-stack-02.txt` sampled the actual parked VM.
- RED owning test: all six single/multithreaded deadline, caller-cancel, and caller-future-drop cases exceeded the child bound. Each owned child was killed and reaped by the parent test. See `red-worker-bound.*`.
- Failed candidates remain in `green-worker-bound.*`, despite the original intended filenames: one compile ownership error, one real QuickJS GC SIGABRT, and one missing-session-authority fixture failure. Their nonzero exits are retained; they are not qualification passes.
- Final program suite: **39 passed** in 1.38 seconds, including all original six bounded worker cases and four queued/already-dispatched unawaited write return/throw cases. Each child exits after Tokio runtime destruction and is reaped. A cancellation-aware child deliberately settling beyond 500 ms also proves the return waits and final receipt completeness stays true. See `green-program-final.*`.
- Original typed fact receipts: **2 passed**, preserving `Failed`, exit 1, `is_error`, real nested read, successful write bytes, and failed-check metadata. See `green-facts-final.*`.
- Existing nested governance: **8 passed**, including permission, shared budget, and cancellation behavior. See `green-nested-governance-final.*`.
- Metrics: **8 passed**, including partial/empty incomplete receipts and legacy successful metadata compatibility. See `green-metrics-final.*`. The initial command missing the required `advanced-harness` feature was rejected by Cargo and remains in `green-metrics.*`.
- All six owned Rust files pass the focused rustfmt check. Root `cargo fmt --all -- --check` exits 1 with differences in 62 other files and zero owned-file conflicts. No unrelated formatting was applied. See `format-own-files.*`, `format-workspace.*`.

## Scope and remaining limits

Only `program_tool.rs` changed production behavior. New sibling tests and one fact-test module registration cover the behavior; `tests/extensibility/metrics.rs` changes only test measurement. Exact file hashes and line counts are in `freeze.json`. Its test binary is a hermetic library-test executable, not a newly frozen paid matrix runner.

Caller audit found only `ProgramTool::execute` and three direct `deep_e2e_live` success/timeout/replay calls to `execute_script_program`. The existing standalone, registry-bound weak, and session-scoped invokers preserve the resulting metadata. Schema, permission, or budget refusals outside the `ProgramTool` body can still lack program metadata; measurement remains missing/null for those cases rather than guessing zero.

The caller-future-drop cases prove defensive cancellation, eventual VM callback/context release, successful child-runtime destruction, and direct child reaping. They do not qualify every public caller-drop ownership contract. A native tool that ignores cancellation and never settles still cannot be safely killed as a Rust thread; this implementation retains ownership and must not be described as a universal 500 ms hard stop. Linux and Windows were not exercised. No end-to-end T01 speed or model quality improvement is claimed from these hermetic tests.
