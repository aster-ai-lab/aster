# Tasks: Knowledge workbench UX

**Input**: [spec.md](spec.md), [plan.md](plan.md), [ux.md](ux.md).
**Status**: T005/T007/T008/T010/T014's owning contracts and tests are complete;
T002–T004 and T006/T011/T012/T015 remain in progress. Scoped full-text search and the
workbench/graph destinations, title/path quick open and retained search-list
position are connected. Tabs/history and native GUI acceptance remain open.
Owner/component checks do not establish T018.
Execution is active. The prior missing-host blocker was resolved by recovering
the retained A3S Test MCP bridge and its background-only guard. The current
isolated native app launches. The initial blank observation is retained as a
failed journey; a later diagnostic session reached the painted shell after a
background Tab and passed body search/open/edit/save with exact native-byte
read-back. Full native acceptance has not passed. The latest wireframe corrections pass 462 connected renderer
cases and both renderer builds; the native host/ACP/Office build also succeeds.
See the [wireframe implementation checkpoint](../../docs/evidence/desktop/2026-10-24-knowledge-wireframe-implementation/OUTCOME.md)
and preceding [exit/recovery checkpoint](../../docs/evidence/desktop/2026-10-24-knowledge-exit-recovery/OUTCOME.md).
Other checkboxes remain open until their full acceptance obligations pass. Native
desktop acceptance is still pending. No branch/staging/commit is requested.
**Rule**: each behavior-changing task includes its owning hermetic test. No task
is marked parallel because most share state/contracts or `KnowledgeNotesPage.tsx`.

## Current execution order — third-edition unified architecture

The user's latest screenshots rejected the second-edition presentation.
[ux.md](ux.md) now defines three peer resources and one shared list/detail frame.
Old A–C owner/component checkpoints remain evidence for their own behavior,
not acceptance of this architecture. The new prototype is separately checked;
the main presentation migration is implemented, including the user's latest
single-row icon actions. The latest renderer campaign passes 100 cases; native
whole-journey acceptance and the real fresh-vault owner failure remain open. See
the [implementation checkpoint](../../docs/evidence/desktop/2026-10-24-knowledge-unified-implementation/OUTCOME.md).

| Slice | Existing tasks / reach | Required regression |
| --- | --- | --- |
| R1 resource navigation | T011/T012/T016; KnowledgePage/KnowledgeNotesPage/locales/fixtures | Notes/Published Pages/Memory peer tabs; one retained Office instance; no duplicate group/scope picker; source jump and independent resource position/draft retention |
| R2 common frame | T011/T012/T017, 018 T004/T005; KnowledgeWorkbench/KnowledgeSearch/KnowledgeMemoryPage | Same geometry and small-screen navigation; filters in a floating panel; conditional recovery; no empty Memory storage panel; first-use preparation before query/recovery |
| R3 review placement | T010/T014/T017; KnowledgeMaintenanceWorkspace/view binding | Review stays within Published Pages workbench; candidate-first comparison, explicit return, exact-version decisions; source/run management secondary |
| R4 integrated acceptance | T017–T020 and 018 T005 | Current-layout cross-resource native journey, responsive/IME/keyboard screenshots, exact source/ACP read-back and owned-process cleanup |

Keep existing task IDs and safety dependencies. Completed T010/T014 owner
contracts remain valid; changed presentation needs new component/native evidence.
Manual save/recovery stays intact. No new SDK/store/index/capability is implied.
No staging/commit is requested.

## Phase 1: Behavior and tests — 10 tasks

### Complete E2E execution slices (2026-10-24 planning, all unexecuted)

These refine existing tasks rather than complete or replace them. The authority is
[e2e-plan.md](e2e-plan.md), with every scenario in [e2e-cases.md](e2e-cases.md).

- [ ] **E2E-I01 [US1–US4]** Freeze all visible controls/Office commands, 47-function
  mapping, fixtures, owner oracles, platform/input capabilities and performance
  sampling. Validate missing/duplicate case coverage and scope/binary mismatches.
- [ ] **E2E-I02 [US1]** Retain KG01/KG04 red cases; repair first-use preparation at
  the correct owner with existing/read-only/collision/symlink hermetic cases, then
  prove actual empty-workspace GUI creation without pre-creating wiki.
- [ ] **E2E-I03 [US1–US4]** Implement private Studio native fixture/oracle and the
  smallest root test-model fixture, with seed/reopen/cleanup and malformed-protocol
  checks. No new SDK, storage owner or production test backdoor.
