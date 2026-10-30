# Tasks: Aster native recording and Capture evidence

**Input**: [spec.md](spec.md), [plan.md](plan.md), [research.md](research.md), [data-model.md](data-model.md), [contracts/](contracts/).

**House rule**: every behavior-changing task carries its focused hermetic test. Native acceptance is additional and never replaced by mocks.

## Phase 1: Evidence engine and native behavior

- [ ] T001 [US1/US2/US3/US4] Internalize `kitlangton/scrawl@7b184c67767a1aee6c1d44640d94d8446cd5fed1` under `crates/capture` with MIT/provenance and root workspace membership; run the unmodified upstream Rust/Swift test baseline before Aster edits.
- [x] T002 [US1] Add the library integration API and closed recording/target/destination/error types in `crates/capture/src/lib.rs` and `apps/studio/src/shared/types/`; sibling tests cover parse/as_str, unknown-value and invalid-transition refusal.
- [x] T003 [US1] Implement the host recording controller and deterministic fake adapters in `apps/studio/src-tauri/src/capture/`; tests cover one device owner, start/stop/cancel/finalize, stale sessions, crash recovery and no duplicate delivery.
- [ ] T004 [US1/US3] Integrate macOS capture, microphone, input timeline, ink/pin/laser and native control surface using the vendored engine/helper; hermetic protocol tests plus a real recording smoke verify media, permission and stop behavior.
- [ ] T005 [US1/US2] Wire transcription, keyframe selection, auto-crop, contact sheet, summary and manifest into one bundle build; fixture tests verify exact filenames, record counts, timestamps, units and upstream cross-reading.

## Phase 2: Composer, editor, agent and delivery

- [x] T006 [US1] Add the typed evidence attachment and send-time projection in `apps/studio/src/renderer/utils/message/` and composer state; tests prove contact-sheet/summary delivery, no implicit source-video upload, stable token identity and no auto-submit.
- [x] T007 [US1] Add `ComposerRecordingControl` immediately after `ComposerScreenshotControl`, native target selection, immutable status events and stop/cancel controls in `apps/studio/src/renderer/components/composer/`; focused tests cover real controls, accessibility, localization, draft preservation and owner changes.
- [ ] T008 [US1/US3] Register narrowly scoped native recording commands and connect the button to the host controller; permission tests prove only the intended main-window capability can record, stop, inspect or export.
- [ ] T009 [US3] Open evidence bundles in the existing visual editor, preserve frame annotation/redaction and create sanitized derivatives; renderer/Rust tests verify source preservation, derivative revision and export refusal.
- [ ] T010 [US2] Add session-bound `request_recording`, authorized inspect/zoom/frames operations and content conversion in `apps/studio/src-tauri/src/agent_plane/`; tests cover foreign/stale sessions, path bounds, cancellation, timeout and untrusted content handling.
- [ ] T011 [US3] Add export, reprocess, recovery, retention, pin/unpin, shrink and cleanup flows; Rust/Swift tests verify timestamps, audio offsets, active markers, pinned preservation and no implicit source overwrite.
- [ ] T012 [US1/US3] Add recording/microphone/transcriber/retention/shortcut preferences through existing ACL sources and regenerate schemas/locales; tests cover conflicts, missing models, permission refusal and disabled operations without another config format.
- [ ] T013 [US4] Implement explicit Aster destination, page registration, bounded localhost matching and smart send/offer/keep with runner-up/undo; tests cover recipient precedence, ambiguous refusal, loopback/token rules and replayed decisions.

## Phase 3: CaptureKit, ingest, CLI and platforms

- [ ] T014 [US4] Preserve and integrate CaptureKit appFrames/ReplayKit, gestures, touches, app events/context, pairing, USB import and delivery replies in `crates/capture/Sources/` and ingest; Swift/Rust tests verify ordered timelines, protocol and idempotency.
- [ ] T015 [US4] Integrate the durable HTTPS outbox and authenticated headless ingest with payload/time/resource bounds; tests cover offline retry, permanent failure, 2xx deletion, auth and duplicate delivery.
- [ ] T016 [US2/US3/US4] Wire every public and diagnostic operation in `contracts/cli-inventory.md`, including inspect, zoom, frames, export, devices, link, import, retranscribe/reprocess, recover, autocrop, clean, pin, route, helper protocols and headless ingest; hermetic tests cover each operation and refusal without new language bindings.
- [ ] T017 [US5] Implement the Windows capture/input/transcription adapter using the same evidence engine; portable adapter tests plus Windows native evidence when hardware is available, with explicit unavailability for unsupported operations.
- [ ] T018 [US5] Implement a real Linux capture/input/transcription adapter or leave the row explicitly incomplete; tests prove no fake success and unrelated composer functions continue when native capability is absent.
- [ ] T019 [US1/US3] Stage and sign helper binaries inside the Aster bundle and validate entitlements/resource manifests; packaged-app tests verify helper signatures, sandbox resources and permission behavior on the actual installed build.

## Phase 4: Native acceptance and evidence

- [ ] T020 [US1/US2/US3] Run isolated macOS CUA for area/window/screen recording, narration, marks, stop/cancel, review, redaction, attachment, agent zoom, export and recovery; reopen actual bundles/media and compare source data.
- [ ] T021 [US4/US5] Run available iPhone/CaptureKit/HTTPS and other-platform native acceptance; retain exact device/OS results and explicit blockers instead of inferring a pass.
- [ ] T022 [All] Run focused Rust/Swift/renderer suites, applicable typecheck/lint/i18n/format and workspace guards; update `contracts/capability-matrix.md` from pending to verified only with direct evidence.
- [ ] T023 [All] Record `docs/evidence/desktop/2026-10-30-scrawl-evidence/OUTCOME.md` with Result, What landed, commands, counts, per-capability evidence and Explicitly not claimed; reopen deliverables and record unresolved work.
- [ ] T024 [All] Inspect final file scope and new build outputs, remove only this task's disposable files, and stage nothing. The later user request explicitly authorizes committing, merging and pushing this bounded change to main; unrelated work stays untouched.

## Final Verification Gate

- [ ] Every owning package's focused tests pass
- [ ] `cargo fmt --all -- --check` and applicable workspace checks pass
- [ ] No new crates.io `a3s-*` dependency, second workspace/lockfile, native `<select>`, or non-ACL configuration
- [ ] Native recording and agent inspection actually passed on claimed platforms
- [ ] Bundle and exported-media contents match their source and compatibility contracts
- [ ] Every capability-matrix row is verified or explicitly incomplete; no silent scope reduction
