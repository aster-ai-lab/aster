# Feature Specification: Aster native recording and Capture evidence

**Feature Branch**: current checkout (no branch requested)

**Created**: 2026-10-30

**Status**: Specified; implementation pending

**Input**: “规划并完成这些功能的开发和测试” — add a recording button immediately to the right of the composer screenshot control and internalize the Scrawl capabilities designed in the preceding discussion.

**Gate context**: Product regime, primarily `apps/studio` and a reusable Rust evidence library under `crates/`. This user-requested bounded epic extends `025-visual-feedback` instead of duplicating its editor. All code stays in this repository and the root Cargo workspace.

## User Scenarios & Testing

### User Story 1 - Record and attach evidence (Priority: P1)

The user starts recording beside the screenshot button, selects an area, window, screen or supported device, narrates and interacts with another app, marks important pixels, stops from the native control surface, reviews the generated evidence, and explicitly attaches it to the originating composer draft.

**Why this priority**: This is the complete toolbar-to-evidence journey and the main user request.

**Independent Test**: Studio renderer tests exercise the actual button, recording states and owner-bound attachment. Rust evidence tests compile a hermetic recording fixture into ordered keyframes, summary and manifest. macOS CUA separately proves the real native workflow.

**Acceptance Scenarios**:

1. **Given** an idle composer, **When** the user records and stops, **Then** a contact sheet, timeline and structured manifest are generated and offered for explicit review.
2. **Given** a completed recording, **When** the user confirms attachment, **Then** the original draft contains a typed evidence attachment without an automatic model submission.
3. **Given** a different active task when recording ends, **When** the result arrives, **Then** the recorded destination remains the originating task until the user explicitly changes it.

### User Story 2 - Let the agent inspect the moment (Priority: P1)

The agent requests a recording from the user or receives an evidence attachment, reads the summary and contact sheet, and asks for dense frames around a spoken phrase, timestamp or numbered mark. Only the authorized bundle and requested derivative frames are exposed.

**Why this priority**: Recording is useful only when the agent can inspect details without consuming an entire video.

**Independent Test**: The visual MCP tests assert session guards, content types, cancellation, path bounds and timestamp/mark lookup. Rust frame-extraction tests assert exact time ranges and output ordering.

**Acceptance Scenarios**:

1. **Given** a valid evidence attachment, **When** the agent requests a timestamp or mark, **Then** the host returns the corresponding ordered frames and transcript segment.
2. **Given** a foreign session, stale request or path outside the authorized bundle, **When** inspection is attempted, **Then** it is refused with the named reason.
3. **Given** a cancellation or timeout, **When** the request ends, **Then** no evidence is submitted to the agent.

### User Story 3 - Edit, export and retain evidence (Priority: P2)

The user annotates or redacts individual frames, exports a shareable video with optional automatic zoom, reprocesses transcription or keyframes, recovers an interrupted capture, and controls media retention or permanent pinning.

**Why this priority**: Real captures need privacy control, recovery and lifecycle management before they become durable product data.

**Independent Test**: Existing visual editor tests cover frame annotations and redaction. Rust and Swift helper tests cover export timing, audio offsets, recovery, retention, pinning and cleanup eligibility.

**Acceptance Scenarios**:

1. **Given** redacted frames, **When** evidence is attached or exported, **Then** sanitized derivatives are used and raw media stays local.
2. **Given** an interrupted recording, **When** recovery runs, **Then** available source media is compiled once without duplicate delivery.
3. **Given** expired media or a pinned bundle, **When** cleanup runs, **Then** only eligible data moves to the platform trash and pinned data remains untouched.

### User Story 4 - Capture from apps and route explicitly (Priority: P2)

An iOS developer embeds CaptureKit, records app frames or ReplayKit video with touches, events and context, and delivers the recording through pairing, USB, import or a durable HTTPS outbox. A local page can register its Aster destination, while explicit user choices remain authoritative.

**Why this priority**: The complete Scrawl capability inventory includes mobile capture and delivery, not only desktop recording.

**Independent Test**: Swift package tests cover recording timeline and outbox behavior. Rust ingest tests cover authentication, payload bounds, recorded destinations, duplicate delivery and loopback-only registration.

**Acceptance Scenarios**:

