# Retain the native file-picker input until selection settles

Result: the actual Bridge now keeps its hidden file input in the current
document while selection and ingestion are pending, then removes it on every
existing terminal path. The focused set passed **42 tests across three files**.

## Basis and limits of the diagnosis

The official [WebKit issue 201289](https://bugs.webkit.org/show_bug.cgi?id=201289)
describes a file picker losing its result after its detached input is collected.
Its reproduction creates a detached input, registers a change listener, opens
the picker, then forces collection before selecting a file. That construction
matches the Bridge's former input lifetime. The owning audit agent also
independently traced WebKit's weak chooser-client references and Inspector's
explicit input-retention workaround.

This is evidence for repairing a real lifecycle gap. It does not prove that
the desktop operator's observed XLSX attempt triggered collection, that its
particular installed WebKit build exhibits that issue, or that this change
resolves the native observation. The operator owns the fixed-renderer retry.

## Minimal production change

Only the existing `studioMainBridge.file.select` function changed. It marks
the file input hidden and appends it to the current document body before the
existing programmatic click. A local `settle` removes the input before invoking
the original resolver or rejector. Cancellation, empty change, successful
ingestion, ingestion rejection and picker-open rejection all use that cleanup.
The input remains attached throughout asynchronous ingestion.

The existing filters, multi-selection flag, byte ingestion and validation,
Promise results and privacy-safe stage records are preserved. No timeout,
focus fallback, automatic retry, new option, permission or dependency was added.
The parallel `app.get_info` hunk in the same production file was retained.

## Red / green evidence

Commands from `apps/work`:

```sh
./node_modules/.bin/vitest run --project renderer src/renderer/shell/studioMainBridgeFileSelect.test.ts -t 'keeps the hidden pending picker'
./node_modules/.bin/vitest run --project renderer src/renderer/shell/studioMainBridgeFileSelect.test.ts src/renderer/pages/agents/__tests__/WorkbenchComposerAttachmentPipeline.test.tsx src/renderer/shell/studioMainBridge.test.ts
./node_modules/.bin/oxlint src/renderer/shell/studioMainBridge.js src/renderer/shell/studioMainBridgeFileSelect.test.ts src/renderer/pages/agents/__tests__/WorkbenchComposerAttachmentPipeline.test.tsx
./node_modules/.bin/oxfmt --check src/renderer/shell/studioMainBridge.js src/renderer/shell/studioMainBridgeFileSelect.test.ts src/renderer/pages/agents/__tests__/WorkbenchComposerAttachmentPipeline.test.tsx
```

- Before changing production: **one failed, 15 skipped**, exit 1. After a later
  event-loop task, `document.querySelector('input[type="file"]')` returned null.
  See [before-fix.log](before-fix.log).
- After the fix: **42 passed, zero failed**, three files, 1.51 seconds, exit 0,
  no unhandled errors. See [after-fix.log](after-fix.log). This contains the
  previous 41 tests plus the new lifecycle case: Bridge selection 16, existing
  real Composer pipeline four, existing Bridge regression 22.
- The new lifecycle case obtains the input from the real jsdom document after
  a later task, verifies it is hidden and retains multi-selection, dispatches a
  controlled change event, and gates the real Bridge's fetch Promise. It verifies
  the node remains attached after ingestion starts and is removed on completion.
  This is a hermetic DOM/Bridge boundary test, not a native WebKit/GC simulation.
- The existing 15-case selection matrix now also asserts pending connection
  and terminal removal across success, cancellation, empty change, upload
  rejection and click rejection, each with a working, rejecting or throwing
  log sink. The real Composer pipeline's one old detached-node assertion was
  updated to require the now-attached input; its other behavior is unchanged.
- Scoped Oxlint, Oxfmt and `git diff --check` passed. The web-config TypeScript
  program reported zero syntactic or semantic diagnostics for the two affected
  test files; see [scoped-typecheck.log](scoped-typecheck.log). This is not a
  whole-renderer typecheck or test pass.

Production and test hashes are recorded in [source-hashes.json](source-hashes.json).
The production SHA-256 at that stage was
`8ba1f5459fbc37d12b8f009caead6f905864ffcebbdff5f0fb43f2a675485d9a`.
The unchanged stage-log contract and safe PID-scoped read command are documented
in [the diagnostic evidence](../2026-10-19-composer-picker-stage-diagnostics/OUTCOME.md).

## Cleanup after native upload verification

The root operator subsequently observed native selection, ingestion, attachment
chips and actual submissions for the allowed A07 workbook and A03 text file in
the frozen v4 desktop. These observations establish successful uploads in that
run; they do not establish the outcome of either original task or prove a GC
cause for the former missing-chip observation.

The temporary stage logger was then removed from `file.select`. The retained
hidden input, all selection/ingestion behavior and every cleanup path remain.
The regression matrix now tests the five actual outcomes once each rather than
repeating them against three diagnostic log sinks. Its delayed-ingestion
lifecycle check remains unchanged.

The same three-file command passed **32/32 tests** (2.08s, zero unhandled errors).
Scoped Oxlint and Oxfmt passed for the two changed files. No new desktop build
or corpus pass is claimed by this cleanup; the next frozen frontend must include
it before repeating native upload checks.

Cleanup source SHA-256:

- `studioMainBridge.js`: `b181b7cc47b2b421ba644c9e05e56b55c0ea619706f6a4837fc3b235fe85a97b`.
- `studioMainBridgeFileSelect.test.ts`: `258aa70e59980b881feedabe10f41ae79eb379c261140c8ae089bc56ffeae367`.

## Explicitly not claimed

- No native picker, CUA, application, provider or corpus task was operated here.
- No actual garbage collection, installed WebKit mechanism, native upload
  recovery or task-quality pass was observed by these tests.
- No attachment, account state or credentials were read or copied. The root
  operator's CUA outcome was not edited; no other production file was changed.
