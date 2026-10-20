# Core zero-count boundary slice

This slice makes `max_tool_rounds=0` unbounded at the capability ceiling,
delegated-child inheritance and legacy loop boundaries. The public `usize` API
and SDK default remain unchanged. Positive limits retain their existing behavior.

`CapabilityExecutionCeiling` accepts zero rounds while still rejecting zero
parallelism and zero optional timeouts. Ceiling comparison treats zero rounds as
the widest value: a bounded parent rejects an unbounded child, and an unbounded
parent can admit an equal or narrower positive child. The regression uses actual
session/run admission, rather than only a numeric comparison helper.

Child inheritance no longer changes parent zero to one. Existing role/default
override precedence is retained; explicit widening is rejected by the capability
boundary rather than silently clamped. The legacy loop excludes zero from budget
finalization and turn-limit errors, and permits its existing bounded continuation
and reasoning-only repair logic to operate. Duplicate finalization, cancellation,
provider timeouts, parallel limits and duration limits are not removed.

Nine owned source/test files are frozen in
[source-freeze.json](source-freeze.json); their initial identities are in
[source-before.json](source-before.json), and their reviewable patch is
[owned.patch](owned.patch). No root-owned configuration, actor, ACP, Effect,
manifest or `agent.rs` file was edited by this slice.

Scoped `rustfmt --check` passed (exit 0, 0.07715125 seconds), and the owned
`git diff --check` passed (exit 0, 0.018239709 seconds). These are static checks.
No Cargo or test command was run by this worker. The root owns compilation and
RED/GREEN execution; these source changes are not yet reported as passing tests.

The ten new hermetic cases have these exact filters:

```text
cargo test -p a3s-code-core --lib agent::tests::no_count_limit::
cargo test -p a3s-code-core --lib agent::execution_state::tests::zero_tool_rounds_preserves_repair_and_continuation_limits
cargo test -p a3s-code-core --lib child_run::tests::delegated_child_preserves_zero_round_inheritance_and_positive_overrides
cargo test -p a3s-code-core --test capability_scope zero_tool_round
```

The six legacy cases use existing controlled model/tool fixtures. They check 51
distinct tool executions followed by an answer, a retained explicit limit of
three, duplicate finalization, one reasoning-only repair, cancellation and a
provider timeout. They invoke no shell, real model, VM, GUI or network service.
Existing connected filters include `child_run::tests::`,
`agent::execution_state::tests::`, `agent::tests::test_agent_max_tool_rounds`, and
the complete `capability_scope` integration target. Execution results must be
recorded separately by the root.
