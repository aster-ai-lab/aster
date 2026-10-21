# Composer picker rejection and destroyed-editor focus recovery

Result: two controlled failures were reproduced before changing production code
and now pass. The final focused regression set passed four files / 15 tests with
no unhandled errors. These repairs do not establish the cause of the desktop
operator's missing attachment chip.

## Production changes

- `AttachmentButton.tsx` catches picker/ingestion failures and shows the existing
  localized `chat.input.file_error` message (`文件处理出错`). `finally` resets
  `selecting` and schedules the existing focus restoration on success, cancel or
  error. Metadata projection runs before the functional state updater so its
  failure is also inside that error boundary. The existing picker, transport,
  supported-extension rules and files store are reused.
- `createComposerInputAdapter().focus` now returns when `editor.isDestroyed`
  instead of accessing the destroyed editor's command manager. All production
  factory calls were inspected: two in `ComposerSurfaceRuntime.tsx` (root panel
  and toolbar adapter) and one in `homeAgentCatalog.tsx` (installed-agent mention).
  The fix is in the shared factory; callers were not individually patched.

Only these two production files changed for this repair. The separate legacy
`useFiles` hook and the Bridge/host upload implementation were not changed.

## Tests and red-to-green evidence

`WorkbenchComposerAttachmentPipeline.test.tsx` runs the actual
`desktopMainBridge.js`, actual attachment runtime/registry, actual composer
surface and actual Tiptap/ProseMirror document with the production token mapper,
draft filter and attachment reconcile hook. Native picker events and host fetch
responses are controlled; file-preview/QuickPanel presentation are stubbed.
The new rejection test first returns HTTP 403 from the upload boundary, verifies
the localized error and empty draft, then returns HTTP 200 through a second
picker on the same runtime and verifies the visible chip, retained file and
enabled send control. It never sends a message.

`composerInputAdapter.test.ts` schedules the adapter's focus on real RAF, destroys
the actual Tiptap editor before that frame, and checks the callback resolves
without throwing. The previous implementation rejects with the observed
`Cannot read properties of null (reading 'commands')` error.

Commands were run from `apps/work`:

```sh
./node_modules/.bin/vitest run --project renderer src/renderer/pages/agents/__tests__/WorkbenchComposerAttachmentPipeline.test.tsx src/renderer/components/composer/__tests__/composerInputAdapter.test.ts -t 'reports a failed upload|ignores queued focus'
./node_modules/.bin/vitest run --project renderer src/renderer/pages/agents/__tests__/WorkbenchComposerAttachmentPipeline.test.tsx src/renderer/components/composer/__tests__/composerInputAdapter.test.ts src/renderer/components/composer/tools/components/__tests__/AttachmentButton.test.tsx src/renderer/pages/agents/__tests__/WorkbenchComposerAttachmentMenu.test.tsx
./node_modules/.bin/oxlint src/renderer/components/composer/tools/components/AttachmentButton.tsx src/renderer/components/composer/composerInputAdapter.ts src/renderer/pages/agents/__tests__/WorkbenchComposerAttachmentPipeline.test.tsx src/renderer/components/composer/__tests__/composerInputAdapter.test.ts
./node_modules/.bin/oxfmt --check src/renderer/components/composer/tools/components/AttachmentButton.tsx src/renderer/components/composer/composerInputAdapter.ts src/renderer/pages/agents/__tests__/WorkbenchComposerAttachmentPipeline.test.tsx src/renderer/components/composer/__tests__/composerInputAdapter.test.ts
```

- Before production fixes: two failed, seven skipped, one unhandled upload
  rejection, exit 1. See [before-fix.log](before-fix.log).
- After production fixes, exact two-test filter: two passed, seven skipped,
  exit 0. See [after-fix-focused.log](after-fix-focused.log).
- Final scoped set at 15:44:19 UTC: four files, 15 passed, zero failed, no
  unhandled errors, exit 0, 1.83 seconds. Breakdown: pipeline 4, adapter 5,
  attachment runtime 4, attachment menu 2. See
  [after-fix-scoped-regressions.log](after-fix-scoped-regressions.log).
