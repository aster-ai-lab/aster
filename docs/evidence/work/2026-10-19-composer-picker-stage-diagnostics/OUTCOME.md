# Temporary picker stage diagnostics

Result: the actual `studioMainBridge.file.select` now emits bounded, content-free
stage records through the existing native `log_to_host` command. The focused
regression run passed three files / 41 tests, with no unhandled errors.

## Scope and observation boundary

The desktop operator observed an XLSX picker close without a chip, visible error,
or corresponding staged file, after separately observing the intended file
selected. This change adds diagnostics for the next fixed-renderer reproduction;
it does not establish the cause of that desktop observation.

Only `studioMainBridge.js` production code changed. Its picker reports `created`,
`change` with the selected-file count, `cancel`, `ingest` with the successful
metadata count, and `error` with the fixed code `file_select_failed`. The log
message and source are fixed strings. No filename, path, content, options, raw
error, runtime descriptor or credential is passed to the logging command.
Synchronous throws and asynchronous rejections from logging are contained, and
logging is never awaited. Picker input lifetime, filters, bytes, transport,
attachment reconciliation and return values retain their existing behavior.

## Validation

From `apps/work`:

```sh
./node_modules/.bin/vitest run --project renderer src/renderer/shell/studioMainBridgeFileSelect.test.ts src/renderer/pages/agents/__tests__/WorkbenchComposerAttachmentPipeline.test.tsx src/renderer/shell/studioMainBridge.test.ts
./node_modules/.bin/oxlint src/renderer/shell/studioMainBridge.js src/renderer/shell/studioMainBridgeFileSelect.test.ts
./node_modules/.bin/oxfmt --check src/renderer/shell/studioMainBridge.js src/renderer/shell/studioMainBridgeFileSelect.test.ts
```

- Final test run: **41 passed, zero failed**, three files, 1.54 seconds. The
  [captured output](focused.log) retains the command's existing Vite configuration
  and route-file warnings; no suite-wide claim is made.
- New actual-Bridge tests: 15 passed. Success, cancellation, empty change,
  upload rejection and picker-open rejection are each exercised with a working,
  rejecting and synchronously throwing host logger. Exact records are asserted
  and checked for absence of the private fixture's names, paths, body, token and
  errors. Upload counts and original promise results are also asserted.
- Existing real Bridge / AttachmentRuntime / ComposerSurface pipeline: four
  passed. Existing `studioMainBridge.test.ts`: 22 passed.
- Scoped Oxlint and Oxfmt passed. TypeScript's web-config program reported zero
  syntactic or semantic diagnostics for the new test source; see
  [scoped-typecheck.log](scoped-typecheck.log). This is not a whole-renderer
  typecheck pass. Scoped `git diff --check` passed.

## Host log locator and safe reading

`host_log.rs::log_dir()` derives the macOS directory from `data_root::home_dir()`
plus `Library/Logs/AsterDream`. On this machine it is
`/Users/roylin/Library/Logs/AsterDream`. `host_log::init()` opens
`aster-dream-YYYYMMDD-HHMMSS-PID.log` for every launch and redirects stdout/stderr
there when stderr is not a terminal. `lib.rs::log_to_host()` writes the structured
record to stderr. No additional sink or permission was added.

Before the next renderer freeze, the latest observed filename was
`aster-dream-20261019-065614-93807.log`; only filenames were listed, not log
contents. A new launch must use its own observed PID and log filename. After the
operator launches the fixed renderer, this command reads only the stage records
for that explicit PID (replace `V3_PID` with that PID):

```sh
rg -n -F 'message=composer-file-select ' /Users/roylin/Library/Logs/AsterDream/aster-dream-*-V3_PID.log
```

The record's JSON `data` contains the stage and optional count or fixed error
code. No matching record alone cannot prove an event was absent: the renderer
must first be confirmed to contain this source, and a functioning logging sink
must be established by a `created` record from that same picker attempt.

## Explicitly not claimed

- No native picker, CUA, application, model or corpus task was operated here.
- No XLSX desktop failure cause, successful native upload, or corpus E2E pass is
  claimed. The controlled upload 403 is a regression fixture.
- No source attachment, account state, credentials or other session content was
  read or copied. The root operator's CUA outcome was not edited.
- Diagnostics are temporary; the desktop operator will review actual stage
  evidence before deciding whether to remove them or retain necessary handling.
