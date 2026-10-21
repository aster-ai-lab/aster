# Workbench attachment menu repair

Result: restored a keyboard-accessible upload entry through the existing
attachment launcher. Four relevant hermetic test files passed: 160 tests,
zero failed, 6.84 seconds. This is not a claim that SH02/SH03 passed in the
desktop; the root operator owns that subsequent CUA verification.

## Observed defect and existing paths

The desktop operator reported SH02 blocked because Workbench's `+` menu exposed
only automatic review and artifact tracing. Source inspection confirmed that
`AttachmentToolRuntime` still registered its picker with `sources: ['popover']`,
while `useWorkbenchHomeComposer` replaced that tool menu with
`WorkbenchComposerMenu`, which never dispatched the registered attachment tool.

Existing drag/drop paths were found and reported before changing code:
`ComposerSurface` still accepted file drops, and the homepage fallback queued
them through `queueWorkbenchDroppedFiles`. They did not provide an accessible
file-picker button. The root operator confirmed adding the accessible entry.
The agentless homepage also lacked an attachment runtime; restoring only the
menu would leave that homepage's launcher unresolved.

## Change

- `WorkbenchComposerMenu` exposes a `type="button"` upload row and retains the
  two existing switches. Its connected variant resolves the exact attachment
  manifest ID from the current popover registry, observes registration changes,
  and dispatches through the existing controller with the current input adapter.
  It uses the registered label, tooltip, suffix and disabled state. Unresolved
  or actionless launchers remain disabled.
- `WorkbenchHomeComposerExtras` renders that same connected menu for homepage
  and conversation composers.
- `MissingAgentHomeComposerInner` mounts only the existing
  `AttachmentToolRuntime`, using its existing files store and supported
  extensions. It does not mount unrelated tool runtimes or create a picker.

No backend command, permission, workspace selection, upload transport, file-type
rule or send behavior was changed. Concurrent input-status and attachment-name
changes in `AgentComposer.tsx` were retained.

## Verification

Commands were run from `apps/work` using the installed executables because
`pnpm` was not present in the shell:

```sh
./node_modules/.bin/vitest run --project renderer src/renderer/pages/agents/__tests__/WorkbenchComposerMenu.test.tsx src/renderer/pages/agents/__tests__/WorkbenchComposerAttachmentMenu.test.tsx src/renderer/components/composer/tools/components/__tests__/AttachmentButton.test.tsx src/renderer/components/composer/variants/__tests__/AgentComposer.test.tsx
./node_modules/.bin/oxlint src/renderer/pages/agents/WorkbenchComposerMenu.tsx src/renderer/pages/agents/WorkbenchHomeComposerExtras.tsx src/renderer/pages/agents/AgentComposer.tsx src/renderer/pages/agents/__tests__/WorkbenchComposerMenu.test.tsx src/renderer/pages/agents/__tests__/WorkbenchComposerAttachmentMenu.test.tsx src/renderer/components/composer/variants/__tests__/AgentComposer.test.tsx
./node_modules/.bin/oxfmt --check src/renderer/pages/agents/WorkbenchComposerMenu.tsx src/renderer/pages/agents/WorkbenchHomeComposerExtras.tsx src/renderer/pages/agents/AgentComposer.tsx src/renderer/pages/agents/__tests__/WorkbenchComposerMenu.test.tsx src/renderer/pages/agents/__tests__/WorkbenchComposerAttachmentMenu.test.tsx src/renderer/components/composer/variants/__tests__/AgentComposer.test.tsx
./node_modules/.bin/tsc --noEmit -p tsconfig.web.json --composite false
```

- Vitest: four files, 160 passed, zero failed, 6.84 seconds. The new integrated
  menu checks use the real launcher registry, controller, attachment runtime,
  file-state provider and Radix popover; the native `window.api.file.select`
  boundary is mocked. They prove that clicking upload passes the existing
  extension rules to the picker, stages its returned file, restores composer
  focus, closes the menu, and does not submit a form, create a topic, or change
  draft text. A disabled launcher does not open a picker. The existing options
  keyboard test and attachment-controller tests also passed.
- The two new agentless-home cases prove that the actual homepage component
  registers the existing attachment picker both before and after model
  readiness, preserves the existing picker filter rules, and does not create a
  task when its picker is cancelled. That broader component suite mocks its
  composer UI/runtime provider; the separate menu integration supplies the real
  registry and runtime evidence.
- Scoped Oxlint: exit 0. Scoped Oxfmt: all six files formatted, exit 0.
- `git diff --check` on the six source/test files: exit 0.
- Whole renderer typecheck did not pass. The retained second run reported 273
  TypeScript diagnostics and exit 1; none referred to the six changed files.
  The full output is [renderer-typecheck.log](renderer-typecheck.log). Other
  renderer/type-migration problems were not changed for this repair. The first
  uncaptured typecheck also failed, with exit 2.

