# Capability and verification matrix

Reference: `kitlangton/scrawl@7b184c67767a1aee6c1d44640d94d8446cd5fed1`. Every row needs implementation and evidence before the complete epic is claimed. Status values are `pending`, `implemented`, `verified`, `blocked`, or `not-applicable` with a reason.

| Capability | Implementation owner | Required proof | Status |
| --- | --- | --- | --- |
| Record/stop button right of screenshot | Composer control | Renderer states + native CUA | verified |
| Area/window/screen target picker and native island | Studio native adapter | Real target selection, stop/cancel | pending |
| Global recording/paste shortcuts and conflict handling | Existing shortcut/preferences | Conflict and accessibility tests | pending |
| Screen video, system audio and microphone | macOS/Windows/native adapters | Actual media and permissions | pending |
| Click/drag/scroll/shortcut and app-change timeline | Native event adapters | Timestamp/order fixtures and CUA | pending |
| Ink, pins, laser, undo and target-relative coordinates | Native overlay | CUA and manifest coordinates | pending |
| Microphone catalog/default/fallback and level | Native audio adapter | Device/fake adapter tests and native check | pending |
| Apple on-device transcription and language model | Swift helper | Real transcription and missing-model path | pending |
| Hex transcription and fallback | Upstream engine + helper | Fake Hex success/failure and estimated timings | pending |
| Custom transcriber argv, timeout and output bounds | Upstream engine | Hermetic command fixture and refusal cases | pending |
| Live transcription | Upstream engine + Swift helper | Same-audio timing and fallback proof | pending |
| Click/mark/laser keyframes | Evidence engine | Ordered candidates and exact reasons | pending |
| Scene-change and measured spoken-cue keyframes | Evidence engine | Pixel/speech fixtures, redundancy refusal | pending |
| Auto-crop and full-frame retention | Evidence engine | Activity crop fixtures and source matching | pending |
| Contact sheet, summary and manifest | Evidence engine | Cross-reader fixture comparison | pending |
| Native preview/review and frame annotation | Existing visual editor | Open bundle, edit, redact, save | pending |
| Inspect and JSON output | Evidence CLI/API | Exact manifest/summary output | pending |
| Zoom by timestamp/phrase/mark | Evidence engine + agent tools | Dense ordered frames and transcript | pending |
| Bounded range frame extraction | Evidence engine + native decoder | Exact from/to/fps output | pending |
| MP4/HEVC export, crop, zoom, ink/click controls | Export engine + helper | Playable media, timing and audio checks | pending |
| Reprocess and interrupted recovery | Evidence engine | No duplicate artifacts or delivery | pending |
| Video/timeline import and synthetic demo | Evidence engine | Import fixtures and cross-reader output | pending |
| Composer typed evidence attachment | Composer/send pipeline | Draft owner, no auto-submit, no full-video upload | pending |
| Explicit copy/paste and agent recording request | Delivery + visual MCP | Owner, cancellation and timeout | pending |
| Page SDK registration and token rules | Ingest/router | Loopback/origin/token refusal cases | pending |
| Localhost port → project → Aster session | Router + native process inspection | Exact/repository root matching and bounds | pending |
| Smart send/offer/keep, runner-up and undo window | Router + review UI | Evidence replay and explicit recipient behavior | pending |
| Cabled iPhone mirror/native video and flags | Swift helper + device adapter | Available-device acceptance or explicit hardware blocker | pending |
| ScrawlKit appFrames and ReplayKit | Swift package | Swift tests and import/cross-reader proof | pending |
| ScrawlKit gestures, events and app context | Swift package | Ordered timeline and manifest proof | pending |
| Pairing, USB import and delivery replies | Swift/Rust ingest | Protocol fixtures and idempotency | pending |
| Durable HTTPS outbox and retry classification | Swift package | Persistence, retry, permanent failure and 2xx deletion | pending |
| Headless authenticated ingest service | Rust ingest | Bounded payload, auth and delivery tests | pending |
| Media/bundle retention and pin | Evidence engine + Studio storage | Eligibility, active markers and cleanup | pending |
| Compatible video shrink | Swift helper | Preserved timestamps/audio and source replacement | pending |
| Native CLI inventory | `scrawl` target/Aster integration | Per-command hermetic smoke and refusals | pending |
| Windows capture/input/transcription | Windows adapter | Windows native evidence; no macOS inference | pending |
| Linux capture/input/transcription | New Linux adapter | Linux native evidence or explicit incomplete | pending |
| Signed helper and packaged permissions | Studio delivery | Installed signed app, helper signature and native permissions | pending |

The separate Scrawl updater, branded installer and OpenCode recipient are not copied. Aster's existing updater, package and session routing replace them. ScrawlKit package/source names and the `.scrawl` evidence contract remain compatible.

Current bounded macOS acceptance and remaining gaps are recorded in [the native outcome](../../../docs/evidence/desktop/2026-10-30-scrawl-evidence/OUTCOME.md). The composite target/native-island row remains pending because external-window recording was not completed.
