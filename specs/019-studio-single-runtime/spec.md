# Feature Specification: Studio single runtime and authority convergence

**Feature Branch**: current branch; no new branch or commit requested

**Created**: 2026-10-25

**Status**: Implementation

**Input**: 根据开发规范完成架构统一和优化；规划整齐，不保留多套相同机制和运行时。

**Gate context**: Product regime. Core/ACP own execution and facts; the private Studio submodule owns desktop presentation and host devices. No second Cargo workspace, product configuration language, foreign SDK, or repository.

## User Scenarios & Testing

### User Story 1 - One execution and event path (Priority: P0)

Interactive prompts, background work, Goal continuation, permissions, Plan and Review use the same Core execution facts and ACP event protocol. Studio renders those facts instead of maintaining parallel executions.

**Why this priority**: Duplicate drivers and ledgers make cancellation, recovery and authorization disagree.

**Independent Test**: ACP owning-package tests drive the same admitted Run through interactive and background profiles; Studio hermetic tests prove one transport facade and one typed projection with no second completion decision.

**Acceptance Scenarios**:

1. **Given** the same Core Run, **When** interactive or background attendance drives it, **Then** event handling, cancellation, terminal settlement and fact identity come from one implementation.
2. **Given** a Core confirmation request, **When** Studio presents it, **Then** Studio only transports the user's answer and never pre-decides Allow/Deny.
3. **Given** a Core Plan revision, **When** Studio renders it, **Then** no TodoWrite rawInput or tool-output channel is consulted.

### User Story 2 - One recoverable session history (Priority: P0)

A user reopens, forks or changes model/MCP on a session without splitting its Core history or duplicating messages.

**Why this priority**: UI history and Core history must not become independent facts.

**Independent Test**: Core/ACP session boundary tests and Studio store tests prove one writer, stable identities, load reconciliation and no fallback to the retired store.

**Acceptance Scenarios**:

1. **Given** a persisted Core session, **When** the runtime restarts, **Then** ACP load restores the same identity and Studio replays each logical event once.
2. **Given** a load failure or foreign workspace, **When** recovery runs, **Then** it refuses with a named reason and never silently creates a replacement session.
3. **Given** a completed turn boundary, **When** forking, **Then** Core creates the child lineage without copying pending approvals, active leases or later facts.

### User Story 3 - One dynamic capability catalog (Priority: P1)

Models, efforts, modes, commands, tools and capabilities shown by Studio are the actual ACP/Core catalogs.

**Why this priority**: Static duplicates advertise unavailable behavior and hide newly available behavior.

**Independent Test**: ACP catalog contract tests and renderer tests consume the same versioned catalog; unknown values refuse at the boundary.

**Acceptance Scenarios**:

1. **Given** a model/catalog generation, **When** Studio displays options, **Then** every selectable item is admitted by that generation.
2. **Given** an unavailable handler or backend, **When** capabilities are negotiated, **Then** the corresponding entry is unavailable with a reason and is not advertised as active.

### User Story 4 - Single-writer migration and clean retirement (Priority: P0)

Existing sessions and schedules migrate without dual writes, silent fallback or permanent legacy switches.

**Why this priority**: A second long-term compatibility runtime is the defect being removed.

**Independent Test**: Migration receipt tests, architecture ratchets and vertical-slice behavior tests prove old read/write paths are removed with each cutover.

**Acceptance Scenarios**:

1. **Given** a domain cutover, **When** the new path activates, **Then** the old authority path is removed in the same structural change.
2. **Given** an incomplete or conflicting migration, **When** the application opens, **Then** it reports recovery required instead of merging by similarity or reading both stores.

## Edge Cases

- Cancel during event delivery, worker panic, EOF without terminal, deadline during confirmation and late approval.
- Replay after repeated loads, unknown session/workspace, changed model or MCP generation, stale Goal/Plan revisions.
- Active Run fork, unresolved tool effect, pending form/approval, orphan ACP session and legacy unbound Studio history.
- Calendar fire and Goal UTC wake at the same instant, duplicate operation ID and runtime restart before admission.
- Unsupported prompt content, unavailable local backend, cross-tenant document/memory access and unknown catalog values.

## Requirements

### Functional Requirements