- [ ] **E2E-I04 [US1]** Cover actual source writes, reviewed moves, quit/restart and
  failure recovery. Each fix includes its owning small test and native reproduction;
  different batch failures are reproduced alone before changing fixtures/code.
- [ ] **E2E-I05 [US2,US4]** Run actual Agent write/extraction, publication/review and
  ask_user/JSON Render flows; verify real tool/owner identity, versions, persistence
  and zero unsolicited model requests; real-model variants remain independent.
- [ ] **E2E-I06 [US1–US4]** Finish T-gate hierarchy, document preview/history and safe
  autosave contracts/implementation under their existing safety dependencies;
  each has owner/renderer rejection tests plus KJ11/KJ12 native journeys.
- [ ] **E2E-I07 [US1–US4]** Qualify background IME/right-click/drag and required
  widths/zoom/accessibility/load tests; leave unsupported input visibly blocked.
  Measurement helpers verify units, timestamps, percentiles and bounded samples.
- [ ] **E2E-I08 [US1–US4]** Promote only observed stable paths to private Studio ACL,
  check then run every required variant and all 12 journeys; P0 journeys repeat on
  three fresh profiles. Check result merging for missing evidence/duplicate IDs.
- [ ] **E2E-I09 [US1–US4]** Run actual final package/build/permission/locale/type/fmt
  and dependency gates. Preserve failed evidence and scope; root/submodule diffs
  reviewed separately. No staging, commit or resumed implementation goal requested.

Dependencies and the non-skippable final gate are in the E2E plan, sections 14–16.
The following product-task history remains unchanged.

- [ ] **T001 [US1–US4] Baseline and fixtures.** Inventory existing command bindings,
  UI/data ownership, source policies and 012 contracts. Extend synthetic fixtures
  under Studio's existing knowledge test tree with duplicate names, 250+ notes,
  Chinese/emoji, external edits and pending review. State the expected regression
  for each new test. Freeze current read/save/query budgets and representative
  wide/narrow screens; do not use real user files. Dependency: none.
- [ ] **T002 [US1] Versioned note writes.** In
  `apps/studio/src-tauri/src/workspace_snapshot.rs` and proposed sibling
  `knowledge_draft.rs`, reuse existing descriptor-confined snapshot/If-Match writes
  and authoring recovery; introduce strict note save/recovery contracts. Add
  sibling tests for stale base, read-only disk, atomic replacement failure,
  tenant/root/path mismatch and recovery storage failure. No autosave yet.
  Register/allow only the main-window surface as part of this same task.
  Dependency: T001.
- [ ] **T003 [US1] Save-state reducer and queue.** Evolve
  `pages/knowledge/heldNoteDraft.ts` and proposed `knowledgeNoteState.ts` into
  scoped per-note state; connect save lifecycle through existing document calls
  to T002. Test two dirty notes, A/B late responses, edits during save and retry.
  A success acknowledges its submitted local revision only. Dependency: T002.
- [ ] **T004 [US1] Draft restore and safe autosave.** Wire private native recovery
  and existing document invalidations into that state. Test restart/changed-base
  comparison, bounded native recovery enumeration with list failure/retry, explicit
  discard, external deletion and non-overwriting save-copy, close during pending/failed
  persistence, workspace switch and IME composition. Integrate the main-window
  close/request_exit seam in `src-tauri/src/lib.rs` without changing hide-on-close;
  test normal quit waiting for draft flush and abnormal restart recovering only
  acknowledged bytes. Cover quit before the renderer guard is registered and an
  unresponsive renderer; editing must not be admitted before its exit protection
  is established. Enable 1-second idle autosave
  only after those checks; test that repeated same-generation events do not create
  duplicate maintenance work in `KnowledgeMaintenanceRuntime.test.tsx`.
  Dependency: T003 and existing 012 runtime.
  Progress: the exit flush now also settles an uncertain private-recovery intent
  after undo or an acknowledged source save. It refuses failed cleanup and waits
  for the clearing acknowledgement; an unresolved recovered candidate remains
  available for an explicit restore/discard decision. Two owner regressions and
  one real held-state/quit-dialog composition case pass in the 457-case connected
  campaign. This does not complete the native quit/restart gate or enable autosave.
