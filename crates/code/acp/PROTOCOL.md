# aster-acp protocol entities

Authority for the names the ACP wire uses between a host (Studio, the pager) and `aster-acp`/Core. ACP v1 stays the protocol; Aster adds a closed set of `aster/…` extensions. This file adds no wire and no bus. Rename a name below only together with `tests/fixtures/acp_wire_surface_v1.json`.

## Entities

| Name | Wire / code identity | Meaning |
| --- | --- | --- |
| Session | ACP `sessionId`; Core `AgentSession` (`coreSessionId` in run observations) | Conversation owned by one workspace. Owns one turn queue. |
| Prompt | `session/prompt`; optional `_meta.promptId` | One user submission. `promptId` is the client's identity for it; the response and `_aster/queue/changed` echo it. |
| Turn | one `session/prompt` request until its `stopReason` | ACP unit of work. At most one running per session. Not the same as a Core model turn. |
| Run | Core `RunHandle`, `runId`, `RunStatus` | Core's admitted execution of an ordinary model Turn. Observed through `_aster/execution/observe` (opt-in by `_meta.asterExecutionObservation = 1`). |
| Model turn | `AgentEvent::TurnStart/TurnEnd`, observation `turn` | One model call inside a Run. A Turn contains one or more. |
| Subagent task | `task` tool; wire `subagent_id`; `_aster/subagent/cancel` | Child execution spawned by a Turn. Aster has no top-level protocol "Task". |

Not entities on this wire: Codex `Op`/`Submission`/`Event` envelopes, SQ/EQ mailboxes, Core `queue::ExternalTask` (SDK handler payload).

## Queue semantics

There are two queues; only the first is visible to hosts.

1. **Session turn queue** (`TurnQueue` in `src/agent.rs`). FIFO per session, one running Turn. A `session/prompt` that arrives during a running Turn waits (it is not rejected). More than `MAX_QUEUED_PROMPTS` (32) waiting is `invalid_params`. When a Turn leaves the queue and starts, the agent sends `_aster/queue/changed` (`sessionId`, `runningPromptId`, `runningText`, `runningKind = "prompt"`) before that Turn's first model output update (agent_message_chunk, agent_thought_chunk or tool_call); mode and command updates may come earlier.
   - `session/cancel` targets the running Turn only (optionally pinned with `_meta.promptId`). A queued prompt survives and starts next.
   - `_meta.sendNow = true` cancels the running Turn first, then queues behind it.
   - `_aster/interject` steers the running Run; it does not enter the queue.
2. **Core lanes** (`aster_core::queue::SessionLane`: Control, Query, Execute, Generate). Core-internal tool scheduling and HITL defaults. Never on the ACP wire; a host must not depend on lane names.

## Wire naming

- Standard ACP methods keep their ACP names (`initialize`, `session/new`, `session/load`, `session/prompt`, `session/cancel`, `session/request_permission`, `session/update`).
- Aster extensions are written `aster/<area>/<verb>` in Rust. The ACP SDK prepends `_` on the wire, so `aster/queue/changed` is `_aster/queue/changed`. Host methods are the closed set in `src/agent/host_methods.rs`; anything else is `-32601`.
- Known seam: `run_observation::METHOD` is already `_aster/execution/observe`, so the wire method is `__aster/execution/observe`. Studio accepts both. Do not change it without a coordinated host change.
- Worktree inspection is exposed as `aster/git/worktree/list`, garbage collection as `aster/git/worktree/gc` (`dryRun = true` selects a preview), and the read-only database location as `aster/git/worktree/db/path`. The pager's delete command performs the confirmed deletion locally after `gc`; there is no ACP delete method.
- A shape change to a `…/v1` method is a new `…/v2` method, not an edit.

## Initialize feature-switch admission (P7)

`initialize` response `_meta` projects mechanism governance (not a remote rollout %):

| Key | Shape | Meaning |
| --- | --- | --- |
| `featureSwitchRegistryDigest` | `sha256:…` | Digest of the Core feature-switch registry rows |
| `featureSwitches` | `{ id: bool }` | Admission map: `Removed` → `false`; other known stages → `true`; unknown names are absent |
| `featureSwitchStages` | `{ id: string }` | Stage string per id (`stable` / `experimental` / `removed` / …). Hosts that need Stable-vs-Experimental must read this map — do not treat `featureSwitches[id]=true` as Stable |

`tools.parallel_task` is `Removed` (`enabled=false`, stage `removed`).
`tools.patch_codes` may still appear `enabled=true` while stage is `experimental` —
production call sites must use Core `feature_switch_call_site_allowed` (Stable/Deprecated only);
dynamic tool registration refuses Experimental/Removed names.

## Gate

- `tests/fixtures/acp_wire_surface_v1.json` is the golden surface.
- `agent::host_methods_tests::host_method_wire_names_match_the_golden_surface` fails on a renamed, removed or unlisted host method.
- `tests/acp_conformance_stdio.rs` cases `queue-running-identity` and `turn-queue-order` run the shipped binary and compare `_aster/queue/changed` keys and queue behavior with the golden.
- Core-side Run/event wire is gated separately by `crates/code/core/tests/{agent_protocol_v1,event_protocol_v1}.rs`.
