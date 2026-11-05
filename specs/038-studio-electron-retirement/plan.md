# Implementation Plan: Retire Studio Electron dependencies

**Branch**: current checkout | **Spec**: [spec.md](spec.md) | **Input**: [spec.md](spec.md)
**Gate context**: Studio product regime; no new workspace or runtime mechanism.

## Summary

Trace dependencies and current callers before deleting obsolete tooling. Migrate retained functionality through `studioMainBridge.js`, typed `ipcApi`, `workspaceBrowser.ts` / `AsterWorkspaceBrowser` and existing native applet/window owners. Remove the old globals and guests after their callers migrate. Preserve data and tenant contracts, then verify real native behavior and resume the frozen campaign.

## Constitution Check

| Principle | Status | Evidence in the plan |
| --- | --- | --- |
| I. Fail closed | PASS | Retain named capability errors; no silent success or empty guests. |
| II. Evidence retained | PASS | Dedicated OUTCOME plus original CUA failures retained. |
| III. Hermetic tests | PASS | Each behavior task carries its focused renderer/native tests. |
| IV. Closed sets | PASS | Existing IPC schemas and native action types own admitted values. |
| V. One workspace / published APIs | PASS | Only Studio and parent wiring; no Core/ACP source edits. |
| VI. Local schema changes | PASS | Current SQLite migrations remain unchanged. |
| VII. Shrink-only ratchets | PASS | No allowlist grows; legacy picker entries shrink only when removed. |
| VIII. Replaceable transports | PASS | Existing renderer facade and Tauri host owners. |
| IX. One configuration language | PASS | No new product configuration. |
| XIII. One repository | PASS | Current checkout and existing private Studio submodule. |
| XIV. Names are not guessed | PASS | Exact source symbols, manifests and typed route names. |
| XV. Meaning is not matched | PASS | No heuristic user-intent routing. |
| XVI. Pickers are drawn | PASS | Existing house controls; no native select. |
| XVII. Dialogs stay in place | PASS | Existing dialog components and motion contract. |

## Project Structure

### Documentation (this feature)

`specs/038-studio-electron-retirement/` contains this plan, specification and ordered tasks. Acceptance lives in the evidence directory.

### Source Code

| Layer | Path | New files |
| --- | --- | --- |
| Desktop host and renderer | `apps/studio/src-tauri`, `apps/studio/src/renderer` | Only owning tests or necessary named native functionality; no second host abstraction. |
| Shared contracts | `apps/studio/src/shared/ipc` | Extend existing route/event schemas as required. |
| Build and tests | `apps/studio/scripts`, `apps/studio/tests`, existing root workflows | Reuse supported Tauri packaging/testing; retire Electron-only callers. |
| CLI / Core / ACP | Existing paths | None; read-only dependency review. |

## Research

- `src/renderer/host/desktopHost.ts` owns host capabilities and platform; do not install a compatibility Electron object.
- `src/renderer/ipc/ipcApi.ts` and `useIpcOn.ts` own requests and subscriptions; migrate active callers at that boundary.
- `AgentBrowserRuntimeHost.tsx` already selects `AsterWorkspaceBrowser` for Tauri; remove the obsolete alternate branch only after preserving its retained contracts.
- `HtmlArtifactPreviewSurface.tsx` already uses `HtmlPreviewFrame`; preserve script consent and sandboxing.
- `rsbuild.config.ts` builds the main window only. Preboot migration/relocation entries require full caller/host audit before removal.
- `scripts/package-host.sh`, product packaging and root release governance own native artifacts. Old before/after-pack, notarization and ABI download hooks must not replace those checks.
- Backup, mini-app and tab capabilities need owner-by-owner tracing; missing native support is a finding, not permission to delete a user feature or add a no-op adapter.

## Verification Plan

- Capture a source/dependency baseline and classify every executable reference, retaining hashes and current dirty state.
- Run focused existing tests before changes; add small regressions for actual new failures, then run the same tests after implementation.
- Run install/dependency checks, lint/typecheck, renderer build and native bundle validation. For Rust changes run pinned root fmt, owning tests and Clippy.
- Use the isolated native instance and current A3S Test/background proxy for visible entry-point checks. No browser substitution for native claims.
- Record existing failures and untested targets. Resume C00–C08 without changing their frozen inputs or counting cleanup checks as their pass.