- Scoped Oxlint, Oxfmt and `git diff --check` passed. TypeScript's `createProgram`
  with the existing web tsconfig reported zero syntactic/semantic diagnostics
  for the four changed source/test files; see
  [scoped-typecheck.log](scoped-typecheck.log). This is not a renderer-wide
  typecheck pass.

## Expanded test failures and the exact baseline comparison

Adding `ComposerSurface.test.tsx` to the focused set produced 163 passed / 18
failed out of 181 tests. No expanded all-green result is claimed. Each distinct
failure class was also run alone in its own CLI process:

```sh
./node_modules/.bin/vitest run --project renderer src/renderer/components/composer/__tests__/ComposerSurface.test.tsx -t 'commits IME composition to a selected prompt variable once composition ends'
./node_modules/.bin/vitest run --project renderer src/renderer/components/composer/__tests__/ComposerSurface.test.tsx -t 'lets composer token shortcuts handle Backspace before removing attachments'
./node_modules/.bin/vitest run --project renderer src/renderer/components/composer/__tests__/ComposerSurface.test.tsx -t 'keeps ArrowUp in the editor when a soft-wrapped line remains above the cursor'
```

Each failed alone (one failed, 165 skipped): respectively a mock passes null
`view` to `handleTextInput`, null `view` to `handleKeyDown`, or a mock view without
`dom`. Their failure sites are the input-event callbacks, before the shared
adapter's delayed focus. To establish independence from this change rather than
infer it, the root operator authorized an exact temporary reversal of only the
two production hunks:

```sh
./node_modules/.bin/vitest run --project renderer src/renderer/components/composer/__tests__/ComposerSurface.test.tsx --reporter=json --outputFile=<variant.json>
```

With the fixes: 148 passed, 18 failed, 166 total. Without the exact two fixes:
148 passed, the same 18 failed, 166 total. The failure-name sets are identical.
The fixes were restored in `finally`, and SHA-256 of both production files
exactly matched their values before the temporary reversal. No checkout or
unrelated source rollback was used. Evidence:

- [surface-with-fixes.json](surface-with-fixes.json)
- [surface-without-fixes.json](surface-without-fixes.json)
- [surface-baseline-comparison.json](surface-baseline-comparison.json)
- [production-before-baseline.sha256.json](production-before-baseline.sha256.json)

The same five-file combination was rerun after fixture cleanup: 163 passed,
18 failed, 181 total, exit 1. The failing names still exactly match the standalone
without-fixes baseline; see [after-cleanup-comparison.json](after-cleanup-comparison.json)
and [after-cleanup-regressions.json](after-cleanup-regressions.json). These 18
baseline failures remain unresolved; no additional production fix was added for
them.

## Pipeline fixture isolation

The actual Bridge installs global API values and listeners, so the integration
fixture now saves and restores its owned global property descriptors and window
API, records and removes the window/document listeners installed by that Bridge,
and restores its fetch/input-click spies and jsdom Range geometry. It cancels
remaining controlled picker inputs, drains editor/picker RAF before unmounting,
destroys the real editor through React cleanup, and releases its action reference.
It does not call `vi.unstubAllGlobals`, which would also remove unrelated setup
stubs. The unchanged standalone failure set above is separate evidence that
the new fixture did not introduce the expanded suite's 18 failures.

## Explicitly not claimed

- No native picker, CUA surface, real provider, task submission, shutdown, shell
  kill or application restart was operated by this subagent.
- HTTP 403 is a controlled regression response, not an observed explanation of
  the desktop operator's missing-chip case. Native WebKit event delivery and
  HMR effects still need the operator's fixed-frontend retry.
- No whole-renderer test/typecheck pass is claimed. The 18 expanded-suite
  baseline failures were retained and left outside these two production fixes.
- No account state, credential, task answer or source attachment was copied.
  The root operator's CUA `OUTCOME.md` was not edited.
