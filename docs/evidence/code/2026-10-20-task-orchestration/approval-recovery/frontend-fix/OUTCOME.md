# Work approval recovery: frontend repair

The old same-message execution overlay can hide a newer persisted approval.
The ordinary composer Pause also depends on a live stream and is hidden by
replacement approval/question layers. Both failures were reproduced before
the production changes: `red-tests.log` records **6 failed, 13 passed**.
The original T027 audit, app, inputs and submission evidence remain unchanged.

## Change

- The native attach response, topic status and atomic SSE snapshot share
  `pendingApprovals`. Each entry binds execution, native anchor, actual card
  message, approval and tool call. The renderer accepts only bounded, unique
  identities belonging to the current `turnId` and active execution/anchor.
  An empty set or disconnected transport supplies no approval authority.
- The composer recovers only matching current receiver parts, preferring the
  persisted actual card over an older overlay. Settled DB answers stay settled;
  the original legacy transport behavior is retained. Captured repair cards
  may have a different message id from their native execution anchor.
- Approval responses send the native controller's six fields: `approved`,
  `sessionId`, `executionId`, `anchorMessageId`, `messageId`, `toolCallId`.
  Missing bindings fail closed. A successful response dismisses only that id;
  the existing native `ok=false` refusal remains an error.
- Attach snapshots cannot overwrite later topic updates, including an ABA
  update/clear to an empty cache. The bridge records per-topic revisions and a
  transport generation. Disconnect withdraws pending authority; a fresh atomic
  snapshot or a current attach may restore it.
- A visible **Stop** action lives outside the inert/replaced composer layers
  during `awaiting-approval` and `awaiting-form`. It uses the existing scoped
  child cancellation and chat stop path. The button remains until native
  status leaves the wait; an abort acknowledgement does not fabricate terminal
  settlement. Editing stays disabled while the turn is active.

Fifteen source/test files are listed with final hashes in `source-final.json`;
the exact nested Work diff is retained in `frontend.diff`. The original
thirteen-file hash snapshot is `baseline.json`; the two additional shared
contract files are identified separately by that final manifest and diff.
No Rust, dependency, global configuration or stored user session was changed
by this frontend worker.

## Verification

Commands run from `apps/work`. `pnpm` was absent in this shell (exit 127,
`red.log`), so existing local executables were used without installing anything.

```sh
./node_modules/.bin/vitest run --project renderer \
  src/renderer/components/composer/__tests__/useToolApprovalComposerOverrides.test.ts \
  src/renderer/components/composer/__tests__/ComposerCore.test.tsx \
  src/renderer/hooks/__tests__/useTopicAwaitingApproval.test.ts \
  src/renderer/pages/agents/__tests__/useAgentChatRuntimeState.test.tsx \
  src/renderer/data/__tests__/desktopAgentPlane.test.ts
```

`green-final.log`: **5 files passed; 76 passed, 1 skipped; exit 0; 4.47 s**.
The skip is the pre-existing externally supplied retained SSE replay fixture.
Coverage includes initial pending recovery, stale overlay, missing/closed
receiver authority, wrong tool/message/anchor, captured question cards,
same-status next receiver, disconnect/reconnect, stale HTTP/SSE and empty-cache
ABA, and Stop outside all three composer layers until native terminal.

The same command plus `IpcChatTransport.test.ts`,
`TopicStreamSubscription.test.ts`, `useChatWithHistory.test.ts`,
`ComposerCore.insertion.test.tsx`, `ComposerSurface.lazy.test.tsx` and
Home `ChatContent.test.tsx` passed **11 files; 182 passed, 1 skipped; exit 0;
3.62 s** (`connected-final.log`). After a final internal memo simplification
that avoids unnecessary legacy scans in native mode, the affected hook suite
passed **16/16**, exit 0, 3.01 s (`hook-final.log`).

Final exact-file checks on the fifteen paths:

- `./node_modules/.bin/oxfmt --check <15 paths>`: exit 0 (`format-final.log`).
- `./node_modules/.bin/eslint <15 paths>`: exit 0, zero errors; three existing
  warnings remain (`eslint-final-cohort.log`): an unchanged unnecessary memo
  dependency, an existing cross-page import, and an existing translation
  template literal.
- `git -C apps/work diff --check`: exit 0.

Full web TypeScript checking first exhausted Node's default 4 GB heap (exit 134,
`typecheck.log`). The single 8 GB retry completed with exit 2 and **274 whole-tree
errors, zero diagnostics in the fifteen owned files** (`typecheck-8gb.log`,
`typecheck-summary.json`). No full pre-change typecheck was run; unrelated
diagnostics are retained without asserting a clean baseline. No additional
heap increase or unrelated type repair was attempted.

The native worker independently read the renderer binding and captured-card
contract and reported no mismatch. Native controlled-provider tests and actual
resource settlement are owned and reported separately by the root/native work.

## Explicitly not claimed

No real model, network request, VM, desktop launch, CUA action, release,
deployment or new T01 run was performed by this worker. These hermetic frontend
proofs do not establish a completed T01 task, end-to-end speed improvement,
whole-tree TypeScript/CI success, or native settlement by themselves.
