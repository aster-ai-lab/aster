---
description: >-
  End-to-end chat turn flow from renderer IPC transport through AiStreamManager
  and Agent loop to persistence
sources:
  - src/renderer/services/aiTransport
---

# Core Architecture

End-to-end view of how a Cherry chat turn moves from user input to LLM
response and back to UI, with pointers to the focused references for
each subsystem.

## Layered view

## Sequence: a fresh chat turn

1. User hits send. `useChat.sendMessages` calls `IpcChatTransport.sendMessages`.
2. Transport packages `AiStreamOpenRequest`, dispatches via
 `streamDispatchService` over IpcApi `ai.stream.open`.
3. The `ai.stream.open` handler in resolves the
 managed sender window, wraps it in a `WebContentsListener`, and calls
 `AiStreamManager.dispatch`.
4. `dispatchStreamRequest` picks the first `ChatContextProvider` whose
 `canHandle(topicId)` matches and asks it to `prepareDispatch`.
5. The provider resolves models, persists the user message for persistent chat
 or skips it for temporary chat, creates `PersistenceListener` per
 execution, returns `PreparedDispatch`.
6. `dispatch` reconciles any live stream, then calls `manager.send(input)`:
 - **chat resubmit** (topic already streaming): the provider persists the
 steer user row and `dispatch` calls `manager.enqueuePendingSteer(topicId)`;
 `send` **injects** (just upserts the subscriber). The running turn yields
 via `steerYield` (persisting as `success`) and `onExecutionDone` chains a
 `steer-continuation` — steering is enqueue + yield + chain, not
 abort-and-restart and not mid-turn injection.
 - **agent-session follow-up**: the stream is left running and `send`
 **injects** — it upserts `listeners` onto the running stream, `models`
 ignored (the message was already enqueued on the session's `pendingTurns`).
 - **no live stream**: `send` **starts** — evict any grace-period stream,
 create an `ActiveStream`, launch one `StreamExecution` per model.
7. For each `StreamExecution`, `AiStreamManager`'s private `runExecutionLoop`
 calls `AiService.streamText(request, signal)`, which builds params
 (`buildAgentParamsFor → buildAgentParams`: `registry.selectActive` +
 `applyDeferExposition` + per-feature hooks), constructs an `Agent`
 (`composeHooks` folds observers + caller + features inside `Agent`), and
 calls `agent.stream(messages, signal)` — which opens AI SDK's stream and
 yields `UIMessageChunk`s. Agent-session runtime requests skip the generic
 agent loop here: `AiService.streamText` calls
 `AgentSessionRuntimeService.openTurnStream` so the registered driver
 can own the concrete agent runtime.
8. `pipeStreamLoop` reads the chunk stream once, tees: broadcast to
 listeners, accumulate via `readUIMessageStream`.
9. On terminal (`done` / `error` / `aborted` / `awaiting-approval`):
 - `PersistenceListener` writes the final assistant message.
 - `WebContentsListener` sends `ai.stream.done` to subscribed windows.
 - Shared-cache `topic.stream.statuses.<topicId>` flips to the terminal status.
10. Renderer's `useQuery('/topics/:topicId/messages')` revalidates; the
 optimistic overlay is disposed.

## Sequence: tool approval pause + resume

1. AI SDK calls `tool.execute(args, toolCallContext)`. The wrapper sees
 `needsApproval(args)` returns true and the assistant's auto-approve
 policy says "ask". It writes an `approval-requested` part on the
 accumulated message and holds the promise.
2. Manager flips status to `awaiting-approval` on the shared cache.
3. Renderer's `useTopicAwaitingApproval(topicId)` returns true; the UI
 shows the approval card.
4. User decides → `useToolApprovalBridge` → `ai.tool.respond_approval`.
5. Main applies the decision to the anchor row, resumes the stream
 (agent-session runtime: resolves the live approval registry entry; MCP:
 dispatches a `continue-conversation` so the existing stream rebroadcasts).
6. Status flips back to `streaming`; UI hides the card.

See [Tool Approval](./tool-approval.md) for invariants and the
overlay-vs-persist conditional write.

## Key subsystems

| Subsystem | Reference |
|---|---|
| Active-stream registry, listeners, persistence backends, reconnect, abort, grace-period eviction | Stream Manager |
| Agent-session host plus Claude Code, Pi, and DSH runtime drivers | [Agent Session Runtime](./agent-session-runtime.md) |
| `Agent.stream` single-pass loop, hooks model, error/abort | Agent Loop |
| `buildAgentParams`, `RequestFeature` composition, `INTERNAL_FEATURES` order | Params Pipeline |
| Tool registry, MCP sync, meta-tools (`tool_search` / `tool_inspect` / `tool_invoke` / `tool_exec`), defer exposition | Tool Registry |
| `Provider.endpointConfigs`, `endpointType` resolution, variant suffixes, custom providers | [Provider Resolution](./provider-resolution.md) |
| `adapterFamily` field, runtime resolver, write paths (catalog / migrator) | [Adapter Family](./adapter-family.md) |
| OTel span tree, `AdapterTracer`, `AiSdkSpanAdapter`, dev-tools view | Observability |
| `IpcChatTransport`, dispatch service, per-execution demux | [IPC Transport](./ipc-transport.md) |
| Approval flow, Main-as-writer invariant, persistent decisions | [Tool Approval](./tool-approval.md) |

## Invariants

- **Topic-level addressing.** Every IPC, broadcast, and shared-cache
 entry is keyed by `topicId`. A topic has at most one active stream;
 subscribers are equal — there is no "owner" window.
- **Main owns persistence.** Renderer closing or crashing does not abort
 the stream or lose data. `PersistenceListener` writes on terminal
 regardless of subscriber state.
- **Main owns approval state.** The renderer is never a writer.
- **Adapter family is per-endpoint.** Multi-endpoint relays may use
 different `@ai-sdk/*` packages on different endpoints under the same
 `provider.id`.
- **`tools/applies` predicates are pure.** They run on every
 `selectActive` pass; side effects there break tool selection
 determinism.
- **Features must not mutate `RequestScope`.** It is shared across all
 features for a single request.

## Code map

