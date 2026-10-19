# Home draft activity scope and same-session selection

The home draft now has its own idle composer presentation while the previously
selected conversation keeps running. A home request resolves its own task before
it can reach a session queue. Selecting an already active session through the
shared selection entry leaves the home overlay without requiring an ID change.
The two production files and two sibling test files are recorded in
[source-sha256.json](source-sha256.json). No application or live host was changed.

## Confirmed causes and bounded changes

`AgentChatSessionCenter` passes the selected runtime's pending state through
`AgentComposerSlot`. The composer previously used that old conversation's
`isStreaming` for its home placeholder, loading/send control, and steer shortcut.
The old `awaiting-form` status also took priority over the home placeholder,
and the old follow-up dock remained visible above the home input.

`AgentComposer` now distinguishes a home draft from a session handoff or edit.
Only its presentation is scoped idle: placeholder, loading/send control, and
steer shortcut. It hides the old follow-up dock, and its required pause callback
returns without stopping the old session when invoked from a home draft. The
old stream subscription, queue identity, queue drain hook and review state keep
running. Returning to the conversation restores its pending/form presentation
and its existing queue. The shared composer interface was not widened.

A home send first resolves its task, including when the selected session has
no rendered messages yet. A missing home creation bridge refuses the send and
keeps the draft. If creation incorrectly returns the same busy/populated source
session, the send refuses with the existing creation error; it cannot fall
through into that session's queue or steer channel. Valid new-task adoption,
permission handoff, member checks, and fresh-task autosend remain on the existing
path. The handoff flag is read at send/cancel invocation, so pending autosend is
not mistaken for another new home request.

The existing `WorkbenchHome.onOpenSession` callback already explicitly left
home. The sibling shared `setActiveSessionAndClearTransient` entry, used by the
session list, resource list, and history, did not. A new actual `AgentPage`
regression opens home, reselects the same current ID through that shared entry,
and observes home remaining true before the fix. That entry now clears home
and catalog explicitly; WorkbenchHome uses the same entry. This proves the
shared selection gap, not which exact live sidebar callback caused the operator's
transient observation. No route-change effect or ID-based heuristic was added.

## RED/GREEN and regression results

Commands from `apps/work`:

```sh
/opt/homebrew/bin/node node_modules/vitest/vitest.mjs run --project renderer src/renderer/components/composer/variants/__tests__/AgentComposer.test.tsx -t 'new home draft idle|hidden conversation form and queue|returns the busy source session'
/opt/homebrew/bin/node node_modules/vitest/vitest.mjs run --project renderer src/renderer/pages/agents/__tests__/AgentPage.test.tsx -t 'already active session is selected again'
/opt/homebrew/bin/node node_modules/vitest/vitest.mjs run --project renderer src/renderer/components/composer/variants/__tests__/AgentComposer.test.tsx
/opt/homebrew/bin/node node_modules/vitest/vitest.mjs run --project renderer src/renderer/pages/agents/__tests__/AgentPage.test.tsx src/renderer/pages/agents/__tests__/AgentComposerSlot.test.tsx src/renderer/pages/agents/__tests__/WorkbenchHome.test.tsx
```

- Original home checks: **0 passed, 3 failed** in [before-fix.json](before-fix.json).
  They detected streaming placeholder inheritance, awaiting-form inheritance,
  and a lost draft after same-source creation.
- Same-ID shared selection: **0 passed, 1 failed**, then **1 passed, 0 failed**;
  see [before](before-reselect.json) and [after](after-reselect.json).
- The final draft-retention fixture explicitly writes its draft before send.
  Removing only the new same-busy-source guard reproduces **0 passed, 1 failed**:
  the old path clears it instead of retaining `new home task`. The exact source
  hunk was restored in `finally` and its full SHA checked. See
  [control result](before-fix-busy-handoff-control.json) and
  [precise hunk](busy-same-session-control.patch). The earlier red check's
  default-draft assumption is not used as independent proof of this branch.
- Final complete `AgentComposer`: **161 passed, 0 failed**;
  [composer-regressions.json](composer-regressions.json).
- Final caller regressions: `AgentComposerSlot` **9 passed**,
  `WorkbenchHome` **86 passed**, `AgentPage` **75 passed, 6 failed**;
  [caller-regressions.json](caller-regressions.json).

Across the four complete files: **331 passed, 6 failed**. The same six sidebar
assertions are the independently reproduced pre-existing failures from
[the earlier exact-hunk control](../2026-10-19-home-task-permission-handoff/sidebar-control-summary.json).
No additional failed names appeared. This slice did not alter sidebar layout
or expectations to force green. All four new checks pass in the final complete
runs, alongside the earlier mode/member and existing follow-up/steer regressions.
The counts are copied from actual JSON reports, also summarized in
[test-summary.json](test-summary.json).

Scoped `oxfmt --check` and `git diff --check` passed for all four files.
Scoped oxlint reports only the unchanged unused `getWorkspaceSourceFromSession`
helper in AgentPage; [lint.log](lint.log). The previous whole-web TypeScript
check was not repeated: it had 273 diagnostics across 93 files, documented in
[the permission evidence](../2026-10-19-home-task-permission-handoff/OUTCOME.md).
The required `onPause` type and existing interface were checked directly; the
new callback uses the same non-optional contract.

## Explicitly not claimed

- No GUI, focus/remount, native dialog, model, provider, account, credential,
  task answer, scoring data, deployment, build or host restart was performed.
- The operator's later brief missing editor after focus was not treated as
  proof of busy inheritance, and no fixture was invented for that inference.
- The old conversation still remains in the accessibility DOM behind the visual
  home overlay: the AgentChat wrapper lacks a home aria-hidden/inert boundary.
  This was reported read-only to root; no accessibility or portal behavior was
  changed in this slice.
- This is hermetic renderer/caller coverage, not a successful CUA task-corpus
  rerun. Existing unrelated sidebar failures and project TypeScript diagnostics
  remain outside this change.
