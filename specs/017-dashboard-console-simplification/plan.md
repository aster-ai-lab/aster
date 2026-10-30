# Implementation Plan: Dashboard console simplification and UX modernization

**Branch**: Current worktree; planning only | **Spec**: [`specs/017-dashboard-console-simplification/spec.md`](spec.md) | **Input**: Architect and product-manager optimization planning

**Gate context**: Kense Console (`apps/dashboard`) Workspace + retained official-platform Administration, aligned to the existing Application PR2–PR8 journey and root `DESIGN.md`.

## Summary

Restructure Console around two distinct jobs: developers progress one Application through its lifecycle, while authorized operators administer the platform in a separate settings area. Build this on the recently established frontend boundaries, not through another menu reskin. Introduce explicit capability and scoped-route contracts, simplify Overview and collections using real owner data, then deliver the Application screens in the existing PR dependency order. Every phase has product, architecture and evidence exits; planning does not certify implementation.

## Constitution Check

*GATE: Re-read `.specify/memory/constitution.md`. A NON-NEGOTIABLE violation stops the plan.*

| Principle | Status | Evidence in the plan |
| --- | --- | --- |
| I. Fail closed | PASS | Workspace/admin capabilities remain independent; unknown routes, scopes and unsupported actions are refused without demo fallback. |
| II. Evidence retained | PASS | Each implementation slice writes a scoped OUTCOME; PR2–PR7 continuity and PR8 UAT remain mandatory. |
| III. Hermetic tests | PASS | Dashboard source/component/browser tests plus owner-required adapter proofs; no unit-test-only product certification. |
| IV. Closed sets | PASS | One typed route/destination/capability registry; unknown values refused. |
| V. One workspace / published APIs | PASS | Existing dashboard, thin member/BFF and owner APIs; no new SDK/workspace/authority. |
| VI. Local schema changes | N-A for planning | Any later owner change follows its owner/regime; Console gets no local schema. |
| VII. Shrink-only ratchets | PASS | Existing native-picker/frontend architecture ratchets stay strict; inert selection and top-level navigation only shrink by default. |
| VIII. Replaceable transports | PASS | Browser/member transport remains behind existing data/shared adapters. |
| IX. One configuration language | N-A | No product configuration added; planning documents do not create runtime configuration. |
| XIII. One repository | PASS | Changes remain in this repository and existing submodules. |
| XIV. Names are not guessed | PASS | Product/route/capability names come from spec and exact owner identifiers, never extracted by matchers. |
| XV. Meaning is not matched | PASS | UX intent is declared by the product spec, not inferred by frontend regex/word lists. |
| XVI. Pickers are drawn | PASS | Root DESIGN and existing native-picker test remain mandatory. |
| XVII. Dialogs stay in place | PASS | Root DESIGN in-place dialog motion remains mandatory. |

## 1. Product-manager assessment

### 1.1 Actual baseline

| Dimension | Current observed implementation | Product consequence |
| --- | --- | --- |
| Navigation | 18 declared menu IDs; orders/tickets retired, leaving 16 for directory administrators; up to three levels | Advanced administration dominates the default surface. |
| Overview | Eight parallel reads (overview, workflows, routes, nodes, logs, usage, suppliers, sign-in) and six content regions (attention, three metric groups, usage, logs) | High cognitive load and broad failure surface; no single next action. |
| Access | Entire `PlatformPage` depends on official-org `canSeePlatformAdmin` | Ordinary Application developers can be excluded before a workspace exists. |
| Domain model | Generic administration `PlatformItem.fields` drives many tables/details | Useful for legacy admin, inappropriate for typed Application lifecycle and capability semantics. |
| Collections | Dense toolbars, expanded details and selection even when only count/clear is available | Controls imply batch capability that does not exist. |
| Design | Shared Aster controls exist, but page hierarchy, content density and task priority vary | UI consistency has not produced product clarity. |
| Verification | Real login/captcha/platform landing works locally; hermetic suite/build exists | This is a useful baseline, not activation, accessibility or M1–M6 acceptance. |

### 1.2 Personas and jobs

