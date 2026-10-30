# Office workbook state — issues #209 and #211

The document host now feeds changed artifacts back to the mounted controlled
editor. Previously it updated only the export reference: sheet commands changed
that reference while the displayed workbook remained on its original content.
Clicking either a worksheet tab or the next-sheet button now changes the active
tab, grid and formula input. Saving keeps the mounted editor and undo history.

The host also owns spreadsheet save status. It suppresses the library's default
"已自动保存" hint; the spreadsheet omits that output when the existing string
override is empty. Dirty and failed-write state continue to come from the host.

Visual review found a further display defect after the semantic flows passed:
Fortune's collection emptiness test discarded raw numbers and booleans without
cached display text. The existing Office dependency patch now rejects only
missing/empty text and converts accepted values to paint text. Numeric zero,
negative values and booleans render; cached formatting and intentionally empty
display strings remain intact. Cell values are not rewritten by this paint fix.

Studio commit: `e518c91e41e220478dadfba69e10488038128f8f`. Product source stays
in that private submodule; the parent carries the Office sources, patch and
evidence. No new dependency or configuration option was added.

## Verification

macOS arm64, Node 26.8.1, pnpm 12.6.0, Playwright 1.62.1, Chromium
151.0.7922.34 and WebKit 26.5. Commands ran from `apps/studio` unless noted:

| Check | Observed result |
| --- | --- |
| `pnpm exec vitest run --project renderer src/renderer/pages/documents/__tests__/OfficeDocumentSurface.test.tsx` before | 2 passed, 1 failed: a controlled draft was lost during the existing lossless retry case |
| Same file plus `DocumentOpenView.test.tsx` after | 20 passed |
| `officeCellRendering.test.ts` before | 4 passed, 5 failed; number, zero and boolean failures were also reproduced independently |
| `pnpm exec vitest run --project scripts scripts/__tests__/officeCellRendering.test.ts scripts/__tests__/officeShortcutLocale.test.ts` after | 10 passed: 9 paint cases plus the existing 14-value locale contract |
| Final baseline browser replay | 2 passed, 8 failed; both selection-consistency cases passed; no page errors |
| `node target/office-issues/browser.cjs after` | All 10 Chromium/WebKit scenarios passed with no page errors |
| `node target/office-issues/visual.cjs` | Both engines painted `330` and `210`; screenshots reopened and visually checked |
| `node target/office-issues/fidelity.cjs` | All 4 saved XLSX files reopened and compared with the source fixture |
| Offline lockfile regeneration and frozen install, both with `--ignore-scripts` | Passed; only 4 references to the existing core patch fingerprint changed |
| `pnpm --filter @a3s-lab/office build` | Passed |
| `pnpm exec rsbuild build` | Production renderer passed; warnings retained in the log |
| Scoped Oxlint, ESLint and Oxfmt checks | Passed |
| Root `cargo fmt --all -- --check` and `git diff --check` | Passed |
| `pnpm typecheck:web` | Failed: 373 diagnostics in 123 files verified byte-identical to committed sources; no changed-file diagnostics |

The browser runs the real Office import/export codecs, document host,
SpreadsheetEditor and Fortune Workbook. Only the file-write boundary is
synthetic. It exercises direct tab selection, next/previous navigation, A25 to
C2 navigation and arrow keys, normal save, failed save, retry and undo after
save. The original CSV inputs and retry assertions are unchanged; the host test
fixture now follows the editor's controlled-content contract rather than
hiding missing parent updates behind its own draft state.

The synthetic XLSX contains two ordered sheets, 27 populated cells and six
detail records. Each saved file retains all names, ordering and populated cells;
only the explicit formula-input edit at C2 changes from numeric `120` to text
`121`. The other 26 values and types are identical. Existing export behavior
expands formatted blank used ranges to `A1:L40`; byte identity and unchanged
unused grid extent are not claimed. Exact source/output ranges and file hashes
are retained in `workbook-fidelity.json`.
The monthly grid captures are available directly for [Chromium](chromium-monthly.png)
and [WebKit](webkit-monthly.png).

The final host baseline replay uses the immutable pre-fix Studio source and
holds the Office bundle/Wasm and browser inputs fixed. The primitive-paint
counterfactual is independently covered by the actual core implementation.
All 70 old/new installed core files were reopened: 68 are identical; only the
two module variants change the emptiness condition and text normalization.
Generated modules and the lockfile were produced by pnpm, not hand-edited.

The first probe needed explicit grid focus after using a fixture control. A
later probe wrote receipts inside Vite's watched tree, causing hot reload and
cancelled fetches. Those diagnostic receipts remain separate. Final receipts
are outside the watched tree, with the same workbook bytes, assertions and
deadlines. No assertion was weakened. The first host-only visual captures also
remain, showing why semantic success alone was insufficient for the paint bug.

The final browser archive preserves 97 named entries. Every gzip, source hash,
saved workbook and archive entry was reopened and compared. All owned browsers
and both dedicated Vite processes are closed. Concurrent main commits were
fast-forwarded before landing; their changes are confined to web files/evidence
and do not alter the tested product sources. The source base and landing parent
are both recorded in `verification.json`.

## Explicitly not claimed

No native Tauri/VoiceOver run, Windows/Linux runtime, original 4,845-byte reported
workbook replay, full Studio lint/test pass, green global CI, installer update
or new release is claimed. A3S Test remains unavailable with browser adapter
0.3.2 (required >=0.4.0,<0.5.0); existing Playwright supplied the browser proof.
The full web typecheck remains red; no guard or diagnostic was suppressed.
The selected A25/C2 sequence did not reproduce #210, so it and #215 remain open.
