# Feature Specification: Consistent Arc foundations

**Feature Branch**: `main` (current checkout; no new branch or worktree)
**Created**: 2026-11-03
**Status**: Implementing
**Input**: Deeply optimize foundational controls and overlays in Arc UI style. The user chose Aster's compact dimensions with Arc material and interaction.
**Gate context**: Shared UI foundations and Studio consumers; governed by `DESIGN.md`.

## User Scenarios & Testing

### User Story 1 - Consistent controls (Priority: P1)
A person moves between settings, a form and the workbench and gets the same control sizes, typography, focus, disabled feedback and pending affordances.
**Independent Test**: UI package tests plus real-browser rendering of the actual controls, including native buttons, links and custom loading glyphs.
**Acceptance Scenarios**:
1. Given equivalent variants, changing a button's rendering path preserves its appearance and disabled contract.
2. Given a compact field or picker, focus and validation do not change its dimensions.

### User Story 2 - Consistent choices and overlays (Priority: P1)
A person opens a menu, picker or command list and follows a single hover plate while labels and hit targets stay stationary. Keyboard navigation is immediate.
**Independent Test**: Browser geometry, pointer, keyboard, disabled-choice and reduced-motion assertions against production components.
**Acceptance Scenarios**:
1. Hover moves feedback without invoking selection. Clicking invokes the existing callback once.
2. Keyboard movement and reduced motion expose the same choices immediately.
3. Nested overlays, scrolling, dismissal and reopening preserve focus and the existing placement owner.

### Edge Cases
Disabled and aria-disabled controls; custom pending icons; polymorphic links; grouped and long labels; nested menus; retained/hidden surfaces; portals; filtering and scroll; reduced motion; explicit caller sizes.

## Requirements

### Functional Requirements
- **FR-001**: Audit all current primitive definitions and their static import consumers. Do not equate a source count with runtime acceptance.
- **FR-002**: Use existing Arc source, semantic roles and installed Motion/Radix. Refuse new runtimes, parallel base controls and native select elements.
- **FR-003**: Preserve public props, refs, callbacks, selection authority, loading semantics and portal/focus behavior. Never infer business identity from labels.
- **FR-004**: Share hover feedback at the primitive owner. Remove redundant composer implementations rather than maintaining two motion owners.
- **FR-005**: Keep compact Aster geometry and in-place modal motion. Refuse layout shifts, positioning-transform animation, overshooting values and delayed business state.
- **FR-006**: Use semantic colors and the same focus/disabled/error recipes across paths; retain readable light/dark controls.

## House Checklist
- [x] New behavior has hermetic tests in the UI package.
- [x] No new Rust enum, panic, configuration language or schema.
- [x] Internal dependencies remain local; no dependency installation is planned.
- [x] Work stays in this checkout; Studio source stays in its submodule.
- [x] Evidence: `docs/evidence/desktop/2026-11-03-arc-foundation/OUTCOME.md`.

## Success Criteria
- **SC-001**: Every primitive definition has an audit decision and coverage/limit record.
- **SC-002**: Existing 335 focused baseline tests and new behavioral tests pass.
- **SC-003**: Chromium/WebKit prove the shared hover geometry, compact sizes, keyboard/disabled behavior, nested dismissal and reduced motion.
- **SC-004**: Reopened visual artifacts match the real component states; untested native targets remain explicit.

## Assumptions
The user's compact-size choice preserves Aster palette and geometry. Existing APIs and engines remain owners. Shared consumers need compatibility, not unrelated page redesign.

## Explicitly Not Claimed
A whole-product Arc migration, Office/editor-engine replacement, Pro source adoption, automatic accessibility certification or native-platform coverage without actual runs.