| Persona | Primary job | Default surface | Must not need to understand |
| --- | --- | --- | --- |
| Independent developer / small-team member | Create and operate one Application, test a real answer, call externally, update and recover | Workspace | Supplier, Fleet, Gateway, OCI digest, ACL, Runtime credential, Workflow internals |
| Team owner / organization admin | Manage members, external access and audit within an organization | Organization & Settings | Official-platform administration unless separately granted |
| Official-platform operator | Govern customers, catalog, suppliers, workloads, gateway, fleet, auth and system | Platform Administration inside Settings | Developer-first overview when performing infrastructure work |
| Auditor / read-only operator | Review authorized records and failures without mutation | Read-only views in the same scopes | Hidden capabilities, raw secrets, generic lifecycle buttons |

### 1.3 Product principles

1. **Application before infrastructure** in Workspace; infrastructure remains complete in Administration.
2. **One page, one primary job and at most one primary action.** Secondary actions are quiet and do not compete visually.
3. **Progressive disclosure by task and capability**, not by hiding essential errors or using role-name shortcuts.
4. **No control without an owner workflow.** Absence is honest; no inert checkbox, fake create, generic retry or demo success.
5. **One Application identity across every view.** Users never manage a release/workload as a new product.
6. **Design tokens are shared; product hierarchy is deliberate.** Reusing a table or card does not justify putting it everywhere.

## 2. Target information architecture

```text
Console
├── Workspace
│   ├── Overview
│   ├── Applications
│   │   ├── Collection / create (when owner capability exists)
│   │   └── Application detail
│   │       ├── Overview / test
│   │       ├── API keys & HTTP example
│   │       ├── Deployments / source / builds / logs
│   │       ├── Usage & activity
│   │       └── Update / cancel / rollback (owner-supported only)
│   ├── Operations
│   ├── Usage
│   └── Organization & Settings
│       ├── Members / roles / external access
│       ├── Audit
│       ├── Preferences
│       └── Platform Administration (only with existing grants)
│           ├── Customers / channels / suppliers
│           ├── Services / models
│           ├── Workloads / gateway / fleet
│           ├── Platform logs / system / sign-in
│           └── Administration overview
```

Rules:

- Workspace has at most five primary destinations and two levels. Administration does not add a sixth primary destination; it is a settings section visible only with an existing grant.
- Administration keeps its current menu IDs, facets and grants. The workspace `Applications` destination remains omitted or honestly unavailable until its real owner contract lands.
- Workspace sidebar search is removed. Administration may retain search because its catalog is legitimately large.
- Operations is Application-scoped in Workspace; platform-wide Operations/observability remains in Administration.

## 3. Page-level product contracts

### 3.1 Overview

**Job:** answer “what should I do next?” and “what needs my attention?”

Layout:

1. Title and one primary action: `创建应用` when supported, otherwise the actual authorized next action.
2. At most four metrics: active Applications, running deployments, failed Operations, requests/tokens in a declared window.
3. Recent Applications (primary content).
4. Actionable Operations / recent activity (secondary content).

Rules:

- No six-region metric wall and no parallel eight-read requirement for first paint. Use an owner/BFF aggregate where available or independent bounded sections with explicit loading/error states.
- A failed metric shows unavailable/retry, never zero. Empty Applications offers the authorized create flow; unsupported create explains availability without a fake CTA.
- Administration overview retains infrastructure summaries inside Administration, not on the developer home.

### 3.2 Applications

**Collection:** name, state, environment, current release, serving URL/state, last deployment and updated time. Only owner-supported filters/sorts are shown. No checkbox column without a real batch command.

**Detail:** one stable Application route with task tabs, not repeated expanded rows. Header shows name, actual delivery state and primary next action. Internal Workload/Route/Runtime identifiers appear only where needed for diagnosis.

**Create:** at most three user inputs for the Chatbot template, with safe defaults and no UUID/ACL/digest/credential fields. Real owner acceptance is required before the entry becomes available.

### 3.3 Operations and Usage

- Operations correlates Application → source/build/deploy/invoke and exposes real terminal states, admitted logs and owner-specific lifecycle actions.
- Usage prioritizes requests/tokens, window, Application and quality markers. Unknown price is `未知`, never zero; estimates are labelled estimates, not invoices.
- Platform-wide views remain available to operators in Administration.

### 3.4 Collections and forms

- Standard order: title/purpose → one toolbar → active filters → table/list → pager.
- Remove selection where only count/clear exists. Keep supplier row actions and any future explicitly supported batch workflow.
- Advanced filters use one named control; no stacked filter rows by default.
- Complex Application detail uses routes; bounded admin editing may use the existing dialog contract.
- Forms use persistent labels/help/errors, dirty protection, reviewed revisions, idempotent creates and recovery; no toast-only error model.

