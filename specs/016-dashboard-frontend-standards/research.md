# Research: Dashboard frontend standards and structural refactoring

## Source inventory

- `PlatformPage.tsx`: 4,644 lines; navigation, access, overview, catalogs, logs, usage, gateway, facets, providers, settings, formatting, list state, feedback, detail history, tables, channels and system views.
- `SupplierEditor.tsx`: 1,085 lines; above the 1,000-line budget and therefore included in bounded extraction.
- Existing authored imports use `@renderer/` and `@shared/`; configuration declares both. No compatibility aliases will remain after migration.
- Existing test baseline: 7 files / 30 tests passed in a standalone run on 2026-10-27. Build/typecheck/CSS contract passed in the same baseline review.

## Decisions

### One application alias: `@/`

Use `@/` for cross-area application imports and relative imports within a cohesive feature area. `@a3s-lab/ui` remains a separate package/design-system import. Remove `@renderer` and `@shared` from TypeScript/Vitest configuration and authored source rather than maintaining three spellings.

**Rejected alternative**: preserve `@renderer` for textual comparison with ToWork. The dashboard is now an independent application and the user explicitly requested naming optimization; structural ownership is clearer than copied-path parity. Git history remains the comparison mechanism.

### Platform-local feature structure, not a global component rewrite

Keep platform-specific record, detail, provider and settings components under `pages/platform/components/`; views under `pages/platform/views/`; hooks under `pages/platform/hooks/` or the existing collection area. App-global `components/` retains only genuinely shared gates/layout/collection/brand controls.

**Rejected alternative**: move everything into app-global `components/` or create a generic entity framework. That would hide platform semantics and encourage premature abstraction.

### Composition first, extraction without behavior redesign

`PlatformPage.tsx` retains account/org loading, access resolution, tenant boundary, supplier provider, sidebar and menu composition. Extracted views retain their current SWR keys, loaders, local state, labels and DOM/class contracts. Shared helpers are moved, not rewritten.

**Rejected alternative**: replace SWR/local state or introduce a router/state library during the same migration. That mixes structural and behavioral risk and is unnecessary for the requested standards work.

### Explicit component APIs

New/touched exported components use domain-named prop types (`PlatformFrameProps`, `RecordTableProps`, etc.) and named exports unless they are actual page entry points. Components do not receive broad opaque objects where a small typed domain value is sufficient. Existing API DTOs and serialized identifiers remain unchanged.

### Mechanical architecture checks with closed exceptions

Tests scan only authored `src/`, enforce file naming, reject legacy aliases, and enforce dependency directions. `main.tsx`, `index.ts`, test setup and generated modules are explicit exceptions. The check does not infer product meaning from names and does not scan vendor/build/dependency trees.

**Rejected alternative**: add ESLint/dependency-cruiser solely for this migration. A small hermetic Vitest source contract uses the existing toolchain and avoids a new dependency/rule ecosystem.

### Complexity budgets

The platform shell must be <=400 lines. Every authored production module must be <=1,000 lines after this bounded migration. This requires a small supplier-editor extraction in addition to the platform page split. Future default component guidance is <=300 lines, with >500 lines requiring review; this guidance does not mandate artificial fragmentation of cohesive declarations.

## Safety invariants

- Preserve `a3s.*` storage keys, member paths, API payload/wire field spellings, SWR keys and menu IDs.
- Keep `PlatformTenantBoundary` and member/org scoping around all platform views.
- Keep preview mode explicit and never use it as production failure fallback.
- Preserve Chinese/English keys, native-picker ban and dialog motion.
- Preserve existing supplier write/recovery behavior while extracting code.
- Do not commit the parent gitlink or unrelated in-flight work.
