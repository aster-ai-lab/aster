# Feature Specification: Knowledge workbench UX

**Feature Branch**: none requested; do not change branches.
**Created**: 2026-10-23
**Status**: Implementation in progress; milestone acceptance remains pending.
**Input**: 对照 Opensidian，规划 Aster 知识页面的可执行优化，重点补强 UI/UX。
**Gate context**: Product regime, `apps/studio`, existing Core workspace
services and Office Markdown editor. This epic completes daily knowledge
reading/writing/navigation. It complements [012 living Wiki](../012-living-knowledge-wiki/spec.md),
which already owns automatic maintenance and reviewed publication; it does not
replace that workflow or claim unfinished cloud controls are available.

## User Scenarios & Testing

### US1 — Edit and follow links without losing work (P0)

Users can identify the open note, see its save state, follow the correct link,
and recover a draft after navigation, a failed write or a desktop restart.
Renaming explains its reference impact before committing.

**Independent Test**: Native temporary-vault tests for versioned writes,
path confinement and recoverable rename; renderer tests for draft/save/navigation
transitions. No provider or live database is needed.

**Acceptance Scenarios**:

1. Given two dirty notes, switching between them preserves each draft and caret;
   a late save response for A never marks B saved or clears A's newer edits.
2. Given an external edit after opening a note, saving refuses the stale base
   and offers comparison, retaining both the disk version and local draft.
3. Given `产品/设计.md` and `技术/设计.md`, a qualified link opens the exact file;
   an ambiguous short link asks for a choice and never opens the first match.
4. Given a rename preview, a changed dependency refuses the old intent. A crash
   during a multi-file change exposes recoverable operation state, not false success.
5. Given a dirty note at normal user-initiated application exit, failed draft persistence
   prevents a clean-exit claim and offers retry, save or explicit discard.

### US2 — Find notes across the authorized vault (P0)

Users search titles and bodies, see paths and highlighted context, and distinguish
no matches from incomplete indexing, unavailable files and request failures.

**Independent Test**: Native paged-query fixtures containing more than 200 notes,
Chinese text, duplicate names, skipped files and changed revisions; renderer
tests for result scope, stale responses, navigation and recovery.

**Acceptance Scenarios**:

1. A unique body term in note 250 is found when its authorized corpus is indexed.
   It is not necessary to open that note first.
2. Results show title, relative path, matched context and source type. Opening a
   result preserves the query, filters and position when returning.
3. A not-ready or partial index cannot display an unqualified “未找到结果”.
   Exact totals are shown only when supplied by a complete owner snapshot.
4. Search A completing after search B cannot replace B's results. A cursor from
   another vault/query/revision is refused and offers an explicit restart.

### US3 — Read several related notes efficiently (P1)

A reusable preview tab, pinned note tabs, quick open and Back/Forward support
comparison without discarding drafts. Related information can be collapsed.

**Independent Test**: Renderer navigation-state tests with real UI components;
one desktop cross-process test verifies switching, saving and restart recovery.

**Acceptance Scenarios**:

1. Single-clicking previews a clean note; double-clicking or editing pins it.
   Opening another preview never replaces an unsaved note.
2. Back/Forward restores the addressed note and its editor state. A deleted or
   moved target is reconciled by exact identity, without guessing by title.
3. At narrow widths, navigation and related information remain reachable through
   named buttons; the editor and save/recovery actions remain usable.

### US4 — Understand and review AI changes (P1)

Users continue reading during maintenance, enter the existing change queue,
compare a proposal with current content and frozen evidence, and accept or keep
the current version without confusing a source edit with a published Wiki edit.

**Independent Test**: Existing OKF maintenance fixtures plus renderer tests for
review selection/focus, stale candidate errors and reading-draft preservation.

**Acceptance Scenarios**:

1. The UI distinguishes “保存中”, “搜索索引更新中” and “知识整理中”. Completing one
   does not imply the other two have completed.
2. The existing policy remains: ordinary new pages may publish automatically;
   replacements, conflicts and consequential new claims require review.
3. Accept acts on the exact displayed candidate generation. A changed source or
   candidate prevents publication and preserves the review context.
