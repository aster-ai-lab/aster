# Research: Studio single runtime

## Decisions and in-repo prior art

### Core owns facts, ACP owns protocol, Studio owns presentation

Core already owns Session snapshots, fact logs, Runs, Goals, Plans, Review findings and permission facts (`crates/code/core/src/store`, `agent_api`, `session_review`). Studio's `agent_plane/store.rs` duplicates parts of that authority. The convergence therefore moves Studio toward existing Core APIs rather than creating another host runtime.

### Share one admitted-Run attempt driver

Interactive `agent/turn.rs` and background `agent/unattended_turn.rs` already share `handle_event` and cancellation helpers but duplicate receive/join loops. The first execution slice extracts only the admitted `AgentRunStream` driver. Goal admission, budgets and result reporting remain behavior-specific callers of that same primitive; no third execution engine is added.

### Plan uses the existing Core task state

`update_plan` already produces Core task state and ACP `SessionUpdate::Plan`. The missing resume piece is a read-only Core plan snapshot and ACP load publication. Studio consumes that standard event and does not parse TodoWrite rawInput for new events.

### RuntimeClient is a facade, not a state store

Existing raw calls span turn setup, model/mode, Goal state, wiki and memory. `runtime_client.rs` centralizes those commands while `acp.rs` retains JSON-RPC framing and process transport. The facade never writes PlaneStore or owns sessions/messages/Goals.

### Permission has one decision owner

Core already emits ConfirmationRequired after its policy decision. Studio's `permission_act` is a second decision based on tool titles/rawInput and is removed. UI remains a broker for Ask answers. Any behavior that still requires automatic admission must first become an explicit Core policy, not a Studio exception.

### Calendar is not the existing Core capacity scheduler

Core `task_scheduler` handles capacity/priority, not cron. Durable calendar due/fire semantics require a separate wall-clock schedule API that feeds the same execution driver. It must not be confused with or wrapped around Studio's existing `turns.open` loop.

### Review migration is more than Finding storage

Core session review already owns Finding lifecycle. Studio also has proof/artifact segmentation and an independent HTTP model reviewer. The vertical Review slice must migrate execution and evidence before deleting that HTTP loop; merely wrapping Finding storage would leave two runtimes.

## Rejected alternatives

- **Keep Studio substitutes forever**: rejected because it preserves two authorities and recovery semantics.
- **Dual-write during migration**: rejected because divergence and uncertain retries cannot be reconciled safely.
- **Add a third orchestration service**: rejected; the missing boundaries are added to existing Core/ACP owners.
- **One global ACP process**: rejected because tenant/workspace credential and state isolation still requires separate runtime cells.
- **Infer task type or permissions from prompt/tool names**: rejected by closed-set and meaning-is-not-matched requirements.

## Current implementation constraints

The parent and Studio worktrees contain staged, unstaged and untracked work from other efforts. Implementation uses exact incremental edits and focused tests. It does not reset, clean, stage wholesale or claim unrelated changes as part of this epic.

## Implementation findings

- `TaskUpdated` had two Core publication paths. `conversation_runtime/host_events.rs` and the owned-fact tool bridge could record a Run event without applying `SessionPersistenceState.tasks`. The fix shares the existing `SessionPersistenceContext` across RuntimeEventSink and owned-fact surfaces; `plan_snapshot` never falls back to RunStore.
- Interactive and background ACP entry points now share `agent/attempt.rs` for admitted-Run receive/cancel/deadline/join/terminal handling. Goal admission, host budgets and outcome mapping remain caller policies rather than another receive loop.
- Studio `RuntimeClient` is the only production A3S command facade. It owns no Session/Message/Goal/Task/Run state; raw transport remains in `acp.rs`.
- Studio no longer pre-decides Core permission requests. Run-admission posture remains Core/ACP policy, while every actual Ask goes to the same approval broker and times out/cancels closed.
- Dynamic catalogs are ACP generation projections, not persisted Studio catalogs. `LaunchGeneration` binds Studio and ACP model IDs explicitly. First-draft prewarm creates a real runtime/session without a prompt or execution facts; no static model/command/tool fallback is allowed.
- Core transcript pages preserve original fact history and expose strict source cuts. Prefix forks use validated turn.complete boundaries, not Studio message positions. ACP restore requires an actual snapshot and never falls back to creating an empty same-ID session.
