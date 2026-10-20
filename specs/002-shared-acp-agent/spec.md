# Feature Specification: Shared ACP agent

**Feature Branch**: `002-shared-acp-agent` (no branch requested or created)

**Created**: 2026-10-20

**Status**: Draft

**Input**: Align Aster TUI product entries with the `a3s-code-acp` process legacy AsterDream and the shared desktop host already start.

**Gate context**: Product path `apps/aster` and `crates/terminal` pager. The desktop hosts already start `a3s-code-acp`. This epic moves the remaining product entries onto that process. It has no roadmap row; the route is the approved shared-agent plan.

## User Scenarios & Testing

### User Story 1 - One agent process (Priority: P1)

A person runs headless, `aster agent stdio`, `models`, or `worktree`. Each command starts the same `a3s-code-acp` executable the interactive pager starts when `A3S_ACP_AGENT_BIN` is set or the sibling executable is present.

**Why this priority**: Two agent implementations diverge on tools, permissions, and model configuration.

**Independent Test**: `a3s-code-pager` unit test `headless_missing_sidecar_uses_the_interactive_error` calls `spawn_agent` with the sidecar absent and compares the error text to the interactive constant.

**Acceptance Scenarios**:

1. **Given** the sidecar executable is present, **When** headless, stdio, `models`, or `worktree` starts, **Then** the process started is `a3s-code-acp`.
2. **Given** the sidecar executable is absent, **When** any of those entries starts, **Then** it returns the same error text the interactive pager already returns.
3. **Given** the switches have landed, **When** the boundary test runs, **Then** headless, `models`, and `worktree` are not shell-spawn call sites, and stdio does not call `run_stdio_agent`.

### User Story 2 - Initialize advertisements stay advisory (Priority: P2)

The interactive client still sends its initialize capability advertisements. A single ordinary prompt does not send an `x.ai/` method. The shared agent is not required to implement those advertisements for the prompt to finish.

**Why this priority**: Treating a similarly named method as implemented would hide a real gap.

**Independent Test**: `initialize_xai_advertisements_are_the_closed_set` for a leader connection, `sidecar_initialize_omits_xai_keys` for a sidecar connection, and `ordinary_prompt_meta_sends_no_xai_extension` in `a3s-code-pager`.

**Acceptance Scenarios**:

1. **Given** default connect flags and a leader connection, **When** initialize capabilities are built, **Then** the `x.ai/` keys are exactly `x.ai/bashOutputNoColor`, `x.ai/gitHeadChanged`, `x.ai/hunkTracker`, `x.ai/incrementalBashOutput`, `x.ai/statusLine`, and `x.ai/userMessageEcho`.
2. **Given** default connect flags and an ACP sidecar connection, **When** initialize capabilities are built, **Then** no key starts with `x.ai/`.
3. **Given** an ordinary text prompt, **When** its request meta is built, **Then** no key starts with `x.ai/`.

### Edge Cases

- When `A3S_ACP_AGENT_BIN` is set to an empty value, a missing path, or a Unix file without an execute bit, AsterDream, AsterDream, and the Aster TUI reject that path and do not start another `a3s-code-acp` found as a sibling, a bundled resource, under `~/.a3s/bin`, or on `PATH`. The TUI returns the interactive missing-backend sentence.
- When `A3S_ACP_AGENT_BIN` names an executable file, that file is the one selected.
- When `A3S_ACP_AGENT_BIN` is unset, the TUI accepts only an executable sibling. The desktop hosts keep their current search order and their current not-found sentence.
- A desktop executable outside a Cargo `target/` directory does not prefer the compile-time `target/debug/a3s-code-acp` over a bundled agent. An executable under `target/` still prefers the workspace debug binary when that file exists.
- An interactive TUI that asked for leader mode and fails to connect reports that failure and does not start the ACP sidecar. `aster agent leader` and `aster agent serve` stay on the in-process agent.
- The leader cluster keeps its in-process agent. This epic does not start it through the sidecar.

## Requirements

### Functional Requirements

- **FR-001**: Headless, `aster agent stdio`, `models`, and `worktree` MUST call the sidecar only after the same presence check the interactive pager uses.
- **FR-002**: When that check finds no executable, each of those entries MUST fail closed with the interactive error text. They MUST NOT call `spawn_grok_shell` or `run_stdio_agent`.
- **FR-003**: The switch order is headless, then stdio, then `models` and `worktree` together.
- **FR-004**: Initialize sent to the ACP sidecar omits client-capability keys that start with `x.ai/`. Initialize sent to the leader advertises exactly `x.ai/bashOutputNoColor`, `x.ai/gitHeadChanged`, `x.ai/hunkTracker`, `x.ai/incrementalBashOutput`, `x.ai/statusLine`, and `x.ai/userMessageEcho`. The shared agent does not implement those keys. The client does not refuse initialize or one ordinary prompt when the agent omits them.
- **FR-005**: No new ACL or TOML setting is added. `A3S_ACP_AGENT_BIN` stays an environment override.

### Key Entities

- **Sidecar**: The `a3s-code-acp` executable resolved by `a3s_acp_executable`.
- **Interactive error text**: The sentence owned by `MISSING_AGENT_BACKEND`.

## House Checklist

- [x] New behavior has a small hermetic test in the owning package
- [x] Closed sets: the initialize advertisement names are asserted as a set
- [x] No production `unwrap()` / `expect()` without an infallibility comment
- [x] Internal `a3s-*` dependencies stay on a path in this repository
- [x] Product configuration, if any, is ACL parsed by `a3s-acl`
- [x] Evidence target: `docs/evidence/development/2026-10-20-shared-acp-agent/OUTCOME.md` for the shared process, and `docs/evidence/development/2026-10-20-acp-lookup-contract/OUTCOME.md` for the lookup, leader-failure, and advertisement split.

## Success Criteria

### Measurable Outcomes

- **SC-001**: `python3 -m unittest tests.agent_runtime_boundary -v` passes with an empty product shell-spawn file set.
- **SC-002**: The headless missing-sidecar test passes under `cargo test --locked --offline -p a3s-code-pager`.
- **SC-003**: Two consecutive live prompts to the shipped `a3s-code-acp` binary against `deepseek/deepseek-flash` from the repo `.a3s/config.acl` official DeepSeek provider each return a non-empty assistant reply and usage tokens greater than zero within 180 seconds, when that ACL and the network are present.

## Assumptions

- The interactive pager path already starts the sidecar when the executable is present. This epic reuses that check.
- An ordinary prompt is `Effect::SendPrompt`, whose meta is `promptId` and an optional `screenMode`.
- Session commands other than initialize and that one prompt are outside this inventory.

## Explicitly Not Claimed

- The leader cluster moves off the in-process agent.
- `event_loop.rs` is split or its inline tests move.
- AsterDream product surfaces are copied into the TUI.
- Crate names, versions, editions, or licenses are unified.
- The OCI sandbox default changes.
- A scripted conformance model counts as the live LLM run.
