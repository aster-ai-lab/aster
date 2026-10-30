# Apofasi auto context-budget optimization

## Result

Production `auto` now preserves the latest user task before applying bounded history truncation. A latest task that cannot fit the 256-token auto decision budget refuses neural routing and falls back to DeepSeek. Older history is retained from the most recent end and is explicitly marked `[history is partial]` when shortened.

The optimized gateway is running in production with managed snapshot revision 74. Binary SHA256: `81034f9c035c806504627977bfbc37b71ddb41a1710c8473869927ad60db1ced`.

## What landed

- `StateTruncation::{Head,Tail,TailAndMark,Reject}` and `StateOverflow` in Apofasi.
- `NeuralEngine::decide_with_required` and `decide_with_required_and_len` with the existing default behavior preserved.
- A required latest-user request that includes the partial-history marker in its budget check.
- A 256-token auto decision budget. The full checkpoint `max_len=1024` is not enlarged.
- Real truncation markers that are reserved within the token budget instead of being added after truncation.
- Six hermetic Apofasi packing tests covering exact boundaries, overflow rejection, markers, and recent-state preservation.
- Real-checkpoint tests for oversized latest-user rejection, long-history preservation, and the existing 36-case routing set.

## Validation

- `cargo test --locked -p a3s-apofasi --all-targets -- --test-threads=1`: 37 tests passed (32 library + 5 packing at that run); the focused packing suite later had 6/6 passing after the exact-boundary case was added.
- Real Laya checkpoint, release mode on the production host: 3/3 tests passed. The 256-token forward took 1746.52 ms; the complete bounded-history decision took 1751 ms. The earlier 1024-token forward took 17630.77 ms.
- Existing 36-case set: complex-to-economy misroutes 0; simple recall 94.4%.
- Shadow HTTP: 26/26 requests passed across short inputs, long history, latest-user overflow, and complex requests.
- Production HTTP after deployment: 26/26 passed. After managed refresh and a gateway restart: 26/26 passed again.
- Public `auto` request: HTTP 200 with the expected Prism route. Production service reported `NRestarts=0` at the final check.
- `PATH="$HOME/.cargo/bin:$PATH" python3 tests/workspace_dependencies.py`: 6 tests passed.
- `cargo clippy --locked -p a3s-apofasi --all-targets --features infer --no-deps`: completed with existing warnings.
- Full gateway library run: 2234 passed, 2 existing Windows TrueType-font fixture failures, 2 ignored.
- Full gateway Clippy remains blocked by the pre-existing `never_loop` error in `apps/gateway/src/entrypoint/inference_distributed_tests.rs:1869`. No new gateway compile error was observed.
- `cargo fmt --all -- --check` is blocked on this Windows host by OS error 206 (path too long); focused `rustfmt` checks on the changed files passed.

## Explicitly not claimed

- No claim of universal classification accuracy, especially for arbitrary long or adversarial conversations.
- No claim that 256 tokens preserve all history. Older history is intentionally shortened and marked partial; the latest user is the protected content.
- No claim that DeepSeek fallback is free or has the same cost or context limit as Prism.
- No claim of zero downtime for every future binary rollout, nor of a full Clippy or formatting pass on the whole workspace.
- No semantic-history summarization, retrieval, multi-pass voting, or larger-context accuracy evaluation was performed.
