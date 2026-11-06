# Recent Studio optimization regression guard

Verified on 2026-11-06, macOS arm64. The review covered the current unified
model selector, reasoning-effort slider, message disclosure and scrolling
changes, Composer and permission controls, startup syntax fixes, and companion
packaging. No commit was created.

## Findings and fixes

- Preserved the single `ModelSelector` popup, full-row reasoning slider,
  `MessageVirtualList` memoization, scroll-cache deduplication, and deferred
  disclosure mounting. The withdrawn always-mounted disclosure animation was
  not reintroduced.
- Removed a duplicate `Plus` import from `GroupSelector.tsx` and a duplicate
  `ReactNode` import from its test. Removed the resulting unused imports.
- Corrected the `GroupSelector` test mock to expose the real `combobox` role.
- Restored `data-checked` on Composer option rows and restored an explicit
  close action that returns focus to the Plus trigger.
- Two legacy local-model naming tests disagreed. Kept the newer provider-group
  contract and existing implementation: remove only the duplicate provider
  prefix, preserve other configured labels, and retain friendly Codex naming.
  Updated the older assertion without changing runtime model IDs.
- Kept the Alpine `python3-3.12.16-r0.apk` pin and Tokio
  `AsyncFd::register_with_interest` fix.

## Automated verification

- Model selector, reasoning slider, model naming and Composer options:
  **9 files, 96/96 tests passed**.
- Message disclosure, message parts, virtual scrolling, scroll runtime and
  account startup: **9 files, 343/343 tests passed**.
- Permission pickers, permission posture state and Composer extras:
  **4 files, 52/52 tests passed**.
- Group selector: **1 file, 6/6 tests passed**.
- Total renderer coverage: **23 files, 497/497 tests passed**.
- `python3 -m unittest apps/studio/scripts/tests/sandbox_assets_test.py`:
  **3/3 tests passed**.
- Scoped Oxfmt over all changed TypeScript files: passed.
- Scoped Oxlint with `--deny-warnings`: **0 warnings and 0 errors**.
- Production Rsbuild output `.studio/dist/recent-regression-guard`: passed.
- `cargo fmt --all -- --check`: passed.
- `STUDIO_RENDERER_DIST=.studio/dist/recent-regression-guard just build-studio`:
  passed with full companion staging; no deferred-component flag was used.
- `git diff --check` in the root and Studio checkout: passed.

## Default-window check

The owner's current default Aster window remained mounted after these changes.
The task list, message stream, disclosures, Composer and workspace controls are
visible; no white boot screen or second model-selector implementation was
observed. Final capture: `artifacts/default-window.png`.

## Explicitly not claimed

- No full frontend suite, full `pnpm lint`, full TypeScript check, native Linux
  PTY runtime test, Windows/Linux GUI run, release installer, or warning-free
  workspace build is claimed.
- Existing unrelated i18n, workspace-resource, automatic-backup-route, Rust,
  and linker warnings remain outside this regression-guard scope.
- This check covers the observed current changes and named regression paths;
  it does not prove every possible product workflow.
