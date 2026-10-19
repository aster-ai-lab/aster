# Feature Specification: AsterDream task execution

**Feature Branch**: `001-task-execution` (no branch requested or created)
**Created**: 2026-10-18
**Status**: In progress; completion is unproven
**Input**: 根据业界最佳实践审查和深度完善和优化 AsterDream 的任务执行能力，确保功能丰富且底层 a3s-code 各项机制有效且高效。
**Gate context**: AsterDream's ordinary task path through the local Code SDK and
ACP companion. This expands the earlier file-delivery repair into a durable
audit of the complete task execution path. The original objective remains
active across slices; a passing subset does not close this feature.

## User Scenarios & Testing

### User Story 1 - Finish a real task with usable results (Priority: P1)

A user supplies files or text, asks for analysis or a deliverable, and receives
grounded content and actual files in the requested format.

**Independent Test**: Work `TurnService` with a real workspace-built ACP/Core
process and controlled loopback provider: discover tools, call them, inspect
saved packages, collect artifacts, and retain final user-visible text.

**Acceptance Scenarios**:

1. Given structured source data, when a task generates a workbook, report,
   presentation or chart, then the saved result can be reopened and its content
   checked, including formulas, tables and embedded charts when requested.
2. Given an unavailable backend, invalid input or denied path, when a tool is
   called, then it refuses with its reason and the task never invents a file or
   reports unverified work as completed.
3. Given the original task corpus, when a live evaluation can be run, then the
   evaluator checks outcomes and file structure and records blocked/missing
   oracle cases separately. Exact user requirements remain binding; equivalent
   punctuation or valid phrasing alone is not a substantive failure.

### User Story 2 - Control and resume long work safely (Priority: P1)

A user can stop, steer, disconnect from and resume a task without duplicate
effects, leaked authority, missing final replies or unbounded retries.

**Independent Test**: Core runtime and ACP duplex tests exercise model setup,
streaming, tool execution, permission/question waits, cancellation, recovery,
resource settlement and final fact boundaries.

**Acceptance Scenarios**:

1. Cancellation during any wait returns within the existing bounded settlement
   policy and prevents new effects. A live worker retains its authority permit
   until it actually settles.
2. Permanent errors and exhausted budgets do not cause a second provider/tool
   invocation. Unsupported streaming may use the existing explicit fallback.
3. Exact recovery restores the owning run's facts and outstanding work; stale
   owners, divergent receipts and unknown versions refuse activation.

### User Story 3 - Use capabilities effectively within finite resources (Priority: P2)

The agent can use the product's document, code, search, memory, plugin,
delegation and scheduling capabilities through their admitted interfaces,
without losing essential context or wasting calls on already-known failures.

**Independent Test**: Hermetic capability, context/compaction, budget,
delegation, memory and scheduler suites; recorded execution metrics and a
representative task evaluation supplement these mechanism tests.

**Acceptance Scenarios**:

1. Selected tools are exposed under their actual permission and workspace
   ceilings; read-only and source-review tasks cannot publish artifacts.
2. Compaction retains requirements, outstanding tool relationships and durable
   source references. Large results are paged from retained output rather than
   requiring repeated execution.
3. Token/call/time budgets cover delegated work and retries. Efficiency claims
   include measured calls, tokens, bytes and elapsed time for the tested path.
4. A configured retrieval capability reaches the actual session or reports a
   precise unavailable/degraded state. Plain text search remains available
   without embeddings; language servers start only when code operations need them.
5. Programmatic tool calls reuse the governed `program` executor. Nested calls
   retain permissions, budgets, cancellation, receipts and failure status;
   a successful wrapper cannot certify an unrecovered failed required child
   operation. Expected failures may be recovered with their receipts retained.

### Edge Cases

- Missing credentials, absent parsers/backends, unsupported Office operations
  and missing oracles are explicit unavailable/blocked results, never passes.
- Zero or missing comparison baselines, Unicode text limits, literal spreadsheet
  formulas, collisions, symbolic links and concurrent file replacement are
  checked at their owning boundary.
- Event EOF, late responses, duplicate terminals, stale generations, context
  overflow and a worker waiting for human input cannot silently become success.

## Requirements

### Functional Requirements

- **FR-001**: The host MUST expose and execute admitted capabilities through the
  actual Core/ACP path; it MUST refuse missing or denied capability use.
- **FR-002**: Existing task families MUST produce usable text, structured data,
  documents, spreadsheets, presentations and charts through native or installed
  tools. Unsupported operations MUST be identified precisely, not substituted
  by a different format or silently omitted.
- **FR-003**: Results MUST preserve source facts, required wording/fields and
  missing-data status, perform deterministic numeric/length checks where
  applicable, and refuse fabricated evidence of completion.
