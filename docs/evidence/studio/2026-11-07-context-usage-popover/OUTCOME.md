# Context-usage popover: reference layout on Aster design tokens

Verified on 2026-11-07, macOS arm64. No commit was created.

The first pass reproduced the reference's bespoke hex palette and a custom card
surface. A follow-up pass brought it back to DESIGN.md; this file records the
delivered state.

## Result

- The composer's context button previously opened a 256px panel with one thin
  bar and a `used / max (pct)` line, while hover showed a dark one-line
  tooltip. It now opens a popover with the reference's information layout:
  title + close, a 28px percentage with exact used/window tokens, a segmented
  category bar, a dot legend with each category's share of the window, a
  divider, a centered window/tier line, and a read-only capacity scale.
- `PopoverContent` keeps the shared `@aster-ai-lab/ui` surface
  (`--border`, `--radius-panel`, `--surface-raised`, `--shadow-floating`).
  No page-level radius, border, background or shadow is painted over it, per
  DESIGN.md §14 (transient popups use shared primitives).
- All colors are design tokens: `text-foreground`, `text-muted-foreground`,
  `bg-secondary` (Field `#f5f5f5`, the documented segmented-track color),
  `border-border-subtle`, `bg-popover`, `shadow-[var(--shadow-elevated)]`,
  `bg-foreground`. Category series use `--chart-1..5`, falling back to
  `--muted-foreground`. A scan found **0 bespoke color literals** in the three
  changed source files.
- Type roles follow §3: 14px medium title/window line (Control), 28px semibold
  reading (Greeting size), 13px rows, 12px meta, 11px status. The close action
  is a 24px hit target with a 14px Lucide glyph at stroke 1.7 and a 6px radius
  (§4). Numbers use tabular figures (§14 numbers and meters).
- Data behavior: real `usage.categories`, hidden window-filler entries, an
  honest "no breakdown" line, and a suppressed hover tooltip while open. The
  bottom scale is a read-only tier readout, not a draggable setting;
  thresholds (500K / 200K / 100K) and marker positions are display assumptions.

## Verification

- Vitest (final state): **6 files, 21/21 passed**.
- Scoped Oxfmt `--check` on all 8 changed source files: **passed**. Scoped
  Oxlint `--deny-warnings`: **0 warnings, 0 errors**. `git diff --check`:
  **passed**. A scan for hex/rgb color literals in the three changed source
  files returned **0**.
- 9 new i18n keys: Unicode-sorted in both catalogs; `validateSource` /
  `validate` reported **0 failures**.
- Full `i18n:check`: **fails on a pre-existing ordering defect** —
  `knowledge.views.navigation` sits among `knowledge.memory.*` keys at HEAD,
  plus two pre-existing locale indent defects. This task's locale diff contains
  only the 9 new keys; those unrelated lines were left untouched.
- `STUDIO_RENDERER_DIST=.studio/dist/context-usage-design-spec rsbuild build`:
  **passed** (58.7s).
- Real Chromium rendering against that production CSS: card **288 x 365px**;
  marker right edge **279px** vs card right edge **312px** (no clipping);
  track resolves to **rgb(245, 245, 245)** (Field), series dots resolve from
  `--chart-*` tokens, and the capacity fill resolves to the Ink foreground.
  [preview.png](preview.png) is the delivered rendering.

## Why the category breakdown is empty

- `apps/studio/src-tauri/src/agent_plane/stream_context.rs` builds the
  `data-context-usage` part with a hard-coded `"categories":[]` and only
  prompt-accounting totals (`totalTokens`, `maxTokens`, `percentage`). Two
  archived native session transcripts confirm every reading carries
  `categories: []`.
- The renderer fallback (`contextUsageFromReportedTokens` in
  `useAgentSessionContextUsage.ts`) also synthesizes `categories: []` from
  `message.metadata.stats.contextTokens`. The reference image's breakdown
  comes from Claude Code CLI's `SDKControlGetContextUsageResponse`, which
  Aster's native/ACP plane does not emit. The empty state is therefore a data
  limitation, not a rendering bug; inventing category numbers was rejected.
- Follow-up fix: with no categories, the progress track used to be a bare
  empty groove even though a total reading existed. It now paints the measured
  total as one Ink segment (`data-context-usage-total`), and the copy states
  that the runtime did not report a breakdown instead of implying zero usage.
- Verification for this pass: **6 files, 21/21 Vitest**; scoped Oxfmt and
  Oxlint (0 warnings / 0 errors); `git diff --check`; 9 i18n keys with
  `validateSource` / `validate` **0 failures**; `rsbuild build` **passed**
  (1m 7.2s); Chromium render of the no-breakdown state measured the total
  segment at **5px** in Ink.

## Explicitly not claimed

- No live Aster window or GUI end-to-end run. The visual check loads the real
  production stylesheet against a faithful static replication of the panel
  DOM, not the running React popover.
- No full frontend suite, full TypeScript check, `pnpm lint`, Windows/Linux
  run, or commit.
- Category labels keep existing i18n names (系统工具 / 消息 / MCP 工具)
  rather than the reference's grouped wording; no semantics were invented.
- The reference's green capsule was not kept as a bespoke color, and it was
  not turned into an interactive context-length control.
- No claim that `--chart-*` tokens have a documented legend palette in
  DESIGN.md; they are the design system's `theme.css` chart tokens.
