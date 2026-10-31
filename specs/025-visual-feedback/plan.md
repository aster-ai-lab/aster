# Implementation Plan: Rust visual feedback

**Branch**: current checkout | **Spec**: [spec.md](spec.md) | **Input**: [spec.md](spec.md)

**Gate context**: Product regime, existing private `apps/studio` submodule in the root workspace.

## Summary

Extend the shared screenshot control with an independently implemented visual feedback workspace. Rust owns typed document validation, image decoding/compositing, geometric candidates, stitching, history and feedback/project encoding. Existing React/Tauri controls provide accessible editing and existing drawing/OCR. The composer starts native capture; Visual Feedback appears only as an advanced action on the captured-image toolbar and opens its edited PNG. Library selection is outside this workflow. Preserve all unrelated working changes. Exercise an isolated managed Aster instance through native Aster Test.

## Constitution Check

| Principle | Status | Evidence in the plan |
| --- | --- | --- |
| I. Fail closed | PASS | Native bounded inputs, closed document kinds, explicit overlap/decoder/session refusals |
| II. Evidence retained | PASS | `docs/evidence/desktop/2026-10-29-visual-feedback/` |
| III. Hermetic tests | PASS | Sibling Rust modules, focused Vitest; CUA additional |
| IV. Closed sets | PASS | Native kinds/modes with parse/as_str |
| V. One workspace / published APIs | PASS | Studio root workspace, existing local deps |
| VI. Local schema changes | PASS | Visual data schemas owned by Studio; no cloud change |
| VII. Shrink-only ratchets | PASS | Narrow existing composer command capability; no allowlist increase |
| VIII. Replaceable transports | PASS | Existing Tauri command boundary and composer callback |
| IX. One configuration language | PASS | Existing preference ACL service for user settings |
| XIII. One repository | PASS | Root and existing private Studio submodule only |
| XIV. Names are not guessed | PASS | Explicit source metadata/filenames and typed formats |
| XV. Meaning is not matched | PASS | Geometric image segmentation, no semantic regex/word list |
| XVI. Pickers are drawn | PASS | House UI buttons/popovers; no native select |
| XVII. Dialogs stay in place | PASS | Existing modal and in-place opacity transition |

## Project Structure

### Documentation (this feature)

`specs/025-visual-feedback/`: spec, plan, research, data-model, tasks, analysis; evidence outside spec as above.

### Source Code

| Layer | Path | New files |
| --- | --- | --- |
| ACP transport | `crates/code/acp/src/agent/session_open.rs` | Preserve bounded explicit MCP timeout metadata; sibling tests |
| Native engine | `apps/studio/src-tauri/src/visual_feedback/` | `mod.rs`, sibling tests and focused image modules as justified |
| Native wiring | `apps/studio/src-tauri/src/lib.rs`, permissions/capabilities | Minimal registration and scoped commands |
| Desktop UI | `apps/studio/src/renderer/components/composer/screenshot/` | Visual workspace, typed bridge, focused tests |
| Shared state | Existing preference definitions/locales | Only feature settings and translations |

## Research

See [research.md](research.md). Reuse the existing screenshot byte decoder for screenshot, project and ZIP responses (ArrayBuffer, typed view or macOS number array), native screen permission checks, PP-OCRv6 worker, file APIs, existing annotation editor, UI primitives, Preferences ACL and managed isolated launcher. Use the already resolved workspace `image` dependency for Rust image operations. No new process, framework, runtime or model dependency.

## Verification Plan

- Focused `cargo test -p aster-studio --lib visual_feedback` from repository root with the Studio target directory.
- Focused screenshot renderer tests and typecheck; i18n sync/check, formatter limited to touched files; `cargo fmt --all -- --check` and existing desktop command authority regression.
- Early native CUA attach smoke, then full user scenarios against an isolated managed instance. Compare actual JSON/projects/images with fixtures and record source/build identity.
- No live model call, no production app replacement, no full unrelated cloud/workspace suite, no Windows/Linux hardware run.
