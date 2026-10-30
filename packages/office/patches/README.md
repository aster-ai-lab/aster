# Office dependency patches

## EmbedPDF text focus, 2.14.3

`embedpdf-free-text-focus.source.patch` changes the original FreeText and
CalloutFreeText components from the upstream `v2.14.3` tag. Their editing flag
was cleared on blur but only restored when `isEditing` changed. Focusing the
same editor again could therefore display new text without committing it.
Both components now restore that flag on focus, keep the focused contenteditable
DOM under native input ownership, and skip commits when text has not changed.
This preserves the caret during keyboard Save and avoids adding empty undo
entries when focus moves to a toolbar.
Default prompt text retains its whole-text selection on re-entry through the
existing cross-framework double-press helper; ordinary text keeps native word selection.

Source: https://github.com/embedpdf/embed-pdf-viewer/tree/v2.14.3/packages/plugin-annotation/src/shared/components/annotations

`@embedpdf__snippet@2.14.3.patch` is the regenerated runtime patch. The snippet
bundles these components, so patching only plugin-annotation would leave the
shipped viewer unchanged. The npm declarations and LICENSE stay unchanged.

Regenerate from the tagged source, not by editing the minified bundle:

1. Apply the source patch at the upstream source root.
2. Build the Preact annotation adapter using upstream `createConfig` and the
   Preact tsconfig aliases. Its declaration output is unnecessary because no
   public type changed. Keep the unchanged published base module at 2.14.3.
3. Clean the previous snippet output, then build with its upstream Rollup config, resolving the annotation
   Preact entry to that rebuilt adapter. Keep upstream locked runtime versions,
   including Preact 10.28.3 and tailwind-merge 3.4.0.
4. Copy the five generated JavaScript runtime files into a `pnpm patch`
   directory. Generate the patch with rename detection disabled; the current
   pnpm patch reader cannot apply rename/copy hunks.
5. Register the resulting patch in Office, the root Bun manifest and Studio's
   pnpm workspace. Regenerate their locks, then verify actual installed bytes
   and run the PDF browser regressions against a rebuilt public Office package.

The source archive digest, build inputs, commands, output digests and browser
results are retained in `docs/evidence/office/2026-10-30-quality`. All 60 runtime
exports are compared with the original package before generating the patch.
