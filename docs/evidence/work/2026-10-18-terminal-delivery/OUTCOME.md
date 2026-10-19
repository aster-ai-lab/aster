# Terminal delivery and failure classification

## Result

Implemented bounded fixes for missing final text and false clean completion.
After the shared workspace dependency changes settled, the owning Core and ACP
unit-test binaries compiled. All 6 model-governance tests and all 3 ACP terminal
tests passed, including the six new regressions below.

## Changes

- A final non-streaming model response resets the preceding response's text
  delivery flag, so settled delivery can publish its answer.
- An empty provider text delta does not count as delivered final content.
- Direct and detached provider reads accept the explicit `Done` terminal
  without waiting for a provider to drop its channel. The governed original
  model path already followed this protocol.
- ACP requires an explicit terminal event before reporting `EndTurn`. A worker
  that returns cleanly after only partial events produces a named error.
  Explicit tool failures and cancellation retain their existing outcomes.
- Cancellation remains observable after the event channel closes while the
  worker is still joining. The existing bounded cancellation path keeps the
  turn permit with that worker until it settles.

The delivery flag does not authorize effects. Mutation receipts, scope cleanup,
verification, the completion gate, and `turn.complete` fact admission remain
unchanged.

## Validation

- `git diff --check -- crates/code/core/src/fact_control/model_call.rs crates/code/core/src/fact_control/model_governance_tests.rs crates/code/acp/src/agent/turn.rs crates/code/acp/src/agent/turn_terminal_tests.rs crates/code/acp/src/agent.rs`: passed.
- `rustfmt --edition 2024 --config skip_children=true --check crates/code/core/src/fact_control/model_call.rs crates/code/acp/src/agent/turn.rs crates/code/acp/src/agent/turn_terminal_tests.rs`: passed.
- `cargo test -p a3s-code-core --lib fact_control::model_governance_tests -- --nocapture`: dependency resolution failed before compilation.
- `cargo test -p a3s-code-acp --lib turn_terminal_tests -- --nocapture`: dependency resolution failed before compilation.

The initial dependency blocker was subsequently removed by concurrent workspace
work. Successful owning Cargo invocations built these exact unit binaries:

- `cargo test -p a3s-code-core --lib output_check_is_a_closed_read_only_tool_in_interactive_modes -- --nocapture`: 1 passed, 5078 filtered out.
- `cargo test -p a3s-code-acp --lib source_review_has_no_tools_even_after_a_yolo_mode_change -- --nocapture`: 1 passed, 298 filtered out.
- `target/debug/deps/a3s_code_core-b2c1f63d266e14c5 fact_control::model_governance_tests --nocapture`: 6 passed, 5073 filtered out.
- `target/debug/deps/a3s_code_acp-7fdaa69d7ef354e3 turn_terminal_tests --nocapture`: 3 passed, 296 filtered out.

Both binaries emitted the existing macOS linker warning about the large
`__eh_frame` section; there were no test failures.

A later queued rerun of
`cargo test -p a3s-code-acp --lib turn_terminal_tests -- --nocapture` reached a
different concurrent source state and failed before tests: unresolved import
`crate::config::A3S_INFERENCE_PROVIDER` at
`crates/code/core/src/agent_protocol_harness.rs:17`. The successful direct-binary
runs above are retained; they do not claim that later source state builds.

Both initial Cargo commands had reported:

```text
error: failed to select a version for the requirement `a3s-box-core = "^3.2"`
candidate versions found which didn't match: 0.1.0
location searched: /Users/roylin/code/aster/crates/box/core
required by package `a3s-box-runtime v0.1.0 (/Users/roylin/code/aster/crates/box/runtime)`
```

Six new hermetic test cases cover fallback delivery, empty deltas, held-open
provider channels after `Done`, missing worker terminals, and explicit success,
error, and cancellation, including cancellation during a worker's tail after
event EOF. All six compiled and passed in the owning unit binaries.

## Explicitly not claimed

- No task corpus case was rerun, and no completion rate improvement is claimed.
- A missing `turn.complete` marker after an interrupted or unverified tool run
  is intentionally preserved; it cannot be repaired by manufacturing success.
- The native process lifetime restriction and artifact generation are outside
  this change.
- No release build, desktop rollout, live model run, or workspace-wide test pass
  was performed by this slice.