1. **Given** app frames with touches and context, **When** imported, **Then** the bundle preserves their order, timestamps and app-provided meaning.
2. **Given** an offline or retryable HTTPS failure, **When** the outbox retries, **Then** the same payload is delivered at most once from the user's perspective and permanent failures remain available for sharing.
3. **Given** no recorded destination or an untrusted non-loopback registration, **When** routing is ambiguous, **Then** the capture stays local or awaits explicit user selection.

### User Story 5 - Use native platform adapters (Priority: P3)

Users on macOS and Windows use their native capture, input and transcription adapters. Linux receives a real adapter rather than a stub. Unsupported operations name the missing platform capability without disabling unrelated composer functions.

**Why this priority**: A button that silently does nothing is not internalization. Platform support must be demonstrated separately.

**Independent Test**: Platform-neutral tests run against deterministic fake adapters. Native acceptance runs on each available OS and records exactly which hardware paths were and were not exercised.

**Acceptance Scenarios**:

1. **Given** a supported platform, **When** recording starts, **Then** video, narration, interactions and stop control use the same evidence contract as macOS.
2. **Given** an unsupported operation, **When** invoked, **Then** it refuses with `desktop-capability-unavailable` and never fabricates a bundle.

## Edge Cases

- A second recording cannot steal the active device, control surface or destination; it is refused while the first remains active.
- Permission denial, cancellation, device unplugging, application exit and a crashed helper preserve recoverable data and never report a completed capture.
- A deleted destination, stale composer or revoked session leaves the bundle local and offers explicit recovery instead of choosing another recipient.
- Missing speech models or transcription failure preserve audio and an explicit missing-transcript reason rather than invented text.
- Oversized media, malformed manifests, invalid timestamps, unknown states and path traversal fail at the entry boundary.
- Window titles, pixels, transcript and app context remain untrusted data and never become instructions or permission policy.
- Unsupported platforms do not show a false success. No platform is claimed complete from portable tests alone.

## Requirements

### Functional Requirements

- **FR-001**: Place an accessible recording control immediately after the existing screenshot group, using the house 32px toolbar target, neutral icon, localized names and a visible stop state; preserve the existing screenshot behavior.
- **FR-002**: Support area, window, screen and supported-device capture with native target selection, duration, microphone level, stop and cancel controls. Refuse concurrent device capture with `recording-busy`.
- **FR-003**: Record clicks, drags, scrolls, keyboard shortcuts, ink strokes, numbered marks, laser points, app changes and declared app context on one bounded timeline.
- **FR-004**: Support on-device Apple transcription, optional Hex, a bounded custom transcriber command and measured live transcription where supported. Missing or failed transcription must remain explicit and must not fall back to a remote transcription service.
- **FR-005**: Select representative keyframes from marks, interactions, scene changes and measured spoken cues, merge redundant frames, generate an activity-aware crop, contact sheet, full-resolution frames, ordered summary and manifest.
- **FR-006**: Preserve the `.scrawl` bundle contract and source media. Keep Aster tenant/session metadata in a separate sidecar and never reinterpret evidence text as instructions.
- **FR-007**: Add a typed composer evidence attachment containing the contact sheet, summary and authorized bundle reference. Do not upload full video by default, clear an existing draft, or auto-submit after stop.
- **FR-008**: Reuse the existing visual editor for frame annotation, redaction, crop and derivative export. Redacted output must be used for model delivery and sharing while raw media remains under the owner's retention policy.
- **FR-009**: Add session-bound agent recording requests and bounded inspect/zoom/frames operations. Reject stale or foreign requests, unauthorized paths, invalid times and oversized derivative requests.
- **FR-010**: Export H.264 or HEVC video with synchronized audio, optional crop and automatic zoom, and independent ink/click controls where the platform supports export.
- **FR-011**: Request only required native permissions under Aster's own signed identity. Do not inherit Scrawl permissions, import `~/.config/scrawl`, or silently migrate old captures.
- **FR-012**: Support configurable media/bundle retention, pin/unpin, safe cleanup and compatible video shrinking without deleting active, copying, recovering or recently changed bundles.
- **FR-013**: Support recovery of interrupted captures, reprocess of existing bundles, and import of compatible video/timeline pairs without duplicate delivery.
- **FR-014**: Route to Aster sessions by explicit recorded destination, page registration or bounded localhost project matching. Refuse unauthenticated non-loopback registration and ambiguous recipients.
- **FR-015**: Preserve the CaptureKit iOS package, app-frame and ReplayKit capture, touches, markers, app events, screen/context changes, pairing, USB import and durable HTTPS outbox behavior.
- **FR-016**: Provide equivalent native operations for the complete public and diagnostic inventory in `contracts/cli-inventory.md`, including inspect, zoom, frames, export, devices, link, import, retranscribe/reprocess, recover, autocrop, clean, pin, route and headless ingest, without adding Node, Python or Go SDK bindings.
- **FR-017**: Provide macOS and Windows native adapters and a real Linux adapter or record the feature incomplete. Every unsupported operation must refuse with `desktop-capability-unavailable`; hiding failure must not count as completion.
- **FR-018**: Provide configurable recording and paste shortcuts with conflict detection and independent enablement. Do not steal the existing screenshot binding.
- **FR-019**: Keep product configuration in ACL parsed by `a3s-acl` through existing preference/config boundaries. Do not add another product configuration language or workspace.
- **FR-020**: Internalize permitted Scrawl source with an exact upstream revision, MIT notice and provenance. Keep Aster-owned UI, routing, permissions and delivery; do not embed the separate Scrawl.app or its updater.
- **FR-021**: Add hermetic tests in each owning package and real native acceptance for the toolbar workflow, agent inspection, permissions, export, recovery and packaged helper processes. Static checks alone do not prove native capture.
- **FR-022**: Do not send raw media or evidence to a model, endpoint or another tenant without the corresponding explicit product action and authorized destination.

