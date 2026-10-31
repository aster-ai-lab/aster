# Implementation Plan: Aster/ToWork shell parity to the no-accounts boundary (phase B)

**Branch**: none (working tree, per owner) | **Spec**: [`specs/007-tenant-shell-parity/spec.md`] | **Input**: same

**Gate context**: `apps/studio` — private submodule, product regime (hermetic
Vitest). Renderer shell slice plus `DESIGN.md` wording. No control-plane path.

## Summary

The baseline revert already sits in the working tree (shell files to HEAD,
`AsterShellNav.tsx` deleted, orphan i18n key removed; 124/124 green) and lands
with this epic's evidence. Phase B then closes the parity question as a
reviewed classification: of the fourteen known divergence sites, twelve are
account-bound keeps (managed identity, org scope, or the specs/004 BYOK
contract), one is conditional (插件 rail entry — unifies only if the aix
catalogue proves locally fed), and one is a verification (per-tenant default
accent via the existing `--aster-tenant-brand` stamp). New code is small:
conditional gate flip + parity/absence tests + theme-default test + evidence
captures. Everything else is documentation of the closed set.

## Constitution Check

*GATE: re-read `.specify/memory/constitution.md`. A NON-NEGOTIABLE violation
stops the plan.*

| Principle | Status | Evidence in the plan |
| --- | --- | --- |
| I. Fail closed | PASS | Account-bound destinations stay row-not-exists (`isHiddenWorkbenchPage`); no disabled dead rows; T1 refuses the plugins flip if the source is org-fed |
| II. Evidence retained | PASS | `docs/evidence/desktop/2026-10-22-tenant-parity-phase-b/OUTCOME.md` + side-by-side captures + gate scan |
| III. Hermetic tests | PASS | Renderer Vitest only; no live services, no PostgreSQL |
| IV. Closed sets | PASS | `SidebarAppId` reused; the classification table itself is the closed divergence set; no new enums |
| V. One workspace / published APIs | PASS | Studio submodule stays in root workspace; gitlink only in parent |
| VI. Local schema changes | N-A | No store changes |
| VII. Shrink-only ratchets | PASS | No allowlist edits; `noNativeSelect.test.ts` stays green |
| VIII. Replaceable transports | N-A | None touched |
| IX. One configuration language | N-A | No new config surface |
| XIII. One repository | PASS | All code in `apps/studio`; parent tracks gitlink only |
| XIV. Names are not guessed | N-A | No name extraction |
| XV. Meaning is not matched | N-A | No pattern-matched decisions |
| XVI. Pickers are drawn | N-A | No new choosers |
| XVII. Dialogs stay in place | N-A | No dialog motion changes |

## Project Structure

### Documentation (this feature)

```text
specs/007-tenant-shell-parity/
├── spec.md
├── plan.md
└── tasks.md          (next phase)
```

### Source Code

| Layer | Path | New files |
| --- | --- | --- |
| Aster desktop | `apps/studio/src/renderer/utils/sidebar.ts` | — (conditional: drop `'plugins'` from `LOCAL_HIDDEN_APP_IDS`) |
| Aster desktop | `apps/studio/src/renderer/components/app/Sidebar.tsx` + `__tests__/` | skeleton-parity test additions |
| Aster desktop | `apps/studio/src/renderer/utils/tenantTheme.ts` + `__tests__/` | default-accent test (mechanism exists; test may be new) |
| Aster desktop | `apps/studio/src/renderer/hooks/useAssistantCatalogPresets.ts` | — (T4 read-only verification) |
| Aster desktop | `apps/studio/src/renderer/services/plugins/aixCatalogueStore.ts` callers | — (T1 read-only trace) |
| Design law | `DESIGN.md` (repo root) | — (≤1 sentence beside the tenant-brand statement, only if wording demands) |
| Evidence | `docs/evidence/desktop/2026-10-22-tenant-parity-phase-b/` | `OUTCOME.md`, captures |

## Research

### Divergence classification (the closed set)