4. Closing maintenance and returning to reading restores the prior note, draft,
   scroll position and focused control. Review history is read-only.

## Edge Cases

- Same basename in different directories; ambiguous legacy links; `#heading`,
  aliases, percent characters, Unicode and emoji; links inside code fences.
- Workspace/tenant changes, revoked access, stale requests and cross-window edits.
- Read-only disk, failed draft write, external deletion, pending save on close,
  source changes during rename and interrupted multi-file publication.
- Skipped or oversized files, index restart, unsupported file type, deleted final
  result, incomplete discovery and disconnected invalidation stream.
- Published pages or pending candidates referencing a renamed source. The fast
  rename path must refuse these until the owning maintenance review can approve
  the affected changes; it must not rewrite published content outside review.
- Input composition in Chinese, keyboard-only operation, long translations,
  reduced motion, 400% zoom and a 320 CSS-pixel knowledge content viewport.

## Requirements

- **FR-001**: MUST use the existing tenant/workspace authority and exact vault-relative
  paths as identity; MUST refuse traversal, symlinks escaping scope and title-based
  identity guesses. A backend-returned canonical identity is authoritative.
- **FR-002**: MUST expose one reading workbench and a maintenance destination,
  with clear source-note versus published-page states; MUST refuse unavailable
  mutations rather than displaying controls whose owner cannot complete them.
- **FR-003**: MUST use versioned reads/writes and preserve newer drafts on late
  responses; MUST refuse stale writes with a recoverable conflict state.
- **FR-004**: MUST retain dirty drafts per tenant/vault/path and base generation;
  MUST refuse silent eviction, cross-tenant restoration and false recovery claims.
  Persisted recovery bytes stay outside the searchable vault and model context.
- **FR-005**: MUST support exact qualified links, explicit ambiguous-link choice
  and unresolved-link creation by confirmation; MUST refuse first-match fallback.
- **FR-006**: MUST preview rename/move impact and bind confirmation to exact
  source/dependency generations; MUST refuse stale or unsupported published-page
  rewrites. Multi-file recovery cannot be described as global filesystem atomicity.
- **FR-007**: MUST search at the authorized owner and return bounded results with
  declared coverage and continuation; MUST refuse client-only “all-vault” claims,
  invented totals and Agent Top-K results presented as exhaustive search.
- **FR-008**: MUST reuse document invalidation and existing catalog/index machinery;
  MUST refuse an incomplete index as a definitive absence of matching notes.
- **FR-009**: MUST support preview/pinned tabs, quick open and navigation history;
  MUST refuse replacing/closing dirty content without safe persistence or an
  explicit save/discard decision. Product limits must be visible, not silent eviction.
- **FR-010**: MUST preserve 012's automatic-maintenance policy and owner checks;
  MUST coalesce repeated save invalidations without submitting the same source
  generation twice or issuing a model request for search/navigation.
- **FR-011**: MUST follow root `DESIGN.md`, existing controls, keyboard/focus and
  localization contracts; MUST refuse native selects, invented progress and
  color-only essential state. See [UX contract](ux.md).
- **FR-012**: MUST show graph/backlink scope and coverage and navigate exact
  note identities; MUST refuse inferred semantic edges or unresolved names being
  silently created as notes.
- **FR-013**: MUST follow the 2026-10-24 third-edition [UX contract](ux.md): one
  shared Knowledge header, peer Notes/Published Pages/Memory destinations and the
  same list/detail workbench geometry. Review is an internal Published Pages
  workflow with explicit return. Retain one mounted note/Office owner, independent
  resource query/selection state and conditional diagnostics; do not duplicate
  resource navigation in the sidebar or add a second scope picker.