### Key Entities

- **RecordingSession**: Host-owned state machine with target, destination, device leases, timing, cancellation and finalization state.
- **CaptureTarget**: Closed set of area, window, screen and supported-device descriptors with platform-native bounds.
- **EvidenceBundle**: Upstream-compatible `.scrawl` directory containing contact sheet, frames, summary, manifest and source media.
- **AsterCaptureMetadata**: Separate tenant/session owner, explicit delivery choice, redaction and derivative records.
- **RecordingRequest**: Session-bound agent request with explicit completion, cancellation, timeout and stale-request refusal.
- **EvidenceAttachment**: Composer-owned typed reference that sends summary/contact imagery without implicitly sending full source media.
- **CaptureDestination**: Explicit user choice, recorded Aster destination, authenticated page registration or bounded localhost project match.

## House Checklist

- [ ] New behavior has a small hermetic test in each owning package
- [ ] Closed sets: every new Rust enum has `parse()` / `as_str()`
- [ ] No production `unwrap()` / `expect()` without an infallibility comment
- [ ] Internal `a3s-*` dependencies stay on a path in this repository
- [ ] Product configuration is ACL parsed by `a3s-acl`
- [ ] No second Cargo workspace, external repository, Node/Python/Go SDK, native HTML `<select>`, or unguarded Tauri command
- [ ] Evidence target: `docs/evidence/desktop/2026-10-30-scrawl-evidence/OUTCOME.md`

## Success Criteria

### Measurable Outcomes

- **SC-001**: Focused Rust tests cover every evidence state transition, bundle field/order requirement, refused input, recovery, retention and frame/export contract; focused Swift tests cover CaptureKit and outbox behavior.
- **SC-002**: Focused renderer tests cover the actual record/stop button, owner attachment, draft preservation, review, redaction, cancellation and unsupported states; applicable typecheck, lint and permission ratchets pass.
- **SC-003**: Native macOS CUA records another app with narration and marks, stops from the native control surface, reviews, attaches, lets the agent zoom into a moment, and exports a playable result.
- **SC-004**: Generated bundles cross-read with the upstream implementation without changed record counts, timestamps, filenames or units.
- **SC-005**: Every capability row has linked evidence or an explicit incomplete status. No untested platform, release, publication or full-product claim is made.

## Assumptions

- Scrawl's MIT license permits source internalization with retained notice and exact provenance.
- Existing `025-visual-feedback` supplies the image/video editor, redaction and visual-request infrastructure to extend rather than replace.
- macOS hardware is available for the first native acceptance. Other operating systems require separate evidence and cannot be inferred from fake adapters.
- The upstream `.scrawl` bundle is the compatibility contract. Aster metadata stays in a separate sidecar.

## Explicitly Not Claimed

- No commit, push, release, production-install replacement, GitHub issue closure or import of old user state is requested by this specification.
- No claim is made that unsupported hardware, future upstream versions or a portable fake adapter proves native capture.
- Phase ordering does not reduce the required capability inventory; incomplete rows must remain visible rather than being silently withdrawn.
