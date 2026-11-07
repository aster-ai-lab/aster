# Single-line tool rows for failed calls

Verified on 2026-11-07, macOS arm64. No commit was created.

## Result

- In the aggregated tool-activity disclosure, a failed call rendered its visible
  `失败` status as a fifth grid item in a four-column summary. The auto-placed
  duration cell (`0.2s`, or `—` when no duration exists) wrapped onto a second
  line.
- Timing and status now share one `[data-task-tool-meta]` cell pinned to column
  3, row 1. The summary grid stays at four columns, so the duration, the failure
  status, the command label, and the chevron all stay on one line.
- Changed `TaskConversationMessage.tsx`, `task-conversation.css`, and one
  regression case in `TaskConversationMessage.test.tsx`. Existing uncommitted
  work in the same files was preserved.

## Verification

- `./node_modules/.bin/vitest run --project renderer` on
  `TaskConversationMessage.test.tsx` and `TaskToolActivity.test.tsx`:
  **145 passed, 3 failed (148 total)**.
- New case `keeps the duration and failure status in one summary grid cell`:
  **passed**. It asserts both facts live in one grid cell and that the group
  summary keeps its four-column, first-row placement.
- The 3 failures are outside the changed paths: `opens an edit as a diff and
  can switch to the split view` (no radiogroup rendered) and two `tool payload
  presentation` cases (JSON payload renders literal `${workbench.json.keys}` /
  `${workbench.json.items}` keys). `apps/studio` is a separate git repository
  that already carries many unrelated uncommitted edits, including both locale
  catalogs. A baseline run was therefore not produced by reverting files: doing
  so would discard that work.
- Scoped Oxfmt check on the two changed TSX files: **passed**. Oxfmt cannot
  parse this CSS file (pre-existing syntax error reported at line 3230, outside
  every edited hunk), so the CSS was checked by `git diff` review only.
- Scoped Oxlint on the two changed TSX files: **0 warnings and 0 errors**.
- `git -C apps/studio diff --check -- <3 changed files>`: **passed**.

## Explicitly not claimed

- No Studio rebuild, renderer bundle, hot reload, or GUI/Playwright run against
  a live window. Chromium for Testing is not installed here
  (`chromium.executablePath()` points to a missing binary), so no real-browser
  layout measurement was made. The window in the reported screenshot is not
  changed by this source edit until Studio is rebuilt or hot-reloaded.
- No full TypeScript check, full lint, full test suite, docs gate, or commit.
- No user-facing setting or "no wrap" toggle was added; this is a source fix.
- No claim that the three failing tests were introduced or fixed by this change.
