# TUI cancellation, resume, and model controls

## Scope

Validated the working tree based on `b1be95c5b`, on macOS arm64 with Rust
`1.97.1`. Existing Studio, verification, Terminal-Bench, and asset changes were
preserved. No commit, push, installation, or ToWork change was performed.

Stop and Ctrl+C used the same cancellation path. The host recorded cancellation
before waiting for the session registry, but notified the running attempt only
after that wait. Moving the notification alone did not cancel a stalled HTTP
request while the registry was held: the stronger regression timed out after
three seconds. The attempt now retains its exact Core Run handle and cancels it
when its generation is marked cancelled. Registry cleanup and subagent handling
follow independently. The notification handler also checks the generation and
cancels a captured Run rather than whichever Run might become current later.

Ordinary follow-ups previously entered a server queue whose mutation interface
(`aster/queue/interject`) Aster ACP does not implement. Aster now uses the existing
local held queue; forcing a row submits its exact content through the supported
`sendNow` request. A delivered request's final error no longer automatically
requeues and re-executes it. Only a typed channel SendFailed, which proves the
request was not enqueued, retains automatic requeue behavior.

`/resume` now exposes the existing session picker and restore workflow. The
permission name and generated UI labels use YOLO; persisted and wire IDs stay
`yolo`.

Model controls now use validated, protocol-specific capabilities. The selected
model's catalog/account lookup runs asynchronously, has a ten-second deadline,
does not generate a model completion, and publishes pending/completed metadata.
Reported context and output limits reach the Core session and transport, not
only the menu. Account defaults, explicit effort overrides, and the Default
reset action remain separate. Unsupported overrides are revalidated on a model
change; missing metadata does not create a universal five-level menu. Explicit
ACL declarations and verified exact-model documentation remain identified
fallback sources. Discovery does not rewrite credentials or user preferences.

## Build and checks

All Cargo commands used `CARGO_TARGET_DIR=/Users/roylin/code/aster/target` and
`CARGO_INCREMENTAL=0`, with one Cargo owner. PTY commands used the current
`target/debug/aster` and `target/debug/aster-acp` through ASTER_TUI_BIN and
ASTER_ACP_BIN. Both final executables contain the same identity:

```text
aster-cli-pair-v1:c6a335a0b787cf2a3a5f30def64642447c53e667:aarch64-apple-darwin
```

| Command | Actual result |
| --- | --- |
| `cargo build --locked -p aster -p aster-acp --bins` | Exit 0; paired identity verified from both executables |
| `cargo clippy --locked -p aster-core -p aster-acp -p aster-pager --lib --no-deps` | Exit 0; existing warnings remain |
| `cargo test --locked -p aster-acp --lib -- --test-threads=1` | 515 passed, 10 opt-in tests ignored |
| `cargo test --locked -p aster-acp --test unattended_e2e -- --test-threads=1` | 33 passed |
| `cargo test --locked -p aster-acp --lib unattended:: -- --test-threads=1` | 34 passed; overlaps the ACP library suite |
| `cargo test --locked -p aster-core --lib config::reasoning::tests -- --test-threads=1` | 3 passed |
| `cargo test --locked -p aster-core --lib model_discovery -- --test-threads=1` | 7 passed |
| `cargo test --locked -p aster-pager --lib acp::model_state -- --test-threads=1` | 17 passed |
| `cargo test --locked -p aster-pager --lib model_switch -- --test-threads=1` | 8 passed |
| `cargo test --locked -p aster-pager --lib acp_handler::tests::settings -- --test-threads=1` | 21 passed |
| `cargo test --locked -p aster-pager --lib app::dispatch::tests::prompt -- --test-threads=1` | 212 passed |
| `cargo test --locked -p aster-pager --lib app::effects::tests::send_now_ -- --test-threads=1` | 1 passed; SendFailed, RecvFailed, and provider-error cases |
| `cargo test --locked -p aster-pager --lib apply_aster_slash_gate -- --test-threads=1` | 1 passed |
| `git diff --check` | Exit 0 |

The selected-model TCP test verifies loading state, the exact advertised effort
list, a real 131072-token session context, a 32768-token output cap, native high
on the first request, and omission of the override after reset. Transport tests
cover the advertised native levels and omission on reset across OpenAI,
Responses, Codex, Anthropic adaptive/output effort, GLM, DeepSeek, Kimi, and
Gemini. Capture-only transport fixtures deliberately return HTTP 400 after
recording the request; those fixtures prove serialization and preflight, not
provider acceptance.

## Shipped TUI

The following Cargo tests used `-- --test-threads=1 --nocapture`:

| Test command suffix | Actual result |
| --- | --- |
| `-p aster-acp --test shipped_tui_e2e session_controls::` | 2 passed: mouse Stop, Ctrl+C, forced queued send against a stalled HTTP server; `/resume` picker restores the original session ID and transcript |
| `-p aster-acp --test shipped_tui_e2e reasoning::` | 1 passed: catalog discovery, native low, Default omission, model switch, and restart in another workspace |
| `-p aster-acp --test account_model_tui_e2e` | 4 passed: YOLO banner/footer, selection/reset persistence, and Codex catalog filtering/switching |
| `-p aster-acp --test shipped_tui_e2e shipped_tui_launch_resume_and_visible_commands` | 1 passed: paired auto-discovery, continue/resume, navigation, palette reachability including `/resume`, permission keyboard ownership, and quit |