## 4. Architecture plan

### 4.1 Target frontend boundaries

```text
App / SessionGate / OrganizationGate
  → ConsoleCapabilityProvider
  → ScopedRouteGuard
  → ConsoleShell
      ├── Workspace composition
      │   ├── features/overview
      │   ├── features/applications
      │   ├── features/operations
      │   └── features/usage
      └── Organization & Settings composition
          ├── features/organization
          └── features/platform-admin (existing platform views)

shared/
  runtime / session / scoped requests
components/
  collection / feedback / navigation / design-system wrappers
data/
  typed owner adapters + strict codecs (React-free)
```

Decisions:

- Reuse the `016` composition and architecture tests. Do not rebuild the app, replace SWR or introduce a new state library as part of visual simplification.
- Keep existing administration modules during relocation; new Application features use typed DTOs and hooks, not `PlatformItem.fields`.
- `ConsoleCapabilityProvider` exposes destination/action availability derived from exact membership and owner grants. It does not invent a universal admin role or turn hidden UI into authorization.
- `ScopedRouteGuard` validates closed destination/tab sets and full scope before feature data loads. Cross-scope or malformed routes fail closed.

### 4.2 Route model

Illustrative contracts, finalized with the member/BFF owner before implementation:

```text
/dashboard/:orgId/:projectId/:environmentId/overview
/dashboard/:orgId/:projectId/:environmentId/applications
/dashboard/:orgId/:projectId/:environmentId/applications/:applicationId/:tab
/dashboard/:orgId/:projectId/:environmentId/operations
/dashboard/:orgId/:projectId/:environmentId/usage
/dashboard/:orgId/settings/:section
/dashboard/:orgId/admin/:menu/:facet?
```

- Default context redirects only to an actually authorized org/project/environment; no hardcoded tenant IDs.
- Tab and menu sets are typed closed sets. Sensitive search text and secrets never enter URLs.
- SWR keys, mutation inputs and page URLs carry the same scope. Changing scope clears protected detail/selection/drafts and restores only that scope's safe view state.
- Migrate `a3s.studio.platform.view:v1` explicitly: authorized legacy menus map to Administration; retired/unknown entries use a named safe fallback.

### 4.3 Data and ownership

- Applications, Identity, Sources, Artifacts, Workloads, Operations, Inference, Edge and Audit remain the sole authorities of their facts.
- Member/BFF adapters compose authorized reads/commands without persisting a second Application/Operation/Usage store.
- Overview aggregation is a read projection/adapter, not a scheduled Console ledger or health authority.
- New Application capabilities and DTOs are explicit; unsupported fields/enum values fail decoding.
- Runtime credentials remain hidden. External keys are Application-bound, one-time-secret and exact-grant.
- Existing generic administration adapters remain until their own slices are deliberately migrated; no forced conversion to a generic Application renderer.

### 4.4 UX system and accessibility

Use only root `DESIGN.md` and existing UI tokens:

- 24px page title, 18px section, 13px row/control, 12px metadata; Body/Ink replaces muted text when contrast fails.
- White surfaces and existing gray roles provide the base, but the interface is not intentionally colorless. Navigation selection stays neutral; meaningful feature color communicates state and priority inside content.
- Use approved semantic colors for success/healthy, warning, error, information and pending states, with text/icon labels so color is never the only signal. Charts follow `apps/dashboard/CHARTS.md` and the shared `@a3s-lab/ui/chart` implementation in `packages/ui`: a small accessible categorical palette, direct labels/legends, typed data-quality semantics, restricted ChartProps and real runtime/accessibility evidence rather than many similar grays, page-local options or project-specific chart runtimes.
- Approved feature/action regions may use subtle tinted surfaces or icon tiles to establish hierarchy. Do not add rainbow navigation, decorative gradients, arbitrary colored buttons or per-page hex values. Missing tokens are reviewed into root `DESIGN.md` with contrast evidence before use.
- 8px spacing rhythm; data pages use available width, focused forms/settings use the existing centered columns.
- One primary filled action; quiet secondary actions; 30px toolbar controls and 48px table rows.
- Cards group substantive content, not every section; avoid nested cards and repeated borders/shadows.
- Every prominent destination/control records its user task, practical value and owner capability; low-value duplicates and unavailable controls are removed, not retained for feature completeness.
- Real loading/skeleton, refresh, empty, no-match, failure, conflict and unresolved-write states are designed per page before implementation.
- Reference matrix: 320px supported reflow, 768px, 1280px, 1440px; keyboard-only, 400% zoom, reduced motion, zh-CN/en-US and long translations.

