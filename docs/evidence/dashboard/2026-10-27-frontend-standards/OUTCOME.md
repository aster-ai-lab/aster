# Dashboard frontend standards — 2026-10-27

## Result

Completed the bounded spec-kit epic `016-dashboard-frontend-standards` across the parent planning/evidence tree and the `apps/dashboard` submodule. The dashboard now has one authored frontend contract, one application import alias, executable naming/dependency/complexity checks, and a composable platform implementation.

The original 4,644-line `PlatformPage.tsx` is now a 113-line composition root. The 1,085-line `SupplierEditor.tsx` is now 969 lines after cohesive form and summary extraction. No authored production module exceeds 1,000 lines.

No commit was created. The dashboard submodule contains the complete migration; the parent contains the spec and this evidence. The parent's pre-existing dashboard gitlink update remains uncommitted.

## What landed

- Added `apps/dashboard/FRONTEND.md` covering file/folder/identifier naming, canonical imports, dependency direction, page/view/component/data responsibilities, component props and state, tenant/session safety, styling, accessibility, complexity budgets, tests and review gates. Linked it from the dashboard README.
- Standardized authored application imports on `@/`; removed `@renderer` and `@shared` from TypeScript/Vitest configuration and authored source. `@a3s-lab/ui` remains the independent design-system package.
- Renamed hook modules to `useOptimisticRows.ts`, `useRestingDialog.ts`, and `useDashboardNavigate.ts`; retained `bridge.ts` for the host bridge contract.
- Added platform composition (`PlatformPage`, `PlatformMenu`, `PlatformSidebar`), feature views, reusable record/detail/status/feedback components, and named hooks for facets, list memory, freshness and search.
- Extracted view configuration, field/status formatting, list state, attention sources, request keys and shared platform types. Existing menu IDs, request paths, storage keys, SWR keys, resource decoders and permission gates remain in use.
- Split supplier form labels/draft/payload conversion and summary rendering without changing supplier writes, recovery, API payloads or dialog implementation.
- Added `frontendArchitecture.test.ts`: authored file naming, legacy-alias refusal, component/data/shared dependency direction, 400-line platform composition budget, and 1,000-line production-module budget. Tests scan authored source only.
- Preserved and passed the existing access, tenant isolation, customer subscription, Operations, i18n and native-picker tests without increased timeouts or new allowlist entries.

## Verification actually completed

Executed in `apps/dashboard` after the final corrections:

- `npm run build`: passed. This includes `tsc -p tsconfig.json --noEmit`, Rsbuild production output, and `scripts/check-css.mjs`.
- CSS contract: passed, 1 file / 171,630 bytes; application utilities present.
- `npm test`: **8 test files, 34 tests passed** in the standalone run. No timeout increase or skipped test.
- `npm run typecheck`: also passed independently during the migration and supplier extraction checks.
- `git diff --check`: passed for the dashboard change and parent spec path. Git emitted only existing LF-to-CRLF working-copy warnings on Windows, not whitespace errors.
- Authored-source census: `PlatformPage.tsx` 113 lines; largest production module `SupplierEditor.tsx` 969 lines; modules over 1,000 lines: 0.
- Source search: no `@renderer`, `@shared`, old optimistic-row hook path, or old resting-dialog hook path in authored dashboard source/config. The sole remaining `@renderer` text is a comment in the read-only `vendor/ui` copy.
- Before implementation, the architecture test failed on the intended legacy aliases, hook naming and oversized modules; after migration all four architecture tests pass.
- `npm run dev -- --host 127.0.0.1 --port 7100`: startup arguments accepted and the preview returned HTTP 200. The temporary process tree was then stopped; a follow-up request confirmed port 7100 no longer served the temporary preview.

## Adversarial review and decisions

- Corrected extraction-only visual drift in the overview attention row, preview badge, supplier row-action shadow, and log-level count presentation before final verification.
- Kept `PlatformMenu.tsx` in the composition layer rather than reusable components, so the component-to-view dependency ban remains meaningful.
- Kept platform-specific record components under `pages/platform/components/`; did not promote them into a premature app-global entity framework.
- Did not retain compatibility aliases or old-module re-export wrappers. Git history, rather than parallel import conventions, is the comparison mechanism with the original ToWork extraction.
- Rejected combining the migration with SWR/router/state-library replacement, backend changes, or the Application MVP feature build.

## Explicitly not claimed

- No authenticated production end-to-end run, live Cloud data verification, deployment, backend/API regression, pixel-level screenshot approval, or full parent Cargo suite was performed.
- No PR2–PR8 Application product gate is claimed. This was a frontend standards and behavior-preserving structural migration only.
- Unit tests and the CSS contract do not prove every visual pixel or production interaction. They support the intended preserved DOM/class/API/cache/access contracts but are not a substitute for independent browser UAT.
- The read-only `vendor/ui` implementation, generated login assets, and ToWork login source were not redesigned.