- **FR-001**: Core MUST be the only authority for execution sessions, transcripts, Runs, Goals, Plans, permissions, Reviews and execution catalogs.
- **FR-002**: ACP MUST be the only Studio-facing agent protocol and runtime composition boundary; it MUST NOT grow a second business ledger.
- **FR-003**: Studio MUST expose one typed RuntimeClient for Aster runtime commands; business modules MUST NOT call raw transport directly.
- **FR-004**: Interactive and background attendance MUST use the same admitted-Run driver, cancellation, event and terminal settlement implementation.
- **FR-005**: Studio MUST NOT pre-decide Core confirmation requests or persist an independent permission authority.
- **FR-006**: Studio MUST render Plan only from the Core/ACP typed Plan projection; new events MUST NOT use TodoWrite rawInput as a second channel.
- **FR-007**: Session load failure, workspace mismatch or identity conflict MUST refuse without fallback to `session/new` or the retired Studio store.
- **FR-008**: Calendar and Goal UTC wake MUST feed one execution scheduler and one operation-idempotent Run path.
- **FR-009**: Review MUST use Core Finding identity and execution; Studio MUST NOT issue an independent model HTTP review loop.
- **FR-010**: Every domain cutover MUST be single-writer and remove the old authority path in the same structural change; permanent dual-read/write or legacy fallback is forbidden.
- **FR-011**: Runtime model/effort/mode/command/capability options MUST come from the current Core/ACP generation.
- **FR-012**: Document reads MUST use one overlay/version path even though Studio remains the unsaved-buffer owner.
- **FR-013**: New behavior MUST have small owning-package hermetic tests and retained evidence with an Explicitly not claimed section.
- **FR-014**: Unknown enums, events, operations and catalog values MUST use typed closed-set refusal; intent MUST NOT be inferred from prompt text or tool names.
- **FR-015**: Studio source MUST remain only in its private submodule; the parent tracks only its gitlink and shared evidence/plans.
- **FR-016**: Product configuration MUST remain ACL parsed by `aster-acl`; protocol and test-evidence JSON are not product configuration.

### Key Entities

- **RuntimeCell**: One isolated tenant/principal/workspace runtime composition; multiple cells may exist, but each has one execution implementation and fact chain.
- **ExecutionRequest**: Prompt/GoalTick/Schedule source plus attendance, permission policy, budget, deadline and operation identity; owned by Core.
- **RuntimeClient**: Studio's sole typed ACP command facade; owns no product facts.
- **RuntimeEvent**: One typed live/replay/recovery envelope; decoded once at the protocol boundary and projected once for UI.
- **SessionBinding**: Migration and presentation correlation only; never a second transcript or execution record.
- **PlanRevision**: Core-owned current plan and lineage; ACP publishes its typed projection.
- **ScheduleFire**: A calendar occurrence mapped idempotently to a Core operation/Run identity.

## House Checklist

- [ ] New behavior has a small hermetic test in the owning package
- [ ] Closed sets: every new enum has `parse()` / `as_str()`
- [ ] No production `unwrap()` / `expect()` without an infallibility comment
- [ ] Internal `aster-*` dependencies stay on a path in this repository
- [ ] Product configuration, if any, is ACL parsed by `aster-acl`
- [ ] Evidence target: `docs/evidence/code/2026-10-25-studio-single-runtime/OUTCOME.md`

## Success Criteria

### Measurable Outcomes

- **SC-001**: Focused Core, ACP and Studio owning-package suites pass with zero new failures.
- **SC-002**: Only RuntimeClient and the raw transport contain production Studio ACP request calls.
- **SC-003**: Interactive and background entry points share the same admitted-Run driver and conformance cases.
- **SC-004**: New Plan rendering has zero dependency on TodoWrite rawInput, including load replay.
- **SC-005**: Studio has no permission pre-decision path and no second Review model HTTP client.
- **SC-006**: Each completed domain cutover has no dual-write, old-store fallback or long-lived feature switch.
- **SC-007**: `cargo fmt --all -- --check`, applicable architecture ratchets and focused renderer tests pass; unrun native/real-model/soak scope is explicit.

## Assumptions

- Single runtime means one implementation and fact chain per isolation cell, not one global process.
- Studio presentation metadata, user PTY, Office rendering, editor buffers and device brokers are host responsibilities, not duplicate agent runtimes.
- Existing dirty changes belong to their authors; this epic uses incremental edits and never resets or wholesale-overwrites them.

## Explicitly Not Claimed

- No claim that all third-party agent types become Core, that all platform processes collapse into one, or that optional Core SDK features become default ACP features.
- No claim of native GUI, real-model, full-platform, migration-of-all-history or soak qualification unless separately executed and recorded.
