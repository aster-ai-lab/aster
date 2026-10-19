---
description: >-
  Entry point mapping the AI pipeline docs, code layout, chat-turn flow,
  runtimes, and key invariants
sources:
  - src/renderer/services/aiTransport
---

# AI Reference

This is the entry point for Cherry Studio's AI pipeline: main-process provider
calls, AI SDK chat execution, registered agent-session runtimes, and the
renderer-side transport that connects to them.

## Quick navigation

### Top-level architecture

| Document | What it covers |
|---|---|
| [Core Architecture](./core-architecture.md) | End-to-end call flow: `ai.stream.open` IpcApi route → context provider → AiStreamManager → runtime → broadcast / persist |
| Stream Manager | Active-stream registry, listeners, reconnect, abort, queue/yield/continuation steering, persistence backends |
| [Agent Session Runtime](./agent-session-runtime.md) | Agent-session host/driver split, follow-up admission, resume persistence, and the registered Claude Code, Pi, and DSH drivers |
| [Agent Session Fork](./agent-session-fork.md) | Native fork behavior, service ownership, opaque checkpoints, workspace handling, publication, and recovery |
| Agent Lifecycle | Archive, restore, purge, schedule recovery, ownership boundaries, and Agent-side backup quiescing |
| [Remote Agent API Design](./remote-agent-access.md) | Target network and package APIs: device-level authorization, complete incremental events, receipts, and weak-network recovery |
| Remote Agent Sequences and Modules | Target module map, connection states, and thirteen normal/failure sequence diagrams |
| Remote Protocol and Desktop Implementation Design | Proposed package/Desktop files, function contracts, atomic admission, recovery, and lifecycle ownership |
| [Remote Agent Testing Specification](./remote-agent-testing.md) | Local WebSocket client acceptance, scenario matrix, crash injection, weak-network measurements, and test rules |
| [Adding an Agent Runtime](./adding-a-runtime.md) | Operational checklist for a new runtime: capability descriptor, driver package, registration points, design rules |
| [Adapter Family](./adapter-family.md) | How `provider.endpointConfigs[ep].adapterFamily` picks the right `@ai-sdk/*` package per request |
| [Provider State Ownership](./provider-state-ownership.md) | Where provider facts, endpoint dialects, connection overrides, and per-request controls belong |

### Subsystems

| Document | What it covers |
|---|---|
| Agent Loop | Main-process `Agent.stream`: single-pass stream, hook composition, observer pattern, error/abort semantics |
| Agent Prompt Layers | Agent System Prompt, workspace `system.md`, `SOUL.md`, precedence, update boundary, and variable lifecycle |
| Params Pipeline | `buildAgentParams` + `RequestFeature` model: how capabilities, plugins, tools, and provider-specific quirks are composed |
| Tool Registry | Built-in web/knowledge/file/image/MCP-resource tools, selected MCP tools, meta-tools, and deferred exposition |
| [Chat Attachments](./chat-attachments.md) | How attached files reach the model: native file parts when supported, capped extracted text otherwise, `read_file` for overflow paging |
| [Provider Resolution](./provider-resolution.md) | `Provider.endpointConfigs` schema, endpoint resolution chain, variant suffixes, custom provider extensions (aihubmix, newapi) |
| [Model Retry & Fallback](./model-retry.md) | `ai-retry` integration: same-model transient retry + user-configured fallback models, `wrapModel` hook, `chat.retry.*` preferences, embedding/rerank policies |
| Observability (trace / telemetry) | `AiSdkSpanAdapter`, root span propagation, OTel attribute shape, local span projection, sinks |
| [Local Models](./local-models.md) | The local embedding / OCR bundle catalog, on-disk registry, and verified acquisition of models and shared native runtimes |
| AI Usage Records | Best-effort per-provider-invocation usage/cost analytics: capture ownership, immutable attribution snapshots, message projection, bounded query API, migration, freshness |
| Browser Use Design | Browser automation ownership, capability gaps, and delivery roadmap |
| Browser Use Implementation | Session engine, MCP contracts, and implementation plan |

### Renderer-side glue

