# Home task permission handoff and member scope

The selected home execution mode now follows the new task before its first send.
Production changes are frozen in the five hashes in [source-sha256.json](source-sha256.json).
No application, UI, model, account, or native dialog was operated in this slice.
These are hermetic renderer regressions, not a task-corpus or GUI pass.

## Confirmed paths and changes

- The live home composer may be mounted over an older session. Its permission
  controls and send preparation now use the home draft until an explicit home
  handoff has begun. A conversation send uses that conversation's own task record.
- `AgentPage` captures the home permission record, including the existing plan
  flag, before asynchronous creation. It writes that record under the created
  session ID before returning it to all five existing `homeTask.create` callers.
  Creation/adoption and both composer callers refuse a result after the exact
  member changes. A different member cannot join another member's pending create.
- The composer run-mode initialization reads the same task record instead of
  replacing it from the shared agent configuration. The selected mode is applied
  on workbench sends only. Classic sends preserve their agent permission flow and
  do not gain a task-mode field in their transport body. Absent agent mode is
  compared as `default`, avoiding a redundant asynchronous update.
- Task records are scoped by explicit member and session. Only the home draft
  may use its member's pending choice; an unrelated named task starts at default.
  Local/legacy values do not elevate a signed member. The local null scope and a
  member literally named `local` have distinct keys; empty identities are refused.
  Picker state is owned by the member/session pair, so switching members cannot
  display the previous member's cached choice.
- Default-agent provisioning cache, pending promise, runtime-unavailable flag,
  and callback ownership now use exact member scope. See the separate
  [provisioning evidence](../2026-10-19-workbench-member-provisioning/OUTCOME.md).
  No account name inference, global permission elevation, new queue, sidebar
  behavior, dependency, host/Core permission receipt, or live host was changed.

## Reproduction before the fix

[before-fix-ownership.log](before-fix-ownership.log) records three failing checks
for cross-member task leakage, local/legacy preference promotion, and applying a
remembered elevated choice to an unrelated task. [before-fix-handoff.log](before-fix-handoff.log)
records two failing checks: the live home send did not apply its selected full
mode, and the newly created task had no copied record before adoption.

## Verification

Commands run from `apps/work` (the node path is explicit because pnpm is absent):

```sh
/opt/homebrew/bin/node node_modules/vitest/vitest.mjs run --project renderer src/renderer/components/composer/variants/__tests__/AgentComposer.test.tsx src/renderer/pages/agents/__tests__/WorkbenchPermissionPicker.test.tsx src/renderer/pages/agents/__tests__/workbenchPermissionPostures.test.ts src/renderer/pages/agents/__tests__/WorkbenchCommandPalette.test.tsx src/renderer/pages/agents/__tests__/useWorkbenchDefaultAgent.test.ts
/opt/homebrew/bin/node node_modules/vitest/vitest.mjs run --project renderer src/renderer/pages/agents/__tests__/AgentPage.test.tsx -t 'hands the selected home permission|refuses an in-flight home creation'
/opt/homebrew/bin/node node_modules/vitest/vitest.mjs run --project renderer src/renderer/pages/agents/__tests__/AgentPage.test.tsx
```

Final actual results:

| Run | Passed | Failed | Skipped | Evidence |
| --- | ---: | ---: | ---: | --- |
| Composer + permission picker/storage + command + provisioning | 211 | 0 | 0 | [JSON](final-regressions.json) |
| Exact AgentPage handoff/member-change checks | 2 | 0 | 78 | [JSON](final-handoff.json) |
| Full AgentPage | 74 | 6 | 0 | [JSON](final-agent-page.json) |

The five-file green run contains Composer 157, picker 19, storage 4, command 5,
and provisioning 26 passing tests. Across the six complete files the result is
**285 passed, 6 failed**; the two handoff checks are included in those totals.
The initial broad run exposed a classic-mode boundary and fixture assumptions;
they were investigated and corrected instead of classifying the large cascade
as a baseline. The existing task-mode transport fixture now explicitly mounts
its workbench context, while a separate classic regression asserts no agent
update or task-mode transport field.

### Independently reproduced sidebars

All six remaining failures are sidebars expecting `paneOpen=true` while the
current independent source supplies false. They were run alone, then rerun
with only this slice's exact `AgentPage` production hunks and test-fixture hunks
removed. Both variants had the identical six failed names and false-versus-true
assertions. The pre-existing sidebar source and every other concurrent hunk
were retained. The original permission hunks were restored and checked before
freeze. No sidebar implementation or assertion was changed to obtain a pass.
See [summary](sidebar-control-summary.json), [current run](sidebar-current.json),
[control run](sidebar-without-own-hunks.json), and
[exact hunk record](sidebar-control-own-hunks.patch). The patch records the source
current-to-control delta followed by the test current delta; it is evidence,
not an executable combined patch.

## Static checks and limits

Scoped `oxfmt --check` passed for all ten source/test files, and scoped
`git diff --check` passed. The full ten-file oxlint check returned exit 1 with
exactly two diagnostics: the unchanged unused `getWorkspaceSourceFromSession`
helper and the unchanged `importOriginal<typeof import(...)>()` test expression.
Their presence in HEAD was checked directly; see [helper check](lint-existing-helper-check.txt),
[import check](lint-existing-inline-import.txt), and [lint output](lint.log).
Neither unrelated expression was changed to force a green result.

The complete web TypeScript check did not pass. Its default Node 4 GiB heap
exhausted memory before producing TypeScript diagnostics (exit 134). The same
check with an 8 GiB process heap completed with exit 2, reporting 273 diagnostics
in 93 files. It was run without changing TypeScript or product configuration:

```sh
/opt/homebrew/bin/node --max-old-space-size=8192 node_modules/typescript/bin/tsc --noEmit -p tsconfig.web.json --composite false
```

Only three diagnostics refer to the ten touched files: the existing unused
helper, an existing `toast.error.mock` test expression, and the existing
`modelId: string` to template-ID assignment in provisioning. The exact unchanged
expressions were compared against HEAD; this is not a whole-project baseline
TypeScript run or a claim that all other diagnostics are understood. No new
permission branch, task storage, picker, Composer, or member-scope test expression
was diagnosed. See [summary](static-check-summary.json),
[expression checks](typecheck-unchanged-expressions.json),
[default-heap failure](typecheck.log), and [complete diagnostics](typecheck-8gb.log).

No real CUA mode transition, new agent on a live account, task-corpus score,
production build, or application restart is claimed. Root owns those checks.
No task sample, output, session body, credential, or runtime configuration was
read or modified. Existing unrelated sidebar failures remain unresolved here.