- **FR-004**: Planning, goal steps and delegation MUST preserve the user's full
  objective, inherited permission/resource ceilings and verifiable child
  outcomes. They MUST refuse authority expansion and untracked child effects.
- **FR-005**: Budgets, deadlines and retry classification MUST prevent unbounded
  work and repeated permanent refusals, including model-open/fallback paths.
- **FR-006**: Cancellation, steering, reconnect and exact recovery MUST remain
  effective during all asynchronous waits, with bounded settlement and no
  success while required effects are unsettled.
- **FR-007**: Context assembly, compaction and durable memory MUST retain
  authoritative constraints and recoverable observations within their budgets;
  they MUST refuse stale/cross-owner authority and expose irreducible limits.
- **FR-008**: Completion MUST be supported by the actual terminal state, final
  user-visible reply and verified requested artifacts. Preview acceptance,
  worker EOF and model assertions MUST NOT certify completion.
- **FR-009**: Diagnostics and measurements MUST correlate with the real session,
  prompt, run and tool invocation, remain bounded, and omit credentials.
- **FR-010**: Evaluation MUST include real pipeline integration, fault cases and
  representative tasks. It MUST distinguish deterministic mechanism evidence,
  model quality, environment blocks and missing acceptance data.
- **FR-011**: The shipped Work host MUST use the workspace-built ACP and Office
  companions. Builds/checks MUST record exact commands and failures without
  editing unrelated allowlists to manufacture a green result.
- **FR-012**: Execution optimization MUST measure sandbox lifecycle cost and
  model/tool orchestration separately. Programmatic execution MUST preserve
  nested failure, truncation and artifact evidence within the owning Run's
  authority and resource ceilings. The existing typed presentation profile
  MUST be selectable through trusted ACL and the actual ACP session path;
  omission preserves adaptive mode, unknown values refuse activation and a
  resumed session MUST reject profile drift.
- **FR-013**: Explicit workspace-retrieval configuration MUST survive the Work
  launch path under the existing trust rules. Missing local assets/providers
  MUST be reported honestly; they MUST NOT enable an implicit remote service,
  download, or workspace-controlled host configuration.

### Key Entities

- **Task/session**: Work's stored user interaction and execution owner.
- **Run/facts/checkpoint**: Core's authoritative lifecycle and recovery state.
- **Capability/tool invocation**: Core admission plus ACP's typed host tools.
- **Artifact/evidence**: Saved bytes, owning execution and Work capture receipt.
- **Evaluation row**: Task/outcome, mechanism coverage, measurements and current
  proof status; owned by this feature's evidence record.

## House Checklist

- [ ] New behavior has hermetic tests in its owning package (T002–T006).
- [ ] New closed sets use typed parsing and stable rendering (review T008).
- [ ] Production unwrap/expect changes are reviewed (T008).
- [x] Internal dependencies remain local; no new workspace or repository.
- [x] Product configuration stays ACL; this feature adds no alternative config.
- [x] Evidence target is `docs/evidence/work/2026-10-18-execution-audit/OUTCOME.md`.

## Success Criteria

- **SC-001**: Every FR has direct, current-state evidence in the acceptance
  matrix; unknown, blocked or partially covered requirements keep the goal open.
- **SC-002**: New behaviors pass small owning-package tests and the real
  Work→ACP/Core controlled-provider path, including denied/failed outcomes.
- **SC-003**: Mechanism suites cover control, budgets, context, permissions,
  delegation and recovery. The task corpus is replayed when its live provider
  and independent oracle inputs are available; otherwise that gate stays open.
- **SC-004**: Efficiency is reported from measured calls/tokens/bytes/time and
  bounded-input tests. No unmeasured speed or task-success uplift is claimed.
- **SC-005**: `just build-work`, the paired terminal/companion build, scoped
  checks and applicable CI workspace checks are recorded with exact results.
- **SC-006**: A controlled runtime/orchestration comparison records actual
  executable identities, all samples, model requests, nested tool calls, VM
  count, output bytes, available tokens and independent correctness checks.
  Historical setup/debug time and post-run acceptance time remain separate
  from task execution. Missing measurements are not estimated.

## Assumptions

- Source and runtime state in the shared worktree are authoritative. Concurrent
  work is preserved and verification identifies the source state it covered.
- A controlled provider proves runtime integration, not real-model task quality.
- Optional external services may require user-owned credentials. Their absence
  does not permit changing the user's objective or claiming completion.

## Explicitly Not Claimed

- This specification is not evidence that the objective is achieved.
- The earlier file-delivery tests do not certify all Code mechanisms, operating
  systems, task families or live providers. Those remain acceptance work.
- macOS native command lifetime qualification remains an explicit open gate;
  permission changes cannot waive it.