## 5. Delivery phases

### Phase 0 — Baseline and contract freeze (P0, prerequisite)

**Product:** record navigation count/depth, overview request/content inventory, primary-task click paths, selection-without-action inventory, viewport baseline and five-user task script. Classify every prominent feature by user frequency, practical value and owner readiness: retain core, demote advanced, remove duplicate/unavailable. Inventory where semantic status/color would improve scanning rather than decorate.

**Architecture:** inventory current grants, route/storage keys, SWR keys, API DTOs and available Application/BFF capabilities. Confirm PR1 status before any PR2 implementation.

**Exit:** spec-approved target IA/page contracts; baseline screenshots/metrics retained; each destination has owner/capability/state declarations. No production code required.

### Phase 1 — Capability, route and shell foundation (P0)

**Product:** validate the five-entry Workspace and Administration-in-Settings with representative developer/operator/auditor scenarios.

**Architecture:** add typed destination/capability registry, scoped routes/guards, workspace/settings composition and legacy-view migration. Decouple workspace from `canSeePlatformAdmin`. Keep Applications unavailable until its real contract exists.

**UX:** implement the refined shell, restrained navigation and responsive behavior without changing API meaning.

**Exit:** ordinary authorized org enters Workspace; all existing admin grants reach the same views; unknown/cross-scope routes fail closed; no fake Application entry; hermetic access/route tests and browser scope-switch proof pass.

### Phase 2 — Overview and collection simplification (P0)

**Product:** deliver the next-action Overview using only real available data, remove inert selection, simplify toolbars/details and write page-level state matrices.

**Architecture:** create bounded overview adapters with independent failures; retain existing owner reads and remove the mandatory eight-read first-paint fan-out where contracts allow. Preserve supplier writes/recovery and Operations read-only semantics.

**UX:** reference-viewport visual pass, one primary action, four-metric/two-section budget, accessible toolbar/table/form behavior. Add reviewed semantic status colors, readable chart colors and selective feature accents while keeping neutral navigation and non-color labels.

**Exit:** SC-001/002/008/009, collection contract evidence, no unsupported controls, semantic-color contrast/non-color identification checks, visual review and real-browser CRUD/read regression pass. Any missing Application data remains explicitly unavailable.

### Phase 3 — Application activation (PR2–PR3, P0 after PR1)

**Product:** deliver template create (≤3 inputs), Application collection/detail, real conversation test, external key creation/copy and HTTP example.

**Architecture:** implement typed Application DTOs and owner continuity evidence for `application_id/workload_id/route_id`; exact grants, hidden runtime credential and scoped mutations.

**Exit:** one real answer and one external call on the same Application; negative authorization/replay/cancellation/billing evidence as required by PR3; at least the activation instrumentation is live. No demo acceptance.

### Phase 4 — Continuous delivery and recovery (PR4–PR6, P1)

**Product:** OCI update, public Git customization, deployment timeline, successful/failed/cancelled build logs, cancel and rollback in the same Application detail.

**Architecture:** preserve Application/Workload/Route/URL/key/grant identity, owner-derived Converging/Ready/Degraded states and immutable successor history.

**Exit:** real continuity, logs, rollback/failure/process-death and post-rollback real-answer evidence. Users see actionable failures rather than generic Operation errors.

### Phase 5 — Application observability and final UX acceptance (PR7–PR8, P1)

**Product:** unified Application activity/usage/cost view, final content and empty/error refinement, second-operator runbook and target-user UAT.

**Architecture:** correlate external caller/invocation/workflow/runtime usage without duplicate ledgers/counting; retain quality markers and tenant isolation.

**Exit:** M1–M6, 4/5 unassisted activation and <=10-minute median, full accessibility/viewport/locale review, real-backend/browser evidence and OUTCOME records. Only then call the redesign complete.

## 6. Measurement plan

