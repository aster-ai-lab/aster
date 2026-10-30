# Feature Specification: Dashboard frontend standards and structural refactoring

**Feature Branch**: Current worktree; no new branch requested.

**Created**: 2026-10-27

**Status**: Implemented

**Input**: "先使用 spec-kit 制定前端开发规范，然后优化 前端文件、文件夹、代码等命名规范和组件设计规范等"

**Gate context**: `apps/dashboard` is the Kense Console frontend. This bounded engineering-quality epic establishes its local frontend contract before the Application console journey grows the existing 4,644-line platform page. It has no independent product roadmap row; it enables the existing PR2–PR7 Console work without claiming any of those product gates. Verification uses the dashboard's own hermetic Vitest project and production build; no control-plane repository or dual-adapter proof is introduced.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Apply one frontend contribution contract (Priority: P1)

A dashboard contributor can determine where a file belongs, how to name it, which imports are allowed, and how to design a component without reverse-engineering unrelated pages.

**Why this priority**: Inconsistent naming and dependency direction make every subsequent Console feature more expensive and increase the chance of tenant, permission, or presentation regressions.

**Independent Test**: A hermetic dashboard architecture test reads the authored source tree and rejects misplaced modules, disallowed dependency directions, ambiguous test placement, native pickers, and newly introduced compatibility aliases. The standards document and actual tree use the same terms.

**Acceptance Scenarios**:

1. **Given** a new platform feature component, **When** the contributor follows the standards, **Then** its file name, folder, export, test location, and permitted imports have one documented answer.
2. **Given** a component importing a page or a data module importing React, **When** the architecture test runs, **Then** it refuses the dependency with the file and rule named.
3. **Given** a native picker or an unspecified compatibility import alias, **When** validation runs, **Then** validation refuses it rather than silently accepting a second convention.

### User Story 2 - Navigate a composable platform frontend (Priority: P1)

A maintainer can open the platform shell, a feature view, a shared component, or a data adapter independently and understand its responsibility from its location and name.

**Why this priority**: The current central page combines navigation, access control, overview, catalog, logs, usage, tables, formatting, and shared controls. Structural separation is required before adding Application workflows.

**Independent Test**: Dashboard unit and component tests retain navigation, access, tenant-cache, subscription, and Operations behavior while source-oriented tests assert the shell/view boundaries and canonical module names. Typecheck and production build prove every relocation is complete.

**Acceptance Scenarios**:

1. **Given** the existing platform page, **When** the refactor lands, **Then** the default export is a composition root and feature views own their rendering and state without changing their public behavior.
2. **Given** shared table, frame, status, formatting, or collection behavior, **When** two or more feature views consume it, **Then** it has a named shared module rather than a private export from an unrelated page.
3. **Given** a renamed or removed legacy module, **When** typecheck and architecture tests run, **Then** no dangling import, duplicate compatibility wrapper, or stale documentation path remains.

### User Story 3 - Review safe, bounded components (Priority: P2)

A reviewer can distinguish presentational components, stateful feature containers, data adapters, and application gates, and can verify that extraction preserved authorization and tenant boundaries.

**Why this priority**: Component design is a safety boundary in this frontend, not merely an aesthetic preference. Extraction must not broaden data access or turn demo fixtures into production fallback.

**Independent Test**: Existing hermetic tests plus focused boundary tests prove cache keys remain scoped, unauthorized menus remain closed, malformed API data remains rejected, and preview mode remains explicit. Review uses the documented component checklist.

**Acceptance Scenarios**:

1. **Given** a view extracted from the platform page, **When** organization or member scope changes, **Then** requests and cached state remain isolated exactly as before.
2. **Given** malformed data or denied access, **When** the extracted component renders, **Then** it preserves the named failure state and never substitutes demo data.
3. **Given** a reusable component, **When** its API is reviewed, **Then** props are typed and domain-named, and the component does not secretly fetch unrelated resources or select a tenant.

## Edge Cases

