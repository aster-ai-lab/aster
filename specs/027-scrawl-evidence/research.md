# Research

Reference implementation: `kitlangton/scrawl` at `7b184c67767a1aee6c1d44640d94d8446cd5fed1` (`0.3.13`, MIT). The revision is pinned so source and behavioral comparisons remain reproducible. No separate repository or updater is introduced.

| Decision | Existing prior art | Reason |
| --- | --- | --- |
| Vendor the evidence engine in `crates/capture` | `src/model.rs`, `keyframes.rs`, `autocrop.rs`, `sketch.rs`, `export/` upstream | UI, agent and headless ingest all need the same evidence logic; copying only selected algorithms would silently drop capabilities |
| Keep Scrawl.app and GPUI shell outside Aster | Existing Tauri/Studio shell | Aster owns its windows, identity, settings and updates; only native capture/review surfaces are integrated |
| Put the button after the existing screenshot group | `ComposerScreenshotControl.tsx`, root `DESIGN.md` §4 | Every composer already shares this entry and the 32px/48px geometry is specified |
| Reuse image/video editing and redaction | `VisualFeedbackEditor.tsx`, `visual_feedback/{mod,media}.rs` | Existing editor already supports imported video, exact frame annotations, projects and sanitized derivatives |
| Add recording operations beside the existing visual request | `agent_plane/visual_mcp.rs` | Existing session-bound request queue, explicit completion, cancellation and timeout are the correct trust boundary |
| Use a typed evidence attachment rather than a fake file | `composerAttachment.ts`, send-time path entries | A `.scrawl` directory is not an image; model delivery defaults to contact sheet + summary + bounded reference |
| Keep canonical `.scrawl`, separate Aster metadata | Upstream `ScrawlBundle`; Studio tenant stores | Cross-reader compatibility and tenant/session ownership must not change upstream fields |
| One host recording controller | `host_family.rs`, `applet_window.rs` | Prevent device contention, duplicate delivery and cross-tenant capture; preserve recoverable state on exit |
| Native helper only for Apple-specific frameworks | Upstream `helper/`; Studio `studio_delivery.py sign-resources` | SpeechAnalyzer, AVFoundation and iPhone capture need Swift; the helper is staged and signed inside Aster, not installed separately |
| ACL preferences only | Existing `usePreference`, native preference store, classification generator | No import of Scrawl JSON configuration and no second product configuration language |
| Explicit recorded destination | Existing visual-request owner and `captureDestination` design | Switching composers cannot leak evidence to another task; no automatic model submission |
| Platform adapters behind one contract | `host_family.rs`, upstream `capture/{macos,windows}.rs` | macOS/Windows reuse native implementations; Linux requires new work and must fail closed until real |
| Bounded agent inspection | `visual_mcp.rs`, existing host workspace trust and path guards | Agent can inspect only authorized bundle derivatives, not arbitrary media or home paths |
| Native proof in addition to hermetic tests | Existing isolated managed launcher and visual CUA receipts | A mock cannot establish ScreenCaptureKit, microphone, Accessibility or packaged helper behavior |

## Capability inventory retained

The implementation and acceptance matrix in `contracts/capability-matrix.md` tracks:

- Area/window/screen/iPhone targets, picker, global shortcuts and paste.
- Narration, microphone selection, Apple/Hex/custom/live transcription and explicit missing-transcript states.
- Clicks, drags, scrolls, keys, ink, pins, laser, app/window/context changes.
- Keyframe selection, scene/spoken cues, auto-crop, contact sheet, summary and manifest.
- Preview, inspect, zoom, frames, export/camera, reprocess, recover, import and demo fixtures.
- Clipboard, explicit session delivery, page registration, localhost routing and smart send/offer/keep decisions with undo.
- CaptureKit appFrames/ReplayKit, gestures, app events/context, pairing, USB, HTTPS outbox and headless ingest.
- Retention, pin, shrink, recovery and bounded cleanup.
- Settings, permissions, CLI operations and local helper diagnostics. Scrawl's separate updater and branded installer are deliberately replaced by Aster's existing mechanisms, not duplicated.

## Source policy

Copy only from the pinned MIT source. Retain LICENSE and exact provenance in `docs/source-manifest.json` and `docs/provenance/`. Keep the existing root Cargo workspace and lockfile. Do not add a second repository, Node/Python/Go binding, model provider, native HTML select or product config format.
