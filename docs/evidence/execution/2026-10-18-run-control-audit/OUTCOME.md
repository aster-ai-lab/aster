# Run-control lifecycle audit

Date: 2026-10-18

## Result

The ACP event dispatcher now observes the active session/generation's Stop
while it awaits a permission, choice answer or OpenUI form. Cancellation
does not open new interactions from queued events. The cancelled event
drain also applies its existing 150 ms deadline to a slow event handler,
and an event-channel EOF still reaches Core's original-worker settlement.
The public join continues to hold the turn permit until its existing
settlement rule releases it.

The detached fact-model path now preserves typed budget/permanent
refusals without invoking a non-streaming fallback. A stream that closes
without Done reports `stream ended before a response`; fallback remains
available for a recoverable stream-setup failure. Host cancellation
interrupts stream setup and non-streaming fallback. Dropping the call
cancels only its child provider attempt through the existing cancellation
drop guard.

Native artifact verification now reaches the existing completion gate
through an additive, default-disabled registered `Tool` hook. Only a
successful, complete typed tool receipt can acquire a proof. Its path,
saved-byte SHA-256 and full observed File identity must match the exact
receipt and an independent bounded workspace re-read. A private, nonserialized
ledger map accumulates these per-file checks; a Passed report binds the
aggregate digest only when every mutated path is covered and the workspace
has no incomplete observation, open child or unresolved invocation. Another
writer invalidates that path's proof. Model/external metadata alone cannot
enable the hook, and checkpoint JSON cannot reconstruct this authority.

No completion-gate condition or waiver rule was relaxed. A native workbook
cannot certify an earlier code edit. Multiple independently validated native
artifacts can finish one turn together.

## Scope

- `crates/code/acp/src/agent/turn.rs`: cancellation around event handling,
  bounded cancelled drain and original-worker settlement on EOF.
- `crates/code/acp/src/agent/turn_terminal_tests.rs`: a real ACP duplex-wire
  test covering permission, choice and form waits, both before Stop and
  queued after Stop (six scenarios).
- `crates/code/core/src/fact_control/model_call.rs`: the shared detached
  completion boundary, used when a fact-model call has no SessionSurface.
- `crates/code/core/src/fact_control/model_error_tests.rs`: terminal refusal,
  incomplete-stream, fallback, cancellation and future-drop coverage.
- `crates/code/core/src/tools/{types,mod}.rs`: additive artifact verification
  result and default-disabled host-tool hook.
- `crates/code/core/src/harness_loop.rs`: private live artifact proofs,
  invalidation and full-effect coverage using the existing byte observer.
- `crates/code/core/src/agent/tool_completion_runtime.rs` and
  `fact_control/receipts.rs`: bind this actual receipt's proof, retain only
  the current receipt's accepted identities, and persist the resulting
  ordinary VerificationReport for normal live/replay consumers.
- `crates/code/core/src/agent/tool_completion_runtime/native_artifact_verification_tests.rs`:
  mixed writes, multiple artifacts, forged external metadata, identity drift,
  incomplete effects, invalidation and recovery refusal.
- `crates/code/core/src/tools/task/delegation_soak.rs`: correct the test's
  source-digest domain using the existing official harness-path boundary and
  prove that real source edits still change that digest.

Existing worktree changes in those files were retained. No provider
transport, permission policy, budget limit, owner/admission contract or
Office/image tool was replaced.

## Validation

Executed from the workspace root with the pinned Rust 1.97.1 toolchain:

| Command | Result |
| --- | --- |
| `cargo test -p a3s-code-acp --lib turn_terminal_tests -- --nocapture` | 4 passed, 0 failed; 0.29 s test time |
| `cargo test -p a3s-code-acp --lib turn_cancel::tests -- --nocapture` | 3 passed, 0 failed; 0.81 s test time |
| `cargo test -p a3s-code-core --lib fact_control::model_error_tests -- --nocapture` | 5 passed, 0 failed; 0.01 s test time |
| `cargo test -p a3s-code-core --lib fact_control::model_governance_tests -- --nocapture` | 6 passed, 0 failed; 0.02 s test time |
| `cargo test -p a3s-code-core --lib -- native_artifact_verification_tests verification_metadata_tests --nocapture` | 13 passed, 0 failed; 0.02 s test time |
| `cargo check -p a3s-code-core --lib --locked --offline` | Passed during native-hook integration |
| `cargo test -p a3s-code-core --lib -- source_digest_excludes_harness_state_and_tracks_source_edits soak_parent_fanout_cancel_leaves_no_child_or_source_change --include-ignored --nocapture` | 2 passed, 0 failed; 8.86 s test time |
| `rustfmt --check --edition 2021` for the four scoped Rust files | Passed |
| `git diff --check --` for the four scoped Rust files | Passed |
| `cargo fmt --all -- --check` | Failed outside this slice: parallel formatting differences and unresolved `crates/use/tests/../crates/extension/src/tuf_test_support.rs` |

The initial ACP interaction test had a test-only wire matcher missing the
required `_` extension-method prefix. It was corrected to the exact
`_a3s/form/request` wire method and rerun alone (1 passed), then in the
four-test terminal suite above.

The first two Core test attempts could not compile because
`protocol_cancel_tests.rs:69` called absent
`AgentProtocolHostError::runtime_code`. The root task repaired that separate
protocol-host boundary, and both focused Core commands above were rebuilt
from the worktree and passed. The four focused suites contain 18 tests;
the earlier isolated ACP run is not counted twice.

The first native proof test run passed 12/13 and exposed a missing comparison
against the *current receipt's* mode identity. The binder had compared only
the ledger and current bytes. It now also requires the exact receipt's
`after` identity, and the rebuilt 13-test command above passed. The final
native binder also reuses `MutationLedger::observes_current_workspace` rather
than adding a second workspace identity reader.

The delegation, budget and context audit is recorded in
[MECHANISMS.md](MECHANISMS.md), including 68 final passing checks and the
separately reproduced ignored-soak observation-domain defect. Its test-only
correction and successful isolated rerun are recorded without erasing the
earlier failures.

## Explicitly not claimed

- No new corpus completion rate, live-provider latency or benchmark score.
- No claim that the entire planning, cancellation, resume and budget matrix
  has passed; this slice covers the concrete boundaries above.
- No claim that existing installed/bundled binaries contain these changes.
- No whole-workspace green test or formatting claim.
- Live native proof accumulation is intentionally not recovered from cache
  JSON. Extending a recovered partial artifact set still fails closed until
  its original host checks are explicitly revalidated; saved Passed reports
  keep their existing digest-bound replay behavior.
- The protocol-host retirement-race review belongs to the root task; the
  focused model tests above do not certify that separate host boundary.
