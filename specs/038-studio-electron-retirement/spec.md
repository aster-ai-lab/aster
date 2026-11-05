# Feature Specification: Retire Studio Electron dependencies

**Feature Branch**: none; current checkout
**Created**: 2026-11-01
**Status**: Implementation and acceptance in progress
**Input**: User description: "全面审查并清理 Electron 依赖，并确保功能不收到影响"
**Gate context**: Product regime, `apps/studio`. Completes the remaining host migration exposed by the HTML preview failure. The existing interactive-coding C00–C08 campaign remains required and its frozen inputs do not change.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Install, build and test without Electron (Priority: P1)

A developer uses the supported Studio workflows without Electron packages, ABI rebuilds, packaging hooks or application-launch fixtures.

**Why this priority**: Dormant hooks currently import removed packages and tests refer to deleted workflows.
**Independent Test**: Studio Vitest checks the real dependency manifests and supported script entry points; renderer and native bundles build through the existing root workspace.

**Acceptance Scenarios**:

1. Given a supported tenant, when dependencies install and Studio builds, then no Electron package, builder or rebuild hook is required.
2. Given retired Electron-only tooling, when it is removed, then no supported caller or test still imports it; native bundle membership checks remain intact.

### User Story 2 - Existing desktop interactions keep working (Priority: P1)

Users keep browsing, previewing HTML, using mini apps, quoting selections, running Python, backing up/restoring data and managing tabs through their existing product entry points.

**Why this priority**: Removing imports alone can leave blank guests, missing events or silent no-ops.
**Independent Test**: Focused renderer and owning native tests exercise actual entry points, event delivery, state preservation and named errors. Isolated native CUA checks verify the changed visible paths.

**Acceptance Scenarios**:

1. Given a supported native host, when a retained action runs, then it uses the existing Tauri owner and preserves its observable result, data and permission boundaries.
2. Given a missing or unsupported host capability, when an action is requested, then it reports the existing named refusal instead of rendering a blank Electron guest or reporting false success.
3. Given obsolete preboot windows, when they are removed, then current SQLite migrations, tenant stores and user data remain unchanged; retirement requires proving they have no current entry point.

### Edge Cases

- Guest navigation, storage/session isolation, external links, annotations and pane visibility must retain their current contracts.
- Backup/restore must not operate on the user's live profile during tests. Use synthetic isolated data and preserve recovery behavior.
- Cancellation and host loss must finish the exact owned CUA session without foreground or coordinate fallback.
- Generated files change only through their existing generators. Licenses and historical evidence retain their original text.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: Studio MUST remove Electron package dependencies and obsolete executable build/release hooks; supported workflows MUST refuse incomplete native bundles.
- **FR-002**: Studio MUST migrate retained renderer host calls to existing Tauri/IPC owners and remove the optional Electron global declaration; missing capabilities MUST fail explicitly.
- **FR-003**: Studio MUST replace retained Electron guest paths through the existing native browser/applet or HTML preview owner, preserving navigation, storage, security, visibility and interaction behavior; no second guest manager or Electron compatibility global is added.
- **FR-004**: Studio MUST prove old windows and scripts unreachable before retiring them; current data formats, SQLite migration history, tenant identities and stores MUST NOT be deleted or imported.
- **FR-005**: Tests MUST stop depending on Electron launch/CDP/main-process facilities, retain equivalent user-outcome coverage on the actual supported host and reject reintroduced Electron imports, globals and product tags.
- **FR-006**: The audit MUST classify every executable dependency/reference with its callers, replacement/removal decision and validation. License/history mentions are retained and do not count as runtime dependencies.
- **FR-007**: Changes MUST retain background CUA constraints, frozen campaign inputs and actual failure evidence. Core/ACP source changes, additional worktrees and commits are outside authorization.

### Key Entities

- Host capability and IPC route: owned by Studio's existing host bridge and native command registration.
- Guest: owned by the current browser/applet runtime, not by an Electron DOM element.
- Retired dependency: a package, hook, fixture, global call or tag whose executable path has been removed or migrated.

## House Checklist *(mandatory — from Agents.md)*

- [ ] New behavior has a small hermetic test in the owning package — T002–T005.
- [ ] Closed sets: new admitted routes/actions use the existing typed schema — T003–T004.
- [ ] No production panic without an infallibility argument — native changes reviewed in T006.
- [ ] Internal dependencies remain local — T006.
- [ ] Product configuration remains ACL; no new configuration layer — T001–T006.
- [ ] Evidence target: `docs/evidence/studio/2026-11-01-electron-retirement/OUTCOME.md` — T006.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: No supported install/build/test/runtime path requires Electron or uses its renderer globals/tags; all audit entries have a reviewed disposition.
- **SC-002**: Affected focused suites, renderer build, native package checks and changed-entry native CUA pass, with exact counts recorded rather than assumed.
- **SC-003**: The original long-horizon campaign continues in its frozen order; cleanup alone does not establish its completion.

## Assumptions

- Scope is the Studio product and its parent build wiring. Core/ACP are read-only.
- Current uncommitted changes belong to concurrent work and must be preserved.

## Explicitly Not Claimed

- A global text search with zero occurrences, deletion of historical evidence/licenses, or product rebranding.
- Parity inferred from mocks, a successful import scan, compilation, or another platform's runtime.
- Passing C00–C08, P0 repetitions, Windows/Linux runtime or release qualification before actual evidence exists.
