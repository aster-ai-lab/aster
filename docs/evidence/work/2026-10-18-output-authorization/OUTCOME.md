# Output-check review and artifact authorization

## Result

Independent review found the new measurement tool's top-level `oneOf` schema
was a provider compatibility risk because the local provider adapters forward
tool schemas unchanged. The implementation now uses an object schema and keeps
action-specific validation in the tagged input type.

The `office_file` name also needed to join existing mutation classifications.
This slice adds it to the fact-path side-effect list, read-only verifier deny
list, specialty permission deny lists, and ACP research fallback deny list.
`output_check` is explicitly Routine/ReadOnly in interactive assessment. An
explicit policy denial still wins. Source-review sessions do not register
either new tool, including after a mode change.

## Validation

The current owning Cargo builds produced Core unit binary
`target/debug/deps/a3s_code_core-b2c1f63d266e14c5` and ACP unit binary
`target/debug/deps/a3s_code_acp-7fdaa69d7ef354e3`.

- `cargo test -p a3s-code-core --lib output_check_is_a_closed_read_only_tool_in_interactive_modes -- --nocapture`: 1 passed.
- `cargo test -p a3s-code-acp --lib source_review_has_no_tools_even_after_a_yolo_mode_change -- --nocapture`: 1 passed.
- Core binary, filter `read_only_specialties_deny_artifact_publication`: 1 passed.
- Core binary, filter `read_only_verifier::tests::presented_tools_exclude_mutations`: 1 passed.
- Core binary, filter `fact_log_plan_guardrail_denies_a_write_the_default_policy_would_ask`: 1 passed.
- ACP binary, filter `host_context::tests::research_denies_writes_and_keeps_caller_denies`: 1 passed.
- ACP binary, filter `output_check::`: 4 passed.
- Scoped `git diff --check` and `rustfmt --edition 2024 --config skip_children=true --check` passed for the touched production files.

These 10 focused tests include measurement bounds, null/zero-baseline handling,
ASCII percentage spelling, Unicode text counts, source-review registration,
explicit denial, and mutation classification. The independent `rustc --test`
metrics run also passed 3 tests; those duplicate the metrics counted above.

A later queued ACP Cargo rerun failed to recompile Core after concurrent source
changes: `crate::config::A3S_INFERENCE_PROVIDER` was unresolved at
`crates/code/core/src/agent_protocol_harness.rs:17`. No build pass for that later
source state is claimed.

## Explicitly not claimed

- No live provider call was made to test schema acceptance.
- No task corpus replay, release build, or workspace-wide test pass is claimed.
- Percentage arithmetic is finite `f64` arithmetic. Overflow is refused; this
  tool does not claim arbitrary-precision decimal arithmetic.
- The artifact implementation and its other path/mutation classifications are
  recorded by the artifact-delivery slice, not verified by this review alone.
