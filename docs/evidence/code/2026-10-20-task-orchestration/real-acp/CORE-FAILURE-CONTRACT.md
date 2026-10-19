# Typed failed-effect completion contract

The controlled Work → ACP required-check fixture reproduced a real defect: a native write followed by an unrecovered failing program could still receive an automatic disk-content pass and a traced artifact after an explicitly false success final. The isolated RED is `required-failure-isolated-red.log`.

The shared fix keeps Work automatic attestation and the public CompletionAttestor API. MutationLedger stores private, serde-default `failed_effect_paths` only from valid typed Failed execution receipts with complete actual changed paths. Expected read failures without changes do not create this state. A succeeded compound invocation that recovers an inner failure remains succeeded. There is no tool-name or error-text inference.

A later complete Succeeded receipt clears only a matching path whose after identity is new. Unrelated writes, successful reads and unchanged bytes cannot clear it. Deletes retain a Missing after identity. A current effect-digest-bound formal verification can still close the ordinary completion gate. The path-only automatic attestor is skipped while this typed negative state remains; this does not disable formal verification.

The effect digest is unchanged for ledgers without negative state. Negative state uses a domain-separated digest containing the existing record digest and sorted length-delimited path/after identity. Thus an old positive report for identical bytes cannot certify a failed invocation. The serde state, original-log replay and conservative cache restore retain negative claims. The cache coverage proof requires the selected exact original receipt prefix to have the same failed path map, in addition to existing receipt and after-state checks.

Independent review found a stronger cache forgery than a missing field alone: remove the negative map and replace its digest with the valid positive digest for the same records. `failed-effects-cache-red.log` reproduces this. The final exact-prefix equality rejects it. Conservative restore still cannot erase the live original negative state.

Final focused checks:

- `cargo test --locked --offline -p a3s-code-core --lib completion_attestor:: -- --nocapture`: 14 passed, including 6 new negative-state/recovery/cache tests; `failed-effects-core-final.log`.
- `cargo test --locked --offline -p a3s-code-core --lib harness_loop:: -- --nocapture`: 60 passed; `failed-effects-ledger-final.log`.
- `cargo test --locked --offline -p a3s-code-core --lib fact_control::receipts::tests::cache -- --nocapture`: 11 passed; `failed-effects-fact-cache.log`.

The source is `crates/code/core/src/harness_loop.rs`, `completion_attestor.rs`, and new `completion_attestor_failure_tests.rs`. The first two also contain other owners' existing native-proof/streaming-digest changes; this slice owns only the typed failed-effect contract described above. The independent reviewer is sandbox_speed.

These hermetic checks do not qualify a model, VM or whole-task speed improvement. Final real Work → ACP qualification and binary identity are recorded separately after the stable companion build.
