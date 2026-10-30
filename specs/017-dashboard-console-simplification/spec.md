# Feature Specification: Dashboard console simplification and UX modernization

**Feature Branch**: Current worktree; no new branch requested.

**Created**: 2026-10-27

**Status**: Planned

**Input**: "站在架构师和产品经理的角度规划优化计划"

**Gate context**: `apps/dashboard` is the Kense Console experience for the existing Application MVP journey (PR2–PR8), while also retaining the already delivered official-platform administration surface. This planning epic addresses the user's reported excessive functional complexity and insufficient UI/UX refinement. It does not create a second product lifecycle, design system, Cloud authority, or acceptance shortcut.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Complete developer work in a focused workspace (Priority: P1)

An independent developer or small-team member signs in and sees a small set of task-oriented destinations: Overview, Applications, Operations, and Usage. They can create, test, call, update, diagnose and roll back one Application without understanding supplier, gateway, fleet, ACL, OCI or workflow administration.

**Why this priority**: This is the product promise and the main reason the current platform-admin-oriented navigation is too complex. The existing MVP requires at least 4 of 5 target users to reach a first real answer without assistance, with a median login-to-first-answer time of at most 10 minutes.

**Independent Test**: Hermetic Console route/access/view tests prove the five-entry workspace navigation, Application detail continuity, unsupported-action refusal and scope-isolated caches. Real M1–M6 acceptance is performed only when each corresponding owner/API phase lands; demo mode never counts.

**Acceptance Scenarios**:

1. **Given** an authorized ordinary organization, **When** the member enters Console, **Then** the workspace is available without requiring official-platform administration permission and the first authorized action is clear.
2. **Given** an Application, **When** the user moves between test, keys, deployments, logs, usage and rollback, **Then** the same application/org/project/environment scope and identity are retained.
3. **Given** an unsupported or unauthorized capability, **When** the page renders, **Then** the executable control is omitted or disabled with its documented reason and no anonymous/demo fallback occurs.

### User Story 2 - Retain platform administration without dominating daily work (Priority: P1)

An official-platform operator opens a separate Administration area and accesses customers, suppliers, models, services, workloads, gateway, fleet, logs, authentication and system configuration with their existing independent grants.

**Why this priority**: Existing administration is real working functionality and must not be deleted or hidden behind a universal admin role. Separating it from the developer workspace simplifies the product without weakening operational ownership or authorization.

**Independent Test**: Access tests prove each existing menu is reachable under the same grant as before, retired destinations remain closed, and Administration visibility never becomes a prerequisite for the developer workspace. Deep-link and remembered-view migration tests prevent stale or unauthorized destinations.

**Acceptance Scenarios**:

1. **Given** directory administration or an independent platform read grant, **When** the operator opens Administration, **Then** only the currently authorized destinations and operations appear.
2. **Given** a user without platform administration grants, **When** they open Console, **Then** no Administration destination is shown and protected routes fail closed.
3. **Given** a remembered legacy menu, **When** navigation migrates, **Then** an authorized view restores correctly and a retired/unknown/unauthorized view falls back to a safe named destination.

### User Story 3 - Read and act through a refined, consistent interface (Priority: P1)

A user sees one clear primary action per page, restrained content hierarchy, readable tables and forms, useful empty/error states, and no inert selection or decorative complexity. Keyboard, touch, narrow-window and Chinese/English experiences follow the same product contract.

**Why this priority**: UI/UX refinement must reduce cognitive load rather than add another visual theme. Current screens mix multiple card groups, dense controls, nested navigation and selection bars without real batch work.

**Independent Test**: Component and browser tests verify the collection contract, actual keyboard/focus flows, 320 CSS-pixel reflow where supported, reduced motion, long translations, real loading/empty/no-match/error states, and no native picker or edge-sliding dialog. Visual review covers the reference viewport matrix.

**Acceptance Scenarios**:

1. **Given** an overview, **When** it loads, **Then** it has one primary next action, at most four summary metrics and at most two main content sections below them; one failed widget does not masquerade as zero or block unrelated sections.
2. **Given** a collection without a supported batch action, **When** it renders, **Then** no checkbox selection or selection bar is shown.
3. **Given** a management collection, **When** search/filter/sort/page/scope changes occur, **Then** controls remain in one coherent toolbar and request/cache identity follows the full scope.
4. **Given** keyboard or a narrow viewport, **When** the user navigates and edits, **Then** controls remain reachable, focus is visible, targets meet the design contract and content does not lose actions.

### User Story 4 - Understand system state through Application-centered observability (Priority: P2)

A developer opens Operations or Usage and sees state correlated to their Application, including build/deploy/invoke history, actionable failures and request/token facts. Operators can still use broader platform observability inside Administration.

