# Shared Chart extraction — 2026-10-27

## Result

Extracted the Studio Chart implementation into the canonical shared `packages/ui` package and exposed it as `@a3s-lab/ui/chart`. Studio and Dashboard now consume the same public Chart API; Dashboard's `vendor/ui` contains a synchronized read-only snapshot rather than a separately authored chart runtime.

## What landed

- Added `packages/ui/src/components/chart/` with the restricted ChartProps, theme resolution, cartesian/pie builders, ECharts runtime and all original chart tests.
- Added the public `packages/ui/src/chart/index.ts` entry and `./chart` package export. Added `echarts ^6.1.0` to the UI package and Dashboard dependencies with updated lockfiles.
- Removed renderer-only `@renderer/utils/style` coupling from the shared runtime; it uses the UI package's own `cn` utility.
- Changed Studio `UsageDistributionChart` to lazy-load `@a3s-lab/ui/chart` and removed the old `src/renderer/components/Chart` implementation.
- Mirrored the public component and package metadata into `apps/dashboard/vendor/ui`, then migrated Dashboard UsageBars to the shared Chart with lazy loading, themed tooltip/axis, ARIA and long-range dataZoom.
- Updated `CHARTS.md`, `FRONTEND.md`, the UI README and Console simplification plan to name the shared package as the canonical implementation.

## Verification actually completed

- Public UI `vitest run src/components/chart`: **4 files, 8 tests passed** (builders, theme, loading/timeline runtime).
- Public UI `pnpm run typecheck`: passed.
- Studio direct `node node_modules/vitest/vitest.mjs run --project renderer src/renderer/pages/settings/UsageSettings/__tests__/UsageSettings.test.tsx`: **1 file, 4 tests passed**. The pnpm wrapper was blocked by its local `.pnpmfile.cjs` `program not found` hook, so the same project Vitest was invoked directly; the wrapper failure is not counted as a product test result.
- Dashboard `npm test`: **8 files, 34 tests passed**.
- Dashboard `npm run build`: passed, including typecheck and CSS contract. ECharts is emitted as an async chunk rather than the first-load entry; the largest async chart chunk is 631.3 kB (213.3 kB gzip).
- `git diff --check`: run for the affected package/source/docs paths.

## Decisions

- Canonical source is root `packages/ui`, not a new Dashboard component or a long-lived copy of Studio code.
- Kept the existing Dashboard `vendor/ui` distribution model and synchronized the shared source instead of introducing a second UI dependency resolution scheme.
- Used the public subpath `@a3s-lab/ui/chart` so consumers do not need the large root barrel or private component paths.
- Preserved existing `apps/studio/packages/ui` gitlink/deletion status and all unrelated in-flight changes; no commit or reset was performed.

## Explicitly not claimed

- No full Studio lint/typecheck/test suite, full root UI test suite, production deployment, or authenticated visual review of the new Dashboard chart was performed. A fresh browser tab had no member session, so authenticated chart rendering was not claimed from it.
- No new Application analytics feature, heatmap, sparkline migration, or complete accessibility certification is claimed by this extraction.
- No performance certification is made from bundle size alone; lazy loading is verified by the emitted async chunk, while real-data runtime/performance remains a separate acceptance step.