| Metric | Baseline action | Target / proof |
| --- | --- | --- |
| Workspace primary destinations | Record current count | ≤5; route registry test |
| Navigation depth | Record current three-level paths | ≤2 in Workspace; authorized admin can be deeper inside Settings |
| Overview content | Current six regions/eight reads | One primary action, ≤4 metrics, ≤2 main sections; bounded/independent reads documented |
| Primary-task clicks | Measure create/test/key/update/diagnose tasks | Each has a named short path; no unexplained detour through Administration |
| Inert controls | Inventory selection/actions without owners | Zero unsupported executable controls; source/browser review |
| Activation | Instrument successful login → first non-empty Answer | ≥4/5 unassisted; median ≤10 minutes; retained timings |
| Scope safety | Test org/project/env/Application switching | No stale data, secret, selection or draft leakage; real negative-owner tests |
| Accessibility | Keyboard, contrast, zoom/reflow, reduced motion, locales | WCAG 2.2 AA requirements for changed workflows with retained evidence |
| Delivery health | Tests/build/browser/API/UAT by slice | Each OUTCOME lists exact commands, counts and unclaimed areas |

Analytics, if added, uses typed product events with Application/scope references only where authorized; no token, secret, message content or sensitive search text.

## 7. Risks and mitigations

| Risk | Mitigation |
| --- | --- |
| “Simplification” hides existing admin functionality | Relocate without changing grants/APIs; run per-destination reachability and supplier/audit regression. |
| Workspace remains locked by official-org checks | Introduce separate Workspace capabilities and ordinary-org tests before visual migration. |
| Fake Application readiness | Do not expose create/detail until real PR2 owner contract and continuity evidence exist. |
| Big-bang route/UI rewrite breaks caches and restoration | Phase shell first, migrate legacy views explicitly, preserve scope keys and test Back/Forward/scope switching. |
| Generic `PlatformItem` spreads into Application domain | Use typed Application DTOs/codecs; contain generic rendering inside legacy Administration. |
| Overview becomes another metric wall or N+1 gateway | Enforce content budget and declared read aggregation; independent errors, no second store. |
| Visual polish hides degraded/incomplete states | State matrix and refusal cases are required design artifacts and acceptance tests. |
| Accessibility is deferred to the end | Keyboard/contrast/reflow/locale checks land with each page, then receive final independent review. |
| Timeline appears to waive PR dependencies | Phase 3 starts only after PR1 and owner readiness; planning estimates do not change the ROADMAP. |

## 8. Project Structure (future implementation map)

| Layer | Path | Planned change |
| --- | --- | --- |
| Console shell | `apps/dashboard/src/` | Console composition, capability provider and scoped route guard. |
| Workspace features | `apps/dashboard/src/features/` or equivalent bounded page areas | Overview, Applications, Operations, Usage; final path follows frontend standards. |
| Organization/settings | `apps/dashboard/src/pages/` | Organization/settings composition and Administration relocation. |
| Existing administration | `apps/dashboard/src/pages/platform/` | Retained views/data/grants, relocated incrementally without authority changes. |
| Shared UI/data | `apps/dashboard/src/components/`, `data/`, `shared/` | Existing collection/runtime reuse; typed owner adapters only. |
| Member/BFF | `apps/api/` | Thin authorized adapters where owner contracts require them; control-plane regime applies. |
| Evidence | `docs/evidence/dashboard/`, existing PR evidence trees | Baseline, slice verification, continuity, browser/accessibility/UAT records. |

## 9. Verification Plan

- Per-slice hermetic dashboard tests: destination/route/capability closed sets, scope isolation, component state matrices, collection behavior and frontend architecture.
- Browser evidence: real login, workspace/admin navigation, Back/Forward, scope switching, keyboard/focus, responsive/reference viewports, real empty/error states and permitted CRUD flows.
- Owner evidence: existing control-plane adapter and API proofs where contracts change; no dashboard-only claim of backend correctness.
- Application continuity: PR2 IDs and PR3–PR7 linked continuity assertions, then PR8 clean-environment full journey and five-user UAT.
- Always: standalone `npm test`, `npm run typecheck`, `npm run build` and CSS contract without weakened tests/timeouts.
- Planning itself runs no implementation tests and claims no product completion.

## 10. Effort and ownership guidance

Use dependency-based slices rather than promising dates. A practical team split is:

- **Product/Design:** baseline, page contracts, prototypes, content, accessibility and UAT.
- **Frontend:** shell/routes/capabilities, views/components, browser tests and visual refinement.
- **Member/BFF + owner teams:** authorized typed contracts, Application lifecycle, usage correlation and owner evidence.
- **QA/Operator:** real environment, negative authorization, failure/restart, second-operator and activation evidence.

Phases 0–2 are the simplification foundation; Phases 3–5 deliver the Application product journey. Parallel work is allowed only where contracts are frozen and no shared route/state authority is being changed.
