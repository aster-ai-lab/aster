# Tasks: Rust visual feedback

**Input**: [spec.md](spec.md), [plan.md](plan.md). Every behavior includes its focused test.

## Phase 1: Behavior and tests

- [x] T001 [US1/US2] Add typed validated native document/feedback/project and Rust image compositing in `apps/studio/src-tauri/src/visual_feedback/`; sibling tests for coordinates, redaction, layout, hash and malformed input.
- [x] T002 [US3] Add geometric candidates, bidirectional overlap stitching and bounded history using existing image/store facilities; sibling tests for nesting, repeated headers, refusal and retention limits.
- [x] T003 [US1/US2] Extend shared screenshot UI with independent visual note/layout/project/export controls and typed native bridge; focused renderer tests for notes, movement, cancellation, export and round trip.
- [ ] T004 [US3] Add long capture, multiple native pin windows and video frame feedback/persistence, plus actual behavior tests and explicit decoder/overlap refusals.
- [ ] T005 [US1] Add exact geometry, magnifier/color, aspect/keyboard/history/toolbar and shortcut preferences, source drop/paste/import and OCR line feedback; focused renderer/native tests.

## Phase 2: Wiring

- [x] T006 [US1/US3] Register native commands narrowly, wire every shared composer caller and origin-bound explicit completion; extend actual command permission regression and localization.
- [ ] T007 [US1/US2/US3] Build isolated managed Aster instance, run early native CUA smoke and complete every native user scenario; compare exported artifacts to synthetic sources. Keep failure receipts and cleanup owned resources.

## Phase 3: Evidence

- [ ] T008 Record `docs/evidence/desktop/2026-10-29-visual-feedback/OUTCOME.md` with Result, What landed, per-FR evidence and Explicitly not claimed; reopen artifacts before declaring completion.
- [ ] T009 Verify individual file scope before any user-requested commit. No commit currently requested.

## Final Verification Gate

- [ ] Focused owning Rust and renderer suites pass
- [ ] `cargo fmt --all -- --check` and applicable workspace checks
- [ ] No new remote A3S dependency, native select, second workspace/lockfile or non-ACL settings
- [ ] Native CUA actually passed; no static/unit substitute

## User follow-up: native window chrome and library sources

- [x] T010 [FR-014] Reuse native traffic-light suppression through the foreground preview frame; check close/tab restoration and physical titlebar menu clicks.
- [x] T011 [FR-015 withdrawn] Remove the rejected library-source UI, its native command/permission and obsolete tests; retain only shared data adapters now used by other existing pages.
- [ ] T012 Repeat the pre-annotated split/move failure after its Rust ownership fix, then complete native library/chrome, project, OCR, pin, video and long-capture acceptance. Keep earlier failed/aborted receipts.
- [ ] T013 [FR-004/FR-005] Verify actual saved project/archive bytes after the macOS IPC array-response fix, reopen the saved project through the file chooser, and preserve the original corrupt-file receipt.
- [x] T014 [FR-016] Remove Visual Feedback from the composer menu; verify native capture → image toolbar → advanced Visual Feedback → annotation → explicit JSON/PNG attachments, plus capture cancellation.

## User follow-up: complete functionality and Arc-style feedback

- [x] T015 [FR-017/FR-018] Fix draft replacement, pending-note completion, point selection, source/history/video transitions and stale tooltips; align editor/pin components with DESIGN.md and reduced-motion-aware microinteractions. Add focused behavioral regression tests.
- [ ] T016 [FR-001–FR-018] Re-run native CUA across source/import, notes/geometry, layout/redaction/crop/drawing, OCR/color/history, export modes/project/archive, multiple pins, video frames, scrolling capture, settings/focus/cancellation. Reopen actual artifacts and record per-function proof and limitations.
- [ ] T017 [Original EditHere parity] Replace the coarse capability inventory with atomic cases from the frozen guide; implement and test omitted pin actions/gestures, horizontal capture and preview/cropping/recovery, automatic editable per-note OCR/export choice, canvas navigation and existing-note geometry adjustment, feedback-JSON reopening, and control-level accessibility candidates. Require native CUA plus real artifact checks for each case; preserve the user's post-capture advanced-entry flow.
