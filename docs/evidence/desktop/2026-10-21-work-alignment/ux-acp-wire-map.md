# Native stdio / pinned ACP mechanism map

Read-only exhaustive inventory of production ACP calls and inbound dispatch in Desktop `agent_plane/{acp,turns,reasoning_effort,goal_snapshot,execution_observation,document_read}.rs`, cross-checked against workspace ACP producers/handlers and the pinned `agent-client-protocol` 0.10.4 / schema 0.11.4 source. Final adapter/session/response corrections are source-reviewed and their maintained negatives plus actual workspace companion integration passed. Final packaged GUI/platform acceptance remains separate; see [final independent review](ux-native-independent-review.md) and its source receipt.

## One encoding boundary

The library's AgentSideConnection and ClientSideConnection `ext_method` and `ext_notification` encode exactly `format!("_{}", args.method)`. Schema decoding strips exactly one initial underscore and preserves the request/notification enum kind. Standard method names are unchanged. Desktop's handwritten AcpClient sends raw JSON and previously forwarded incoming raw notification names unchanged.

The minimal adapter correction is **incoming notifications only**: keep the standard `session/update`; for valid extension notifications remove exactly one underscore; preserve remaining underscores; never recursively trim; ignore invalid unprefixed custom notifications; do not allow `_session/update` to become the standard notification when the enum kind has been erased. Incoming requests remain raw because Desktop's existing request dispatch deliberately matches `_a3s/...` names. Outgoing raw extension requests must already include the single transport underscore.

## Host → agent (production inventory)

| Native raw method | Library/internal target | Payload / result boundary | Disposition |
| --- | --- | --- | --- |
| `initialize` request | Standard initialize | protocolVersion 1, client capabilities/info; current code records/does not otherwise consume capabilities | Correct standard spelling. Handshake is bounded. |
| `session/new` request | Standard new session | cwd, MCP servers, typed host/plugin/component metadata; requires nonempty result.sessionId | Correct. Also creates isolated review worker sessions on the same process. |
| `session/set_mode` request | Standard set mode | current ACP sessionId, closed modeId; cache changes only after successful response | Correct; incomplete matching JSON-RPC replies must not count as success. |
| `session/set_model` request | Standard set model | current sessionId, provider/model, typed reasoning-effort meta; resets posture cache | Correct. |
| `session/prompt` request | Standard prompt | exact sessionId, content blocks, promptId, observation flag, independent permissionPosture/a3sTurn | Correct; response stopReason is validated by PromptEnd. |
| `session/cancel` notification | Standard cancel | exact current/worker sessionId | Correct; acknowledgement is not settlement. |
| `_x.ai/interject` request | Internal `x.ai/interject` | exact current ACP sessionId and text | Correct single raw extension prefix. |
| `_a3s/wiki/search` request | Internal `a3s/wiki/search` | admitted wiki/query; published result sanitized | Correct single raw extension prefix. |
| `_a3s/goal/state` request | Internal `a3s/goal/state` | snake-case session_id; direct JSON object with explicit definition/tick | Previously incorrect bare name; actual companion integration exposed it. Native owner corrected raw spelling; strict peer and actual workspace companion pass typed goal projection. |

The actual integration fixture additionally uses raw `_a3s/goal/definition` and `_a3s/goal/control`, each mapping to the same internal name without the transport underscore. These are fixture calls, not newly added product write routes. Production does not currently send session/load, session/list, session/set_config_option or generic arbitrary extension calls.

## Agent → host requests (raw dispatch is retained)

| Raw method | ACP producer / identity | Native result |
| --- | --- | --- |
| `session/request_permission` | Standard RequestPermission includes sessionId; ordinary boolean permission or typed `_meta.question` | `{outcome:{outcome:'selected',optionId}}`, or cancelled. Typed question adds its existing follow-up metadata. |
| `_a3s/form/request` | Core agent uses `request_form_in_session`: internal `a3s/form/request` plus current sessionId/formId/spec/format | `{values}` or JSON-RPC error; exact live form lease required. |
| `_a3s/document/read` | Internal `a3s/document/read`; snake-case session_id/path/run binding | DocumentReader validates exact bound session, path and optional Run pin before returning typed document snapshot. |
| `_a3s/document/release_run` | Internal `a3s/document/release_run` | Bound session/Run release; late release permitted for cleanup, without a new read. |
| `_a3s/encyclopedia/search` | Internal `a3s/encyclopedia/search`; query-only read request | Current admitted organization/tenant scope supplies passages; no sessionId is in this protocol payload. |
| Everything else | No admitted host method | JSON-RPC method-not-found, without granting permission or starting a tool. |

**Closed identity gap (retained mechanism history):** ordinary permission and form dispatch did not compare incoming `params.sessionId` to `run_prompt`'s exact `acp_session`, although typed question already did. A shared process can have parent and review worker sessions. Foreign/missing scoped requests must be refused before opening UI authority; validation must use the current prompt's session argument, not unconditionally the outer runtime's parent session. Do not invent a sessionId for the query-only encyclopedia protocol. The document bridge already validates its separately typed snake-case session binding.

