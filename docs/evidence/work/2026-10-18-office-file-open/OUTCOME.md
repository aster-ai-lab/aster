# Office file open

Date: 2026-10-18. Local source evidence for the AsterDream office editor open path. Not a published release.

The renderer unit tests open a representative workbook, a Word file, and a deck in the in-app editor. The spreadsheet path records its own import segment. The Word open does not record spreadsheet segments.

## Verification

Commands run from `apps/work`. Vitest is `./node_modules/.bin/vitest` v5.0.1, project `renderer`.

| Check | Result | Evidence |
| --- | --- | --- |
| `vitest run --project renderer` on `OfficeDocumentSurface.test.tsx`, `spreadsheetOpenPath.test.ts`, `spreadsheetPreloadSplit.test.ts`, `preloadWorkspaceEditor.test.ts` | 4 files passed, 11 tests passed, 0 failed. Exit 0. Duration 2.01s. Start 13:44:44 | `{SCRATCH}/office-open.log` |

Per-file counts from that log: `preloadWorkspaceEditor.test.ts` 1 passed; `spreadsheetPreloadSplit.test.ts` 1 passed; `OfficeDocumentSurface.test.tsx` 6 passed; `spreadsheetOpenPath.test.ts` 3 passed.

## Explicitly not claimed

The previous 20% cold-import median gate was not measured again. No desktop process was replaced. No claim that the whole Work renderer suite or a Playwright office spec passed in this run.