**Why this priority**: The current generic Operation list and platform-wide usage are useful but do not form one Application story. This is the PR7/M6 product requirement.

**Independent Test**: Typed decoding and component tests prove correlation IDs, unknown price/usage quality markers, pagination and no cross-tenant/Application leaks. Live owner evidence is required for the actual M6 gate.

**Acceptance Scenarios**:

1. **Given** a failed build, **When** the user opens it from the Application, **Then** terminal state, admitted logs, next action and unchanged serving state are clear.
2. **Given** incomplete usage or unknown pricing, **When** Usage renders, **Then** it is explicitly marked and never shown as zero or an invoice.
3. **Given** an Operation without a supported owner lifecycle action, **When** it renders, **Then** no generic retry/cancel control is invented.

## Edge Cases

- Ordinary non-official organizations must not be locked out of the developer workspace merely because official-platform administration is unavailable.
- A user may have supplier/model read grants without directory administration; those grants open only their existing Administration destinations.
- Application routes require validated org/project/environment/application scope. Unknown, stale or cross-scope IDs fail closed without leaking resource existence.
- Legacy menu storage and shareable URLs need explicit migration; unknown destinations do not become arbitrary components.
- An API/read failure remains distinct from a successful empty collection and does not activate demo fixtures.
- Removing checkbox selection must not remove supported supplier row actions or future declared batch operations.
- Administration may have several sections, but workspace navigation stays small; new features must not reintroduce top-level growth by default.
- Visual simplification must not hide degraded/converging state, incomplete data, write conflicts, unresolved intents or required recovery actions.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: Divide Console into a task-oriented Workspace and a permission-gated Administration area. Workspace top-level destinations are Overview, Applications, Operations, Usage and one Organization/Settings entry; Administration is one separate destination, not a parallel set of daily-work menus.
- **FR-002**: Decouple workspace access from `canSeePlatformAdmin`. Authentication, organization membership and Application owner capabilities govern the workspace; existing official/directory/resource grants continue to govern each Administration destination. No universal admin bypass is allowed.
- **FR-003**: Limit Workspace to at most five primary destinations and at most two navigation levels. Remove the workspace sidebar search when the set is small; Administration MAY retain search. New top-level destinations require an explicit product decision.
- **FR-004**: Rebuild Overview around the next user action, recent Applications and actionable state, with at most four summary metrics and two main content sections. Do not show a platform metric wall or replace failed values with zero/em dash success.
- **FR-005**: Center the Application experience on one stable `ApplicationId` and one detail route covering overview, test, external keys, source/build/deploy activity, logs, usage and rollback according to owner availability. Do not create a second Product identity or expose runtime credentials.
- **FR-006**: Introduce typed, validated routes carrying full org/project/environment/Application scope and Back/Forward restoration. Legacy view restoration must be migrated explicitly; unknown/unauthorized routes and malformed scopes must fail closed.
- **FR-007**: Keep Cloud and owning services authoritative. `apps/api` and member adapters remain thin; Console MUST NOT create local Application, Grant, Route, Workload, Usage, Operation or health truth.
- **FR-008**: New Application screens use typed owner DTOs and capabilities, not the administration-only generic `PlatformItem.fields` rendering model. Unsupported operations are omitted or explained, never simulated.
- **FR-009**: Preserve existing administration APIs, independent grants, supplier writes/recovery, audit and Operations behavior while relocating their navigation. No enabled generic Operation cancel/retry or unsupported create/edit/delete/export action is allowed.
- **FR-010**: Apply the root `DESIGN.md` as the only visual authority: white work surfaces, existing type/color/shape roles, one primary action, coherent toolbars, 48px table rows, drawn pickers and in-place dialogs. Do not create a dashboard-specific palette or second design spec.
- **FR-011**: Remove checkboxes and selection bars from collections without supported batch actions. Keep safe frequent row actions visible, secondary actions in named overflow menus, and destructive actions last.
- **FR-012**: Simplify each collection to title/purpose, one toolbar, conditional active filters, table/list and pager. Detail complexity chooses an Application route or bounded dialog/drawer; avoid nested cards and multi-level expanded content without a user task.
- **FR-013**: Preserve distinct loading, refresh, empty, no-match, read-failure, conflict, uncertain-write and authority-withdrawn states. Failed/incomplete data MUST NOT appear as zero, successful emptiness or an invoice.
- **FR-014**: Meet WCAG 2.2 AA for new/changed workflows, keyboard/focus behavior, reduced motion, 320 CSS-pixel reflow where supported, locale-complete text and long-translation resilience. Essential text contrast takes precedence over muted styling.
- **FR-015**: Establish reference-viewport visual regression, actual browser interaction, accessibility and real-backend acceptance before calling the redesign complete. Component snapshots, demo data and build success alone MUST be rejected as acceptance.
- **FR-016**: Preserve the frontend standards and architecture tests from `016-dashboard-frontend-standards`. New feature composition follows `App/gates → pages/views → components/hooks/data`; data adapters remain React-free and shared modules do not import features.
- **FR-017**: Prioritize practical, high-frequency workflows over feature counts. Every destination and prominent control MUST have a declared user task, owner capability and measurable value; low-value, unavailable or duplicate controls are removed rather than retained for completeness.
- **FR-018**: Use meaningful semantic color for status, health, trends, charts, actionable alerts and approved feature/action regions. Navigation selection stays neutral, status never relies on color alone, and any missing semantic tokens are added through the root `DESIGN.md` after contrast review rather than hardcoded page palettes.