## Agent → host notifications

| ACP internal producer name | Actual raw method from pinned library | Adapter output / consumer |
| --- | --- | --- |
| Standard session update | `session/update` | unchanged standard name; exact sessionId checked. Text/thought/tool calls feed TurnStream; canonical rawInput retains lossless TodoWrite states. |
| `x.ai/turn/outcome` | `_x.ai/turn/outcome` | `x.ai/turn/outcome`; PromptStop checks sessionId + promptId + epoch and validates the closed forced-stop category. |
| `_a3s/execution/observe` (existing internal constant includes underscore) | `__a3s/execution/observe` | `_a3s/execution/observe`; Observer checks protocolVersion, exact session/prompt, safe sequence, Run identity and kind transitions. Do not rename the shared internal constant or pager API. |
| `x.ai/session_notification` | `_x.ai/session_notification` | `x.ai/session_notification`; Desktop has no matching consumer. Its legacy task/subagent extension data is currently intentionally ignored; it does not grant goal/plan/approval authority. Required planning comes from standard tool rawInput, not this extension. |
| Unknown valid extension | One raw prefix stripped | Unrecognized internal name ignored; cannot masquerade as a standard method. |

The two required notifications were previously ignored by the raw native client because it did not perform the library's decoding step. Bare-name fake notifications masked the mismatch. The terminal pager uses library-decoded names and must keep its current public/internal spellings.

## JSON-RPC responses and settlement

Native requests allocate unique nonnegative integer IDs; only that ID's pending callback is resolved. Standard results and transparent ExtResponse results are already direct JSON values, so there is no extra result wrapper to strip. Agent-initiated request IDs are echoed unchanged in result/error responses. Cancellation/timeout drops the pending callback; process EOF clears all pending callbacks and signals exit. Permission and form submission remain separately tied to live host ownership.

**Closed envelope gap (retained mechanism history):** a matching response with neither `result` nor `error` was accepted as `Ok(null)`. Response success must require an explicit result; exactly one result/error member is allowed. Explicit `result:null` is distinct and may be valid. Ambiguous/incomplete matching replies must fail, not update mode/config caches. A malformed frame must not be reclassified as a legitimate notification or permission request simply because a field could not be parsed. The pinned library's typed decoder also retains JSON-RPC kind; any raw adapter needs to preserve the same distinction.

PromptEnd recognizes end_turn, cancelled and the explicitly trusted read_only_evidence exception. Missing/unknown/refusal/max-token/budget outcomes refuse success. **Closed stop-outcome gap:** present unknown/non-string forced must fail; missing/null remains ordinary no-forced state. A foreign session/prompt or stale host epoch must not affect this prompt. Reconciliation errors remain failures with partial evidence, not a retry instruction.

## Negative and real gates for closure

1. Real stdio peer sends `session/update`, `_x.ai/turn/outcome` and `__a3s/execution/observe`; assert exact internal names after one decode and that actual consumers act. Send `_session/update`, bare custom notifications, extra-prefix outcome names and wrong session/prompt/epoch to prove no aliasing or cross-owner authority.
2. Matching responses: explicit result (including null), error, missing both and both present. Malformed matching envelopes must fail a pending request; unknown/late IDs must not resolve another callback.
3. One peer prompt sends foreign/missing session permission/form requests: no approval/form registration, negative response; valid current parent and worker session requests remain functional. Exact host form owner execution/attempt remains required at submit.
4. Terminal: valid forced categories, missing/null, unknown string/wrong type and contradictory notifications followed by end_turn. Only the documented explicit read-only-evidence exception may convert the corresponding stop reason to success.
5. Actual just-built workspace ACP: planning prevents attempted write, review/execution uses a real bound approval, file is written/read back, goal state is known through the raw extension, nativeExecution appears with exact Run/prompt identity, and correct terminal classification is observed. A fake-only pass cannot replace this gate.
6. After protocol closure: rebuild Desktop host/companion and renderer from the final frozen sources, then run actual GUI plan/review/approval/Stop/reconnect and typed goal workflows. Parent owns that acceptance, including the isolated organization-model mock; no credential or entitlement bypass is authorized.

All new protocol corrections should remain in the raw adapter/consumer layer and maintained tests. No shared public method rename, new UI authority fallback, model-name inference or product permission broadening is needed.

## Final closure receipt

All three previously identified raw-transport defects are fixed in the final 23-file native manifest: exactly-one notification decoding, explicit exclusive JSON-RPC responses, and current-prompt permission/form session refusal. The actual peer reads `-32602` for wrong/missing scoped identities, rather than merely observing no UI. Required nullable goal plan revision and fail-closed unknown forced outcomes are also covered. Final maintained native tests pass 28; the separately executed actual workspace companion fixture passes 1/1 with raw method trace, native Run observation, typed goals, plan-write refusal and bound approval/write/readback. The complete final evidence is [native-final-validation.json](native-final-validation.json). Remaining gate 6 belongs to the parent’s fresh rebuild and actual GUI/platform runs; this mapping does not promote it to passed.
