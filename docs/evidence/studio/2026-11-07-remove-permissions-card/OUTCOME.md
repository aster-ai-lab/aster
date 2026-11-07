# Remove the General settings "Permissions" card

Verified on 2026-11-07, macOS arm64. No commit was created.

## Result

- Removed the red-framed 权限 / 电脑操控权限 card from 通用 — the
  `ComputerUsePermission` component and everything that existed only for it.
- Removed: `ComputerUsePermission.tsx`, its dedicated test, the
  `GeneralSettings.tsx` import and JSX, the heading assertion in
  `GeneralSettings.test.tsx`, the permission-only helpers in
  `desktopSettings.ts` (`DesktopPermissions`, `readDesktopPermissions`,
  `writeDesktopPermission`, `readComputerUseRuntimePresent` and the Tauri
  invoke helper), six `settings.aligned.permissions.*` keys from both locale
  catalogs, the 电脑操控权限 label and row/control counts in both e2e page
  baselines (13 → 12), the component path from
  `specs/040-arc-foundation-consistency/contracts/components.json`, and the
  权限 group from the DESIGN.md settings description.
- `desktopSettings.ts` keeps `fallbackAbout()` — it belongs to the About
  surface, not this card.

## Deliberately retained

These are host/preference capabilities, not component code; removing them
would change session behavior and touch generated Tauri schemas:

- `app.permissions.computer_use` / `agent_control` in
  `preferenceSchemas.ts`, `preference_keys.generated.rs`, the preference mock
  and the data-classify key definitions.
- `COMPUTER_USE_PREFERENCE` in `agent_plane/turns.rs`, the
  `computer_use_runtime_present` Tauri command, its ACL permission file, and
  the capability references.
- The onboarding wording about macOS permissions (a different flow) and the
  archived evaluation-hardening source snapshots.

## Verification

- Vitest: settings pages **4 files, 28/28 passed**; after oxfmt,
  `GeneralSettings.test.tsx` **3/3 passed** again.
- Scoped Oxfmt `--check` and Oxlint `--deny-warnings` on the three changed
  TS/TSX files: **passed**, 0 warnings / 0 errors. `git diff --check`:
  **passed**.
- All five edited JSON files parse; the two locale catalogs have identical key
  sets (**12806** each) and the six removed keys exist in neither.
- Repository-wide scan for `ComputerUsePermission`,
  `settings.aligned.permissions`, `电脑操控权限`,
  `readComputerUseRuntimePresent`, `readDesktopPermissions` and
  `writeDesktopPermission`: **0 matches**.
- Full `i18n:check`: **still fails on the pre-existing**
  `knowledge.views.navigation` ordering defect (first mismatch at key 2746),
  unchanged by this deletion.

## Explicitly not claimed

- No GUI/e2e run against a live Studio window; the e2e baselines were updated
  and parsed but not replayed.
- No claim that the computer-use runtime capability or the preference was
  removed from the host; see "Deliberately retained".
- No full frontend suite, full TypeScript check, `pnpm lint`, build, or commit.