- [x] **T005 [US1] Exact path resolution.** Change
  `knowledgeLinks.ts` once, then update `knowledgeWikiLink.ts`, `noteReading.ts`,
  `knowledgeGraph.ts`, and every discovered caller. Add sibling cases for
  qualified paths, ambiguous short names, aliases/anchors, escaped syntax,
  Unicode, traversal and code fences. Return explicit ambiguity rather than a
  guessed note. Test picker choice with actual stored identity. Dependency: T001.
  Owning evidence: `t005-connected-regression.json` (300 passed, including the
  connected Office tests), `real-office-anchor-edit-state-rerun.json` and
  `anchor-source.json` in the 2026-10-23 knowledge-workbench evidence directory.
  Native GUI journey acceptance remains tracked separately by T018.
- [ ] **T006 [US1] Recoverable rename/move.** Extend
  `src-tauri/src/local_documents.rs` with reviewed impact/revision intent and
  journaled recovery using existing safe writes; wire caller/permission tests.
  Prove same-name collision, stale inbound links, interrupted multi-file writes,
  exact span rewrites and title-independent identity reconciliation. Generated
  or pending-publication dependencies and unsupported directory impact refuse
  with affected-note detail. Do not mark this task complete on UI preview alone.
  Dependency: T002/T005 and T007's complete source-discovery/read admission.
  Progress: complete bounded admission, Core maintenance/publication guards,
  sealed review intents, private durable journal and rechecked recovery now serve
  the main-only `knowledge_move` command. Rename and drag actions use its reviewed
  UI without a legacy fallback. Drafts need explicit save/recovery first; the
  dialog shows exact counts, bounded pages, first-change samples and complete
  before/planned/current comparison. Closing retains a resume action; stale
  previews require recalculation. Source move acknowledgement reconciles exact
  identities without carrying pre-rewrite text onto the new file. macOS owner
  tests cover interruption, collision, tampered intent, published dependencies,
  active maintenance and directory refusal with actual descendant paths. Real
  house-dialog tests cover focus, cancel, pending/duplicate submission and resumed
  review. See the journal continuation in the evidence outcome. T006 remains open
  for native desktop journey acceptance; Linux execution is not qualified here.
- [x] **T007 [US2] Owner catalog and source scope.** Adapt existing Core manifest,
  catalog and document event ownership through proposed Studio `knowledge_query.rs`.
  Prove source versus published scope, private/staging exclusion, partial discovery,
  skipped file reasons, invalidation/reconnect and tenant switching. Any missing
  public Core capability is added/tested at its owner; do not add a second engine.
  Dependency: T001; settle contract before T008.
  Evidence: Native `knowledge_query.rs` adapts bounded descriptor-confined discovery
  into Core's public manifest records/passive catalog, retaining the existing
  `ChangePump`. `native-query-coverage-final.log`: 11 owner cases; shared-watch
  coverage includes immediate/scoped invalidation, lost-history/overflow fallback,
  source/published separation, root replacement and instance isolation. T018's
  desktop journeys remain separate.
- [x] **T008 [US2] Bounded complete-query contract.** Implement owner title/body
  matching, stored filters, stable sort and snapshot-bound cursors in T007's seam.
  Test note 250, Chinese substring matches, Unicode offsets, incomplete coverage,
  no false exact totals, payload bounds, repeated/foreign/expired cursors and
  no duplicate/lost results within a settled snapshot. Prove why Agent Top-K is
  not the exhaustive result source. Add main capability registration checks.
  Dependency: T007.
  Evidence: `query-core-coverage-final.log` (170 connected retrieval cases),
  `native-query-coverage-final.log` and `query-coverage-renderer-final.json` cover
  literal matching, complete continuation, coverage/count reconciliation, whole-page
  bounds, source revisions, strict IPC/shared schemas and main-window registration.
  The shared 255-note wire fixture is checked by both native and renderer tests.
  T012 connects these results/filters and still owns quick open and full UI acceptance.
- [ ] **T009 [US3] Tab/history state.** Add proposed
  `pages/knowledge/knowledgeNavigation.ts` using T003's note state. Test replaceable
  clean preview, pin-on-edit, dirty-tab protection, stale/deleted history target,
  rename reconciliation, caret/scroll/mode restoration and scoped overflow limits.
  Dependency: T003–T005.