| # | Site | aster divergence | Depends on | Verdict |
| --- | --- | --- | --- | --- |
| 1 | `utils/sidebar.ts` `LOCAL_HIDDEN_APP_IDS`: `messages`, `org`, `platform` | rail + workbench page hidden | managed message center; org membership; platform admin | **keep** (account-bound) |
| 2 | `utils/sidebar.ts` `LOCAL_HIDDEN_APP_IDS`: `plugins` | rail + workbench page hidden | aix catalogue (`aixCatalogueStore`, fed by `publishAixCatalogue`) | **T1 conditional unify** — flip only if trace proves a local feed |
| 3 | `utils/sidebar.ts` aster rail-hide `mail`, `schedule` | rail hidden | team sidebar carriers (DESIGN.md §11) | **keep** (account-bound) |
| 4 | `im/useIm.ts` (3 gates) | IM sync off | managed message center | **keep** |
| 5 | `hooks/useAvatar.ts`, `hooks/useUserName.ts` | local vs managed identity | accounts | **keep** |
| 6 | `windows/main/MainApp.tsx` onboarding `null` | onboarding not loaded | account login flow (Cherry/cloud login in `OnboardingPage`) | **keep** |
| 7 | `MessageHeader.tsx` + `useMessageHeaderCapabilities.ts` | identity/profile popup off | managed user profile | **keep** |
| 8 | `hooks/useAssistantCatalogPresets.ts` (3 gates) | published/preset catalog off | org-published catalog | **keep** (T4 verifies the source is org) |
| 9 | `components/layout/ShellTabBarActions.tsx` | settings-only vs +Org workbench/inbox/launchpad | org surfaces (`OrgWorkbenchButton` needs `orgId`) | **keep** — same settings action in both; extras are org-only |
| 10 | `components/app/Sidebar.tsx` | `SidebarContent` local product identity vs `ManagedSidebar` | account header (`AccountCenterMenu`, member binding) | **keep content**; T2 adds a skeleton-parity test (both render the same `UISidebar` frame) |
| 11 | `components/settingsMenu.ts` local-model pages | aster-only additive pages | specs/004 FR-013 BYOK contract | **keep** (product-scoped at the no-accounts boundary) |
| 12 | native `tenant-aster` / `tenant-toyou` cargo features | mutually exclusive builds | build contract (specs/004 FR-009) | **keep** |
| 13 | `tenantBrand.ts` | name/mark/company strings | — | **sanctioned mechanism**, not a gate |
| 14 | `utils/tenantTheme.ts` + `scripts/studioTenantBrand.ts` | `--aster-tenant-brand` stamp | — | **T3 verify + test**: stamp drives per-tenant default accent; saved preference wins |

### In-repo prior art per decision

- Row-not-exists hiding: `isHiddenWorkbenchPage` in `utils/sidebar.ts`
  already implements it; tests in `Sidebar/__tests__/activityBarData.test.tsx`.
- Rail position after a plugins flip: `canonicalActivityBar` /
  `withPluginsOnRail` in `utils/sidebar.ts` already place 插件; no new order
  code.
- Per-tenant brand stamp: `scripts/studioTenantBrand.ts` (injects
  `style="--aster-tenant-brand:…"` at build) and `utils/tenantTheme.ts`
  (reads it). specs/004 FR-002 is the contract.
- Skeleton parity testing: `tenantBrand.test.ts` and `AppShell.test.tsx`
  establish the render-both-modes pattern (`vi.stubEnv('PUBLIC_STUDIO_TENANT', …)`).
- Evidence format: `docs/evidence/desktop/2026-10-21-*` directories.

## Verification Plan

- Hermetic test (owning package = studio renderer):
  `./node_modules/.bin/vitest run --project renderer <files>` covering
  `AppShell.test.tsx`, `Sidebar.test.tsx`, `activityBarData.test.tsx`,
  `tenantBrand.test.ts`, `tenantTheme` test, and the new parity tests.
  Baseline today: 124/124 on AppShell + WorkbenchHome.
- Ratchets that must stay green: `noNativeSelect.test.ts`,
  `activityBarData.test.tsx`, `useSidebarResize` tests, i18n gates via
  `pnpm lint` (or targeted equivalents if pnpm is unavailable in the agent
  shell — note the deviation in evidence).
- Explicitly not run: full `pnpm test` workspace sweep, Playwright e2e
  suites (side-by-side captures are manual evidence), any live OS/PostgreSQL
  service, any packaged build.
