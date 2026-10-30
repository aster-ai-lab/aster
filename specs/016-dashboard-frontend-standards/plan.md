# Implementation Plan: Dashboard frontend standards and structural refactoring

**Branch**: Current worktree; no new branch requested | **Spec**: [`specs/016-dashboard-frontend-standards/spec.md`](spec.md) | **Input**: User request to establish frontend standards and improve dashboard naming and component design

**Gate context**: `apps/dashboard` engineering-quality enablement for the existing Kense Console PR2–PR7 journey; no product availability gate is claimed.

## Summary

Add a dashboard-owned contributor contract, migrate authored imports to one `@/` alias, and turn the 4,644-line `PlatformPage.tsx` into a small composition root backed by named views, record components, hooks, and view/formatting modules. Split the remaining over-budget supplier editor, add source architecture/naming tests, preserve all API/tenant/access/storage/visual contracts, and retain exact verification evidence.

## Constitution Check

*GATE: Re-read `.specify/memory/constitution.md`. A NON-NEGOTIABLE violation stops the plan.*

| Principle | Status | Evidence in the plan |
| --- | --- | --- |
| I. Fail closed | PASS | Existing permission, decoding, tenant and preview behavior is invariant; architecture violations fail tests with file/rule details. |
| II. Evidence retained | PASS | `docs/evidence/dashboard/2026-10-27-frontend-standards/OUTCOME.md`. |
| III. Hermetic tests | PASS | Dashboard Vitest source-contract and existing component/data tests; no live service required. |
| IV. Closed sets | PASS | No new runtime enum; existing menu/status/wire closed sets remain unchanged. Architecture checks use explicit exception lists. |
| V. One workspace / published APIs | PASS | Only dashboard source/config/docs; no Cargo workspace or package API changes. |
| VI. Local schema changes | N-A | No persistence schema or wire contract changes. |
| VII. Shrink-only ratchets | PASS | Existing native-picker test stays strict; new source ratchet has no legacy allowlist beyond explicit entry/generated exceptions. |
| VIII. Replaceable transports | N-A | Member/browser transport remains in existing data/shared modules. |
| IX. One configuration language | N-A | No product configuration; Markdown contributor standards are documentation. |
| XIII. One repository | PASS | Edit existing dashboard submodule and parent specs/evidence only; no new repository. |
| XIV. Names are not guessed | PASS | Names come from existing domain identifiers and explicit spec terms, not inferred runtime names. |
| XV. Meaning is not matched | N-A | No user-intent matcher; static import/path checks are mechanical source validation. |
| XVI. Pickers are drawn | PASS | Existing `noNativeSelect` test remains unchanged and green. |
| XVII. Dialogs stay in place | PASS | Supplier extraction preserves existing dialog implementation and motion; no new dialog animation. |

## Project Structure

### Documentation (this feature)

```text
specs/016-dashboard-frontend-standards/
├── spec.md
├── plan.md
├── research.md
└── tasks.md

apps/dashboard/
├── FRONTEND.md
└── README.md

docs/evidence/dashboard/2026-10-27-frontend-standards/
└── OUTCOME.md
```

### Source Code

| Layer | Path | New files / migration |
| --- | --- | --- |
| Application composition | `apps/dashboard/src/` | `App.tsx`, `main.tsx` retained; all authored aliases migrate to `@/`. |
| Platform composition | `apps/dashboard/src/pages/platform/` | Reduce `PlatformPage.tsx` to tenant/access/provider/sidebar/menu composition; extract `components/PlatformSidebar.tsx`, composition-only `PlatformMenu.tsx`, and `hooks/usePlatformFacet.ts`. |
| Feature views | `apps/dashboard/src/pages/platform/views/` | Add `OverviewPage.tsx`, `CatalogPage.tsx`, `RecordPage.tsx`, `ObservationViews.tsx` (Logs/Usage), and `ResourceViews.tsx` (Gateway/Facet/Channels/System); keep existing Administration/Audit/Operations entry components in the parent page folder. |
| Platform reusable components | `apps/dashboard/src/pages/platform/components/` | Add named frame/feedback/status/navigation/record/detail/provider/settings components; no imports from pages or views. |
| Platform state and presentation rules | `apps/dashboard/src/pages/platform/` | Add `platformView.ts`, `platformFormatting.ts`, `platformList.ts`, and `hooks/usePlatformFreshness.ts`; rename hook modules to `useOptimisticRows.ts` and `useRestingDialog.ts`. |
| Supplier editor | `apps/dashboard/src/pages/platform/` | Extract supplier form types/field presentation or bounded subcomponents from `SupplierEditor.tsx` so no authored module exceeds 1,000 lines without changing writes/recovery. |
| Shared/data | `apps/dashboard/src/components/`, `data/`, `shared/`, `utils/` | Retain ownership, update imports only; dependency tests prevent upward imports and React in data adapters. |
| Tests/config | `apps/dashboard/src/__tests__/`, `tsconfig.json`, `vitest.config.ts` | Add `frontendArchitecture.test.ts`; configure only `@/` and existing UI aliases. |

## Research

For each decision, name the in-repo prior art: the file that already does this, and why a new pattern is required if one is.

- **Canonical alias**: Existing `tsconfig.json`, `vitest.config.ts`, and all authored imports currently split `@renderer/` and `@shared/`. Select `@/` as the single application alias; keep `@a3s-lab/ui` as the external design-system contract. Do not retain compatibility aliases.
- **Composition root**: Existing `src/App.tsx` already demonstrates a small provider/gate composition root. Apply the same pattern to `PlatformPage.tsx`.
- **Collection primitives**: Existing `src/components/collection/` already owns reusable controls and hooks. Platform record/table components remain platform-specific under `pages/platform/components/` rather than being promoted prematurely to app-global primitives.
- **Data boundary**: Existing `src/data/platformOperations.ts` demonstrates transport plus strict closed-set decoding without React. Preserve it as the model for data adapters.
- **Tenant boundary**: Existing `PlatformTenantBoundary.tsx` and `PlatformTenantBoundary.test.tsx` are the safety prior art; extraction keeps the boundary above all platform views.
- **Native controls**: Existing `src/__tests__/noNativeSelect.test.ts` is the source-scan prior art. Generalize carefully into a complementary architecture test without weakening its UI/vendor coverage.
- **Supplier design**: Existing `SupplierEditor.tsx` and `SupplierCreateRecovery.tsx` own complex writes and recovery. Extract cohesive form definitions/presentation only; do not redesign the state machine.

Detailed alternatives and line-budget decisions are in `research.md`.

## Verification Plan

- Hermetic tests: `npm test` in `apps/dashboard`, standalone; all existing tests plus `frontendArchitecture.test.ts` must pass without increased timeouts.
- Focused checks during migration: `npm run typecheck`, `npx vitest run src/__tests__/frontendArchitecture.test.ts src/pages/platform/__tests__ src/data/__tests__/platformOperations.test.ts`.
- Ratchet that must stay green: `src/__tests__/noNativeSelect.test.ts`; no native picker or new alias may be introduced.
- Build: `npm run build`, including TypeScript, Rsbuild and CSS contract.
- Structural metrics: record `PlatformPage.tsx` <=400 lines, every authored production module <=1,000 lines, no legacy aliases, no old-module compatibility wrappers.
- Adversarial review: inspect changed import graph and tests for tenant-key loss, accidental demo fallback, renamed storage/API keys, and tests weakened merely to pass.
- Explicitly not run: no live authenticated production E2E, backend suite, deployment, pixel-perfect screenshot approval, or full parent Cargo workspace campaign; these are not required for a behavior-preserving frontend structural migration and will not be claimed.
