# Research: Dashboard console simplification

## Evidence reviewed

- `apps/dashboard/src/pages/platform/platformNav.ts`: 18 declared menus, two retired, nested facet/reference/SMS navigation.
- `PlatformMenu.tsx`: current mapping of Overview, Operations, Administration, catalogs, resources, observability and system pages.
- `views/OverviewPage.tsx`: eight parallel data reads and six content regions; no single Application next action.
- `memberOrgs.ts` / `platformAccess.ts`: official-org and independent Administration grant behavior.
- `apps/cloud/docs/mvp-product-journey.md`: Application-first journey, scope continuity, activation target and M1–M6 acceptance.
- Root `DESIGN.md`: single visual authority, collection development standard, no inert controls, accessibility/reflow requirements.
- `apps/dashboard/FRONTEND.md` and spec `016`: composition, naming, dependency, test and complexity contracts.
- Local browser evidence from 2026-10-27: real captcha/login and platform landing work; this does not prove activation or M1–M6.

## Key architectural finding

A menu-only redesign would leave the core defect intact: the current composition root requires official-platform visibility before any page is available. The target developer workspace therefore needs its own membership/Application capability boundary, while Administration keeps the existing official/directory/resource-grant boundary.

## Decisions and rejected alternatives

### Workspace + Administration, not deletion

Retain advanced administration inside Organization & Settings. Deleting or globally hiding it would break real operator workflows and independent grants.

### Capability/scoped-route foundation before visual migration

Routes and access determine what can safely render. Implementing a new shell first would create fake destinations and scope-restoration debt.

### Typed Application features, not more generic `PlatformItem` views

The generic string-field renderer is suitable for parts of legacy Administration but cannot express Application lifecycle, exact grants, delivery state and typed error semantics. Keep it contained; use typed owner DTOs for Application features.

### Existing design system, not a new theme

The user's refinement problem is primarily hierarchy, density and task flow, but the interface also needs meaningful semantic color rather than an intentionally all-gray presentation. Root `DESIGN.md` remains the only visual authority. Add status/chart/feature-accent tokens there after contrast review when existing roles are insufficient; do not create a dashboard palette, rainbow navigation, decorative gradients or color-only states.

### Practical value over feature completeness

Every prominent feature needs a declared high-frequency task, practical value and ready owner capability. Core workflows stay prominent, advanced workflows move to on-demand Administration, and duplicate/unavailable controls are removed. This is stronger than visual hiding: a feature inventory with retain/demote/remove reasons is part of Phase 0.

### Incremental migration, not big bang

Phase capability/routes first, simplify real existing screens second, then land PR2–PR7 owner-backed Application features. Applications remains honestly unavailable before PR2 rather than becoming a demo entry.

### Independent page states, not a blocking overview request wall

Overview uses bounded owner/BFF reads with per-section loading/error behavior. A read aggregate is not a new Console database, scheduler or health authority.

## Open implementation decisions (not blockers to the plan)

- Exact member/BFF route names and DTOs for Application context/capabilities must be finalized with their owners before Phase 1/3 implementation.
- The final source path for new Workspace features (`features/` versus bounded `pages/` areas) must follow the existing frontend naming standard and avoid a second composition root.
- Overview aggregate request counts and performance budgets require Phase 0 measurements on the reference deployment; no unmeasured latency target is claimed.
- Exact implementation dates depend on PR1 and Application owner readiness; phase exits, not dates, govern completion.