- **FR-014**: MUST compose [018's local-memory owner](../018-local-memory-studio/spec.md)
  without inventing capabilities. Content/type/time are primary; exact scope,
  storage identity and advanced fields remain inspectable. Scope, conflict and
  deletion failures remain visible even when diagnostic metadata is collapsed.
- **FR-015**: The target Encyclopedia navigation MUST derive topics, child topics,
  uncategorized entries and breadcrumbs from an explicit owner classification
  contract, never guessed titles/content. The current owner contract stores an explicit
  compiler-proposed `categories` map for stable existing page identities; new
  pages may use real category directories under `concepts/`. The tree and
  breadcrumbs consume that contract. Native acceptance remains pending; [the E2E plan](e2e-plan.md)'s T gate must prove the hierarchy and safe
  classification changes before that capability is claimed complete.
- **FR-016**: Full acceptance MUST cover the [function matrix](e2e-plan.md) and
  [scenario catalogue](e2e-cases.md), including mandatory parameter variants and
  twelve independent business journeys. GUI, real process/owner, controlled-fault
  and real-model results remain distinct. Missing driver capability, unimplemented
  targets, skips or seeded final outputs MUST NOT become full E2E success.

## Key Entities

- **NoteRef**: tenant + canonical vault identity + relative path; owned by the host.
- **NoteRevision**: exact bytes/base generation used for write, search and review.
- **NoteDraft**: local text, base revision, save state and editor restoration state.
- **QuerySnapshot**: scope/query/sort + index/source revisions + bounded continuation.
- **RenameIntent**: reviewed old/new paths, impacted references and expected revisions.
- **WorkbenchView**: scoped tabs, history, selection and pane presentation metadata.
- **Maintenance job/change**: existing OKF-owned entities; no second publication owner.

## House Checklist

- [ ] Owning hermetic tests accompany each behavior — tasks T002–T017.
- [ ] Rust wire enums have parse/as_str and unknown-value refusal — T002/T006/T008/T016.
- [ ] No unargued production unwrap/expect — T020 review.
- [ ] A3S dependencies remain local; root workspace/lockfile — T020.
- [ ] No new configurable policy surface; any later configuration must use ACL — T020.
- [ ] Landing evidence at `docs/evidence/desktop/<date>-knowledge-workbench-ux/OUTCOME.md` — T018–T020.

These boxes are implementation obligations, not completed checks.

## Success Criteria

- **SC-001**: The US1–US4 acceptance scenarios pass in the owning hermetic suites.
- **SC-002**: Synthetic fixtures beyond the former 200-note UI limit are searchable;
  skipped coverage and stale continuation are visible and cannot yield false “no results”.
- **SC-003**: Duplicate-name, stale-write, rename interruption and draft recovery
  tests prove no wrong-note navigation or silent data loss in the tested cases.
- **SC-004**: New/search/follow-link/save/review workflows work by keyboard; layouts
  at 1440, 1024, 768 and 320 CSS-pixel content widths retain essential actions.
- **SC-005**: Baseline targets: warm search P95 ≤300 ms and opening a ≤100 KiB note
  P95 ≤250 ms on the recorded development host, using 5,000 synthetic notes and
  about 50 MiB text. These are targets to measure, not current performance claims.
- **SC-006**: At least six proved desktop journeys are retained through A3S Test:
  find, resolve ambiguity/rename, preserve drafts, recover a conflict/restart,
  review a proposal, and operate the narrow/keyboard layout.
- **SC-007**: Reading/search/navigation makes no model request; repeated autosaves
  coalesce into the existing maintenance workflow for actual source generations.
- **SC-008**: The complete E/T gates and final verification gate in
  [e2e-plan.md](e2e-plan.md) pass with per-variant evidence and owned-process cleanup.
  The six journeys in SC-006 are the original minimum; the current complete
  catalogue requires all twelve, not a selected passing subset.

## Assumptions

- Personal local Markdown is the first delivery scope; 012 owns generated content.
- Existing automatic maintenance remains enabled according to the prior user decision.
- Files remain portable Markdown; local recovery data is private host-owned data,
  not product configuration and not content supplied to the Agent.
- The plan defines the implementation contract; completed behavior and acceptance
  evidence are recorded separately in the tasks and evidence outcome.

## Explicitly Not Claimed

Realtime collaboration, cloud synchronization, an Obsidian plugin/theme runtime,
semantic-link inference, a new vector engine, arbitrary split trees, or a replacement
editor are outside this epic. Full published-citation rename is a follow-up unless
the existing review owner can complete it safely; the first delivery must explain
and refuse that unsupported case.
