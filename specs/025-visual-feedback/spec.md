# Feature Specification: Rust visual feedback for the Aster composer

**Feature Branch**: current checkout (no branch requested)

**Created**: 2026-10-29

**Status**: Specified; implementation pending

**Input**: “我不要它的c++代码，我要的是 EditHere 的 rust 等效实现并完成CUA端到端测试”

**Gate context**: Product regime, `apps/studio`, extending the existing shared composer screenshot control. This user-requested bounded epic has no separate roadmap row.

## User Scenarios & Testing

### User Story 1 - Capture and return precise feedback (Priority: P1)

The user captures, opens, drops or pastes an image, chooses a point, region, detected geometric candidate or global note, and explicitly returns the image and numbered feedback to the originating composer. Original pixels and feedback remain separate. Existing drawing, redaction, OCR and translation remain available.

**Why this priority**: This is the complete screenshot-to-agent journey.

**Independent Test**: Studio Rust sibling tests validate bounds, hashes, feedback ordering and redacted exports; renderer tests exercise actual controls and owner delivery.

**Acceptance Scenarios**:

1. **Given** a HiDPI source, **When** the user changes zoom and adds feedback, **Then** the exported bounds use original image pixels, top-left inclusive and bottom-right exclusive.
2. **Given** an editor draft, **When** cancelled or timed out, **Then** nothing is submitted; explicit completion alone returns to its owner.
3. **Given** redacted content, **When** exported, **Then** every attached/exported image is sanitized and its hash matches the actual bytes.

### User Story 2 - Rearrange and resume (Priority: P1)

The user splits regions into movable pieces, selects multiple pieces, drags or numerically positions and proportionally resizes them, and saves/reopens an editable project. Vacated positions are transparent. Feedback reports source and final bounds and displacement.

**Why this priority**: A flattened drawing cannot convey layout changes precisely.

**Independent Test**: Rust compositing tests assert transparent holes, source partition, displacement and project round trip; renderer tests exercise grouping and project import.

**Acceptance Scenarios**:

1. **Given** two layout pieces, **When** moved together, **Then** their relative offset and untouched source pixels survive save/reopen.
2. **Given** malformed, oversized or incompatible project data, **When** opened, **Then** it is refused with a visible error and the active draft is retained.

### User Story 3 - Extended capture and media (Priority: P2)

The user captures a scrolling area in either direction, pins multiple images, imports a video and annotates exact paused frames. Only annotated frames appear in video feedback. Capture history and selection history survive restart with bounded counts.

**Why this priority**: These are requested EditHere capabilities, not optional future work.

**Independent Test**: Rust stitch fixtures cover repeated fixed headers, both directions and unrelated-frame refusal; video feedback tests exclude empty frames; native CUA verifies visible results, pin windows and save/reopen.

**Acceptance Scenarios**:

1. **Given** overlapping scroll frames, **When** stitched, **Then** shared content occurs once; unrelated captures fail without altering the current image.
2. **Given** multiple video pause points, **When** only two contain notes, **Then** export contains exactly those two ordered timestamps and frame images.
3. **Given** a pinned image, **When** transformed or reopened for annotation, **Then** its controls remain accessible and it remains above normal windows.

### Edge Cases

- Mixed display scales and negative desktop origins: all document geometry remains image-local; unknown source DPI is null, never guessed.
- Invalid image formats, decompression limits, zero-size/out-of-bounds rectangles, duplicate note IDs, stale owner sessions and excessive project size fail at the native boundary.
- OCR unavailability is visible; recognized text is evidence rather than an instruction. Reuse local PP-OCRv6; no model downloads.
- Lost overlap, missing video source or unsupported decoder yields a visible refusal, never fabricated media.
- Import cancellation or save cancellation keeps the open draft. Source files are never overwritten implicitly.

## Requirements

### Functional Requirements