- [x] **T010 [US4] Maintenance view continuity.** Extend
  `knowledgeMaintenance.ts` view binding and maintenance workspace state without
  changing publication policy. Test keep/read context, displayed candidate revision,
  failure/retry, next-item selection and read-only history; retain existing stale
  source/candidate OKF tests. Dependency: T003 and 012.
  Owning evidence: `maintenance-final.json` (394 connected renderer cases) and
  `maintenance-okf-owner.log` (19 owner cases). The main reader remains mounted;
  actual Office source caret/scroll/focus and maintenance filters survive return.
  Hidden detail reads pause; reopening revalidates the selected root/job/path and
  candidate version. Failed/stale comparisons retain frozen evidence, successful
  decisions select the next pending path, and archived history remains read-only.
  This does not establish the separate native GUI journeys in T018.

## Phase 2: Wiring and UI acceptance — 7 tasks

- [ ] **T011 [US1] Reading shell and honest states.** Extract proposed
  `KnowledgeWorkbench.tsx` from `KnowledgeNotesPage.tsx`; compose navigation,
  editor and optional related pane per [ux.md](ux.md). Keep title-only search
  labelled accurately before T012 lands. Add component tests for create/open,
  unsaved/failure/conflict indicators, background-maintenance focus and source
  versus published mutation availability. Dependency: T003/T005; full M1 also T004/T006.
  Progress: the extracted workbench keeps the reader mounted when visiting the
  full graph. Related information is optional/collapsible; an empty rail reserves
  no width. New note is the primary action. Real house-dialog component cases
  cover medium/small navigation, focus return and width transitions. This does
  not establish T017's screenshots/zoom/native focus acceptance or M1 safety gates.
  Replan progress: A/B compose the shared Library/Memory header and resource-local
  controls, relocate create/review/graph actions, fold memory storage/advanced
  fields and put save state/actions in one document control row. Actual Office
  mode/save and memory content/selection/deletion cases are in the 452-case B
  checkpoint. C then prioritizes candidate comparison, folds secondary reading
  metadata and labels partial graph scope accurately (454 connected passes).
  T011 stays open for the original safety/native gates.
  Screenshot/wireframe follow-up: the unselected desktop home has no document
  control row. Recent Modified shows exact paths and full localized dates;
  selection opens that path. Document controls belong inside the reader column.
  Search has one shared Filter/More/New Note group, and small-screen creation
  still reaches the existing filename/creation flow. These are part of the
  462-case checkpoint, not completed native/responsive acceptance.
- [ ] **T012 [US2] Search results and quick open.** Add proposed
  `KnowledgeSearch.tsx` and a small scoped query state module; reuse shared controls,
  command registry and dialog pattern. Test composition/debounce, stale request
  rejection, coverage/error/no-match, load-more, query restoration, duplicate-title
  paths and version-checked body-range navigation. Default shortcuts must not
  collide with existing commands. Dependency: T008/T011.
  Include first-search preparation without blocking the editor, skipped versus
  pending coverage, retained stale results, explicit expired-cursor refresh and
  bounded latest-request-only busy retries cancelled on scope change. Test these
  states against the existing strict native client, not an invented progress API.
  Progress: `KnowledgeSearch.tsx` and `knowledgeSearchState.ts` now connect the
  strict native client on the personal vault. Stored folder/tag filters, stable
  sort, context/field/path rows, coverage/skipped reasons, retained stale results,
  explicit refresh and bounded busy retries are wired. IME/debounce and one
  in-flight query avoid duplicate cold scans. Existing document events invalidate
  results without starting another watcher or scan. Exact revision/body mapping
  reaches the public Office source-selection API, including read-only published
  pages; changed or dirty text refuses the old range. Title/path quick open now
  queries the same complete owner catalog, ranks up to 20 exact recently opened
  identities, and uses the registered CommandOrControl+Shift+O binding. Duplicate
  titles show exact paths; stale/IME input cannot select a different query's row.
  Refresh and continuation use the existing bounded state. Search-list position
  survives hiding/remounting and same-query refresh, and resets for a newly
  acknowledged query. The real command-provider case retains two dirty drafts.
  Evidence: `docs/evidence/desktop/2026-10-24-knowledge-quick-open/OUTCOME.md`
  records 440 connected renderer, 19 Core collection and 12 native query passes.
  T012 remains open for native desktop journeys and measured UI latency.
  Replan progress: B provides one browser search, explicit note/page scope while
  searching and advanced filters disclosed with visible active conditions. Quick
  open remains registered but leaves the page-level toolbar. No invented mixed
  collection total or another client-side query owner.
