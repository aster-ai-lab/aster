# Feature Specification: Living knowledge Wiki

**Feature Branch**: none requested; working tree remains on main
**Created**: 2026-10-23
**Status**: Implementing
**Input**: Rebuild the knowledge compilation UI, UX and business workflow around
LLM Wiki. User selected automatic incremental maintenance with review of important changes.
**Gate context**: Product regime, personal Aster knowledge, `apps/studio` and
`crates/okf`. The current compile dialog obscures selection scope and exposes
two implementation paths instead of one knowledge maintenance workflow.

## User Scenarios & Testing

### US1 — Keep knowledge current (P1)

After a source is saved or imported into the personal vault, changed sources
are queued once. An LLM maintains persistent, interlinked knowledge pages.
Existing sources and knowledge remain readable during the run.

**Independent Test**: Native hermetic fixture proves unchanged sources do not
queue; source changes do; source bytes never change. Renderer fixture proves
filesystem invalidation schedules incremental work without a second user action.

**Acceptance**: Given one changed source and many current sources, automatic
maintenance processes that source, may consult existing knowledge, and records
the pages and citations it actually changed. A failed run is visible and does
not loop automatically or erase staged work.

### US2 — Review consequential changes (P1)

New ordinary pages can publish automatically. Any replacement of an existing
page, any model-declared conflict, and important or uncertain new claims need review. A reviewer sees the reason,
before/after text and exact sources, and accepts or keeps the current version.

**Independent Test**: Native fixtures prove updates never publish before
acceptance, current source/page revisions are checked again on accept, and a
candidate changed after review is refused. Renderer tests prove review actions
operate on the displayed page and preserve an editable note draft.

### US3 — Understand and recover (P2)

The knowledge workspace exposes sources, pending changes and recent activity.
Users can update explicitly selected sources, inspect provenance, retry a
failed/interrupted run, and continue reading while maintenance runs.

**Independent Test**: Renderer tests cover selection scope, search/page reset,
busy/error/empty states, close/reopen and a native candidate-review fixture.

## Requirements

- FR-001: Use one maintenance workflow; refuse unsupported/cloud roots and
  unknown operation, page-kind or change-kind values.
- FR-002: Queue by exact source content generation. Never infer entity names,
  importance or contradiction from a regex, keyword list or capitalization.
- FR-003: Preserve original sources. Stage model output and reject unlisted
  page writes, changed source bytes, unsafe paths, symlinks, missing citations,
  invalid manifests and files exceeding bounded limits.
- FR-004: Every generated page has model-declared source paths and a page kind
  (source, entity, concept or synthesis); entity/page names come from the model.
- FR-005: Automatically publish only newly created pages explicitly declared non-conflict and not requiring review. Refuse
  to replace a published page without an explicit review action.
- FR-006: Bind review to candidate bytes, source generations and published page
  generations; refuse stale candidates instead of blessing stale content.
- FR-007: Keep a durable run record, pending review and chronological log. A
  restart must expose interrupted work, never silently discard or repeat it.
- FR-008: Use existing agent sessions, filesystem invalidation, source readers,
  UI controls and root Cargo workspace. Extend existing Tauri capabilities only
  for the authorized main knowledge surface.
- FR-009: Collection controls have truthful scope, 20-row pages, stable path
  sorting and selection cleared by query/page changes. No selection means the
  incremental pending-source scope, never an implicit whole-vault rewrite.
- FR-010: Run relevant knowledge health checks during maintenance; model-declared
  conflicts remain unresolved until reviewed. Existing save-to-knowledge entry
  points feed the same incremental source pipeline.

## Key Entities

- Source: original vault file and exact generation, owned by the filesystem/OKF.
- Knowledge page: published Markdown with declared source lineage.
- Maintenance job: bounded source snapshot, isolated staging and durable status.
- Proposed change: model-declared page/meaning plus owner-verified before/after
  generations and user review decision.
- Activity: actual publication/review/failure facts, never simulated progress.

## House Checklist

- [x] Product-regime hermetic tests are assigned in tasks.
- [x] New Rust closed sets have parse/as_str and unknown-value refusal.
- [x] No new production unwrap/expect without an infallibility argument.
- [x] Aster dependencies remain local; installed workspace serialization is reused.
- [x] No new product configuration format or configuration switches.
- [x] Evidence: `docs/evidence/desktop/2026-10-23-living-wiki/OUTCOME.md`.

## Success Criteria

- SC-001: An unchanged source produces zero model runs; a changed source queues
  once and source bytes remain identical after publication/review.
- SC-002: Existing page replacement and conflict require review and stale source
  or page mutations refuse publication in the owning Rust tests.
- SC-003: The UI has one maintenance entry and shows exact selection scope,
  review, actual activity, provenance and actionable failure states.
- SC-004: Focused native and renderer tests, permission tests and a renderer
  production build pass; live UI evidence uses a disposable fixture.

## Assumptions

Personal vault Markdown is the source boundary; existing document conversion
and save-to-knowledge flows continue to supply it. Generated Markdown remains
OKF-compatible. User-authored drafts and published knowledge stay usable.

## Explicitly Not Claimed

New cloud/team compilation services, a new embedding store, a replacement
document converter, native app publication, or a new autonomous web crawler.
No real user's knowledge is sent to a provider as a test fixture.
