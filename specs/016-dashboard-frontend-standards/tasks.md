# Tasks: Dashboard frontend standards and structural refactoring

**Input**: Design documents from `specs/016-dashboard-frontend-standards/`

**Prerequisites**: plan.md (required, including its Constitution Check), spec.md (required), research.md

**House rule**: every task that changes behavior carries its hermetic test in the same task. This is a behavior-preserving structural epic; each migration task carries source-contract and existing regression verification, and no behavior assertion may be weakened.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies)
- **[Story]**: Which user story this task belongs to (US1 standards, US2 structure, US3 component safety)

## Phase 1: Behavior and tests

- [x] T001 [US1] Add `apps/dashboard/FRONTEND.md` as the canonical contributor contract and link it from `apps/dashboard/README.md`; document naming, aliases, ownership, dependency direction, component props/state/data boundaries, styling/accessibility, complexity budgets, testing and review gates. Preserve ToWork/vendor ownership notes without retaining legacy import aliases.
- [x] T002 [US1] Add `apps/dashboard/src/__tests__/frontendArchitecture.test.ts` with hermetic checks for authored TypeScript file names, canonical `@/` imports, forbidden legacy aliases, data/shared/component dependency directions, `PlatformPage.tsx` <=400 lines, and authored production modules <=1,000 lines. Keep explicit closed exceptions for entry/index/setup/generated files; run it and confirm it fails on the pre-migration tree for the intended reasons.
- [x] T003 [US3] Preserve the existing regression suite as the compatibility contract: `platformAccess`, `PlatformTenantBoundary`, `CustomerSubscription`, `OperationsPage`, `platformOperations`, `i18n`, and `noNativeSelect`. Add focused source assertions only where extraction could drop tenant/access/preview boundaries; do not change expected product behavior or increase timeouts.

## Phase 2: Wiring

- [x] T004 [US1] Migrate `apps/dashboard/tsconfig.json`, `apps/dashboard/vitest.config.ts`, `apps/dashboard/src/test/setup.ts`, and every authored source/test import to `@/`; remove `@renderer` and `@shared` configuration and imports. Rename `optimisticRows.ts` to `useOptimisticRows.ts` and `restingDialog.ts` to `useRestingDialog.ts`, updating all callers. Run T002 and typecheck.
- [x] T005 [US2] Extract platform presentation/domain helpers from `PlatformPage.tsx` into `platformView.ts`, `platformFormatting.ts`, `platformList.ts`, and `hooks/usePlatformFreshness.ts`; extract `hooks/usePlatformFacet.ts`. Preserve closed sets, SWR keys, list-memory semantics, formats and status labels. Run focused architecture and existing platform tests.
- [x] T006 [US2] Extract platform reusable UI into `pages/platform/components/`: sidebar/navigation, `PlatformFrame`, refresh/freshness display, feedback/loading, status/name marks, row actions, record table/detail/history, provider list, grouped settings and captcha rules as cohesive named modules. Keep view-composing `PlatformMenu.tsx` in `pages/platform/`; reusable components must not import pages/views and use typed domain-named props. Run T002 and typecheck.
- [x] T007 [US2] Extract menu-level views into `pages/platform/views/`: Overview, Catalog/RemoteCatalog, generic Record, Logs, Usage, Gateway, Facet, Channels and System. Keep existing Administration/Audit/Operations pages as named entries. Preserve data loaders, request paths, tenant scopes, empty/error states and interactions. Run existing platform/data tests and typecheck.
- [x] T008 [US2] Rebuild `pages/platform/PlatformPage.tsx` as the <=400-line composition root for account/org access, tenant boundary, supplier provider, sidebar and current view. Remove all extracted private implementations and compatibility re-exports; verify T002, tenant/access tests, and typecheck.
- [x] T009 [US3] Extract cohesive supplier form definitions or subcomponents from `SupplierEditor.tsx` so every authored production module is <=1,000 lines, without changing supplier access, optimistic writes, recovery, dialog motion, or API payloads. Run T002 and the full standalone dashboard suite.
- [x] T010 [US3] Adversarially review the final import graph and diff for lost tenant-scoped keys, changed storage/API/user-facing identifiers, demo fallback, widened access, stale documentation paths, duplicate old/new implementations, and weakened tests. Resolve confirmed findings or record exact remaining follow-up.

## Phase 3: Evidence

- [x] T011 [US1] Run standalone `npm test`, `npm run typecheck`, and `npm run build` in `apps/dashboard`; record exact test files/test counts and CSS result. Record before/after module line counts, zero legacy aliases, and architecture-test results.
- [x] T012 [US1] Write `docs/evidence/dashboard/2026-10-27-frontend-standards/OUTCOME.md` with Result, What landed, Verification, Abandoned alternatives/decisions, and Explicitly not claimed. Do not claim visual/production E2E, deployment, Application MVP gates, or backend validation.
- [x] T013 Confirmed no commit was requested; no files were staged or committed. If a commit is later requested, stage task files individually and verify `git diff --cached --name-only` first.

## Final Verification Gate

- [x] The owning package's focused test passes
- [x] Full standalone dashboard Vitest passes without increased timeouts
- [x] `npm run typecheck` and `npm run build` pass, including CSS contract
- [x] `cargo fmt --all -- --check` is N-A because no Rust files are edited
- [x] No new crates.io `a3s-*` dependency
- [x] No native `<select>` added on a product surface
- [x] Product configuration, if added, is ACL (none added)
- [x] `PlatformPage.tsx` <=400 lines and all authored production modules <=1,000 lines
- [x] No `@renderer`/`@shared` imports or compatibility wrappers remain in authored dashboard source/config
- [x] Existing access, tenant, Operations, i18n, subscription and picker assertions remain intact
