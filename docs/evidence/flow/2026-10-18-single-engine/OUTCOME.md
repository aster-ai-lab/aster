# One Flow engine

Core, Work, and Studio resolve the same `a3s-flow` 2.0.0 package under
`crates/flow`. The separate `flow-v2` workspace member and legacy engine are
removed. The old engine is retained outside the workspace for comparison.
The source provenance manifests and authoring UI aliases use the canonical
path. This is an uncommitted structural migration in the working tree.

Cancellation now projects as cancelled in Core's live observer, replay plan,
and graph. The recovery fixture records valid StepCreated/StepStarted facts
before completion. Published migration SQL bytes were not changed; checksum
tests use the canonical migration identifiers. The native TypeScript runner
passes the compiler's actual `--a3s-flow-runtime` argument.

## Validation

- `python3.13 tests/workspace_dependencies.py`: 6 passed, including one engine
  and identical Core/Work/Studio dependency paths.
- Both authoring alias tests and frontend builds passed.
- `cargo test -p a3s-flow --all-targets --features sqlite`: 556 passed,
  1 explicitly ignored real-Bun test.
- The ignored real-Bun test was then run explicitly with
  `A3S_FLOW_NATIVE_TS_COMPILER=target/debug/a3s-flow-native-compiler`:
  1 passed after repairing the runtime flag.
- Flow all-target/all-feature strict Clippy passed.
- Core Flow-related tests: 121 passed, 1 failed. The failing parallel research
  permission test also fails alone on the unchanged `b8521531` baseline with
  `tool_effect_unresolved`; its non-local fixture lacks effect attestation.
  The receipt gate was not weakened to hide the failure.
- Core cross-process workflow control/recovery: 2 passed.
- `cargo check -p aster -p a3s-code-acp -p aster-dream -p a3s-studio
  --all-targets`: passed, with existing warnings.

## Explicitly not claimed

No live PostgreSQL qualification, release, push, complete workspace test pass,
or resolution of the baseline remote effect-attestation failure. No unrelated
Studio import or concurrent working-tree changes are included by this record.
