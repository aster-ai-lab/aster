# Cursor account stream compatibility

Status: repair complete. Scoped Core tests/lint, paired builds, and real Auto/Composer task and TUI acceptance passed. Broader checks were interrupted and are not claimed as passes.

## Scope and reproduced causes

The TUI selects the imported Cursor provider through ACP and the shared Core
`CursorAccountClient`. The original client asked models to encode Aster calls
inside text JSON and refused Cursor-native execution messages. A real Auto
request ignored that text contract and requested `shellStreamArgs`; Composer
returned the text JSON. No native shell request was executed by the probes.

The replacement advertises the presented Aster tools as Cursor MCP schemas,
answers bounded MCP catalog requests, and converts exact `aster` provider/tool
identities to ordinary Aster tool calls. Aster retains permission checks and
execution. Complete legacy JSON replies remain accepted. Real Auto and Composer
probes both produced native MCP `read` requests. A separate probe confirmed that
using the actual advertised unprefixed tool name also resolves to the exact
provider/tool identity. Sanitized structure is in `wire-reproduction.json`.

Before the fix, focused tests reproduced three failures: truncated streams could
return partial text as success; permanent Connect errors lacked the existing
non-retryable marker; account quota detail and retryability were lost. The
before run had 24 passed and 3 failed (`regression-before.log`).

The first native Auto task exposed an additional ordering defect in the initial
bridge: `toolRequestsListed` arrived before `execServerMessage.mcpArgs`. The raw
probe also contains this exact sequence, including an earlier completed catalog
query (`auto-event-order.json`). The bridge now retains the declared count and
accepts both orders, returns all collected calls, and refuses a stream ending
with an incomplete declared batch. The hermetic parallel-call test covers both
orders and the prior completed catalog-query batch.

Final review moved error handling ahead of native-call dispatch and refuses RPC
messages in Connect terminal envelopes. A new adversarial regression covers
these combinations; the follow-up Core run contains 38 tests.