An initial menu test run failed during module collection because its narrow UI
mock lacked a transitive `BlurCancelPointerSensor` export. The tests were bounded
to the actual primitive popover/button/switch and QuickPanel provider/hook
modules; the final integrated run above passed. This was a test-boundary setup
error, not evidence of a desktop product failure.

## Follow-up diagnosis after the native picker retry

The desktop operator subsequently reported that the picker opened and closed
after selecting the intended TXT, while no attachment chip appeared. No further
production upload code was changed for this diagnosis. The first menu tests
above only observed the provider store, and the broader AgentComposer tests
mocked the editor, so neither established the complete ingestion-to-token path.

Added `WorkbenchComposerAttachmentPipeline.test.tsx` to exercise the actual
`desktopMainBridge.js` file-selection implementation, actual attachment runtime
and launcher registry, actual `ComposerSurfaceRuntime`, actual Tiptap/ProseMirror
document and node views, and the actual attachment-token reconcile hook. Its
small wrapper uses the production file-to-token mapping and draft-token filter;
it does not mount the entire AgentComposer application. The native input click,
input `change`/`cancel` events and host HTTP response are controlled. File-preview
leaf presentation and QuickPanel list presentation are stubbed. jsdom Range
geometry is supplied for the editor's focus/scroll behavior.

```sh
./node_modules/.bin/vitest run --project renderer src/renderer/pages/agents/__tests__/WorkbenchComposerAttachmentPipeline.test.tsx
./node_modules/.bin/oxlint src/renderer/pages/agents/__tests__/WorkbenchComposerAttachmentPipeline.test.tsx
./node_modules/.bin/oxfmt --check src/renderer/pages/agents/__tests__/WorkbenchComposerAttachmentPipeline.test.tsx
```

Final observed run at 15:30:53 UTC: three passed, zero failed, no unhandled
errors, 1.28 seconds; scoped Oxlint and Oxfmt passed. TypeScript's `createProgram`
with the existing web tsconfig reported zero syntactic or semantic diagnostics
for this new test file; that is not a whole-renderer typecheck pass. A controlled successful
change reads the fixture file bytes through the real Bridge, reaches the mocked
host upload, creates a visible file node in the real editor, retains exactly one
staged file and a matching serialized file token/path, and enables sending
without sending. Managed synchronization does not invoke the prune callback.
Controlled cancel and empty-file change produce no upload or token, keep sending
disabled, and allow the picker to open again.

The existing managed-token, cached-file, partial-token-send refusal and
attachment reconcile tests were also run with an exact name filter: three files,
seven passed, 316 skipped, zero failed, 4.64 seconds. That result does not
establish native picker behavior.

Retained failed diagnostic observations:

- One early positive assertion compared Uint8Array objects from different realms;
  it was corrected to compare their byte arrays. This was a test assertion issue.
- A run at 15:28:11 UTC had three passing assertions but exit 1 due to an
  unhandled teardown race: the picker's queued focus restoration ran after
  editor destruction. The observed stack was `AttachmentButton.tsx:44` →
  `composerInputAdapter.ts:98` → Tiptap `Editor.get commands`, with
  `TypeError: Cannot read properties of null (reading 'commands')`. This is a
  controlled stale-callback finding, not proof of the desktop's missing-chip
  cause. A follow-up would guard or cancel delayed focus when the owning editor
  is destroyed; production was left untouched under the root operator's freeze.
  The later authorized repair and red-to-green evidence are recorded in
  [composer picker error recovery](../2026-10-18-composer-picker-error-recovery/OUTCOME.md).
- The diagnostic fixture now owns cleanup and drains editor/picker animation
  frames before unmounting. Intermediate runs exposed jsdom's absent Range
  geometry and initial editor-focus/menu timing; geometry shims and waiting for
  actual initial frames resolved those fixture conditions. Failed runs were
  not counted as passing evidence.

Normal controlled ingestion did not reproduce reconcile pruning. A detached
input's native WebKit event delivery, native picker lifetime during HMR, and
actual host upload failures remain outside this hermetic result. The operator
is isolating the desktop on a fixed frontend build before drawing a product
conclusion from another retry.

## Explicitly not claimed

- No GUI, mouse, CUA surface, real file picker, live provider or task submission
  was operated by this subagent. SH02/SH03 and actual task-corpus delivery still
  require the root operator's desktop verification.
- No model answer, source attachment, account state or credential was copied.
- No renderer-wide typecheck pass, full application build or complete frontend
  test-suite pass is claimed. Only the commands and counts above were observed.
- No native WebKit picker success, HMR cause, or full live AgentComposer upload
  result is claimed by the controlled ingestion tests.
- The root operator's `2026-10-18-task-corpus-cua/OUTCOME.md` was not edited.