An intermediate run correctly rejected a mixed CLI/ACP build after a test
source changed the pair fingerprint. Both executables were rebuilt together;
the final no-override companion-discovery checks passed. The approval assertion
was updated to the requested `Ctrl+o:YOLO` label without removing its keyboard
ownership or no-side-effect assertions.

## Real accounts and provider calls

Opt-in tests used the existing authorized ACL via ASTER_EFFORT_LIVE_CONFIG and
the current sources:

```sh
cargo test --locked -p aster-acp --test reasoning_live \
  current_catalog_retains_capability_sources -- --ignored --test-threads=1 --nocapture
cargo test --locked -p aster-acp --test reasoning_live \
  configured_native_efforts_complete -- --ignored --test-threads=1 --nocapture
```

The second command ran separately with ASTER_EFFORT_LIVE_MODELS set to each
model below. Requests asked only for READY and exposed no tools. The capture
stores control fields, not credentials or conversations.

| Model | Actual result |
| --- | --- |
| `deepseek/deepseek-flash` | Default, none, low, high, max: five successful responses and matching native request fields. Catalog reports 1048576 context tokens; its native off switch is retained from the documented contract |
| `codex/gpt-6.1-sol` | Default, low, medium, high, xhigh, max: six successful responses and matching native request fields. Account catalog reports 500000 context tokens and native default low |
| `glm-01/glm-5.3` | Catalog responds but omits context/effort fields. Default completion exceeds the 120-second probe deadline; remaining effort calls are not claimed |
| `kimi/k3.1-pro` | Catalog returns HTTP 404. Default completion exceeds the 120-second probe deadline; remaining effort calls are not claimed |

The imported catalog contained 276 account-sourced, eight documented, and two
unknown entries. A real Cursor Auto lookup confirms preset control, with no
independent effort menu and no reported fixed context limit.

A separate ad hoc Python stdio client exercised the real GLM and Kimi sessions
at low effort: initialize with `aster/modelDiscovery`, create an isolated
session, send the same READY prompt, wait 30 seconds, then send session/cancel.
Unknown client-side operations were refused; no tool approval was granted.
Both prompts returned stopReason cancelled within the ten-second bound:

| Real session | Measured cancel-to-prompt-response time |
| --- | --- |
| `glm-01/glm-5.3` | 3 ms |
| `kimi/k3.1-pro` | 2 ms |

Private diagnostic results remain under
`/tmp/aster-controls-live-cancel/<provider>/result.json`. These are individual
control-path observations, not a latency benchmark or proof that a provider
immediately stopped all remote computation.

## Other verification failures

- `cargo test --locked -p aster-core --lib llm:: -- --test-threads=1`:
  685 passed, eight ignored, two unchanged split-CR assertions failed in
  `llm::openai::tests::streaming_split_cr_does_not_emit_partial_event` and
  `llm::codex_login::tests::codex_login_split_cr_does_not_emit_partial_event`.
  The existing SSE parser documents immediate termination on a blank trailing
  CR; no parser change was made here.
- `cargo test --locked -p aster-pager --lib slash:: -- --test-threads=1`:
  427 passed; the unchanged rendering test
  `rendered_user_and_streaming_markdown_commands_keep_their_text_and_layout`
  fails at width 18. It also fails when run alone. No highlight/layout source
  or assertion was changed to hide it.
- All-target Clippy stops at the existing String-versus-Arc<str> error in
  `crates/code/core/tests/test_persisted_schema_roundtrip.rs:211`.
- Workspace format checking reports 21 other files, including pre-existing
  Studio changes and untouched tests. It reports no remaining formatting diff
  in this task's edited Rust files. Unrelated files were not reformatted.

## Explicitly not claimed

- A fully green workspace test, format, or all-target Clippy run.
- Successful complete-effort acceptance for GLM or Kimi; the provider timeout
  cause was not established. Their real cancellation path was verified.
- A fixed context limit for Cursor Auto or values absent from a provider's
  metadata. Missing fields remain unknown or use an identified configured or
  documented source. No large-context boundary stress test was performed.
- Measurement of a provider's internal reasoning budget, reasoning quality,
  or remote GPU termination from one accepted effort field.
- Codex orchestration-only ultra as an independent provider reasoning effort.
- Linux, Windows, Mobile, or Studio GUI runtime acceptance.
- Updating a running TUI or replacing the user's installed binaries. Start
  `target/debug/aster` to use this verified pair.

## Primary contracts consulted

- [OpenAI reasoning](https://developers.openai.com/api/docs/guides/reasoning)
  and [GPT-6 model controls](https://developers.openai.com/api/docs/guides/latest-model).
- [Claude effort](https://platform.claude.com/docs/en/build-with-claude/effort)
  and [Models API](https://platform.claude.com/docs/en/api/http/models).
- [DeepSeek model metadata](https://api-docs.deepseek.com/api/list-models/).
- [GLM thinking controls](https://docs.bigmodel.cn/cn/guide/capabilities/thinking)
  and [GLM-5.3](https://docs.bigmodel.cn/cn/guide/models/text/glm-5.3).
- [Kimi model controls](https://www.kimi.com/code/docs/en/kimi-code/models.html).
- [Gemini model metadata](https://ai.google.dev/api/models).