- [ ] **T013 [US3] Tabs and history UI.** Add proposed
  `KnowledgeNoteTabs.tsx`; wire preview/pin/close/overflow and Back/Forward to T009.
  Reuse existing menu/focus patterns. Component tests prove keyboard selection,
  dirty-close decisions, tab-to-editor focus, state restoration and draft-preserving
  link/result navigation. Dependency: T009/T011.
- [x] **T014 [US4] Review layout and action clarity.** Update
  `KnowledgeMaintenanceWorkspace.tsx`: change reason, current/proposed comparison,
  frozen source evidence, exact-scope action labels, next-item focus and explicit
  return-to-reading. Keep before/after until a shared diff is justified; no agents
  layer import. Test accepted/kept/stale/failed/empty/history outcomes with exact
  candidate identity and retained reading draft. Dependency: T010/T011.
  Owning evidence is the same connected renderer/OKF campaign as T010. Full paths
  accompany titles, before/proposed content and frozen source evidence remain
  available through refresh failures, and actions wait for the displayed queue's
  exact generation. The final result stays visible and receives deliberate focus;
  an intervening user selection is not replaced by an older review completion.
  Visual/narrow/native keyboard acceptance remains T017/T018.
- [ ] **T015 [US2/US3] Related pane and graph entry.** Evolve `NoteReadingRail` and
  `KnowledgeGraphView.tsx` with collapsible context, coverage, source-context
  backlinks, explicit graph scope and full-workbench graph navigation. Reuse T005
  identities and existing engine. Test no empty reserved pane, partial coverage,
  keyboard node/list navigation and return to the same graph viewport.
  Dependency: T005/T008/T013.
  Progress: graph is a full destination with retained viewport, explicit note/library
  scope, a fixed/recenterable note identity and keyboard-accessible full-path list.
  Exact rename reconciliation is covered. C folds secondary metadata together,
  names an incomplete graph Currently Loaded Notes and preserves the real Office
  undo state on Back to Note. Related discovery still uses the honest
  200-note reading coverage; whole-query relationships, tabs/history integration
  and native visual/keyboard acceptance are not complete.
- [ ] **T016 [US1–US4] Locale and boundary reconciliation.** Update affected
  `zh-cn.json`/`en-us.json`, shared request types/validators, Tauri main permissions
  and caller fixtures. Test malformed/unknown contracts and cross-root responses;
  run locale checks and no-native-select/permission ratchets. Reconcile source-save
  invalidations with 012 without creating another scheduler. Run per milestone,
  and finish after T012–T015.
- [ ] **T017 [US1–US4] Responsive and keyboard pass.** Validate named regions,
  actual command bindings, IME, focus return, disabled states, context collapse,
  tab overflow, save/review errors and reduced motion at 1440/1024/768/320 CSS
  content widths and 400% zoom. Use Body/Ink for essential text contrast. Add only
  behavior/accessibility regression tests that can catch a real failure; retain
  visual screenshots for layout. Dependency: each milestone's connected UI.

## Phase 3: Evidence and release gates — 3 tasks

- [ ] **T018 [US1–US4] Desktop journeys.** Use a3s-test GUI agent sessions on an
  isolated Aster instance with synthetic notes, background-only semantic input
  and disk/native read-back. Prove six SC-006 journeys, plus no new model calls
  from search/navigation. Observe before authoring deterministic ACL under
  `tests/e2e/`; do not treat ignored AX writes or action completion as successful
  text input. Finish every owned session and record cleanup. Dependency: T016/T017.
  For the new shared Memory workspace, also cover inspect, cancel deletion and
  confirm deletion using synthetic records through 018's actual native/ACP owner.
  Updated source/GUI evidence must correspond to the third-edition unified layout.
  Progress: the current 1440px native home, exact body result beyond entry 250,
  opening and Chinese/emoji source edit/save are captured. Exactly one of 259
  seeded sources changed and its saved bytes match the displayed edit; 258 stayed
  unchanged. Nine typed inputs report background delivery. The first blank paint
  and initial failed session remain retained. Native Memory empty state is also
  captured. The separate native-profile checkpoint qualifies the doubled-instance
  fix and synthetic Memory inspect/cancel/delete with fresh ACP read-back. The user
  rejected the visible IA; current-layout journeys and the other gates stay open.
