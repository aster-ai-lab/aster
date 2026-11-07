# Direction-safe titles and workspace tab dragging

Verified on 2026-11-07, macOS arm64. No commit was created.

## Result

- Added a shared `sanitizeDisplayText` path for sidebar session rows and
  workspace Wiki titles. It strips ANSI styling, NUL/C0/C1 controls, invisible
  and directional override controls, normalizes whitespace, and applies NFC.
- Literal markup, CJK, emoji including ZWJ sequences, RTL text, and long
  unbroken strings remain plain text. Titles use `dir="auto"` and
  `unicode-bidi: plaintext` with the existing single-line truncation.
- The workspace header keeps its drag region, while the controls container is
  explicitly `data-tauri-drag-region="false"`, so tabs and close actions are
  not intercepted by window dragging.

## Verification

- `displayText.test.ts`: **4/4 passed** for ANSI/control cleanup, whitespace,
  NFC, literal markup, emoji/RTL, and long unbroken input.
- `SessionItem.test.tsx`: **2/2 passed**, including normalized direction-aware
  special titles.
- `WorkspaceWikiCatalog.test.tsx`: **9/9 passed**, including an actual click on
  the artifacts tab after title normalization.
- `KnowledgeNotesPage.test.tsx -t "hides memory and board only on task details"`:
  **1/1 passed**, covering header drag-region and control boundaries.
- Total targeted coverage: **16/16 passed**.
- Scoped Oxfmt passed. Scoped Oxlint reported **0 warnings and 0 errors**.
- Production Rsbuild output `.studio/dist/title-drag-fix`: passed.
- `STUDIO_RENDERER_DIST=.studio/dist/title-drag-fix just build-studio`: passed
  with full companion staging.
- `git -C apps/studio diff --check` passed.

## Explicitly not claimed

- No full `KnowledgeNotesPage` suite pass is claimed; that suite has an existing
  `SelectionHighlight` mock omission unrelated to this change.
- No real-window visual acceptance, full TypeScript check, Windows/Linux GUI
  run, release installer, or full-repository lint result is claimed.
