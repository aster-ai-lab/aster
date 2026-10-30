# Shared lightweight charts — 2026-10-27

## Result

Extended `@a3s-lab/ui/chart` with shared `Sparkline` and `ActivityHeatmap` implementations and connected them to existing Studio and Dashboard metric surfaces. Public components are domain-independent: projects keep their own DTO aggregation, metric selection, translations and tooltip meaning.

## What landed

- Added `Sparkline` with a pure `buildSparklinePoints` helper and decorative 48×32 SVG behavior.
- Added `ActivityHeatmap` with generic date buckets, consumer-supplied tooltip/ARIA renderers, locale month labels, horizontal latest-position scrolling, loading skeletons and public reduced-motion CSS.
- Added pure heatmap utilities for continuous one-year calendar grids, DST-safe date stepping, range-active days, positive-value quantile thresholds and intensity calculation.
- Exported the new components/types/utilities from `@a3s-lab/ui/chart` and synchronized them into Dashboard `vendor/ui`.
- Replaced Studio's private MetricSparkline with the public Sparkline and moved its UsageHeatmap grid/threshold/layout implementation to the public component. Studio retains metric persistence, DTO mapping and localized tooltip content.
- Added Token/request sparklines to Dashboard UsageSummary without adding requests or changing metric meaning. Success-rate and active-customer cards do not invent a trend.

## Verification actually completed

- Public UI `vitest run src/components/chart`: **6 files, 12 tests passed**, including sparkline point mapping, one-year grids, locale week starts, quantile thresholds and null/zero intensity.
- Public UI `pnpm run typecheck`: passed.
- Studio direct project Vitest for UsageSettings and UsageHeatmap: **2 files, 5 tests passed**. The heatmap test was updated from the old `date` object field to the generic API's `dateValue` field; the assertion's one-year/range meaning was not weakened.
- Dashboard `npm test`: **8 files, 34 tests passed**.
- Dashboard `npm run build`: passed, including typecheck and CSS contract (172,636 bytes CSS). The existing ECharts implementation remains an async chunk; lightweight components do not add another chart library.
- `git diff --check`: run for affected source and documentation paths.

## Decisions

- Public components do not import Studio's cache, Usage DTO, i18n catalogs or metric names; those stay in feature adapters.
- Heatmap dates use `dateValue: Date` for calculations and `date: YYYY-MM-DD` for identity, avoiding ambiguous field semantics and fixed-millisecond DST arithmetic.
- Public heatmap animation lives in the UI package rather than depending on a Studio-global class; reduced motion disables it.
- Dashboard heatmap was not added merely for feature count. It remains available for a real density use case; only useful existing sparklines were added.

## Explicitly not claimed

- No full Studio lint/typecheck/test suite, full root UI test suite, authenticated visual review, accessibility certification, performance certification or deployment is claimed.
- No real Application analytics feature or Dashboard heatmap product gate is claimed.
- The pnpm wrapper's pre-existing `program not found` hook remains outside this change; Studio focused tests were run directly through its existing Vitest installation.