- Existing external entry points must remain valid; a relocation that cannot preserve one must update all consumers in the same complete migration.
- Existing product/API identifiers such as `PlatformPage`, `PlatformMenuId`, `PlatformOperation`, URL paths, storage keys, and wire fields are closed contracts; code-only naming cleanup must not rename their serialized meanings.
- A failed or partial move must fail typecheck or the architecture test, never fall back to a duplicate old/new implementation.
- Vendor UI is a read-only copied dependency and is excluded from dashboard-owned naming migrations, but existing native-picker and dialog-motion constraints still apply.
- No new enum is needed for this structural epic; any future closed set must use one typed definition and explicit parsing rather than parallel string lists.
- A convenience refactor must not change session storage, API URLs, permission checks, org selection, or visual product chrome.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: Add one dashboard-owned frontend standards document covering file/folder names, TypeScript identifiers, imports, module responsibility, component APIs, state/data ownership, styling, accessibility, testing, and review gates. It MUST refuse contradictory second conventions rather than list equally preferred alternatives.
- **FR-002**: Use `PascalCase.tsx` for React components/pages, `useCamelCase.ts` for hooks, and `camelCase.ts` for other TypeScript modules. Tests live in sibling `__tests__` directories and follow their owning module (`ComponentName.test.tsx` for components, `camelCase.test.ts` for other modules). Folders use lower case with hyphenated multi-word names. Acronyms in code identifiers use ordinary Pascal/camel casing; serialized protocol values MUST NOT change.
- **FR-003**: Define one application source alias and bounded relative-import rules. New code MUST refuse undocumented compatibility aliases and deep imports across feature ownership boundaries. Generated files and vendor copies remain explicitly named exceptions.
- **FR-004**: Separate application composition/gates, feature pages, reusable feature components, shared primitives, data adapters, hooks, utilities, and generated assets. Components MUST NOT import pages; data adapters MUST NOT import React or feature presentation; shared modules MUST NOT import features.
- **FR-005**: Convert `PlatformPage.tsx` from a multi-view implementation file into a composition root. Move overview, catalog, logs, usage, shared presentation, and helpers into explicitly owned modules while preserving menu IDs, storage keys, API calls, and rendering behavior.
- **FR-006**: Give components typed, domain-named props and a single primary responsibility. A page may compose and coordinate; reusable components MUST NOT choose tenants, inject sessions, or silently fetch unrelated resources.
- **FR-007**: Preserve fail-closed access, tenant-scoped caches, API decoding, explicit preview behavior, internationalization, drawn pickers, and in-place dialogs. The refactor MUST refuse any demo fallback, widened permission, or unscoped request introduced by extraction.
- **FR-008**: Keep tests beside the owning source area and add hermetic architecture/naming checks for the new contract. Tests MUST NOT scan `node_modules`, `dist`, or generated build output as authored source.
- **FR-009**: Update dashboard documentation and every import/reference affected by a relocation in the same change. A stale path, duplicate compatibility wrapper, or renamed user-facing/API identifier is a failed migration.
- **FR-010**: Preserve unrelated work in the parent repository and dashboard submodule. Do not commit unless separately requested; the complete dashboard migration must remain one coherent change set.

### Key Entities *(include if the feature involves data)*

- **Composition root**: Dashboard application or platform shell that wires gates, navigation, providers, and feature views; owns no generic catalog implementation.
- **Feature view**: A route/menu-level component owning its local UI state and composition under `pages/` or a bounded feature folder.
- **Reusable component**: Typed presentation or interaction consumed by more than one view; never an authority for tenant or session selection.
- **Data adapter**: Browser/member API transport and strict decoding under `data/`; owns no React rendering.
- **Authored source**: Dashboard-owned `src/`, excluding generated files; the only tree subject to naming and dependency migration.

## House Checklist *(mandatory — from Agents.md)*

- [ ] New behavior has a small hermetic test in the owning package (architecture and preserved-behavior Vitest coverage; addressed by plan/tasks).
- [ ] Closed sets: every new enum has `parse()` / `as_str()` (no new enum planned; existing TypeScript closed sets stay unchanged).
- [ ] No production `unwrap()` / `expect()` without an infallibility comment (TypeScript scope; existing non-null assertions are reviewed during extraction).
- [ ] Internal `a3s-*` dependencies stay on a path in this repository (no dependency changes planned).
- [ ] Product configuration, if any, is ACL parsed by `a3s-acl` (no product configuration added; frontend standards are contributor documentation).
- [ ] Evidence target: `docs/evidence/dashboard/2026-10-27-frontend-standards/OUTCOME.md` when the landing is recorded.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: The dashboard standards document exists and all mandatory naming/dependency rules have executable checks or an explicitly documented review check.
- **SC-002**: `PlatformPage.tsx` is at most 400 lines and acts only as platform composition/navigation; overview, catalog, logs, usage, and their private helpers live in named modules.
- **SC-003**: No authored production module exceeds 1,000 lines after the bounded migration; larger legacy files not in scope are listed explicitly rather than silently ignored.
- **SC-004**: All dashboard Vitest files/tests pass in a standalone run, including new architecture/naming tests; no test timeout is hidden by increasing its limit.
- **SC-005**: `npm run typecheck` and `npm run build` pass, including the CSS contract, with no dangling imports or changed API/user-facing identifiers.
- **SC-006**: Existing navigation/access, tenant isolation, subscription, Operations, i18n, and native-picker tests pass without weakened assertions or new allowlist entries.
- **SC-007**: Final evidence records exact commands/counts, before/after file-size measurements, preserved behavior, and explicit unclaimed visual/production E2E validation.

## Assumptions

- This epic covers dashboard-owned frontend source and contributor documentation, not the read-only `vendor/ui` implementation, login source copied from ToWork, API/backend behavior, or the new Application MVP features themselves.
- Visual appearance, interaction semantics, URLs, storage keys, API payloads, and user-visible product names are compatibility invariants.
- The current dashboard submodule is clean; its updated parent gitlink remains unrelated in-flight work and is not committed by this task.
- A single canonical application alias will be selected during planning after inventorying imports; migration will be complete rather than retaining parallel aliases.
- File-size thresholds are reviewable complexity budgets, not a reason to split cohesive trivial declarations into artificial files.

## Explicitly Not Claimed

- No PR2–PR8 Application product gate, authenticated production workflow, visual regression approval, backend/API change, or deployment is completed by this epic.
- No broad redesign, dependency upgrade, state-management replacement, CSS redesign, or vendor UI rewrite is included.
- No claim is made that every pre-existing dashboard defect is fixed; only the bounded standards and structural migration are accepted.
