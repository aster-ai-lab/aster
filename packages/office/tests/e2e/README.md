# Markdown command regression

This browser harness mounts the shipped public `MarkdownEditor` with the same
`presentation="note"` used by Aster knowledge notes. It uses real Office, TipTap,
dialogs, Markdown serialization, selection and keyboard events. The two read-only
fields expose the host's change/save callbacks; they do not implement formatting.
The outside input catches shortcuts escaping the editor.

The suite catches toolbar no-ops, lost selections, advertised shortcut aliases
that do not execute, broken link/image insertion, malformed tables, broken
history, and a save callback missing the current edit. Inputs and expected
Markdown are synthetic literal contracts, not results computed by the editor.

From `packages/office`, serve the built package (rebuild Office after source fixes):

Stop this harness before rebuilding Office, then restart it; a live rebuild
temporarily removes lazy chunks. Keep built artifact hashes fixed during a gate.

```sh
./node_modules/.bin/rsbuild dev --config tests/e2e/rsbuild.config.ts
```

From the repository root:

```sh
python3 packages/office/tests/e2e/markdownMatrix.py target/markdown-editor.acl
aster-test check target/markdown-editor.acl --json
aster-test run target/markdown-editor.acl \
  --browser-driver standalone --browser-executable agent-browser \
  --max-parallel-scenarios 2 --infrastructure-retries 0 --json
aster-test run packages/office/tests/e2e/markdown-journeys.acl \
  --browser-driver standalone --browser-executable agent-browser \
  --max-parallel-scenarios 2 --infrastructure-retries 0 --json
```

The generator asserts a closed inventory: 28 toolbar actions and 44 advertised
shortcut chords, each in visual, source and split modes (216 scenarios), plus
18 link/image validation and dismissal scenarios (234 total). The separate six
journeys cover history in both mode directions, selection and marks after undo,
outside-input shortcut scope, Unicode undo/save, table cell restoration and
split-preview task checkbox history. Use
the installed runner's `run --help` for supported scenario filtering; the current
runner has no `--only` option. The matrix generator's `--base` accepts an
explicit loopback origin when another test lane owns port 58128. The installed
driver must support Aster Test protocol 15; the observed standalone driver is
`agent-browser 0.26.0`. Value assertions use role/name because this driver's
label-state probe only matches associated HTML labels. Textarea selections use
arrow keys because its CDP Meta+A did not invoke native textarea Select All.
Dialog URL fills use the observed input aria-label selector because semantic
fill is not supported by this adapter; validation checks the explicit
aria-invalid contract rather than native text-input validity.
Visual block expectations include Office's empty trailing paragraph; source
expectations preserve the original Markdown bytes.

This is browser editor coverage, not certification of the complete knowledge
page, native Tauri draft storage, native keyboard delivery, disk persistence,
physical IME, clipboard, or drag-and-drop. Those require the separate owned
native knowledge campaign. No fake `window.api` or Tauri IPC is installed here.

## Shared controls

The `/controls` entry mounts the real Office picker, format dialog and proofing
dialog with the built package stylesheet. `controls-journeys.acl` covers keyword
search, nested Escape and focus restoration, pointer selection, query-free
keyboard selection, custom drafts with Tab, exact editor patches and a 420px
window. The base-control case verifies that a pending button retains the idle
button's exact dimensions, and that text inputs and form pickers have the same
height. It waits for the shared popup's real exit before checking sustained
absence; the trigger must become collapsed immediately.
The English-locale case checks the real proofing dialog title, Close control
and focus returning to its invoker, plus the imperative prompt's default labels.
The motion case reads actual animation events: 160ms enter and 200ms leave.
The prompt cancellation case catches blur resurrecting a request during exit.
All dialog lifecycle tests wait for actual entry/exit before the original
visibility, content and focus assertions.

```sh
aster-test check packages/office/tests/e2e/controls-journeys.acl --json
aster-test run packages/office/tests/e2e/controls-journeys.acl \
  --browser-driver standalone --browser-executable agent-browser \
  --max-parallel-scenarios 2 --infrastructure-retries 0 --json
```

The reduced-motion suite uses a real preference in its owned Chromium process;
it asserts that the platform query reports reduce and that no dialog animation
events occur. It does not change the operating system preference:

