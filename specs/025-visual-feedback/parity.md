# EditHere behavior alignment

Baseline: `codex/native` commit `8cf803a0f9cf6c8be4ffe27817f1c808fbf6b459`, frozen `docs/USER-GUIDE.md`, `LONG-CAPTURE.md`, `VIDEO-ANNOTATION.md`, `FEEDBACK.md` and schemas recorded in `target/visual-feedback-20261029/reference.json`.

The product contract is the Aster screenshot/feedback workflow: capture first, then the advanced feedback button, no library step; Aster owns branding, settings, updates and agent delivery. Native image/data processing remains independent Rust. Shared house controls, layout and motion follow root DESIGN.md. Aster-specific labels, shortcut defaults and menu placement may differ; the capabilities and observable outcomes below must remain available. No C++/Qt implementation or assets are copied.

Source presence and acceptance are separate. `Native` means a real GUI operation with captured output, not a unit test or a visible button. An incomplete row stays open even when part of it is implemented. The previous six-row gap summary was not exhaustive.

| ID | Observable acceptance case | Current implementation / proof |
| --- | --- | --- |
| IN01 | Scissors captures immediately; advanced feedback opens that edited image; cancellation preserves attachments | Prior native pass: `toolbar-capture` |
| IN02 | Open/paste/drop PNG, JPEG, WebP and BMP without changing source geometry | Native PNG open/paste; format decode tests; full input matrix pending |
| IN03 | Open editable image/video projects, retain source references and reject malformed input without losing draft | Native Aster image project round trip; video/native failure matrix pending |
| AN01 | Point, rectangular, smart and global comments preserve verbatim text, order and unique numbering | Engine/renderer tests; region/global native proof; point/smart native pending |
| AN02 | Smart wheel selection retains candidate level, including non-parent overlapping candidates and pixel nudges | Partial; detailed keyboard/wheel semantics pending |
| AN03 | Clicking a marker selects its comment; leaving an edited comment preserves text | Partial; full marker hit testing and blur persistence pending |
| AN04 | Move/resize an existing note rectangle and move a point note; Delete removes selected note | Canvas drag/handles, numeric geometry, arrow-key movement and Delete with undo/OCR updates; 224 renderer tests pass; native CUA pending |
| AN05 | Hide marks without hiding the note list or altering clean source pixels | Implemented; native replay pending |
| AN06 | Undo/redo covers comments, geometry, layout and deletion | Partial tests; combined native sequence pending |
| AN07 | Replacement/close/save cancellation keeps edits; explicit return alone delivers feedback | Focused tests, earlier native owner/manual return and actual image-project replacement |
| CV01 | Zoom about the pointer with wheel; middle-button pan works even when the image is smaller than the viewport | Missing |
| CV02 | Fit/original view and fullscreen remain accessible; window drag gestures preserve editing | Partial; complete view-state/native matrix pending |
| LY01 | First explode detects blocks; leaving/reentering retains the same blocks and adjustments | Engine exists; native mode/cache replay pending |
| LY02 | Nested/multiple selection moves and resizes groups while preserving child relationships | Engine tests; earlier native move proof; full edge/corner/group replay pending |
| LY03 | Edge handles change one dimension; corner handles preserve aspect; wheel over a piece scales it | Incomplete |
| LY04 | Numeric position/dimensions/scale and 1px keyboard movement affect actual source-pixel output | Partial native geometry proof; scale and combined cases pending |
| LY05 | Moving produces an ordered numbered movement record; returning an uncommented block removes that record | Movement data exists; numbered timeline/list and automatic removal semantics incomplete |
| LY06 | Vacated areas stay transparent; attached notes follow parent motion/scale and detached blocks stop following | Engine tests; earlier native pre-annotated move proof |
| OC01 | Manual OCR exposes source-highlighted lines, exact line/all copy, language choices and large-image tiling | Native lines/all copy pass; language/large-image behavior incomplete |
| OC02 | Point/region/smart/movement notes automatically acquire local OCR from the proper source/result image | Missing; currently reads manually acquired lines |
| OC03 | Per-note OCR is editable/removable, export inclusion is independent and unavailable OCR never prevents writing | Missing |
| CP01 | Global capture freezes the screen; candidate hover, manual region and post-capture toolbar all work | Capture/toolbar native proof; full frozen-candidate path pending |
| CP02 | Magnifier follows normal selection/nudges, avoids controls, and always reports correct image pixels | Partial; currently exposed only in color mode |
| CP03 | Four color representations, cycling, keyboard/click copy and escape/right-click exit work | Helpers and native HEX pass; full interaction incomplete |
| CP04 | Move/resize/reselect bounds, fixed/custom aspect including 3:2, exact dimensions and 1px contraction/expansion | Partial; detailed direction semantics and native matrix pending |
| CP05 | Copy/save/pin include chosen corner/border styles; style reset and per-capture defaults persist | Pin style render exists; ordinary capture style/persistence incomplete |
| CP06 | 20 screenshots and 10 regions persist; previous/next, individual selection and restore-last work | Persistence tests; complete native navigation pending |
| PN01 | Multiple pins stay above normal windows and start at their original captured coordinates | Multiple windows exist; captured placement/native proof pending |
| PN02 | Center drag, edge/corner scaling, wheel scaling and fit share the same 10–400% scale | Implemented this phase; renderer wheel/fit proof; native drag/resize pending |
| PN03 | Left/right rotation, both flips, opacity, corner/border and shadow controls affect rendered/copied image pixels | Implemented this phase; real canvas pixel tests; native proof pending |
| PN04 | Pin copy and PNG/JPEG save preserve styles, handle cancellation/errors and never silently overwrite a file | Implemented this phase; UI outcome and real filesystem/codec tests; native proof pending |
| PN05 | Pin OCR highlights/copies lines; rotation/flip clears stale results; annotation excludes shadow/display opacity | Implemented this phase; renderer/pixel tests; native proof pending |
| PN06 | Context menu, Command/Ctrl+C, Escape, double click, original size and restore-position work | Implemented this phase; partial renderer proof; captured-position and full native cases pending |
| PN07 | Shadowless image has no forced outline; active/inactive shadow state is distinguishable | Default border removed; focus-related shadow state pending |
| LC01 | Vertical and horizontal collection grow from the initial viewport without duplicate revisits | Vertical/horizontal engine and renderer regressions, including repeated viewports; native scroll CUA pending |
| LC02 | Manual collection keeps waiting when scrolling stops; transient mismatch can recover by revisiting content | Manual unchanged keeps the session active; native viewport repositioning revisits older content without duplication; mismatch retry proof passes; real CUA pending |
| LC03 | Fixed-size preview follows the matched viewport and reports unmatched state without shrinking the entire long image | Fixed 240×160 latest-viewport preview with matched/unmatched/unchanged renderer proof; native CUA pending |
| LC04 | Automatic scrolling is optional, stops at actual end and preserves usable partial results on mismatch | Optional auto-scroll requires two unchanged frames and real growth before finishing; mismatch preserves partial output; native automatic/end-state proof pending |
| LC05 | Endpoint crop and reverse automatic crop affect all outputs; moving the region preserves data, resizing restarts it | Endpoint/reverse crop and region move/resize have native pixel and renderer proof: moving preserves the long document, resizing restarts on the same axis; real CUA pending |
| LC06 | Stop/cancel/restart/finish have distinct outcomes; unchanged initial frames cannot masquerade as a completed long capture | Stop discards the round and enables restart; cancel/Esc restores the original document, selection and draft; finish requires real growth. Renderer proof passes; native CUA pending |
| LC07 | Long images use bounded/tiled storage, with explicit output-specific limits and save-only oversized mode | Limits, 600 frames, owner-scoped references, bidirectional tiled stitching/migration, streaming save and full Studio build pass; real oversized CUA pending |
| VD01 | Play/pause/seek and ±1s/space navigation pause at exact timestamps without generating empty exported frames | Partial; explicit seek controls/space/native decoder matrix pending |
| VD02 | Annotating creates independent frame state; timeline/list navigation restores it and preserves pending text | Renderer pending-text regression; full native proof pending |
| VD03 | Video project reopens saved frames without source; exact source relink resumes playback | Source identity tests; native missing-source/relink pending |
| VD04 | Only annotated frames export, ordered with exact frame IDs/timestamps and proper file/embedded images | Engine tests; actual multi-frame artifact/native proof pending |
| EX01 | File, embedded and metadata-only JSON output contain source dimensions, hashes, geometry, movement and original comments | Engine tests; native file-mode completion; remaining modes pending |
| EX02 | Redaction permanently sanitizes every exported/project/archive/frame source before hashing | Engine tests; native redacted artifact replay pending |
| EX03 | JSON preview/copy/save and optional embedded compression do not change other remembered output choices | Partial JSON copy; preview/compression/separate choices incomplete |
| EX04 | Copy JSON+image as actual files uses its own temporary directory and exposes a usable fallback | Missing |
| EX05 | Annotated image output includes readable numbered feedback; clean output excludes those marks | Canvas output exists; full annotated layout/native comparison pending |
| EX06 | Saved projects restore full editing/view/block state; archive contains exactly the expected sanitized assets | Native image-project proof; full state/ZIP native proof pending |
| EX07 | Reopen feedback JSON and supported legacy versions, reconstruct movement and verify matching sidecar images | Missing |
| EX08 | Agent completion/cancel/timeout stays bound to its originating owner and resumes only after explicit completion | Unit/transport and earlier native manual-return proof; full owner-request native matrix pending |
| ST01 | Shortcut capture detects conflicts/occupied bindings, can disable actions and preserves settings on failed save/cancel | Existing preference/registration code; complete capture/draft validation missing |
| ST02 | Toolbar/default tool/default zoom and output/OCR/temporary-directory behavior persist through ACL preferences | Partial preferences; omitted settings pending |
| ST03 | Light/dark/system, Chinese/English and reduced motion preserve results/actions and restore cancelled previews | House themes/locales/motion code; full native matrix pending |
| AX01 | Capture candidates incorporate actual system control boundaries with clear accessibility permission/refusal behavior | Window/image candidates only; control-level integration missing |
| ER01 | Permission, decoder, bounds, hash, save and stale-owner failures explain recovery and retain existing work | Multiple engine/UI tests; complete native refusal matrix pending |

Aster's application installer, updater, account/tenant identity and diagnostic infrastructure remain their existing owners. They are not replaced with EditHere services. Platform claims require actual platform evidence; macOS testing does not establish Windows/Linux hardware acceptance.