| Document | What it covers |
|---|---|
| [IPC Transport](./ipc-transport.md) | `useChat` + `IpcChatTransport`: `sendMessages` / `reconnectToStream`, dispatch service, topic-status mirror |
| [Execution Overlay](./execution-overlay.md) | `TopicStreamSubscription` + `useExecutionOverlay`: ref-counted attach, execution + anchor demux, one-shot `readUIMessageStream` per turn (the renderer half of the same merge function Main uses) |
| [Text Translation](./translation.md) | `translate.open` prompt streams, renderer-owned result handling, and Home `data-translation` persistence |
| [Tool Approval](./tool-approval.md) | Approval registry, Main-as-writer model, persistent decisions, `useToolApproval` hook |

## Where the code lives

> **Scope of the focused docs.** The reference documents in this folder map
> the **chat / stream pipeline** (dispatch → stream manager → runtime →
> tools → persistence → renderer transport). The `channels/`, `skills/`, and
> `mcp/` subsystems are mapped in the tree below but do not yet have dedicated
> deep-dive docs.

## How a chat turn flows

1. Renderer `useChat({ transport: IpcChatTransport })` calls `sendMessages` →
 IpcApi `ai.stream.open` (`{ topicId, trigger, userMessageParts,
 parentAnchorId?, mentionedModelIds? }`).
2. The thin handler in resolves the caller's
 `WebContents`, wraps it in a `WebContentsListener`, and delegates to
 `AiStreamManager.dispatch`. Stream state stays in the manager; transport
 registration stays in IpcApi.
3. `dispatchStreamRequest` picks the first `ChatContextProvider` whose
 `canHandle(topicId)` matches (persistent chat / temporary / agent
 session) and calls `prepareDispatch` — that resolves models, persists
 the user message, builds listeners, and returns a `PreparedDispatch`.
4. `AiStreamManager.send(input)` **starts** a turn (no active stream): creates
 an `ActiveStream`, launches one `StreamExecution` per model. (A chat
 resubmit on a live topic is persisted + queued as a steer and takes the
 **inject** path — the running turn yields and `onExecutionDone` chains a
 continuation; an agent-session follow-up also injects, upserting listeners.)
5. Each execution's `runExecutionLoop` calls `AiService.streamText(request,
 signal)`, which builds params (`buildAgentParams`) and constructs an `Agent`
 composing hooks from `RequestFeature[]` (anthropic cache, gateway usage
 normalisation, reasoning extraction, …), then calls `agent.stream(messages,
 signal)` to open the AI SDK stream and yield `UIMessageChunk`s.
 Agent-session runtime requests are the exception: `AiService.streamText`
 routes them to `AgentSessionRuntimeService.openTurnStream` so the
 registered driver can own the concrete agent runtime.
6. `pipeStreamLoop` tees the chunk stream: one branch broadcasts to listeners
 (WebContents / SSE / channel-adapter / persistence), one branch runs
 `readUIMessageStream` to accumulate a `CherryUIMessage` snapshot.
7. On terminal (done / error / aborted / paused-for-approval), listeners get
 a typed terminal callback. `PersistenceListener` writes the final
 message via the appropriate `PersistenceBackend`.
8. Renderer reads the persisted row through `useQuery('/topics/:topicId/messages')`
 and disposes its overlay.

## Key invariants

- **Topic-level addressing.** Every IPC and broadcast is keyed by `topicId`.
 A topic has at most one active stream; subscribers are equal — there's no
 "owner" window.
- **Main owns persistence.** Renderer closing or crashing does not abort the
 stream and does not lose data — `PersistenceListener` writes on terminal
 regardless of who is listening.
- **Tool approval is Main-authoritative.** The renderer never writes
 `approved`/`denied` parts. It posts the decision over IPC and re-reads the
 authoritative row. See [Tool Approval](./tool-approval.md).
- **Adapter family per endpoint, not per provider.** Multi-endpoint relays
 (MiniMax, Silicon, AiHubMix, …) carry one `adapterFamily` per endpoint.
 Picking the SDK package never reads `apiHost` or provider id heuristics
 at request time. See [Adapter Family](./adapter-family.md).
- **One provider fact, one owner.** Host facts live on registry providers,
 protocol deviations on endpoint configs, user connection deltas on provider
 rows, and per-request choices on assistants. See
 [Provider State Ownership](./provider-state-ownership.md).

## Related references

- Service Lifecycle — `AiService` extends `BaseService`
- [Data Layer](../data/README.md) — `MessageService`, `ModelService`,
 `ProviderService` (called from main-side AI code)
- Window Manager — `WebContentsListener`
 attaches to whatever windows are open
