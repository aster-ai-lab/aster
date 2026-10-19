# Pending approval recovery and cancellation audit

Read-only investigation of host session
`01a155cf-02f1-7478-8105-070a97a1507c`. No other session contents, model
reasoning, current task body or tool command/output text were inspected.
No source, frozen configuration or session data was modified; no GUI action,
HTTP/Tauri request, signal, test, model or VM was started by this audit.

## Observations and evidence limit

The specified session's existing execution row records `awaiting-approval`,
observed at `2026-10-19T20:36:58.652Z`. Its assistant row is pending, and its
last dynamic Bash part has `state=approval-requested` and a present approval
id. The preceding tool part is `approval-responded` without an output. Root's
native-fact audit found the pending Bash intent at sequence 125 with no
subsequent answer/preparation. Root observed an ordinary idle composer without
the permission interface or Pause button, including after home → exact-session
reopening. This audit did not operate the GUI or capture its in-memory overlay.

These are persisted/observed facts, **not proof that a live approval receiver
still exists**. The live oneshot map and the renderer's current overlay were
not read. The exact initiating race is therefore unproved.

A later narrow timestamp check found this same assistant row changed to
`status=error`, `updatedAt=2026-10-19T20:44:45.304Z`. This audit did not trigger
the change or read the error text. That later persisted terminal supersedes
the earlier pending observation; it is not independent proof of native/ACP
or complete resource settlement.

## Confirmed source behavior

### Sidebar “End” does not cancel

`renderer/pages/agents/WorkbenchHome.tsx:952–955` implements `settleSession`
only with `setSettledIds` and an undo record. The sidebar button at 1132–1137
and command-palette action at 1867–1868 call it. The label is translated as
“结束” / “End”. It does not call `abort`, wait for settlement or change the
native Run. It must not be used as evidence that this waiting Run stopped.

### Actual cancellation is coupled to the missing composer controls

The actual Stop path is `AgentComposer.tsx:1463–1471` →
`taskConversation/workbenchRunHost.ts:108–112` →
`hooks/useChatWithHistory.ts:70–75`, which invokes the session-scoped
`ai.stream.abort` route and SDK stop. The normal composer displays Pause only
when `isLoading && sendDisabled`
(`components/composer/ComposerSurfaceRuntime.tsx:2280–2296`).

`shared/ai/transport/turnState.ts:80–84` defines `awaiting-approval` as
`isStreamLive=false` but `isTurnActive=true`.
`hooks/useTopicStreamStatus.ts:47–48` exposes `isPending=isStreamLive`, and
`AgentChat.tsx:709` passes that to the composer's streaming prop. Thus an
approval pause does not display that normal Pause control. When the permission
composer is mounted, its buttons and Esc deny the exact approval
(`variants/PermissionRequestComposer.tsx:260–284`); Esc cannot be assumed to
perform that action when this component is absent.

No independent visible single-Run stop control was found in the inspected
session/Workbench UI. Closing a tab or reopening a route is designed to retain
stream readers, while archiving/deleting changes stored conversation data.
None is an equivalent, proven replacement for the missing Stop control.

### Persisted approval and live approval are separate authorities

`src-tauri/src/agent_plane/turns.rs:143` stores live approvals as an in-memory
map of oneshot senders. `respond_approval` at 1359–1365 removes the sender and
returns whether delivery succeeded; the controller at `controllers.rs:713`
returns that bool as `ok`. `useAgentChatRuntimeState.ts:421–422` correctly
rejects `ok=false` rather than dismissing it as a successful answer.

`turns.rs:1281–1285` attach returns only `attached + bufferedChunks` or
`not-found`. It does not return the current status, current approval set or
whether an approval receiver is still open. The normal approval path persists
before publishing at 2577–2586; captured repair streams persist the request
without emitting its chunk. The latter is a general recovery boundary, not a
claim that this observed session was in captured repair mode.

Status recovery also exists independently: `events.rs:56–66` includes live
statuses in the atomic SSE snapshot, consumed by
`renderer/data/studioAgentPlane.ts:570–583`. Therefore this audit does **not**
claim that all status recovery is absent merely because attach is incomplete.

An abort request only sets the live turn's abort watch
(`turns.rs:1288–1293`); the approval wait at 2588–2615 observes abort and sends
a refusal before prompt cancellation. Its controller always acknowledges the
request at `controllers.rs:667–668`; that acknowledgement alone is not Run
settlement. With no live turn, abort is a no-op. The existing messages endpoint
also reconciles orphan pending rows (`controllers.rs:575`), so calling it is
not a passive liveness probe. This audit read existing files instead.

## Narrow frontend recovery candidate

`useToolApprovalComposerOverrides.ts:48–67` removes all live message ids from
the historical candidate map and selects current candidates from the projected
parts map. Permission selection at 81–88 uses those two maps.
`persistedPartsByMessageId` is used for AskUserQuestion draft cleanup, not as
an independent permission recovery source.

`useStableMessagePartsLayers.ts:107–116` gives a nonempty execution overlay
priority over the database parts for the entire message. A stale same-anchor
overlay lacking a newly persisted approval can consequently mask that new
approval from both candidate maps. `ExecutionStreamOverlayService.ts:313–324`
preserves snapshots whose execution still appears in the active union during
remount reconciliation, so route reopening need not discard that overlay.

The existing hook test at
`components/composer/__tests__/useToolApprovalComposerOverrides.test.ts:106`
protects the opposite case: a new live answer must hide an older pending
historical card. It does not prove recovery when a newer persisted pending
approval is masked by an older overlay. The latter is a bounded regression
candidate consistent with the symptom, **not the established unique cause**.

## Minimal follow-up and regressions (not implemented or executed)

Reuse the existing native turn owner, approval ids and SSE/attach paths to
recover the **current live** pending approval and its anchor. Reconcile it with
the projected parts without reviving old answered cards. Do not simply treat
every persisted `approval-requested` part as a live future. Keep a visible
session-scoped Stop action for a host-owned active approval wait, independently
of whether its permission composer rendered; record actual terminal settlement
before reporting it stopped. Sidebar End must retain a clearly separate meaning
or deliberately use that same settled stop path in a later reviewed change.

Small owning tests should cover:

1. A real controlled pending approval, stale same-anchor overlay and newer DB
   request: route remount restores the exact current card; deny/approve delivers
   once to the original receiver, without a new Run or model call.
2. Prior answered/denied approvals remain settled while the next unique pending
   approval appears; never recover an old sender from history. Include ordinary
   and captured-stream recovery, with a lost cache/SSE reconnect boundary.
3. A persisted pending row with no live owner/closed receiver is reported as
   unavailable/interrupted and cannot authorize tool execution. Preserve the
   existing `ok=false` refusal and orphan reconciliation behavior.
4. Awaiting approval with the permission component absent still exposes the
   existing scoped Stop action. It causes refusal/cancellation and actual
   terminal settlement; neither an abort acknowledgement nor sidebar End alone
   satisfies that assertion.

The independent read-only backend review was supplied by
`/root/runtime_evidence`; it executed no tests or requests. This investigation
does not hot-fix the frozen CUA run, claim that its receiver remains live, or
claim that the waiting task has now been cancelled.
