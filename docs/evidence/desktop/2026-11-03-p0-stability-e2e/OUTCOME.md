# Aster P0 stability consolidation

Date: 2026-11-03. Host: macOS 26.6.2 arm64. Rust: pinned 1.97.1.

## Outcome

Three production fixes are implemented and their focused checks pass:

- Restore the existing provider-store status, reload and quarantined-reset routes used by the existing recovery UI. Healthy configuration cannot be reset; corrupt bytes are retained in a backup. Reopening preserves the tenant policy. Lock failures return the existing recoverable error.
- Repair legacy MCP HTTP+SSE at its existing transport owner: connect to the configured URL, await the advertised message endpoint, POST there, and match JSON-RPC replies received on SSE. Notifications and independent tools-list refresh retain their existing owner. Requests have cancellation/timeout cleanup; close stops the reader and pending requests. The existing incremental UTF-8/frame parser is reused. Headers and URLs containing credentials are omitted from diagnostics.
- Give the API-key reveal/hide control its existing translated accessible name. Its test now verifies actual masking, explicit reveal and hiding.

The HTTP+SSE behavior follows the [published MCP transport contract](https://modelcontextprotocol.io/specification/2024-11-05/basic/transports). No new third-party harness support, product page, dependency or parallel configuration owner was added.

**The bounded P0 acceptance is complete.** On a stable, isolated instance using the production renderer preview, the real-model file task completed through Aster ACP. Independent project tests passed 12/12 and the unchanged external acceptance passed all 8 queries. Native-window input/send, cancellation, provider configuration and usage checks also passed. This is not a claim of complete Magpie parity.

The first file-task attempt remains a recorded failure: the owned application exited with status 0 before completion, leaving 4 passing and 6 failing project tests and a failing external acceptance. Its exit cause was not established. The successful retry used a fresh copy of the same original fixture and the same already-confirmed prompt; it did not replace or relax the external grader.

## Actual verification

| Check | Result |
| --- | --- |
| Provider/model/usage/stream renderer selection | 732 passed, 82 files |
| Provider and model-control native selection | 177 passed, 1 existing seed helper ignored |
| New HTTP+SSE sibling tests | 6 passed |
| Public HTTP notification/cache-refresh integration | 3 passed |
| Native accounting tests, using the just-built library test binary | 14 passed |
| SSE parser selection, using the just-built Core test binary | 15 passed; includes the 6 transport tests above |
| Workspace dependency guard, Python 3.13 | 7 passed |
| Existing connector browser/native-authority/real-ACP campaign | 1 passed, 47.7 s; final attempt 9 |
| Real account-backed model invocation | Exact `P0_REAL_PROVIDER_OK` response; no model tool calls |
| Reopened native accounting database | Three distinct requests/attempts retained: two succeeded, one cancelled. Main response reported 19,941 tokens. Unknown pricing and the cancelled call's unknown usage remain unknown |
| Clippy, Core and Studio, all targets, no dependency linting | Exit 0; existing warnings remain; no diagnostics in the changed blocks were identified |
| Scoped Rust format, frontend format/lint and diff checks | Passed |
| Final full-workspace format | Failed on an unrelated change in `crates/code/acp/src/goal.rs:813`; an earlier full check passed |
| Web / E2E TypeScript checks | 281 / 18 diagnostics outside this task's changed files; no full typecheck pass claimed |
| Real-model coding task, fresh retry | 12 project tests passed; unchanged external acceptance passed 8 queries; 13 original files compared, only the intended 3 changed |
| Native task input/send | Actual composer and Send button; exact `P0_NATIVE_INPUT_OK` reply, successful terminal message |
| Native tool cancellation | Pause stopped the running `node board/long-check.mjs` process before its deadline; task paused; business-file hashes unchanged |
| Native upstream cancellation | Pause cancelled one dispatched request; no replay; cancelled ledger entry retained unknown usage and pricing |
| Native provider configuration | Added a synthetic custom service, synchronized and verified `json-render-cua` using explicit Chat Completions; reopened settings retained the endpoint/model; native ledger recorded the successful 2-token probe |
| Native usage UI, final snapshot | 43 attempts matched the reopened ledger: 39 succeeded, 4 cancelled; 4 incomplete usages and 1 unknown cash cost; unknown amounts were not displayed as zero |

The connector campaign exercises catalogue installation/removal, persistence after reopening, stdio, the real MCP SDK's advertised SSE endpoint and Streamable HTTP, actual tool results, and non-use after deletion. Its model oracle, tool names, expected receipts and timeouts were retained. The fixture was updated to discover the existing Agent Plane, register a resolved project, use an explicitly declared Chat protocol, launch a complete prepared bundle, and inspect the current `dynamic-tool` message contract.

This campaign uses a browser renderer attached to real native services and ACP. The browser proxy cannot invoke Tauri's runtime-component inspection; the test dismisses that browser-only error dialog through its normal close control. It does not fake successful component inspection. Native-window evidence is separate.

The real account smoke used `codex::gpt-6.1-sol` through Aster's own Agent/ACP path. No credentials or account/session files were copied. Aster created its ordinary `.aster` runtime metadata in the isolated smoke directory; the model made no tool calls or user-file edits. Initial exploratory assertions incorrectly assumed an entirely empty directory and one model call per user turn; verification was corrected against the actual runtime metadata and separate ledger records, without another model run.

The successful file task changed only `search.mjs`, `search.test.mjs` and `board/server.test.mjs`. Direct comparison confirmed case-insensitive title/tag substring matching, trimmed/empty queries, stable order and unchanged input. Its original submission used the native Agent Plane HTTP API. The independent input/send check used the native window. The task's actual tool history, completed plan, final response and three file artifacts were rendered in that window; these two submission paths are not conflated.

The existing native tests also exercise real loopback HTTP receipts for retry before output, refusal to replay after text/tool output, invalid/disabled configuration refusal before dispatch, exact protocol/model/key identity, known and unknown spend, cancellation and provider-store reopen/recovery. The provider manual check reopened the settings pane; full process-reopen persistence is covered by the existing native store tests, not claimed as a manual app-restart check.

## Commands

From the repository root, unless stated otherwise:

```sh
CARGO_TARGET_DIR=target/studio/aster RUSTC_WRAPPER= cargo test --locked -p aster-studio --lib -- model_control providers:: agent_plane::model_control --nocapture
CARGO_TARGET_DIR=target/studio/aster RUSTC_WRAPPER= cargo test --locked -p aster-core --lib mcp::transport::http_sse::tests -- --nocapture
CARGO_TARGET_DIR=target/studio/aster RUSTC_WRAPPER= cargo test --locked -p aster-core --test test_mcp_notification_delivery -- --nocapture
target/studio/aster/debug/deps/aster_studio_lib-6b271dfacea096af model_accounting:: --nocapture
target/studio/aster/debug/deps/aster_core-a0c5195b46df798b sse::tests:: --nocapture
CARGO_TARGET_DIR=target/studio/aster RUSTC_WRAPPER= cargo clippy --locked -p aster-core -p aster-studio --all-targets --no-deps
python3.13 tests/workspace_dependencies.py
cargo fmt --all -- --check
```

From `apps/studio`:

```sh
node_modules/.bin/vitest run --project renderer src/renderer/pages/settings/ProviderSettings src/renderer/pages/settings/ModelSettings/__tests__ src/renderer/pages/settings/UsageSettings/__tests__/LocalUsageSettings.test.tsx src/renderer/shell/studioMainBridge.test.ts src/renderer/utils/__tests__/aiGeneration.test.ts src/renderer/services/aiTransport/__tests__/ExecutionStreamOverlayService.test.ts src/renderer/hooks/__tests__/useExecutionOverlay.test.ts
CARGO_TARGET_DIR=/Users/roylin/code/aster/target/studio/p0-stability-e2e node_modules/.bin/playwright test --config playwright.connectors.config.ts
node --max-old-space-size=8192 node_modules/typescript/bin/tsc --noEmit -p tsconfig.web.json --composite false
node node_modules/typescript/bin/tsc --noEmit -p tsconfig.e2e.json
```

The desktop and its ACP, Office and sandbox companions were built through `just studio aster` with the existing `CARGO_TARGET_DIR` override and isolated `ASTER_STUDIO_INSTANCE=p0-e2e-20261103`. A dedicated cache was used after repeated shared-target lock contention. No worktree or repository was created. No staging or commit was performed by this task.

## Retained evidence and remaining work

Private logs, preimages, source fingerprints, model replies and ledger verification: `apps/studio/.studio/cache/p0-stability-e2e/`. Private connector service receipts: `work/connector-e2e-*`. Source inside the private Studio submodule is not copied into this parent evidence directory.

The first owned native process was PID 98249, window `w-96531`; agent-desktop session `run-1793681360660-32030-0`. The retry used PID 53881, window `w-115194`, session `run-1793687755438-51172-0`, and the same isolated instance `p0-e2e-20261103`. The user authorized foreground input on the independent test window only. Input-target failures and unexpected navigation were retained as automation observations without assigning an unproved cause. Delivered text and terminal results were checked against actual snapshots and native records; failed input attempts were not counted as successful actions.

Retry evidence lives under `apps/studio/.studio/cache/p0-stability-e2e/retry-1/`: project/external test logs, original/final source hashes, artifact comparison, actual messages, probe and cancellation ledger records, native snapshots and scoped screenshots. The original interrupted fixture remains retained separately. Original campaign inputs and external graders were not edited. Production and test source fingerprints were unchanged during this acceptance continuation.

Temporary model and MCP services started for the manual campaign were stopped. Connector campaign fixtures clean up their own processes and isolated profiles. Other sessions' processes and changes were preserved.

No further feature development was added during the acceptance continuation. Temporary native/renderer/model processes belonging to this campaign are closed after verification; unrelated processes and user configuration remain untouched. The private isolated profile and evidence remain available for diagnosis. Follow-up work is recorded in the [remaining roadmap](../../../../specs/030-magpie-compatible-local-gateway/ROADMAP.md).

## Explicitly not claimed

- Successful completion of the first interrupted coding attempt, an all-native-window submission of the file task, a manual full-app restart check, or the entire long-horizon coding campaign.
- Full Magpie parity, third-party harness hosting, paid-provider interoperability across the entire catalogue, or complete MCP protocol conformance.
- A clean whole-workspace format/typecheck, zero existing Clippy warnings, Windows/Linux runtime validation, performance improvements, or release/archive certification.
