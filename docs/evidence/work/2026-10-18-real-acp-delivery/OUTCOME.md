# Work native artifact delivery through real ACP

Result: seven focused tests passed, including all three real-ACP scenarios.

## Scope

The regression uses Work's `TurnService`, its generated ACL configuration, the
workspace `a3s-code-acp` executable, and the actual Core tool execution path.
Only the OpenAI-compatible provider is a loopback HTTP/SSE fixture. No external
API credential or provider is required. Permission requests are approved through
the real `TurnService.respond_approval` route, restricted to the two scripted
tool names; the test does not disable the sandbox or bypass permission handling.

- The success scenario checks that the model request includes `office_file` and
  `output_check`, executes percentage calculations, writes native DOCX/XLSX/PPTX/PNG,
  streams and persists the final answer, and retrieves Work's immutable artifact
  snapshots with bytes identical to the workspace files. ZIP member text and the
  PNG signature distinguish actual native formats from renamed text files. It
  also checks DOCX paragraphs/table content, a recalculated XLSX formula and its
  numeric cache, and a PNG embedded on the second presentation slide.
- The failure scenario sends an invalid Office extension, verifies tool-error
  feedback reaches the following model request, and then returns an explicit
  provider HTTP 400. It requires a user-visible failed terminal, failed producer
  proof, zero artifacts, and unchanged source bytes.
- The truncation scenario returns one actual provider `finish_reason=length`
  response. It requires the partial text to survive streaming and persistence,
  an error terminal and failed producer proof, no retry, no tool call, and no
  artifact. The fixture does not manufacture an ACP terminal.
- After building the product and companion, CI runs the four ordinary
  stop-reason/delivery-contract tests and the exact ignored module
  `agent_plane::turns::local_acp_tests::delivery::`. It does not select live
  scheduler tests.

## Commands and results

The initial diagnostic command used a filter that was too broad:

```sh
cargo test -p aster-dream --lib real_acp_ -- --ignored --nocapture --test-threads=1
```

It executed three tests: zero passed and three failed. The two new scenarios
stopped because the test incorrectly assumed `auto` mode would omit permission
cards for these dynamic tools. The third was the existing live scheduler test,
which requires an explicit live-test environment variable. Neither result
established a production failure. The fixture now acknowledges actual permission
cards through the host, and all subsequent runs use the exact module filter.

The next exact-module attempt stopped during compilation. It exposed a fixture
mistake (`respond_approval` returns `bool`, corrected to `assert!`) and concurrent
updater-test compilation errors outside this change. No scenarios executed in
that attempt.

The production companions were then freshly rebuilt with:

```sh
cargo build --locked -p a3s-code-acp -p a3s-office-cli
cargo test --locked -p aster-dream --lib -- prompt_stop_reason_tests delivery_contract_tests --nocapture --test-threads=1
```

The build passed in 1m 08s. The fresh Work test binary ran four ordinary tests:
four passed, zero failed, zero ignored (0.00s). The first exact three-scenario
delivery run on that binary passed the two failure scenarios and stopped the
success scenario at the fixture's expected request count: Core correctly issued
a seventh, asynchronous memory-extraction request after the final delivery
response. Source inspection identified `schedule_turn_memory_extraction` and
`complete_memory_extraction`; this was not a seventh delivery-tool round.

The fixture now responds to that request with a valid non-streaming empty-memory
JSON answer including the required OpenAI `usage` field, checks its
no-tools/non-streaming wire shape and fixed system instruction, and waits for it
to be served before ending ACP. A single-test re-run passed; the initial reply
without `usage` produced a fixture parse warning, so it was corrected before the
final exact-module run.

Final verification:

```sh
cargo test --locked -p aster-dream --lib agent_plane::turns::local_acp_tests::delivery:: -- --ignored --nocapture --test-threads=1
target/debug/deps/a3s_desktop_lib-104c4ea3ed9a9da3 prompt_stop_reason_tests delivery_contract_tests --nocapture --test-threads=1
rustfmt --edition 2024 --check apps/work/src-tauri/src/agent_plane/local_acp_delivery_tests.rs apps/work/src-tauri/src/agent_plane/local_acp_tests.rs
git diff --check -- .github/workflows/ci.yml apps/work/src-tauri/src/agent_plane/local_acp_tests.rs
shasum -a 256 target/debug/a3s-code-acp
```

- The three real-ACP tests passed: zero failed, zero ignored, 758 filtered out,
  1.85s total. Work test compilation completed in 22.72s. There was no memory
  extraction parse/transport warning in this final run.
- The same freshly compiled Work test binary then passed the four ordinary
  tests: zero failed, zero ignored, 757 filtered out, 0.00s.
- Formatting and scoped diff whitespace checks passed.
- ACP SHA-256:
  `ee5893698ecfc7431a2f897fd255fc27b427ccfe0e719d7da2a4e592317ceb97`.

These measurements come from actual observed request arrays, settled tool parts,
decoded artifact bytes, and a monotonic timer around the host turn. The timer
ends at the user terminal and excludes the subsequent wait for the background
memory request.

| Scenario | Host turn (ms) | Provider requests | Tool calls | Artifacts | Artifact bytes |
| --- | ---: | ---: | ---: | ---: | ---: |
| Native file delivery | 1130 | 7: 6 streaming + 1 background | 5 | 4 | 14551 |
| Invalid extension, then provider HTTP 400 | 409 | 2 | 1 | 0 | 0 |
| Provider response truncated | 274 | 1 | 0 | 0 | 0 |

Captured sizes: `report.docx` 2722 bytes; `report.xlsx` 3263 bytes;
`slides.pptx` 7560 bytes; `trend.png` 1006 bytes. Every retrieved immutable
snapshot matched the corresponding workspace file bytes.

## Explicitly not claimed

- These controlled provider responses establish execution and delivery wiring,
  not autonomous model tool choice, natural recovery from every tool error, or
  the quality/pass rate of the 22-task evaluation corpus.
- Reported turn latency is a single local loopback-provider observation, not a
  live-model latency or throughput benchmark.
- The failure scenario deliberately ends with a provider error. It does not
  establish that every isolated tool refusal should fail an otherwise complete
  conversation.
- The tests do not enable unqualified macOS native process execution. File
  generation uses the in-process Office/image tools while that gate stays closed.
- The renderer is not launched. Host stream events, persisted messages, and
  artifact retrieval are checked; visible card rendering and opening files in a
  desktop Office application are not checked here.
- Tests have not been run on Linux or Windows in this local verification.
- The successful companion rebuild preceded subsequent parallel sandbox/OCI
  migration edits. This record identifies the tested ACP binary; it does not
  claim a successful build of those later edits or of the whole current worktree.
- Workbench independent review was disabled. XLSX/PPTX independent-review
  evidence support remains outside these three scenarios; byte provenance and
  artifact retrieval were verified here.