### Key Entities *(include if the feature involves data)*

- **Workspace**: Authenticated developer/member experience; availability derives from membership and Application owner capabilities, not official-platform administration.
- **Administration**: Existing official-platform management experience; every destination retains independent grant checks and its owning APIs.
- **Application**: Applications-owned user-facing identity; Console routes and views correlate its releases, builds, deployments, invocations, keys, usage and audit without owning them.
- **Console scope**: `{organizationId, projectId, environmentId, applicationId?}`; required in routes, requests, mutations and caches as applicable.
- **Capability**: Explicit owner/grant-derived ability used to expose a destination or action; role names and menu labels are not authorities.
- **UX contract**: Per-page declared primary action, content hierarchy, collection behavior, state matrix, responsive/accessibility behavior and retained evidence.

## House Checklist *(mandatory — from Agents.md)*

- [ ] New behavior has small hermetic tests in the owning package; owner persistence follows its existing adapter regime.
- [ ] Closed sets: new route/destination/capability sets have one typed definition and explicit unknown-value refusal.
- [ ] No production `unwrap()` / `expect()` without an infallibility comment.
- [ ] Internal `a3s-*` dependencies stay on a path in this repository.
- [ ] Product configuration, if any, is ACL parsed by `a3s-acl`; UX specifications are documentation, not a second runtime configuration language.
- [ ] Evidence target: `docs/evidence/dashboard/<date>-console-simplification/OUTCOME.md` for each implementation slice.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: Workspace exposes at most five primary destinations and at most two navigation levels; Administration retains all currently authorized workflows under one separate destination.
- **SC-002**: Overview has one primary next action, at most four metrics and at most two main content sections; no inert selection controls remain on collections without batch actions.
- **SC-003**: Ordinary authorized non-official organizations can enter the workspace without platform-admin permission, while cross-scope and unauthorized Administration/Application requests fail closed in hermetic and real-owner tests.
- **SC-004**: PR2–PR7 retain one `application_id/workload_id/route_id` continuity chain; Console never creates a second authority and real M1–M6 acceptance remains the gate.
- **SC-005**: Existing activation target is preserved: at least 4 of 5 target users reach first real answer without assistance, median login-to-first-answer <=10 minutes, with timings retained.
- **SC-006**: New/changed workflows pass keyboard/focus, reduced-motion, both-locale, long-text and supported 320px/400% reflow checks plus reference-viewport visual review; essential text meets WCAG 2.2 AA contrast.
- **SC-007**: Every implementation slice passes dashboard standalone Vitest, typecheck, production build/CSS contract, applicable real-browser/API checks and retained OUTCOME evidence; no timeout/allowlist weakening.
- **SC-008**: Baseline and final UX reviews record navigation count/depth, primary-task click count, collection-control inventory, request counts and representative load/error behavior so simplification is measurable rather than subjective.
- **SC-009**: Every prominent destination/control has a documented task/value/owner reason. Semantic status and chart colors pass contrast and non-color identification checks, with no rainbow navigation, decorative status recoloring or dashboard-local color tokens.

## Assumptions

- This request authorizes planning only; no production code or backend contract is changed in this epic's planning artifacts.
- Existing real platform administration is retained. It is not deleted merely because it is advanced or unrelated to the Application MVP.
- The root `DESIGN.md`, frontend standards and current Cloud MVP journey are the governing baselines. Conflicting visual ideas are resolved in their favor.
- Implementation follows dependency-ready slices, not a big-bang redesign. Application screens land only with their real owner contracts; navigation must not advertise fake readiness.
- Visual/interaction redesign is allowed to change current DOM/layout, but not API meaning, authorization, tenant isolation, secrets or owner identities.

## Explicitly Not Claimed

- No UI redesign, Application MVP gate, production deployment, backend completion, accessibility certification, performance certification or UAT result is claimed by this specification.
- No second design system, generic low-code entity renderer, universal admin role, new Product aggregate, local Console ledger, or demo-only Application journey is permitted.
- Exact delivery dates are not promised; phase order is dependency-driven and implementation estimates require the owning API teams' review.
