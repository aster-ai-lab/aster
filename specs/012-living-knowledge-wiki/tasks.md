# Tasks: Living knowledge Wiki

Input: [spec.md](spec.md), [plan.md](plan.md).

## Behavior and tests

- [x] T001 US1/US2 Add bounded durable staging, exact incremental source plans,
  strict typed proposals, provenance and review/publication in
  `crates/okf/src/personal/maintenance.rs`; sibling hermetic tests prove scope,
  source immutability, unsafe-path refusal, automatic-create/manual-update,
  stale-source/page refusal and restart recovery.
- [x] T002 US1/US2 Wire owner commands through Studio's existing OKF Tauri
  boundary and main capability; permission/serialization tests reject missing
  or unauthorized command surfaces.
- [x] T003 US1/US3 Evolve the compile run driver and use the existing source
  invalidation stream in a main-window runtime. Tests prove exact selected
  inputs, single-run scheduling, retry boundaries and continued review state.
- [x] T004 US2/US3 Replace the compile dialog with the maintenance workspace,
  source selection, before/after review, cited navigation and actual activity.
  Renderer tests cover empty/error/busy/review and search/page selection reset.
- [x] T005 US3 Update affected translations, callers and contracts; run focused
  knowledge tests, production renderer build and applicable design ratchets.

## Evidence and Final Verification

- [x] T006 Record commands, counts, live fixture UI observations and limitations
  in `docs/evidence/desktop/2026-10-23-living-wiki/OUTCOME.md`.
- [x] Owning focused native/renderer tests pass.
- [x] Rust fmt and relevant workspace checks run; unrelated failures retained.
- [x] Internal A3S dependencies stay local; no native select or new config format.
- [x] Source/review bytes and stale revisions are verified by owner-side tests.
- [x] No provider calls on real user knowledge, no commit without request.
