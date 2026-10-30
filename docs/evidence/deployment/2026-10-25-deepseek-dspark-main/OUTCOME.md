# Selected DeepSeek DSpark deployment and readiness repair

One DeepSeek instance is deployed on GPU6 through a3s-power's managed native Xolotl backend. Six Prism instances and the speech/OCR/embedding services remain available. The selected engine profile uses 1,048,576 context, FP4 KV, DSpark depth 4/confidence 0.6000/eviction 4096, and indexer tile 2^26.

The measured native profile improved warm decode from 26.0 to 34.8 tokens/s (median, six 128-token samples) and the cold 1,000,123-token prefill from 4015.662 to 2865.0067 seconds. Full inputs, responses, quality probes, sampling checks, cache counts and resource samples are retained in the adjacent dspark-prefill and exact-performance evidence directories.

Concurrent runtime readiness checks could falsely reject a healthy loaded engine when another probe held the lifecycle mutex over HTTP. The deployed wrapper now waits within a two-second bound. Before the fix, four of eight concurrent probes returned 503 on the same process; after the fix, 32 of 32 returned 200, maximum 6.8002 ms. The managed-snapshot status remained applied and ready. A model reload during the preceding rollout also accounted for the reported 14:26 UTC 503 window.

## Commands and counts

From the initialized Aster root, using pinned Rust 1.97.1 and the unchanged root lockfile:

- cargo test --locked -p a3s-power --no-default-features --features server --lib: 2030 passed, 0 failed on the final source. Focused Xolotl run: 45 passed.
- cargo test --locked -p a3s-gateway --lib inference::openai_request: 26 passed.
- Gateway prefix-affinity tests: 13 passed; usage-report tests: 5 passed.
- python3 -B -m unittest discover -s tools/model-runtime/deepseek-v41-xq -p test_*.py -v: 22 passed.
- python3 tests/workspace_dependencies.py: 6 passed.
- python3 tests/governance_regime.py: 4 passed.
- Scoped rustfmt --check, edition 2021: passed for changed Power and Gateway sources.
- cargo build --release --locked -p a3s-power --no-default-features --features server --bin a3s-power: passed; final compiler artifact SHA256 899084c8e07d7da2a902169dd21d6dc4e294352caf7b110c4eded34a78e59d28.
- Public endpoint from the Windows client, POST http://117.162.0.179:8080/v1/chat/completions, deepseek-v4.1-flash: HTTP 200, 2.0845529 seconds, correct JSON answer 323 for 17*19.
- Native best-profile campaign retained ten successful WorkBuddy tool/streaming requests and the cold-1M/cache-replay proofs.

The first 16-output-token public probe returned HTTP 200 but exhausted the generation budget in reasoning with no visible answer. It is not counted as a content-quality pass. The 128-token arithmetic probe above verified visible content.

Global cargo fmt --all -- --check and crate_drift_report.py --baseline tests/crate_drift_baseline.json both fail on pristine parent main 57cc7f7 and on this landing. baseline-workspace-checks.json proves no growth in either failure set. No ratchet or allowlist was relaxed.

Power main contains the merged upstream compatibility and existing H200/native-storage history. Final Power commit and engine/source hashes are retained in final-power-release-identity.json. Live rollout receipts record unchanged PIDs for the other model services. Gateway Auto routing has a separately active optimization goal and is not claimed complete by this deployment record.

## Explicitly not claimed

The full cold-1M experiment was not repeated after the readiness-only Power wrapper rebuild; engine, weights and selected native arguments stayed identical. Results do not establish an absolute hardware limit, broad task-quality equivalence, or uninterrupted availability while a single engine is deliberately restarted. This landing does not import unrelated sessions' Cloud/API/Box changes, model weights, credentials or user session state. The pre-existing full-workspace format/drift checks are not claimed green.
