# Feature Specification: Office code, behavior and design alignment

**Feature Branch**: none requested; current worktree
**Created**: 2026-10-30
**Status**: Implementing
**Input**: 完成全部aster-office的代码优化、功能修复、样式对齐
**Gate context**: All existing Office editors in `packages/office`, their Rust
implementation in `crates/office`, and their Studio integrations. This epic
exists to fulfill the user's explicit quality goal and the root `DESIGN.md`.

## User Scenarios & Testing

### User Story 1 — Consistent controls (Priority: P1)

Use document, Markdown, spreadsheet, presentation and PDF controls with the
same colors, typography, focus, picker, menu and dialog behavior as Aster.

**Independent Test**: Real Office primitive DOM tests, native-control scan,
computed-style browser assertions, keyboard and reduced-motion cases.

**Acceptance Scenarios**:
1. Given any editor, opening its controls uses the DESIGN.md roles and supports
   keyboard interaction, dismissal and focus restoration.
2. Given a dialog or clipped workspace, nested menus remain visible, retain
   the owning surface's theme and never escape their focus/ownership boundary.
3. Given a picker with more than eight choices, search filters its real choices;
   disabled or absent choices cannot be committed.

### User Story 2 — Correct editor workflows (Priority: P1)

Open, edit, undo/redo, save, reopen and export each existing supported editor
format without losing unrelated content, formatting or metadata.

**Independent Test**: Existing and new hermetic import/export, command, history,
save/error and Rust core tests; real browser journeys using actual editors.

**Acceptance Scenarios**:
1. Given a supported file, an edit survives save and reopen with expected
   content and unchanged unrelated structure.
2. Given an invalid/unsupported/truncated input or failed save, the editor
   names the failure, preserves the last good content and offers recovery.
3. Given read-only, embedded or editable presentations, the editor exposes
   the correct operations without shortcuts changing an outside input.

### User Story 3 — Maintainable shared implementation (Priority: P2)

Fix confirmed defects at their shared owner, remove redundant code/styles,
and use existing repository components and workflows.

**Independent Test**: Package typecheck/build, public export checks, host
integration tests, Rust focused/workspace checks and complete acceptance audit.

### Edge Cases

Empty documents, large bounded inputs, disabled/missing picker choices, IME,
multiple editors, clipped/portaled overlays, dirty/pending dialogs, narrow
viewports, reduced motion, preview/read-only mode, failed worker/save/export.

## Requirements

- **FR-001**: All five editor families MUST satisfy applicable DESIGN.md chrome,
  interaction, accessibility, localization and responsive requirements; refuse
  unavailable actions instead of presenting working-looking no-ops.
- **FR-002**: Shared controls MUST use the existing design roles and preserve
  document-content formatting; native select/datalist controls MUST not ship.
- **FR-003**: Pickers, forms, dialogs and floating surfaces MUST preserve keyboard
  and focus contracts; invalid/disabled input MUST not alter content.
- **FR-004**: Existing import/edit/history/save/reopen/export and collaboration
  capabilities MUST be reviewed across editor families; confirmed defects MUST
  be fixed with small tests, never hidden by mocks or weakened graders.
- **FR-005**: Rust core/kernel/CLI and React/Vue/web-component public entry points
  MUST remain compatible and build within the existing root workspaces.
- **FR-006**: Studio file tabs, message previews and knowledge editors MUST use
  the corrected library behavior and preserve dirty state and error recovery.
- **FR-007**: Code optimization MUST remove demonstrated duplication, dead paths
  or unnecessary work without speculative abstractions or new product features.
- **FR-008**: Design checks MUST include Office sources and actual rendered
  controls. Function tests alone MUST not be claimed as visual acceptance.
- **FR-009**: Completion MUST be supported by requirement-by-requirement evidence
  across this full scope. Unverified items remain pending.

## House Checklist

- [ ] New behavior has a small hermetic test in the owning package
- [ ] Closed sets preserve typed parsing and refuse unknown input
- [ ] No new production unwrap/expect without an infallibility argument
- [ ] Internal dependencies remain local; public APIs remain compatible
- [ ] Product configuration remains ACL; no new configuration is planned
- [ ] Evidence: `docs/evidence/office/2026-10-30-quality/OUTCOME.md`

## Success Criteria

- **SC-001**: Every task and FR has direct retained acceptance evidence.
- **SC-002**: Office package typecheck/build and owning tests pass; each new
  regression is shown against the real implementation.
- **SC-003**: All five editor families pass real browser workflow/style checks;
  Studio integration and relevant Rust tests pass with failures triaged alone.
- **SC-004**: No known confirmed in-scope defect or unverified requirement is
  marked complete. No Office design exception or test allowlist is added.

## Assumptions

The goal covers existing capabilities and confirmed defects, rather than adding
new editor products. Existing dirty work is preserved. No commit, push or
release was requested. This does not narrow the code, functional or design scope.

## Explicitly Not Claimed

Planning and intermediate checks are not full acceptance. Deployment and
publication are not part of the requested quality work.
