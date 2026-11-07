# Full cleanup of the removed `aster/kbox` runtime-component dependency

Verified on 2026-11-07, macOS arm64. No commit was created.
User decision: **Aster Studio does not need to depend on `aster/kbox`.**

This pass completes phases 1+2 of the repair plan: every remaining
runtime-component reference to kbox/sandbox was removed, while keeping kbox as
an **external tool** (`binaryTools.ts`, the environment-dependency tool cards,
and plugin-catalog fixtures) where it is still an independent concept.

## Changes

**Shared contract and renderer**
- `src/shared/ipc/schemas/runtimeComponents.ts`: enum reduced to
  `aster-document` / `aster-office` / `aster-ocr`.
- `.../DependenciesSettings/RuntimeComponentsSection.tsx`: `ComponentId`, the
  `CARDS['kbox']` entry and the now-unused `Box` icon removed.
- `src/renderer/i18n/locales/{en-us,zh-cn}.json`: the two
  `runtimeComponents.sandbox.*` keys removed from each locale.
- Test fixtures updated to valid 3-component ids:
  `FirstRunRuntimeComponentsDialog.test.tsx` (kbox → `aster-office`, with i18n
  assertions switched to the office name/subtitle),
  `FirstRunRuntimeComponentsDialog.status.test.tsx`,
  `RuntimeComponentsProgressButton.test.tsx`,
  `RuntimeComponentsSection.test.tsx`,
  `shared/ipc/schemas/__tests__/runtimeComponents.test.ts`,
  `tests/e2e/workbench-office/first-run-components-motion.spec.ts`.
- `first-run-components-motion.spec.ts`: first test now installs `aster-ocr`;
  second test drops the 4th row (`toHaveCount(4)` → `3`). Its completion
  assertion now checks the manually opened builtin-plugin view, whose title is
  `内置插件`; completion is verified by the row status and the disappearing
  progressbar.

**Rust**
- `src-tauri/src/runtime_components.rs`: removed the now-orphaned
  `sandbox_dir()` (it returned `None` and had one caller).
- `src-tauri/src/agent_plane/acp.rs`: removed the dead
  `sandbox_dir()` discovery branch; sibling `sandbox/` discovery remains.
- `src-tauri/src/macos_app.rs`: removed `SANDBOX_FILES`, `sandbox_digest()` and
  its `dev_bundle_digest` input; the format string is now two fields.
- `src-tauri/src/macos_app/sandbox_tests.rs`: removed the test that exercised
  the deleted digest helper; the `remove_deferred_runtime_payloads` test stays.

**Scripts and documentation**
- `scripts/studio_delivery.py`: removed the `sandbox_assets` import, sandbox
  receipt/verification, sandbox required-file extension, and the legacy shim
  entitlement branch in `sign_resources`.
- `scripts/package_runtime_components.py`: removed the dead `kbox` license
  mapping.
- `scripts/tests/studio_delivery_test.py`: removed sandbox fixtures and the
  hypervisor-entitlement assertion; the signing/seal coverage remains
  (`test_native_signing_seals_final_resource_bytes`). Also removed a redundant
  `document.mkdir(exist_ok=True)` that the environment shim refuses.
- `plugins/README.md`, `plugins/docs/layout.md`: kbox is now described as an
  external tool, not a Studio runtime component.
- Six `tests/e2e/*/README.md`: `apps/desktop` → `apps/studio`,
  `DESKTOP_E2E_BASE_URL` → `STUDIO_E2E_BASE_URL`,
  `DESKTOP_RUNTIME_FILE` → `STUDIO_RENDERER_DIST`, port `4537` → `4318`.

## Verification (actual results)

| Check | Result |
| --- | --- |
| `just build-studio aster` (from repo root) | **passed**, dev profile 32.61s |
| `tsc --noEmit -p tsconfig.web.json --composite false` | no kbox errors; remaining diagnostics are pre-existing (`workInbox/client.test.ts`, etc.) |
| `tsc --noEmit -p tsconfig.e2e.json` | no kbox errors; remaining diagnostics are pre-existing (`document-message-content.spec.ts`, etc.) |
| vitest (5 affected files) | **34 passed / 34**, exit 0 |
| vitest `FirstRunRuntimeComponentsDialog.test.tsx` rerun | **16 passed / 16**, no duplicate-key warnings |
| `playwright.workbench-office.config.ts first-run-components-motion.spec.ts` | **2 passed / 2**, exit 0 (renderer preview on 4318) |
| `oxfmt --check` / `oxlint --deny-warnings` (8 TS files) | **passed**, 0 warnings |
| `python3 -m py_compile` (changed scripts) | **passed** |
| `scripts/tests/studio_product_test.py` | **OK** |
| `scripts/tests/studio_delivery_test.py` | **OK**, 29 tests |
| `scripts/test_runtime_components_defer.py` | **OK** (after building `aster-use`) |
| `scripts/tests/sandbox_assets_test.py` | **OK** |
| `validate_pin` on the 3-component lock | **passed** |
| `git -C apps/studio diff --check` | **passed** |
| `cargo build --locked -p aster-use --bin aster-use` | **passed**, 32.19s (unlocks the packaging tests) |

## Pre-existing issues left alone

- `cargo fmt --all -- --check` and `rustfmt --check`: diffs in unrelated files
  (`crates/*`, `apps/aster/*`, `runtime_components_tests.rs:274`); my three Rust
  files are clean.
- `scripts/i18n-check.ts`: `en-us.json` key ordering is unsorted (pre-existing);
  both locales remain in sync after the two keys were removed from each.
- Root `git diff --check`: two pre-existing EOF blank-line diagnostics in
  `crates/code/**`.
- `just studio-land-check`: still refused, Studio worktree dirty.

## Explicitly not claimed

- Phase 3 (product gaps: Mock Webview, `notOpenToast` entrances, commented
  toolbar/search buttons), phase 4 (E2E B1–B7 infrastructure, full matrix,
  agent-desktop's 13 remaining failures) and phase 5 (office dts, notarization,
  signed installer, land-check, gitlink bump) were **not done**. They need
  product decisions, external credentials, or new infrastructure.
- No signed/notarized installer, publishing, commit, push or gitlink bump.
- No change to the user's ~2990 uncommitted root-worktree edits; those deletion
  files were not touched, restored or committed.
- `sandbox_assets.py` itself was kept (still used by `studio_product.py`'s
  conditional stage and by its own test), but no delivery path references it.
- No test was weakened to obtain a pass: assertions were retargeted to the
  valid 3-component contract and to the current dialog title semantics.
