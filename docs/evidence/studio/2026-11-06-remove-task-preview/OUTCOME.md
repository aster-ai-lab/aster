# Remove task-plan preview and keep failed calls on one row

Verified on 2026-11-06, macOS arm64. No commit was created.

## Result

- Removed `TaskPlanSnapshot` from both task-message rendering paths and deleted
  the component, its dedicated test, and obsolete CSS selectors.
- Plan data, plan mode, and right-side task/workspace capabilities were not
  changed.
- Failed agent tool calls no longer expose inline error details or an expand
  button. Their title remains a single truncated row with optional
  right-aligned duration, including Read and Bash failures and cached historical
  expansion states.

## Verification

- Plan-focused renderer tests: **7/7 passed**.
- Failed-call renderer tests: **4/4 passed**.
- The broader task-message and tool-renderer run passed **196/197 tests**. One
  unrelated existing diff split-view test failed because its expected
  `radiogroup` was absent; it also failed when rerun alone and was not changed
  as part of this task.
- Scoped Oxfmt passed. Scoped Oxlint reported **0 warnings and 0 errors**.
- Production Rsbuild output `.studio/dist/remove-task-preview`: passed.
- `STUDIO_RENDERER_DIST=.studio/dist/remove-task-preview just build-studio`:
  passed with full companion staging.
- `git -C apps/studio diff --check` passed.

## Explicitly not claimed

- No full frontend suite pass is claimed because of the unrelated diff test
  above.
- No real-window visual acceptance, full TypeScript check, Windows/Linux GUI
  run, release installer, or full-repository lint result is claimed.