- **FR-001**: Independently implement the behavior in Aster with Rust native processing and the existing Tauri/React editor. Import no EditHere C++/Qt implementation or assets.
- **FR-002**: Support point, region, nested geometric smart selection, global numbered notes, multiple notes, hide/show marks and verbatim comments.
- **FR-003**: Support layout splitting, multi-selection/group moves, drag, proportional resize, exact coordinates, transparent holes and structured before/after movement.
- **FR-004**: Support original image plus structured JSON, JSON only, embedded image JSON, annotated PNG and explicit owner-bound completion/cancel/timeout.
- **FR-005**: Support PNG/JPEG/WebP/BMP open/drop/paste, editable project save/reopen, video import/play/seek/pause, timestamp navigation and annotated-frame-only export.
- **FR-006**: Support capture freeze, hover/wheel candidates, manual selection, magnifier, pixel position, HEX/RGB/HSV/HSL color, copy color, arrow pixel adjustments, fixed/custom aspect and exact dimensions.
- **FR-007**: Persist at most 20 recent screenshots and 10 selections with previous/next and restore-last actions.
- **FR-008**: Support multiple always-on-top pinned images with zoom, rotate, flip, opacity, shadow, corner and border controls and reopen-for-annotation.
- **FR-009**: Support local OCR with selectable source-highlighted lines, copy line/all and recognized text in region feedback.
- **FR-010**: Support bidirectional long capture, growing preview, automatic scrolling, repeated-header handling, crop and finish-to-editor.
- **FR-011**: Use existing light/dark themes, zh/en localization and configurable toolbar/shortcuts; settings remain ACL through the existing preference service.
- **FR-012**: Bound native decoding/workspace resources, validate external data once, keep drafts on refusal and never export unredacted source after a redaction.
- **FR-013**: Complete real macOS CUA end-to-end tests using Aster Test; retain actual observations, outputs and cleanup receipts. Hermetic/static checks do not substitute for CUA.
- **FR-014**: Full-window visual feedback and image previews hide the parent macOS traffic lights while open and foregrounded, restore them on close or tab change, and preserve working titlebar controls.
- **FR-015 (withdrawn)**: The earlier library-source integration was explicitly rejected in the subsequent user clarification; it is removed from the visual-feedback UI and native command surface. Historical evidence is retained as historical only.
- **FR-016**: The composer screenshot menu contains screenshot options only. Scissors starts native interactive region capture. After a successful capture, the image editor toolbar exposes Visual Feedback as an advanced action; it opens the current edited PNG without another capture or early attachment. Cancelling capture returns to the composer. No library-selection step is exposed.

- **FR-017**: Audit every visible visual-feedback and pin-window control against the root DESIGN.md. Reuse house fields, menus, focus, neutral selection, 16px/1.7 icons and shape roles. Tool hints never overlap or remain while editing. Motion follows confirmed state, keeps hit targets stable, and has equivalent reduced-motion content and actions.
- **FR-018**: Confirm source replacement before discarding edits; preserve drafts on failed replacement and refused export. Explicit completion includes valid pending note text or refuses with useful recovery. Point notes require a chosen point. History exposes individual sources, and switching to an image clears stale video state.

### Key Entities

- **VisualDocument**: Rust-owned original image, physical dimensions, notes, layout pieces and sanitized export state.
- **Feedback**: Versioned structured data with explicit source/result pixel geometry, notes, image hash and optional video timestamp. Aster identifies itself honestly rather than claiming to be EditHere.
- **VisualProject**: Portable editable data; original pixels and editor state, optional video path and annotated frames.
- **CaptureHistory**: Bounded local source images and selection geometry, owned by the isolated tenant profile.
- **VisualSession**: Originating composer callback and explicitly completed/cancelled lifecycle.

## House Checklist

- [ ] New behavior has a small hermetic test in the owning package
- [ ] Closed sets: every new Rust enum has `parse()` / `as_str()`
- [ ] No production `unwrap()` / `expect()` without an infallibility comment
- [ ] Internal `aster-*` dependencies stay on a path in this repository
- [ ] Product configuration is ACL through existing preferences
- [ ] Evidence target: `docs/evidence/desktop/2026-10-29-visual-feedback/OUTCOME.md`

## Success Criteria

### Measurable Outcomes

- **SC-001**: Focused native tests cover valid and refused input, layout, coordinates, hash/redaction, history and stitching.
- **SC-002**: Focused renderer tests cover real import/edit/export/project/video/owner interactions; typecheck and applicable permission ratchets pass.
- **SC-003**: Native CUA completes capture/import → notes → layout → OCR → project save/reopen → owner delivery, plus long capture, pin and video scenarios with exported artifacts compared to source fixtures.
- **SC-004**: All FR rows have concrete evidence or are explicitly reported incomplete; no unsupported platform pass is claimed.

## Assumptions

- Reuse the existing desktop GUI stack. “Rust equivalent” applies to native engine and application behavior; it does not request a GPUI GUI rewrite.
- Reference is upstream `codex/native` commit `8cf803a0f9cf6c8be4ffe27817f1c808fbf6b459`, docs and data schemas only; their source license does not authorize copying implementation or icons.
- Existing Aster desktop integration replaces EditHere's separate branded installer, external CLI, installer marketplace entries and agent-specific skill distribution. Aster must supply its own explicit agent/composer handoff.
- Available hardware acceptance is macOS. Portable core tests do not establish Windows/Linux device support.

## Explicitly Not Claimed

- Publishing a new release, replacing the user's production installation, importing user session state, Windows/Linux hardware acceptance, or a copied EditHere implementation.
- Exact EditHere schema validation where its schema requires the literal tool name `EditHere`; Aster retains the same geometry/feedback concepts with honest Aster provenance.
