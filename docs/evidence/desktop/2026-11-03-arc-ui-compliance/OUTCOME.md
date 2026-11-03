# ArcUI / motion near-term compliance (B0–B4)

Date: 2026-11-03. Host: macOS arm64.

## Result

**Near-term compliance batches B0–B4 landed** against DESIGN.md §5–6 and the ArcUI optimization plan.
Product renderer `SelectTrigger` debt is empty; SearchSelect is the product default; segmented surfaces
use shared highlight / SegmentedControl; search/sidebar chrome and UI package hygiene updated.

This does **not** complete A0–A8 full Arc replacement.

## Batches

### B0 — Gate repair
- Migrated `DisplayLanguageSettings.tsx`, `MigrationApp.tsx` to `SearchSelect`.
- Removed stale `AppearanceSettings.tsx` from `RADIX_SELECT_DEBT`.
- Excluded `browserAnnotations/guest/` from native-select product scan (probe only).

### B1 — Radix Select → SearchSelect
- Migrated all remaining product `SelectTrigger` screens (settings, history, resource catalog, code, org, onboarding, etc.).
- `RADIX_SELECT_DEBT` is now `[]`.

### B2 — Segmented / tabs
- Plan editor, WorkScheduledTasks mode switch, AccountCenterMenu ScopeTab, Capabilities categories, TaskDiffView → `SegmentedControl` / `SelectionHighlight`.

### B3 — Search + chrome
- Catalog / MiniApps / Plugin search → 30px borderless `#f5f5f5`.
- Floating Sidebar: in-place fade/scale, elevated shadow (no slide / `shadow-2xl`).
- Softened Launchpad/Applets/ProviderLogo scale; UpdateFab + scroll FAB radii 8–12px; GlobalSearch elevation token.

### B4 — packages/ui hygiene
- Deprecated Radix `Select*` exports (JSDoc).
- Arc dialog: in-place opacity+scale 160/200; removed travel/`filter` blur on title swap.
- `motionTokens.duration.exit` → 0.2; legacy `--font-sans` no longer defaults to Inter.
- README picker guidance.

## Verification

```sh
cd apps/studio && ./node_modules/.bin/vitest run src/renderer/__tests__/noNativeSelect.test.ts
# Observed: 3 passed

cd packages/ui && bun run test -- --run src/components/primitives/__tests__/dialog.test.tsx
# Observed: 19 passed
```

Focused segmented/settings tests were run during migration by owning agents; re-run owning suites before release if those files change again.

## Explicitly not claimed

- A0 full reachability inventory or A1–A8 Arc component replacement completion.
- Performance improvement from Arc.
- Office kernel / form / flow full chrome migration.
- Every `rounded-full` in the product tree cleared (session dots / rail tooltips retained; many decorative uses remain).
- Cross-platform GUI e2e for every migrated setting screen.
- Cold `just build-studio` + agent-desktop pass for this compliance batch (hermetic vitest only here).