- [ ] **T019 [US2/US3] Large-vault measurement.** Reuse the representative synthetic
  corpus; record host, revisions, cold/warm query/open P50/P95, payload, skipped
  coverage, event-update lag and repeated navigation memory behavior. Check
  SC-005 targets and 256 KiB page payload; retain failures rather than change
  budgets/allowlists to get a pass. Dependency: T008/T012/T013.
  Partial evidence: `native-query-5000-coverage-final.log` proves 5,000 notes /
  51,016,142 bytes, owner warm-query P95 117.97 ms, owner read P95 0.81 ms and
  first ready query 21.57 s. These are owner measurements, not UI/open timings.
  UI latency, source-update/rebuild lag and navigation memory remain unverified;
  investigate incremental reuse through the existing catalog/watch owner if
  measured source changes repeatedly incur the full cold cost.
- [ ] **T020 [US1–US4] Final verification/evidence.** Run relevant focused native,
  renderer/Core/OKF tests, production build, locale/design/permission gates, Rust
  fmt and workspace dependency guards. Record commands, counts, scope, pictures,
  native bytes and unverified limits in
  `docs/evidence/desktop/<date>-knowledge-workbench-ux/OUTCOME.md`. Keep other
  actors' dirty changes; Studio stays a private submodule. No commit unless asked.
  Dependency: T018/T019.

## Milestone map

| Milestone | Required task slices |
| --- | --- |
| M1 | T001–T006, T011, US1 portions of T016/T017 |
| M2 | T007/T008/T012, US2 portions of T016/T017; preserve M1 gates |
| M3 | T009/T013/T015, corresponding T016/T017 gates |
| M4 | T010/T014, all remaining T016/T017, T018–T020 |

T007 is a contract risk gate; T002/T004 gate safe autosave and dirty-preview
replacement. No user clarification blocks planning. Unsupported published-source
rename is explicitly scoped/refused, not an untracked assumption.

## Final Verification Gate

- [ ] Owning focused tests pass with no skipped required acceptance scenario.
- [ ] Full-text result 250, duplicate paths, stale writes and two dirty drafts pass.
- [ ] Actual native publication/recovery bytes match the displayed outcome.
- [ ] SC-005 measurements recorded; performance targets are not inferred from unit tests.
- [ ] Six background GUI journeys and exact owned-process cleanup retained.
- [ ] Relevant renderer/native build and locale/design gates pass.
- [ ] `cargo fmt --all -- --check` run; unrelated failures identified without editing them.
- [ ] No external `a3s-*` dependency, new workspace/lockfile, native select or allowlist growth.
- [ ] Product configuration, if scope changes to require it, remains ACL.
- [ ] Root and submodule diffs reviewed separately; stage individual files only if
  the user later requests a commit. Nothing is staged by this plan.


## 2026-10-25 active tabs, graph and classification slice

The note and encyclopedia reader now share the task workspace tab presenter.
The graph ports OpenSidian's pinned WebGL draw layer and retains accessible SVG
interaction/fallback, with floating filters/groups/display/forces controls.
Compiler authorization is restricted to the active owned staging job. Topic
classification is an explicit validated owner map; it preserves existing page
identities and supports physical category directories for newly compiled pages.
Unclassified legacy pages schedule one guarded audit, with incomplete or unsafe
classification refused. Search/refresh live in the shared page header, and Memory
exposes type icons and owner-side time filtering with cursor binding.

Focused evidence so far: 324 renderer cases; 29 maintenance owner cases; 5 scoped
ACP cases; 7 local-memory management cases; 13 query-owner cases (one explicit
large-vault measurement ignored). Native kn16 completed the first real-model compile and a subsequent automatic
classification audit: all 8 legacy pages received explicit topic paths with their
published paths/bytes and all 7 source note hashes unchanged, without tool approval
records. Graph zoom, node activation and returning through document tabs were
observed. The five type and three time filter icons, shared header placement, and
filter retention across native search/refresh are observed. A native encyclopedia
quick-open scope red exposed the notes-only legacy query and flat-only discovery;
connected and native-owner regressions now pass after repairing both boundaries.
Final native scope requalification passed with null cleanup error; see
`docs/evidence/desktop/2026-10-25-knowledge-tabs-graph-header/OUTCOME.md`.
This completes this bounded toolbar/scope slice only. These are scoped checks, not completion of T018 or the full E2E matrix.
