# Research

Reference revision: `8cf803a0f9cf6c8be4ffe27817f1c808fbf6b459`. Documentation/schema receipt: `target/visual-feedback-20261029/reference.json`.

| Decision | Existing prior art | Reason |
| --- | --- | --- |
| Reuse shared composer entry | `ComposerScreenshotControl.tsx` | All input surfaces already share this caller |
| Preserve drawing editor | `ComposerScreenshotEditor.tsx`, `editor/` | Drawing, redaction, OCR and zoom already work |
| Rust data/image engine | `src-tauri/src/screenshot.rs`, root `image` dependency | Native boundary already owns screen permissions and OCR; typed data and exact image operations belong here |
| Offline OCR | `document_processing`, `crates/ocr` | Existing ~31 MB PP-OCRv6; no additional model |
| File dialogs/attachments | `window.api.file`, `attachScreenshotFile` | Existing trusted path and owning-composer delivery |
| Preferences | `usePreference` / native data store | Preserve ACL configuration and tenant isolation |
| Native GUI evidence | Existing upgrade Aster Test bridge | Installed CUA driver supports native macOS; browser adapter mismatch cannot be called a pass |

Functional parity inventory: point/region/global notes; geometric wheel candidates; layout split/move/resize/group; original/JSON/embedded/annotated export; explicit handoff; source/drop/paste; video frames; project persistence; frozen capture; magnifier and color formats; pixel keyboard; aspect/dimensions; 20/10 history; multi-pin transforms; OCR; bidirectional long capture/auto-scroll; theme/languages/shortcut and toolbar settings.

Clarify phase: zero unresolved markers. The user's rejection of C++ is resolved by an independent Rust engine with existing Aster GUI. No approval gate or new architecture migration is inferred.
