# H200 repeated-tool repair (R11)

WorkBuddy repeatedly requested the same two searches, 56 times each. The model
server completed short generations between requests; it was not one endless GPU
generation. Private session bodies, queries and search results remain unpublished.

The pinned frontend dropped both assistant tool-call IDs and tool-result IDs.
OpenAI and Anthropic conversions now preserve them so DeepSeek's encoder can
associate parallel results in call order. A public reverse-arrival counterexample
reproduces the original defect. This defect is not proven to be the sole cause
of the reported search loop.

For Chat Completions, three adjacent completed rounds with identical exact tool
names, JSON arguments and matched result bodies trigger one bounded recovery.
The policy does not guess tool semantics. It retains the rendered schema prefix
for KV reuse, offers no further tools, and caps this recovery at 8192 output
tokens while preserving smaller explicit client limits. A failed recovery cannot
release another tool call. Ordinary streams remain incremental.

The first candidate incorrectly let an absent max_completion_tokens override a
smaller max_tokens value. Its failed test is retained. The first actual-client
repeat test stopped after three calls but the model reported four. A clearer
execution-status note fixes the exact count. An intermediate note accurately
reported three but failed the requested strict response format; that failure
is also retained.

The final actual installed WorkBuddy CLI deliberately asks for four sequential
identical searches. Only three are executed and the final reply is exactly
COMPLETED=3. A separate successful Read-tool roundtrip returns exactly 442.
Two real-model protocol probes pass reverse parallel association and repeated
round recovery. Live SSE recovery ends with one stop and DONE, exposing no tool
call. The same cluster VIP and API key serve the repaired API on the same C13
H200 and unchanged native binary. No Windows forwarding is introduced.

Validation commands: python -B test_tool_roundtrip.py (8 hermetic methods);
remote complete-module protocol checks (8 methods), serve.test_security
(23 methods), and the original test_serve_ds41.py harness (all passed).
Live commands and sanitized receipts are retained under operators/ and receipts/.

## Explicitly not claimed

The CLI search tool returns Authentication required for web search. Its error
roundtrip and bounded repeat termination pass, but successful authenticated
search and the GUI workflow are not accepted as passed. The old private session
is not replayed as a live HTTP request. Native speed gains, a fresh formal 1M
execution, concurrent-output identity repair, 300/1000 token/s, and goal
completion are not claimed by this API repair. The independent C12 comparison
continues through its existing handles; its known identity failure remains.

Exact deployed source bytes are retained. Any per-file blank-at-EOF attribute
only preserves those frozen bytes; it does not suppress other whitespace checks.
No weights, binaries, raw profiler reports, credentials or user session state
are included. Rust workspace checks were not run for this deployment-only change.
