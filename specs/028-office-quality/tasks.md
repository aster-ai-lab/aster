# Tasks: Office quality

**Input**: [spec.md](spec.md), [plan.md](plan.md)
**Status**: Active. Check only when direct evidence is retained.

Current partial results: T002 now reuses shared SearchSelect/Combobox, removes
all Office native dropdowns and has 36 package / 57 shared UI tests. An actual
browser reproduced additional pointer/Enter/Tab defects, repaired at the shared
source; nine normal controls and one real reduced-motion browser cases pass.
T003 base buttons, fields,
tabs and editor zoom/selection use shared roles, with Word blue, Excel green
and PowerPoint orange retained per the user's format-identity requirement. All 28 Dialog callers supply
localized close labels; scrim dismissal preserves existing draft/pending rules,
and the default geometry is aligned. All 27 conditional dialog owners retain
the 200ms exit, preserving background isolation and focus restoration. A shared
reduced-motion specificity defect and imperative prompt blur-reopen/catalog
defects were reproduced and repaired. Scrollbars, feature panels and full
rendered acceptance remain pending.
Six Markdown journeys and 27 focused dialog/insertion cases pass on the new
presence build, with every original non-wait line and deadline preserved.
The 234-case command matrix and six history/save journeys were replayed and
passed on frozen build files. All 4516 original non-wait lines and deadlines
are preserved; the last spreadsheet-only compatibility change leaves the
Markdown payload and styles byte-identical. The complete five-editor matrix
remains pending.
Real DOCX/XLSX/PPTX browser round
trips pass; XLSX imported-sheet locking and PPTX slide-name loss were repaired
with owning regressions. T009's shared UTF-16 and fixture
path repairs pass the 466-test core suite. Parser/kernel/CLI library tests pass
9/33/51, with another 23 kernel integration cases and 156 CLI --tests cases.
Package/UI typechecks and the full package build pass. These do not
complete the broader tasks; the remaining matrix is in OUTCOME.md.

T007 now has 15 real PDFium-worker browser journeys covering navigation,
search, read-only/error recovery, physical PDF save/reopen, all page operations
and new text annotation persistence. Automatic save error handling, search
viewport navigation and PDF chrome are repaired. Annotation text re-entry,
native text Backspace/history and caret continuity across Save are repaired
from original EmbedPDF source; the frozen re-entry case, four keyboard cases
and complete 15-case PDF suite pass. T007 remains open. Studio
still overrides the toolbar to 24px. Complete mobile dialog motion, host
integration and the other editor families remain required.

T010 now removes the library editor's duplicate back/Agent header, spacing and
structured-file status row. Tabs retain switching, dirty state and guarded
closing; the editor footer receives real host save state, and hidden Office
tabs ignore Save. Library menus are non-modal so the same outside press closes
the menu and activates its target. A global Office CSS leak that squared the
shared picker is repaired at its source. Current checks: 52 Studio renderer
tests, 38 Office tests / 199 assertions, and real shared-control browser
evidence. Full Studio typecheck has unrelated diagnostics; packaged native
acceptance and the complete library UX audit remain pending. See
`docs/evidence/office/2026-10-30-library-chrome/OUTCOME.md`.

The real Studio host fixture now mounts two actual editors and reads back
exported files. It exposed and repaired a second tenant accent override, stale
controlled content, missing native File/Save wiring, false initial spreadsheet
dirty/history state, DOCX revision-mode/whitespace loss, and the OS/host theme
mismatch. Four real host browser journeys, 58 Office tests / 277 assertions and
32 focused Studio tests pass. These cover real file contents, active-tab Save,
refused writes, XLSX Undo/Redo and PPTX names/notes. Native IPC/disk acceptance,
the full dark-theme matrix and broad editor/adapter requirements remain open.
See `docs/evidence/office/2026-10-30-studio-host/OUTCOME.md`.
Final screenshot review still shows the newly typed Word paragraph near the
paper edge instead of the preceding section inset. T004's section/paragraph
margin acceptance remains open despite the passing text round trips.

## Inventory and baseline

- [ ] T001 [US1/US2/US3] Inventory all editors, public adapters, styles, Rust
  crates, host consumers and available tests. Record baseline and a coverage
  matrix in `docs/evidence/office/2026-10-30-quality/OUTCOME.md` (FR-001–009).

## Shared behavior, design and tests

- [ ] T002 [US1] Fix shared Office picker/listbox/combobox behavior, remove native
  select/datalist, preserve draft validation and add real DOM/keyboard tests
  under `packages/office/tests` (FR-002/003/008).
- [ ] T003 [US1] Align design roles, fonts, buttons, fields, menus, dialog motion,
  scrollbars and floating theme ownership at shared sources; test actual styles,
  reduced motion, focus and clipping (FR-001/002/003/008).

## Complete editor families

- [ ] T004 [US2/US3] Review and fix document import/edit/history/layout/save/export,
  dialogs/panels, UI localization and responsive behavior; use real codecs and
  browser journeys, preserve file fidelity (FR-001/004/005/007).
- [ ] T005 [US2/US3] Review and fix spreadsheet import/types/formulas/selection,
  formatting/sort/filter/dialogs/history/save/export with actual grids and Rust
  tests; reopen outputs (FR-001/004/005/007).
- [ ] T006 [US2/US3] Review and fix presentation import/edit/selection/design,
  charts/animations/history/save/export and preview behavior with real browser
  and round-trip checks (FR-001/004/005/007).
- [ ] T007 [US2/US3] Review and fix PDF rendering/annotation/organization/history,
  save/export/error and read-only states with actual worker/browser tests
  (FR-001/004/005/007).
- [ ] T008 [US2/US3] Review and fix Markdown visual/source/split editing, commands,
  IME/history/serialization/save and preview using existing real ACL journeys
  plus small regressions (FR-001/004/005/007).

## Wiring and whole-scope validation

- [ ] T009 [US2/US3] Review and verify Rust core/kernel/CLI, collaboration and
  React/Vue/web-component adapters. Fix proven defects with owning tests and
  build/check public exports (FR-004/005/007).
- [ ] T010 [US1/US2] Align Studio tabs/message/knowledge integrations, reduce
  redundant host style overrides, extend design guards to Office, and test real
  mounted editors plus dirty/read-only/failure behavior (FR-006/008).
- [ ] T011 [US1/US2/US3] Run full coverage matrix, browser checks, package build,
  root checks and an adversarial completion audit. Reopen artifacts. Record
  commands, counts and Explicitly not claimed in OUTCOME.md (FR-009).

## Final Verification Gate

- [ ] All FRs and five editor families have direct passing evidence
- [ ] Owning package tests, typecheck and builds pass
- [ ] Relevant Rust tests and cargo fmt --all -- --check pass
- [ ] Studio integration and rendered keyboard/style checks pass
- [ ] Public APIs, output fidelity and local dependencies remain intact
- [ ] No native picker, new design exception or weakened grader
- [ ] No unverified requirement or known confirmed in-scope defect remains
- [ ] No commit/push unless separately requested