```sh
AGENT_BROWSER_ARGS=--force-prefers-reduced-motion \
  aster-test run packages/office/tests/e2e/controls-reduced-motion.acl \
  --browser-driver standalone --browser-executable agent-browser \
  --max-parallel-scenarios 1 --infrastructure-retries 0 --json
```

The default `aster` adapter currently reports browser 0.3.2 as unsupported. The
repository's standalone adapter works with protocol 15. This harness does not
claim native IME, screen-reader, complete editor or Studio workflow acceptance.

## Real file round trips

The independent editor profile serves port 58129. It mounts the built public
DocumentEditor, SpreadsheetEditor and PresentationEditor with real WASM/font
assets. Export creates a real file, import reopens it, and the returned content
is mounted in the editor. HMR, live reload and lazy compilation are disabled so
an entry compilation cannot reset a workflow during conversion.

From packages/office, after building:

```sh
./node_modules/.bin/rsbuild dev --config tests/e2e/editors.rsbuild.config.ts
```

From the root:

```sh
aster-test check packages/office/tests/e2e/editors-journeys.acl --json
aster-test run packages/office/tests/e2e/editors-journeys.acl \
  --browser-driver standalone --browser-executable agent-browser \
  --max-parallel-scenarios 2 --infrastructure-retries 0 --json
```

The three cases preserve edited Unicode DOCX text through undo/redo and reopen,
keep an unprotected XLSX sheet editable across two save/reopen cycles, and
preserve all PPTX slide names and edited speaker notes. They are bounded file
and editor checks; full formatting fidelity and Studio persistence are separate.

For initial-import diagnostics, `?editor=spreadsheet&import=1` exports and imports
the template before mounting. Add `&derived=1` to retain recalculated results
through the public derived callback. The read-only `Initial content` and
`Editor content` fields expose actual codec/editor values for comparison.
The default journeys keep their existing inputs. The real Studio host suite
is documented in `apps/studio/tests/e2e/office/README.md`.

`spreadsheet-derived-journeys.acl` verifies that consumers using only `onChange`
still receive computed formula caches, retain them through a real XLSX round
trip and do not acquire an initial Undo entry. It uses the same editor profile.

## PDF worker and file journeys

The PDF profile serves port 58130 and mounts the built public PdfViewer with
its real PDFium worker. pdf-lib creates the three-page source with distinct
widths (420, 421, 422) and rotations (0, 90, 180), then independently reads the
actual saved/exported bytes. Reopening passes the saved Blob back through the
public source API. The fixture does not replace the worker or plugin methods.

From packages/office, after building:

```sh
./node_modules/.bin/rsbuild dev --config tests/e2e/pdf.rsbuild.config.ts
```

From the repository root:

```sh
aster-test check packages/office/tests/e2e/pdf-journeys.acl --json
aster-test run packages/office/tests/e2e/pdf-journeys.acl \
  --browser-driver standalone --browser-executable agent-browser \
  --max-parallel-scenarios 2 --infrastructure-retries 0 --json
```

The cases cover page navigation, search, read-only mode, source retry, refused
and rejected saves, page insertion/deletion/rotation/reordering with undo,
extraction/splitting, actual file-input merge, new text annotation persistence,
and visible failure/search feedback at 375px. Accepted-save counts advance only
after parsing the resulting PDF. The small merge.pdf fixture was generated
with the installed pdf-lib: one 777 × 333 page, rotation 270, Helvetica text
“Merge page”. Its size and rotation are asserted after upload/merge.

pdf-text-edit-regressions.acl preserves the repaired annotation re-entry defect:
after leaving for the Select tool, returning to the same box must save Reviewed.
Its original input and expected text remain unchanged. The four cases in
pdf-keyboard-regressions.acl also cover Backspace, native text undo/redo,
continuing to type after keyboard Save, and a single meaningful Undo after Save.
Run both suites with the same options above. The focus repair is regenerated
from EmbedPDF source; see patches/README.md. The input observer only records
native composed input events and never edits the annotation model.

This profile does not certify signatures, encrypted PDFs, tagged structure,
forms, physical native input, complete mobile dialog motion or Studio storage.
