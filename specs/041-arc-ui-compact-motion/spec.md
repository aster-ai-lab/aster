# Feature Specification: ArcUI compact motion and radii

**Feature Branch**: `main` (current checkout; no new branch or worktree)
**Created**: 2026-11-03
**Status**: Implementing
**Input**: Continue full-product ArcUI style alignment with emphasis on micro-interactions, while preserving Aster compact geometry including all border radii.
**Gate context**: Shared UI foundations, Studio overlay consumers; governed by `DESIGN.md` and `docs/arc-ui-migration-plan.md`.

## User Scenarios & Testing

### User Story 1 - Compact floating surfaces (Priority: P1)
A person opens dialogs, side panels, toasts and anchored popovers and sees Aster compact radii (8px controls / 12px floating), not Arc's larger defaults.
**Independent Test**: Source and focused component tests assert radius tokens and class contracts.
**Acceptance Scenarios**:
1. Given a PageSidePanel or dialog surface, the resting radius is 12px.
2. Given a toast card, the resting radius uses `--radius-panel` (12px).

### User Story 2 - In-place dialog micro-motion (Priority: P1)
A dialog opens and closes where it rests: opacity and scale only, 160ms enter / 200ms leave, scrim fade without blur or travel.
**Independent Test**: Dialog CSS contract tests and existing dialog-motion checks.
**Acceptance Scenarios**:
1. Given open/close, content does not translate vertically.
2. Given the scrim, there is no backdrop blur and opacity is 20% black.

## Requirements

- **FR-001**: Align shared overlay owners to DESIGN.md compact radii and in-place dialog motion.
- **FR-002**: Prefer Arc micro-interaction recipes already fused in DESIGN.md; refuse Arc large radii and traveling modal entrances.
- **FR-003**: Continue A0 inventory in parallel; this batch does not claim full-product migration complete.
- **FR-004**: Preserve public props, selection authority and existing portal/focus owners.

## House Checklist
- [x] New or tightened hermetic tests in the UI package.
- [x] No new Rust enum, panic, configuration language or schema.
- [x] Internal dependencies remain local.
- [x] Work stays in this checkout; Studio source stays in its submodule.
- [x] Evidence: `docs/evidence/desktop/2026-11-03-arc-ui-compact-motion/OUTCOME.md`.

## Success Criteria
- **SC-001**: Shared PageSidePanel, toast card and Arc dialog CSS match compact radii / in-place motion.
- **SC-002**: Focused UI tests for dialog, toast and page-side-panel pass.
- **SC-003**: Remaining oversized product surfaces are inventoried with decisions, not silently left.

## Explicitly Not Claimed
Whole-product A0–A8 completion, Office engine replacement, Pro source, every Studio page motion pass, or native multi-OS qualification.
