# Native artifact delivery

Date: 2026-10-18

## Result and implementation

ACP now registers the bounded `office_file` tool outside source-review sessions.
It uses the existing local `a3s-office` public Rust APIs to create, populate,
save, reopen, and validate XLSX/PPTX packages. PNG creation uses the existing
local `a3s-code-mermaid` rasterizer through `artifact_image`. Neither route
requires Bash, the Office sidecar, Python, Node, a browser, or an external Office
application. The macOS native-process qualification refusal remains intact.

The tool takes `file_path` and the closed `kind` set `xlsx`, `pptx`, or `png`:

- XLSX requires named `sheets` containing `rows` of JSON strings, numbers,
  booleans, and nulls. Strings are literal cell contents, including strings
  starting with `=`. The limits are 32 sheets, 1,000 rows per sheet, 100 columns
  per row, 10,000 cells total, and 1 MiB of serialized input.
- PPTX requires `slides`, each with a title and body text blocks. There are at
  most 30 slides, 40 characters per title, and three body blocks of at most
  100 characters each. These bounds fit the current native title/body frames;
  the fourth default body frame would wrap above earlier blocks. Tests inspect
  native semantic geometry for frame overlap and the 16:9 slide boundary.
- PNG requires self-contained `svg` source, at most 64 KiB, with numeric/px
  dimensions or a viewBox. Presentation attributes are supported. CSS styles,
  image/use/filter/script/foreignObject elements, external resources, DTDs,
  and processing instructions are refused. Dimensions are bounded to 4,096
  pixels per axis and eight megapixels.

The tool refuses existing destinations and never opens the user's source file
for modification. Source reading stays on the existing document-tool route.
Office validation and text readback happen after a save and reopen of the
generated package, before publication. The final published bytes are read back
and compared in full. The response includes the saved file path, format,
size, validation receipt, and bounded text readback.

## Collection and presentation generation

`agent/tool_render.rs` maps `office_file` to ACP's typed `Edit` kind and preserves
the explicit `rawInput.file_path`. Work's existing `ArtifactStore::capture`
accepts this path even when there is no top-level `files` list. A focused test
exercises that exact collector contract. Work now assigns the XLSX and PPTX
OOXML MIME types; PNG already had its own MIME type.

Presenton's language list was empty in Rust: the shipped JavaScript list uses
single quotes while the Rust reader accepted only double quotes. The reader
now isolates the declared list from its comment and accepts both quote styles.
The MCP schema publishes all 90 canonical language values and the working
Simplified Chinese default. Unsupported values return the canonical default
and retry instructions instead of an opaque refusal. Non-string language
values are refused. Preview acceptance explicitly says that it is not a saved
PPTX receipt, and the homepage task prompt permits native file delivery.

## Safety boundaries

`office_file` uses the session's workspace path normalization, local workspace
write capability, direct-write credential boundary, and session write claim.
Non-local backends and unbound sessions are refused. Paths are opened from a
system-root directory capability through `open_dir_nofollow` at every directory
component, including the workspace root itself. The generated bounded bytes
are written to a new temporary file beside the destination and published with
an atomic, no-clobber hard link. Cancellation is checked before publication;
failed generation leaves no named destination. Existing symlink destinations,
symlink parents, and a workspace root replaced by a symlink are refused.

The tool remains under normal governed-tool admission. Core classifies its
explicit `file_path` as a bounded workspace mutation and supports path-scoped
deny rules. Source-review, specialty, read-only verifier, and fact side-effect
classification changes are verified in the parent workstream.

## Validation

Completed verification:

| Check | Result | Scope |
| --- | --- | --- |
| Work `presenton_mcp` unit-test filter | 4 passed, reported by root | Current language-list and preview-receipt changes |
| Renderer `presentonHomepage.test.tsx` | 1 file, 3 tests passed | Current homepage task prompt |
| Native PNG module unit tests | 4 passed, reported by root | Current bounded renderer and refusal checks |
| Early Office tool test snapshot | 6 passed, reported by root | Earlier native package/PNG creation and refusal checks; does not establish final-source coverage |
| Scoped `git diff --check` | Passed | Tracked files changed by this workstream |

The renderer command was run from `apps/work`:

```text
/opt/homebrew/bin/node node_modules/vitest/vitest.mjs run --project renderer src/renderer/pages/agents/__tests__/presentonHomepage.test.tsx
```

It passed in 2.37 seconds. The installed `pnpm` command was not on this shell's
PATH, so Node invoked the already-installed Vitest entry point directly.

An explicit old-binary audit was also run:

```text
target/debug/deps/a3s_code_acp-7fdaa69d7ef354e3 office_file --nocapture
```

That executable contained seven tests: six passed, and its old PPTX geometry
test failed because it parsed a centimetre string as an integer EMU value.
The source had already been corrected to read the native centimetre format
and contained an eighth root-replacement refusal test. The old executable
therefore does not establish a failure or a pass of the final eight-test
source snapshot. No native-PPTX content failure was observed in that audit.

Final source validation completed in the parent workstream:

- `cargo test --locked --offline -p a3s-code-acp --lib office_file`: **8 passed**,
  including the corrected PPTX geometry test and replaced-root refusal.
- The freshly built ACP unit binary ran `output_check artifact_image
  turn_terminal_tests host_context::tests tool_cards_use_pager_kind_and_raw_input
  source_review_has_no_tools_even_after_a_yolo_mode_change --nocapture`:
  **21 passed**, including Edit metadata, native PNG, caller-deny and
  source-review restrictions.
- `cargo test --locked --offline -p aster-dream --lib
  office_file_inputs_are_captured_as_typed_binary_artifacts`: **1 passed**,
  covering input-path collection and the three artifact MIME types.
- Core permission tests and the later unrelated Core test-compilation failure
  are recorded by the authorization and parent workstreams. No whole-workspace
  build pass is inferred from these artifact checks.

## Explicitly not claimed

- This record does not claim a new pass rate for the complete `../tasks`
  corpus or a live model run. It does not read or alter benchmark oracles.
- It does not claim final-source coverage from the older ACP unit executable.
- It does not claim complete Office visual fidelity, embedded editable chart
  authoring, formula calculation, or editing of an existing source document.
- Chinese PNG glyph availability still depends on the existing system-font
  fallback. The PNG renderer does not add a bundled font or a network font
  download, and cross-host font fidelity is not claimed.
- No commit or publication was performed. Concurrent repository changes were
  preserved, including the independent removal of the `sheet-compute` skill.
