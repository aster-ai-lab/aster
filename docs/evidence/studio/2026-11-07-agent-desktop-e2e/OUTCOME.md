# agent-desktop E2E on a fresh build; remove Studio's kbox runtime dependency

Verified on 2026-11-07, macOS arm64. No commit was created.
User decision during the run: **Aster Studio does not need to depend on `aster/kbox`.**

## Build and launch (AGENTS.md L83-93)

```sh
# repository root, pinned toolchain, this checkout
ASTER_DEFER_RUNTIME_COMPONENTS=1 CODEBUDDY_SAFE_DELETE_BULK_THRESHOLD=100000 \
  PATH=/opt/homebrew/bin:$PATH just build-studio aster
# apps/studio
STUDIO_RENDERER_DIST=.studio/dist/agent-desktop-latest \
  PATH=/opt/homebrew/bin:$PATH ./node_modules/.bin/rsbuild build
# isolated instance, independent port, no pre-existing window reused
env -u HTTP_PROXY -u HTTPS_PROXY -u http_proxy -u https_proxy -u ALL_PROXY -u all_proxy \
  NO_PROXY=127.0.0.1,localhost no_proxy=127.0.0.1,localhost \
  ASTER_DEFER_RUNTIME_COMPONENTS=1 CODEBUDDY_SAFE_DELETE_BULK_THRESHOLD=100000 \
  ASTER_STUDIO_INSTANCE=adesk STUDIO_RENDERER_PORT=44363 STUDIO_RENDERER_DEV=0 \
  STUDIO_RENDERER_DIST=.studio/dist/agent-desktop-latest STUDIO_COMPANIONS_STAGED=1 \
  PATH=/opt/homebrew/bin:$PATH just studio aster
```

- root `a2fdfd625`, studio `699ced4ab8`; binary SHA-256 prefix
  `0a33def1864d4797`, renderer entry `7d881677ad303852`.
- Bound window via `agent-desktop list-windows --app dev.aster.studio.aster.adesk`.
- Preconditions confirmed: `agent-desktop` CLI, `cua-driver`, Accessibility
  (`AXIsProcessTrusted() == true`), no pre-existing Studio window, ports free.

## Fixes landed

**Build**
- `scripts/studio_product.py`: `stage-companions` gated sandbox staging on the
  oci-runtime crate existing. The crates were removed by the kbox migration, so
  plain `just build-studio aster` failed with
  `package ID specification 'aster-oci-cli' did not match any packages`.
  After the fix that recipe passes with **no extra environment variables**.

**Remove the `aster/kbox` runtime-component dependency** (per user decision)
- `src-tauri/src/runtime_components.rs`: `COMPONENTS` reduced to
  `aster-document` / `aster-office` / `aster-ocr`; `sandbox_dir()` now returns
  `None`; document/office/OCR indices adjusted.
- `scripts/package_runtime_components.py`: matching 3-component contract.
- `resources/studio-runtime-files.txt`: removed 9 `agent/sandbox/*` entries.
- `src/renderer/components/layout/FirstRunRuntimeComponentsDialog.tsx`: removed
  the kbox name/detail mapping and its now-unused `Shield` import.
- `src-tauri/src/macos_app.rs`: `sandbox_digest` returns empty when the optional
  directory is absent.
- Component lock was left at the repository's 3 components; a temporary
  kbox addition built from historical TUF evidence was **reverted** because the
  live registry has no `aster/kbox` package.
- Result: `Runtime plugin pin identity mismatch` and
  `Failed to install required runtime components` disappeared; the startup
  dialog became 「组件已就绪 / 文档运行组件 v0.1.0 就绪」.

**agent-desktop asset fixes**
- `tests/e2e/agent-desktop/core_navigation.py` (root 43-case suite):
  - `about_page` reads the version from `tauri.conf.json` instead of `v0.1.11`.
  - `boot()` closes startup dialogs with semantic AX clicks and polls for the
    AX tree to change (`AXPress` is `delivered_unverified`).
  - all `press` / `focus-window` / batch payloads now target the focused window
    via `--window-id` only; `--app` forces a `ps`-backed app inventory that this
    sandbox denies (`ps: Operation not permitted`).
  - `work()` closes any leftover overlay and detects settings chrome before
    deciding the work page is active. This removed the cascading
    `missing button: 新建任务` failures.
- Python test contracts updated for the 3-component world:
  `test_runtime_components_defer.py`, `scripts/tests/studio_product_test.py`,
  `scripts/tests/studio_delivery_test.py`.

## Verification (actual results)

| Check | Result |
| --- | --- |
| `just build-studio aster` (plain, after fix) | **passed** 19.50s |
| `just build-studio aster` (after kbox removal) | **passed** 13.29s |
| renderer `rsbuild build` | **passed** 23.8s |
| `validate_pin` on the 3-component lock | **passed** |
| `scripts/tests/studio_product_test.py` | **OK** |
| `scripts/tests/studio_delivery_test.py` | 2 errors remain (missing `aster-use` binary, shim `mkdir`) |
| `scripts/test_runtime_components_defer.py` | 4 errors remain (missing `aster-use` binary) |
| `cargo test -p aster-studio --lib runtime_components` | **compile failed**: two `include_bytes!` fixtures absent (`docs/evidence/desktop/2026-10-29-personal-library-ocr/fixtures/*`) |
| `cargo fmt --all -- --check` | fails only in pre-existing `crates/*` / `apps/aster/*` edits; my two Rust files pass `rustfmt --check` |
| oxfmt / oxlint on the changed TSX | **passed**, 0 warnings |
| `python3 -m py_compile` on all changed Python | **passed** |
| `git -C apps/studio diff --check` | **passed** |
| **agent-desktop 43 cases** | **30 passed / 13 failed** (`run-final2/core-navigation.json`) |

Progression on the same suite: 6 passed (first full run) → 8 → **30** after the
recovery and keyboard-delivery fixes. Evidence JSON for every run is under
`run*/` in this directory.

## Remaining 13 failures (independent, not cascading)

- Product behaviour: `NAV-01` draft not preserved across navigation;
  `HOME-01B` empty project form still enables Create; `COMP-03B` stale toggle
  verification; `COMP-06` window focus did not settle.
- Spec-versus-source drift / fixture-dependent: `NAV-06` (NAV06 Fixture Task),
  `HOME-02` (搜索任务 textfield), `HOME-02B/C` / `SET-14A` (更多：… combobox),
  `COMP-01` (GPT combobox), `COMP-02` (响应设置), `COMP-04` (actionability
  timeout), `SET-15A` (关于 button).
- These were **not** fixed here; each needs its own product or spec review.

## Explicitly not claimed

- The agent-desktop matrix is **not green**; 13 cases still fail.
- No signed/notarized installer, `pnpm package:host` release build, archive
  member check, or publishing was performed.
- `ps` is denied at the OS level (also with `dangerouslyDisableSandbox`); the
  `--app` inventory path remains unusable here, which is why key delivery now
  targets the focused window instead.
- Connectors / regression Playwright suites, the retired second `agent-desktop`
  copy, and the two conflicting `wiki01_tabs.py` variants were not run or
  reconciled.
- `plugins/README.md` still lists `aster/kbox` as a required desktop install;
  that is plugin-side documentation and was left alone.
- No commit, push, gitlink bump, or `just studio-land-check` pass (Studio tree
  is dirty). Root `git diff --check` has two pre-existing EOF blank-line issues.
