# Feature Specification: Studio / Work behavior alignment

**Feature Branch**: `003-studio-work-alignment` (no branch requested or created)
**Created**: 2026-10-21
**Status**: Specified
**Input**: 完成 Studio 侧上述规划的对齐优化。
**Gate context**: Product regime, `apps/studio`. Implements the approved six-area synchronization plan: shared ACP, input/terminal reliability, planning/goals, approvals, model preferences, and independent packaging/updating. Work is prior art; Studio remains a separate application.

## User Scenarios & Testing

### US1 — Reliable input and visible replies (P1)

A person composes Chinese text, selects candidates, cancels composition, and sends a request whose reply finishes immediately. The result appears in the correct session without duplicate sends, lost cancellation semantics, or rich-editor render loops.

**Independent test**: Studio renderer IME, actual Tiptap lifecycle and SWR handoff cases reject the old behavior and pass after the repair.

**Acceptance**: Candidate Enter/Escape cannot submit/approve/close; a later deliberate Enter sends once; empty composition cancellation is retained; old committed callbacks cannot populate another session; accepted sends remain accepted when a background refresh fails.

### US2 — Reviewable planning and truthful goal tracking (P1)

A person enables planning, reviews and revises a structured plan, then starts execution under the existing authorization policy. Progress updates live and completed controls leave the composer while history remains accessible.

**Independent test**: Studio plan-mode submission, review, task projection, task progress, composer and goal cases. Native interactive fixture proves plan → review → approval → execution → completed cleanup.

**Acceptance**: Planning is independent of permission mode; running/queued turns retain their submitted choice; a nonempty draft prevents accidental execution; unknown or failed task states never become completed; a new home task cannot inherit an old plan/goal. User-facing system states use concise localized text and expandable details.

### US3 — Bound approvals and durable model defaults (P1)

A person reconnects during approval or stops a task, then changes model defaults and restarts. Approvals remain bound to the current execution and saved model choices remain effective.

**Independent test**: Native approval/reconnect/cancellation and provider/preference persistence/refusal fixtures, paired renderer tests, local ACP wire acceptance.

**Acceptance**: Wrong member/session/execution approval replies refuse without delivering a decision; cancellation withdraws pending approvals; local runtime reconciliation never blindly resends a tool. Default/quick/translation choices update the native preference owner, preserve unrelated settings, and report write failure with rollback rather than success.

### US4 — Coherent Studio build and upgrade (P1)

A person installs or upgrades Studio with its corresponding frontend, ACP and Office. An incomplete or foreign update is refused while a recoverable failure permits retry.

**Independent test**: Root workspace build, companion-resource identity, isolated Studio old-app upgrade, malformed/foreign/incomplete archive and failure/retry cases; Studio Linux/IME package tests and bounded native GUI evidence.

**Acceptance**: Studio's server update protocol, product/tenant identity and data paths remain authoritative. Installation cannot leave a host/ACP/Office mixed generation. The Linux launcher uses the tested display/input-method chain with the Studio resource layout. Missing required assets refuse packaging.

## Edge Cases

- Stale callbacks, queued turns, rapid completion, reselecting the same session, background WebView focus and concurrent sessions preserve exact ownership.
- IME keyCode 229/native flags/ProseMirror composing, empty composition data, repeat keys and attachment registration must be tested through actual existing entry points.
- Unknown plan status, invalid approval binding, missing update size/digest/identity or unavailable preference storage refuse with precise existing or typed reasons.
- Local ACP, cloud and other declared runtimes remain separate lanes. Shared Core improvements alone do not prove cloud behavior or a new Studio rules input.

## Requirements

- **FR-001**: Studio MUST build/load the workspace ACP and Office from the same delivery; missing, stale or mismatched required companions MUST refuse activation or packaging.
- **FR-002**: Every affected input entry MUST honor composition ownership and cancellation; candidate selection MUST NOT invoke send/approve/close.
- **FR-003**: Rich input and accepted-send handoff MUST converge to the correct durable reply; cross-session callbacks MUST be refused.
- **FR-004**: Plan-first submission, review/edit/execute and structured progress MUST be wired end to end; unknown states and stale reviews MUST refuse execution/completion projection.
- **FR-005**: Goal details MUST be compact/localized and bound to the current task; terminal controls MUST clear from the composer without deleting history or guessing success from text.
- **FR-006**: Approval attach/reconnect/response/stop MUST use exact execution identity; foreign/stale replies MUST not reach another execution.
- **FR-007**: Model default persistence MUST synchronize the three native device preferences, retain Studio organization eligibility, and refuse partial-write success with rollback.
- **FR-008**: Studio updates MUST retain its own server channel, verify the complete product/binary generation, serialize install state, preserve retry on recoverable failure and refuse foreign/incomplete payloads.
- **FR-009**: Studio Linux/IME packaging MUST reuse qualified module/protocol mechanisms with the Studio product layout and fail on missing required resources.
- **FR-010**: Studio MUST preserve dockContent, background timer/focus, Paperloom/ComposerMode, tenant and data ownership. Synchronization MUST NOT replace these with Work's differing application behavior.
- **FR-011**: New product configuration MUST remain ACL; state/identity/meaning MUST come from explicit typed sources rather than name/intent guessing.
- **FR-012**: Every changed behavior MUST carry hermetic regression coverage, current build/interactive evidence and independent review; weak or missing evidence MUST NOT be labeled complete.

### Key Entities

Studio owns submitted plan posture, rendered plan review, task/goal projection, approval execution binding, model device preferences, and update staging. ACP/Core owns execution/goal/memory/tool semantics. No store is moved between products.

## House Checklist

- [ ] Owning hermetic tests — tasks T002–T007
- [ ] Closed sets parse/as_str — T004/T006 and review T009
- [ ] No undocumented production unwrap/expect — T009
- [ ] Local dependencies and one root workspace — T001/T008
- [ ] ACL-only product configuration — T006/T007/T009
- [ ] Evidence: `docs/evidence/studio/2026-10-21-work-alignment/OUTCOME.md` — T010

## Success Criteria

- **SC-001**: All new behavior cases and owning focused regressions pass; failures from a broader run are isolated, triaged and do not hide new regressions.
- **SC-002**: `just build-studio`, the Studio renderer production build and the applicable local release/package gates succeed with exact companion identity.
- **SC-003**: Native interactive Studio evidence proves Chinese input, plan review/execute/cleanup, approval recovery/stop, rapid reply, model persistence and isolated old-package upgrade.
- **SC-004**: Root fmt, local-dependency/workspace and relevant UI ratchets are executed; final outcome records commands/counts, remaining independent baseline failures and a requirement-by-requirement audit.

## Assumptions

Use synthetic providers and isolated test state, never user credentials/session contents. Existing product and tenant data identities are preserved. Publishing a new release, changing a live feed, committing or pushing is not requested by this implementation goal.

## Explicitly Not Claimed

No replacement cloud engine, new configuration language, second Cargo workspace, renamed existing identifiers, unrelated feature redesign, external release publication or user-data migration. Physical hardware/platform results are claimed only when actually observed; virtual/fixture evidence is labeled by its true scope.
