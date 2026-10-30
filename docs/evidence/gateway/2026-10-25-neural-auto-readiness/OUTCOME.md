# Unified Gateway availability and neural Auto routing

The gateway now uses one typed task-routing path for Auto and all configured models. Auto asks the existing Prism model to judge a bounded user turn against the declared rubric. Simple turns use the six-backend Prism pool; complex turns, invalid/unreachable decisions and oversized observations use the single GPU6 DeepSeek target. This is neural classification through Prism. The separate platform/apofasi endpoint still uses its lexical engine and is not represented as a trained neural decision model.

## Availability faults and retained counterexamples

Power readiness used try_lock over a lifecycle lock also held by another HTTP health probe. The published Power repair waits within a two-second deadline. Before: 4/8 concurrent probes returned false 503 on an unchanged healthy process. After: 32/32 returned 200, maximum 6.8002 ms. Power main 24b364c6935c1b62a689d91839ec394bfc74c059 and the parent deployment evidence retain its 2030 library and 45 focused tests.

Gateway supplier-key loading also converted a failed control-plane read to a successful empty key list. The isolated shadow reproduction applied such a snapshot with ready=true and then returned 503 for Auto. The repair distinguishes a failed read from an authoritative empty list and rejects the failed activation. The same isolated outage then returned HTTP 422 for the update while the previous applied revision continued serving Auto through Prism with HTTP 200. No live API service was stopped for this test.

A keyed supplier missing from a successful console response remains disabled. No environment-key fallback or caller-credential borrowing is introduced. Keyless routes have no console dependency. Supplier secrets are bounded to 1 MiB, redirects are refused, and OCR, System One, embedding and rerank share the same explicit console-key binding.

## Routing and compatibility

The policy is retained in selected-auto-policy.acl. Its operator base ACL is compiled and published through the existing a3s-web managed snapshot workflow. The Prism pool credential is registered through the model-service supplier API; its six existing backends were verified to use that credential. No model process or weight was added. DeepSeek keeps its 7200-second header timeout and 21600-second stream limit.

Decision requests use the configured upstream and a full protocol authorization header exactly once. The OpenAI chat wire uses portable fields supported by Prism; unsupported JSON-output/vendor flags are not sent. It strictly decodes one completed assistant result against the typed rubric. Unknown ids, incomplete generations, tools, invalid probabilities and malformed JSON refuse the decision. HTTP requests have a five-second total deadline, a two-second connection deadline and a 64 KiB response bound.

A credential-isolated, policy-bound turn cache retains facts for at most 15 minutes and 4096 entries. Per-turn bounded coalescing makes 32 simultaneous duplicate turns issue one neural call while unrelated turns remain concurrent. Tool continuations reuse the frozen group. The 64 KiB observation bound covers leading system content as well as visible history, preventing a cached small turn from bypassing the primary-model context floor.

Known WorkBuddy message annotations are removed before both native and ordinary proxy dispatch, preserving role/content/tool calls/reasoning and unknown provider fields for fail-closed validation. The initial public regression caught a landing omission of queuePosition and queueTotal; that failed run is retained. The exact two fields were added, with direct-dispatch and Auto tool-continuation regression tests that inspect actual forwarded messages. Queue-only isolation on the deployed predecessor returned 200 and a valid tool call once those two annotations were removed, confirming the cause.

## Commands and counts

From the initialized Aster root, pinned Rust 1.97.1 and unchanged Cargo.lock:

- cargo test --locked -p a3s-gateway --lib -- --test-threads=1 with the server ONNX Runtime library path: 2125 passed, 0 failed, 5 existing ignored tests after the activation repair.
- After the two-field queue correction: focused OpenAI request tests 26 passed; neural-routing/forwarded-message tests 7 passed.
- Supplier-key tests: 8 passed, including six new activation/boundary cases.
- Decision transport tests: 9 passed; existing typed routing and real-HTTP HiRoute tests retained.
- cargo build --release --locked -p a3s-gateway --bin a3s-gateway: captured the owned compiler artifact by stable dependency inode and SHA256 for each rollout.
- Changed-source rustfmt check, edition 2021 with skip_children=true: passed.
- python3 tests/workspace_dependencies.py: 6 passed.
- python3 tests/governance_regime.py: 4 passed.
- Global cargo fmt --all -- --check and crate-drift ratchet retain pre-existing failures. workspace-no-growth-proof.json proves the same 16-entry drift set and only three unchanged format-failure files. No allowlist or ratchet was relaxed.

The first full run's seven failures were individually rerun and retained. Two needed the existing ONNX Runtime dynamic library. Stale fixtures were corrected to test admitted rerank suppliers, the single-backend passive-health guard, native SSE truncation, and unprivileged listeners. The missing-model case runs in an isolated child with empty explicit model directories so installed server weights cannot change its result.

Configured shadow Auto calls returned 200: simple first 1.540 seconds through Prism, same-turn replay 0.180 seconds, complex through DeepSeek. The first production Auto checks returned 200: simple 2.180 seconds, replay 0.312 seconds, complex through DeepSeek. Raw responses and actual timings are retained; these are individual examples rather than a latency distribution.

Post-readiness-wrapper native validation passed five quality probes. Seven 128-token decode trials reported a 31.6 tokens/s median under the concurrent validation workload. The earlier selected-profile campaign reported 34.8 tokens/s and a 2865.0067-second cold prefill of 1,000,123 tokens. The final gateway rebuild does not change the native engine, weights, DSpark profile or model-service PIDs.

Final queue-corrected public regression from the Windows client through http://117.162.0.179:8080/v1 passed all 18 cases / 30 HTTP requests across DeepSeek, Prism and Auto, including JSON/SSE, automatic/named/required tools, tool-result continuations, and Hi with tool definitions. The queue-annotation-rollout receipt records gateway PID 3406639 and binary SHA256 af7ccb465e147e56afbddd0bc22257db444c600e83ddfedff76e648ab5d78d24. All 15 preserved model-service PIDs remained unchanged.

The official publisher applied revision 50. A further normal official refresh applied revision 51 with the same Auto policy, unchanged Gateway PID and all model PIDs preserved. The timer remains enabled and active. This verifies persistence through the existing publication workflow rather than a one-off in-memory route patch.

During actual DeepSeek inference, all 32 additional concurrent readiness probes returned 200; all 32 observed admission active=1, maximum 3.576 ms. Every one of the six Prism workers separately completed a direct inference request with 200. The live OCR endpoint returned 200 on the synthetic bitmap fixture; the glyph S was recognized as 5, so this is availability evidence rather than an OCR accuracy pass. Embedding, rerank and the separate lexical platform decision endpoint returned 200 in the shadow check. Both preserved voice services returned health 200; speech transcription quality was not retested.

## Explicitly not claimed

No absolute performance ceiling, broad classifier accuracy, universal handling of unknown future client fields, or zero downtime during a gateway binary restart is claimed. The full cold-1M campaign was not repeated after these wrapper/gateway repairs. Prefix affinity is a routing hint, not cross-engine KV transfer. OCR glyph accuracy and voice transcription quality are not claimed by these availability probes. The existing five ignored library tests and pre-existing workspace format/drift debt are not claimed fixed. No credentials, private ACL, actual customer prompts, model weights, or another session's Cloud/API changes are included.
