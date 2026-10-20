# Child continuation and cancellation regression correction

Root's Core GREEN-v2 execution passed eleven of twelve selected tests. The new
legacy cancellation case failed because it unwrapped an actual existing API
error, `Operation cancelled by user`, as a successful interrupted result.
Cancellation itself worked. This increment corrects only that test expectation,
asserts cancellation, then releases the agent/client and verifies the provider's
weak reference no longer upgrades. The temporary workspace is explicitly closed.
No legacy cancellation production behavior changed.

Root also identified a missed zero-count caller in the fact child-run continuation
loop. Its model-turn comparison now applies only to a nonzero round limit.
The independent continuation counter and repeated-incomplete-response check stay
in place. A new actual `ChildRun` test uses existing controlled model responses
and four scenarios: zero rounds can continue to a complete reply, zero allowed
continuations still prevents continuation, a positive one-round limit still
prevents it, and repeated incomplete content under zero rounds still stops with
the no-progress diagnostic. No model service, shell, GUI or VM is used.

Only the previously owned legacy test sibling and the newly authorized
`agent_api/conversation_runtime/child_run.rs` changed. Earlier freezes remain
intact. [source-before.json](source-before.json), bounded source before-images,
[owned-increment.patch](owned-increment.patch) and
[source-freeze.json](source-freeze.json) record this increment separately.
Scoped formatting and diff checks passed (exit 0, 0.027579584 and 0.01243375
seconds respectively). This worker did not execute Cargo or tests; root must
record the corrected GREEN result separately.

```text
agent::tests::no_count_limit::zero_tool_rounds_retains_cancellation
agent_api::conversation_runtime::child_run::tests::zero_tool_rounds_child_continues_without_removing_independent_guards
```
