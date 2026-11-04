# Restore confirmed Core / Effect regressions

Status: scoped source restoration complete; not installed or release-qualified.

## Changes

- Restored the exact7-line public-tool-progress projection from405a70e72in Effect and its original52-line regression. The current compact_with_tail integration remains intact. Core manual projection and its original test module matchHEAD.
- Restoring Effect alone reproduces duplicate public prose at the Core provider boundary. Core now recognizes the exact prose immediately preceding its original closed tool batch and reattaches the authoritative call metadata once. The same helper covers summary prefix/tail projection. Independent earlier messages with identical text remain; original facts, calls, results and private continuation contracts are preserved.
- Existing November2scope/convergence instructions remain present and their regressions pass. This task does not reverse the ab6fc743bwithdrawal of the failed393aa8c3bexperiment or overwrite unrelated working changes.

## Actual verification

Host: Darwin arm64; pinned rustc1.97.1(8bab26f4f2026-07-14). Commands also use the existing qualification target cache, --offline and sha2profile override; full arguments and elapsed times are retained in each *-status.json.

| Check | Actual result | Record |
| --- | --- | --- |
| Original Effect regression before restoration |0passed,1failed: public count0expected1 |effect-public-progress-red-status.json |
| Core compatibility before bridge fix |90passed,1failed: public count2expected1 |core-compatibility-status.json |
| cargo test --locked -p aster-effect -- --test-threads=1 |168passed,1ignored across package targets |effect-restored-status.json |
| Core tool/batch history, convergence, prompt, manual projection and compaction connected tests |113passed,0failed |core-final-context-contract-status.json |
| cargo clippy --locked -p aster-effect --all-targets --no-deps |exit0 |effect-clippy-status.json |
| cargo clippy --locked -p aster-core --all-targets --no-deps --features serde_json/preserve_order |exit0 |core-clippy-final-status.json |
| cargo check --locked --workspace --all-targets |exit0 |workspace-check-status.json |
| cargo check --locked -p aster-core --features advanced-harness,deep-live |exit0 |optional-check-status.json |
| Pinned rustfmt check for all5changed Rust paths; git diff --check |both exit0 |changed-format-status.json; diff-check-status.json |
| cargo fmt --all -- --check |failed only in non-task tool-acquire/pager files |root-fmt-status.json; root-fmt.log |

The full-format failure identifies crates/use/tool-acquire/src/store.rs128, crates/terminal/crates/codegen/aster-pager/src/env_manager_cmd/upgrade/tests.rs138and upgrade.rs209. Those files were preserved. Owning Clippy reports no finding in the changed Core bridge/test paths; unrelated warnings remain, so no lint-free repository or full compliance claim is made.

The new Core regression exercises1/2/8-member batches, actual captured provider requests, summary/tail boundaries, preserved private wire data, an independent identical prior message, and unchanged source facts. Actual provider calls may append only normal context.sections records. Two intermediate new-fixture failures are retained: the existing provider_request helper returns its first capture (fixed by separate clients), and actual model calls legitimately append context sections (pure projection equality and original-fact-prefix preservation are now checked separately). No original regression, threshold or acceptance gate was weakened.

Final source hashes and before/after patches are retained in final-source.json and *.restoration.patch; restoration-fidelity.json confirms the exact published Effect block, preserved tail support, and original manual/test files. Deliverables and status records were re-opened before closeout.

## Explicitly not claimed

- No installed CLI/ACP replacement, commit, tag, Release or channel update. The user's installed pair remains the separately audited CLI0.1.7/ACP9.1.6; source edits do not update those executables.
- No real-model long-task retry, Kimi-loop repair, token/latency improvement, complete feature-matrix rerun, native Windows/Linux runtime proof, final-archive sandbox success, or overall release readiness.
- No attribution of the original working-tree regression to a particular writer or merge. Full repository formatting remains blocked outside this task.