Envelope reference: [Connect protocol](https://connectrpc.com/docs/protocol/).
Cursor message fields were checked against the installed CLI schema and real responses.

The reader now rejects malformed or prematurely closed streams and generation
deadlines, waits through partial frames and reasoning, and does not complete a
native tool preamble merely because text pauses. It retains reported reasoning
and terminal usage and honors the existing active-generation timeout hook.

## Rejected approaches and external limits

- A system-prompt override was tested and rejected by this account as disabled.
  The implementation does not send that field or rely on enabling it.
- An initial diagnostic used an incorrectly formatted client-version header and
  received an outdated-client error. Repeating with the importer's actual
  `cli-2026.10.22-27dcf5f` header disproved a production version-header defect.
- GPT-5.2 returned `resource_exhausted` with explicit `isRetryable: false` account
  usage-limit detail. The official Cursor CLI independently reported the same
  usage limit. This is not a successful model-generation or task proof.

## Verification

Host: macOS, aarch64. Rust: pinned 1.97.1. Studio was already initialized at the
parent-pinned submodule revision. Commands run from the repository root; default
package features unless explicitly listed below; no new dependency, worktree, commit, or installed-binary change.

| Command | Actual result |
| --- | --- |
| `cargo test --locked -p aster-core --lib llm::cursor_account -- --test-threads=1` before implementation | 24 passed, 3 failed; regression evidence retained |
| First post-change focused run | Test compilation failed on an owned String passed to `Message::assistant`; corrected to a borrow |
| `CARGO_BUILD_JOBS=2 cargo test --locked -p aster-core --lib llm::cursor_account -- --test-threads=1` | 37 passed, 0 failed (`core-final.log`) |
| `CARGO_BUILD_JOBS=8 cargo test --locked -p aster-core --lib llm::cursor_account -- --test-threads=1` | Final review: 38 passed, 0 failed (`core-reviewed.log`) |
| `CARGO_BUILD_JOBS=2 cargo test --locked -p aster-acp --test cursor_native_tools_live -- --ignored --nocapture --test-threads=1` | First run: 0 passed, 2 failed. Auto reached the fixed 240-second timeout. Composer wrote exact source bytes but its trace was `read, write`, omitting the requested final `read`; the strict trace assertion failed. Individual reruns identified the fixture contract issue below; after matching the real ACP/TUI host contract, the unchanged strict assertions passed for both models (`native-tasks-final.log`). |
| `RUST_LOG=aster_core::llm::http=debug,aster_core::llm::cursor_account=debug CARGO_BUILD_JOBS=8 cargo test --locked -p aster-acp --test cursor_native_tools_live -- --ignored --nocapture --test-threads=1` | 2 passed, 0 failed; both models copied and read back exact random bytes (`native-tasks-final.log`), 44.13 seconds |
| `CARGO_BUILD_JOBS=8 cargo check --locked --workspace --all-targets` | Interrupted with exit 130 after the user questioned the expanded verification scope (`workspace-interrupted.log`). No ORM or inference-engine source was edited for this task. Initial runs were also rescheduled to prioritize focused validation. |
| `CARGO_BUILD_JOBS=8 cargo check --locked -p aster-core --features advanced-harness,deep-live` | Interrupted with exit 130 while waiting for the build lock (`optional-check-interrupted.log`); not claimed |
| `CARGO_BUILD_JOBS=8 cargo clippy --locked -p aster-acp --all-targets --no-deps` | Interrupted with exit 130 during dependency checks (`acp-clippy-interrupted.log`); not claimed. The changed ACP tests compiled and passed live task/TUI acceptance as listed. |
| `CARGO_BUILD_JOBS=8 cargo clippy --locked -p aster-core --all-targets --no-deps` | Passed (`core-clippy.log`); existing warnings retained. Initial two-job metadata build was interrupted and rescheduled after focused checks |
| `CARGO_BUILD_JOBS=2 cargo build --locked -p aster -p aster-acp` | Passed (`pair-first.log`) |
| `CARGO_BUILD_JOBS=8 cargo build --locked -p aster -p aster-acp` | Passed after final error-frame review (`pair-reviewed.log`) |
| `CARGO_BUILD_JOBS=2 cargo test --locked -p aster-acp --test cursor_account_tui_e2e -- --test-threads=1` | 0 passed, 4 failed, 3 ignored. Historical run against the retained tested source: contradictory namespace guard, all failures rerun individually. Concurrent changes to that driver were detected later; see limits below. |
| Compiled `cursor_account_tui_e2e` binary, `--ignored --exact live_native_tui_auto_and_composer_return_assistant_text --nocapture --test-threads=1` | 1 passed in the first run (38.85 s) and 1 passed against final rebuilt binaries (37.20 s), each covering real Auto and Composer TUI turns from the retained tested driver (`tui-live.log`, `tui-delivery.log`) |
| `cargo fmt --all -- --check` | Final invocation passed. An intermediate invocation saw an unrelated in-flight formatting change in `apps/aster/tests/tui_913_real_llm.rs`; no unrelated file was reformatted for this task. |
| `python3.12 tests/workspace_dependencies.py` | 7 passed; system `python3` lacks `tomllib`, so the existing Python 3.12 installation was used |
| Scoped `rustfmt`, `git diff --check` | Passed on final task paths |

## Review and limits

The hermetic TUI guard was internally contradictory in pre-task HEAD and in the retained tested snapshot:
it asserts that `.aster` contains only `index`, then rejects `index` while
allowing `sessions`, `memory`, and `effect-log` in that very same directory.
`state_root.rs` intentionally maps both state and index roots to `.aster`, and
session configuration uses `.aster/memory`. The guard self-test fails alone at
line 551; the three TUI scenarios fail alone at line 537 with real expected
state directories. An initial isolated disable test additionally hit the fixed
30-second ACP startup timeout under concurrent builds; repeating it reproduced
the namespace assertion in 2.27 seconds. No assertion or timeout was weakened.
The final source-hash audit detected concurrent edits to this driver: binary
resolution and the workspace guard had changed after the retained runs. Those
edits were preserved and are not attributed to this fix. Their newer state was
not rebuilt or retested here. See `concurrent-source-changes.json`; the original
tested checksum remains in `source-sha256.json`. All other source checksums in
that manifest, including the production Cursor code and strict live task
fixture, remained unchanged.

The new live Agent fixture initially omitted the product host contract
`with_assistant_final_response(true)`, which `acp/src/agent/session_open.rs` and
`unattended.rs` already set. Its `DiskContentAttestor` therefore activated
`fact_control::tool_results::verified_turn_text` after `write` and returned a
host-generated completion before the requested final read. Isolated logs showed
both models successfully copying exact random file bytes, with `read, write`
traces, so those strict three-call tests correctly failed. The fixture now uses
the same assistant-final-response requirement as the actual TUI; prompts,
models, timeout, artifact comparison, and strict `read, write, read` assertions
remain unchanged. The corrected product-host-contract run passed both models in 44.13 seconds
(`native-tasks-final.log`), with actual `read, write, read` traces and exact
random-byte comparisons.

Existing Core scanner/source-egress duplicate-pattern warnings, unused imports,
and dead-code warnings were observed. They were not changed as part of this
adapter repair. Native calls use exact provider and tool-name fields, never a
regex or a reconstructed display-name alias. Native Cursor shell, foreign tools,
approval-only requests, and interactive queries fail closed.

## Explicitly not claimed

- Passing every repository test: the retained historical TUI guard run failed as described above, and its concurrent replacement was not retested. The interrupted workspace, optional-feature, and ACP all-target lint checks are not represented as successful.
- Successful generation with every model in the account catalog.
- Bypassing account quotas, purchasing usage, changing account or ACL state, or
  updating the installed release binaries.
- Native runtime validation on Linux, Windows, or other architectures.
- Per-token delivery to the TUI; the existing buffered `complete_streaming`
  contract remains. Reported reasoning is delivered as a reasoning event.
- Complete accounting for native tool-call cutovers when the server has not yet
  supplied terminal usage, or decoding opaque protobuf error details when the
  service omits its JSON debug detail.
