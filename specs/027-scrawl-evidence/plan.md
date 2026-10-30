# Implementation Plan: Aster native recording and Capture evidence

**Branch**: current checkout | **Spec**: [spec.md](spec.md) | **Input**: [spec.md](spec.md)

**Gate context**: Product regime, reusable Rust evidence engine plus the existing private `apps/studio` submodule in the root Cargo workspace.

## Summary

Internalize Scrawl's MIT-licensed evidence engine at `7b184c67767a1aee6c1d44640d94d8446cd5fed1` as a root-workspace library, not an external application. Extend the existing composer screenshot area with a recording control and a native recording lifecycle. Reuse the existing visual editor, attachment delivery and visual MCP ownership. Generate upstream-compatible `.scrawl` bundles, store Aster metadata separately, and route evidence only to explicit authorized Aster destinations. Preserve the full upstream capability matrix, including CaptureKit, export, recovery, retention, CLI and ingest; platform adapters land independently and are never inferred from portable tests.

## Constitution Check

| Principle | Status | Evidence in the plan |
| --- | --- | --- |
| I. Fail closed | PASS | One active recording, bounded inputs, explicit permission/path/session/destination refusals |
| II. Evidence retained | PASS | `docs/evidence/desktop/2026-10-30-scrawl-evidence/OUTCOME.md` plus native receipts |
| III. Hermetic tests | PASS | `scrawl`, Studio Rust, renderer Vitest and Swift package tests; native CUA additional |
| IV. Closed sets | PASS | Recording states, targets, destinations and errors use Rust `parse()`/`as_str()` and Zod schemas |
| V. One workspace / published APIs | PASS | Add `crates/capture` to the existing root members and lockfile; `publish = false`; local A3S paths only |
| VI. Local schema changes | PASS | `.scrawl` owned by evidence engine; Aster sidecar and preferences owned by Studio; no cloud schema |
| VII. Shrink-only ratchets | PASS | Add scoped recording permissions without weakening existing facade/native-dropdown ratchets |
| VIII. Replaceable transports | PASS | Native capture adapters behind the evidence engine; transport selection stays out of renderer and bundle schema |
| IX. One configuration language | PASS | Existing ACL preference service; no `~/.config/scrawl` import or second config format |
| XIII. One repository | PASS | Vendored source under `crates/capture`, existing Studio submodule, no new repository |
| XIV. Names are not guessed | PASS | Names come from typed app/window/session/registration metadata, not capitalization heuristics |
| XV. Meaning is not matched | PASS | Destination/state decisions use typed closed sets and explicit user/recorded choices |
| XVI. Pickers are drawn | PASS | Existing house buttons/popovers/target picker; no native `<select>` |
| XVII. Dialogs stay in place | PASS | Existing in-place editor/dialog behavior; native recording island is not a sliding modal |

No NON-NEGOTIABLE violation is planned. Upstream has no Linux adapter; this is implemented as new platform work or reported incomplete, never waived or hidden as a pass.

## Project Structure

### Documentation (this feature)

`specs/027-scrawl-evidence/`: spec, plan, research, data-model, contracts and tasks. Acceptance evidence lives separately under `docs/evidence/desktop/2026-10-30-scrawl-evidence/`.

### Source Code

| Layer | Path | New files / changes |
| --- | --- | --- |
| Evidence library | `crates/capture/` | Vendored upstream Rust sources, helper, Swift package, fixtures, LICENSE and `src/lib.rs` integration API; sibling tests |
| Workspace/provenance | `Cargo.toml`, `Cargo.lock`, `docs/source-manifest.json`, `docs/provenance/` | Root membership, pinned upstream revision and MIT provenance |
| Native desktop | `apps/studio/src-tauri/src/capture/` | Recording controller, platform adapters, permissions, storage, commands and sibling tests |
| Native registration | `apps/studio/src-tauri/src/lib.rs`, `agent_plane/visual_mcp.rs`, delivery scripts | Narrow command/agent registration, helper staging and signing |
| Shared contracts | `apps/studio/src/shared/types/`, `shared/ipc/` | Typed recording state/events/requests/errors; no new top-level directory |
| Composer UI | `apps/studio/src/renderer/components/composer/` | Recording button/control surface, typed bridge, evidence attachment and review using existing screenshot editor |
| Settings/locales | Existing classification/preferences and locale sources | Recording/microphone/transcriber/retention/shortcut settings; regenerate generated files |
| Native CLI | `apps/aster/` and/or `crates/capture` binary target | Equivalent upstream operations without Node/Python/Go bindings or another workspace |
| CaptureKit | `crates/capture/Sources/CaptureKit/`, `Tests/CaptureKitTests/` | Preserved Swift package, capture, context, pairing and outbox tests |

## Research

See [research.md](research.md) for upstream scope, in-repo prior art and integration decisions. Key existing patterns are `ComposerScreenshotControl.tsx`, `VisualFeedbackEditor.tsx`, `visual_mcp.rs`, `screenshot.rs`, `host_family.rs`, `composerAttachment.ts`, ACL preferences and the managed isolated launcher.

## Verification Plan

- `cargo test --locked -p scrawl` for upstream and integration evidence behavior.
- Focused `cargo test --locked -p a3s-studio --lib scrawl` and visual MCP/session guard tests from the repository root.
- `swift test --package-path crates/capture` for CaptureKit and outbox behavior on macOS.
- Focused renderer tests for the real record/stop control, target/destination owner, attachment, cancellation, redaction and unsupported states; applicable typecheck, lint, i18n and formatter checks.
- `cargo fmt --all -- --check`, `tests/workspace_dependencies.py`, existing Tauri permission and workspace guards.
- Isolated native macOS CUA for target selection, narration, marks, stop, review, attachment, agent zoom, export, cancellation and recovery. Reopen actual bundles and exported media.
- Packaged-app helper/signature/permission validation after the real implementation passes. No production replacement unless separately requested.
- No unrelated cloud campaign, live model call, full unrelated workspace suite, or unsupported-platform claim.
