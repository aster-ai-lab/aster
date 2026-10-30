# Implementation Plan: Knowledge workbench UX

**Branch**: unchanged; no branch requested. **Spec/Input**: [spec.md](spec.md).
**Gate context**: Product regime; personal knowledge reading/writing UX.
**Status**: incomplete; active implementation and native diagnosis. The retained
background GUI MCP bridge is restored. A native search/open/edit/save slice now
passes with disk read-back; the first blank content observation is retained as a
failed journey. The native profile correction is qualified separately; the user rejected the current IA. Unified-layout migration and remaining native gates are open.
No milestone acceptance claimed.

## Summary

The third-edition [UX contract](ux.md) supersedes the rejected two-level
Library/Memory composition. Notes, Published Pages and Memory are peer resource
destinations sharing the same list/detail workbench. Review is an internal
Published Pages workflow using that frame. Keep the existing independent data,
query, draft, memory and publication owners.

The main third-edition presentation is implemented, including direct single-row
icon actions and separate search fields. Its latest connected renderer campaign
passes 100 cases; the [implementation checkpoint](../../docs/evidence/desktop/2026-10-24-knowledge-unified-implementation/OUTCOME.md)
retains the real fresh-vault failure. The old A–C records below are historical
checkpoints; whole-feature safety/native gates stay open.

## Immediate delivery sequence

The user's full end-to-end testing request is now specified in
[e2e-plan.md](e2e-plan.md) and [e2e-cases.md](e2e-cases.md): 47 inventoried functions,
146 scenario families including 12 business journeys, separate actual native/
owner/real-model evidence, and explicit implementation/capability gates. These
are planning artifacts, not a newly passing test campaign. Existing T018–T020
and 018 T005 must satisfy that complete gate rather than a subset of screenshots.

| Slice | Reach | Observable acceptance |
| --- | --- | --- |
| R1 resource navigation | KnowledgePage/KnowledgeNotesPage, locales, callers and fixtures; T011/T012/T016 | Three peer tabs, no duplicate bottom Published Pages group or scope picker; exact source jumps; one retained Office instance and independent resource positions |
| R2 common workbench | KnowledgeWorkbench/KnowledgeSearch/KnowledgeMemoryPage; T011/T012/T017, 018 T004/T005 | Shared list/detail geometry and narrow navigation; floating filters; no full-width Memory toolbar or empty storage detail; first empty workspace is valid |
| R3 review integration | KnowledgeMaintenanceWorkspace and existing view binding; T010/T014/T017 | Published Pages owns review; same frame, candidate-first detail, explicit return, exact-version decisions; source/run lists are secondary |
| R4 whole journey | T017–T020 and 018 T005 | Fresh native cross-resource journeys, draft/ACP/disk checks, keyboard/IME and responsive screenshots for the unified layout |

Retain manual save and all current recovery/conflict/permission guards. Do not
instantiate two note workspaces to implement two resource tabs. Memory reuses
the current workbench layout while retaining its SDK/ACP owner; maintenance
keeps 012's policy, including ordinary automatic publication where authorized.
No new database/index/scheduler/dependency/capability is part of this migration.
Tabs/history/autosave and measured UI performance still need their original
gates; presentation coherence does not waive them.

### Existing implementation checkpoint — 2026-10-24

T005's exact-link contract and T007/T008's catalog/query contracts are complete
at their owners. T002–T004's writing protection and T011's reading surface remain
in progress. The personal-vault search field now uses native title/body search,
stored filters, continuation and explicit coverage. Other vault/browser contexts
retain their existing labelled search. The extracted workbench has an optional
related pane and full graph destination; component coverage is not native visual
or responsive acceptance. T012's title/path quick open and list continuity are
now connected through the existing owner query/state and command registry.
Exact recent identities, duplicate paths, IME/stale selection, refresh/paging
and two dirty drafts have hermetic coverage. The
[quick-open checkpoint](../../docs/evidence/desktop/2026-10-24-knowledge-quick-open/OUTCOME.md)
records 440 connected renderer passes; native GUI and UI latency remain open.

T010/T014 now preserve the mounted reader and maintenance view across explicit
destination changes. Verified frame bindings, retained comparisons on error,
next-pending selection/focus and persistent outcomes complete their owning
contracts. The connected 394-case renderer campaign and 19 OKF owner tests are
recorded in the outcome. This remains separate from native GUI/visual acceptance.

T006 now connects the main-only reviewed move command and its private recovery
journal to both rename and drag. It uses the complete admitted source scope,
the existing Core maintenance lock and publication-reference parser. The UI
requires explicit draft handling, shows exact impact and full saved/planned/current
versions, and resumes an interrupted review after closing or remounting. Owner
and component checks do not establish the outstanding native desktop journey;
T006 and M1 remain open. Other platforms are not inferred from macOS tests.

The retained [owner measurement](../../docs/evidence/desktop/2026-10-23-knowledge-workbench-ux/native-query-5000-coverage-final.log)
uses 5,000 notes / 51,016,142 bytes: warm query P95 117.97 ms, first ready query
21,565.90 ms and a 1,326-byte result page. These exclude renderer work. First-use
search preparation and rebuilding after source changes are therefore explicit
UX/performance work; a passing warm-query budget cannot hide them. Preserve the
last usable view while preparation runs, withdraw obsolete requests on scope
change and measure update lag before accepting M2. Do not add another watcher.

### Historical second-edition execution checkpoint (superseded for presentation)

The daily workflow is the unit of delivery: find → open → edit → follow a link →
return → save/recover. A shell extraction or a preview dialog alone is not a
completed milestone. Preserve the existing 012 maintenance/publication owner.

| Priority | Next concrete change | Observable acceptance |
| --- | --- | --- |
| P0: visible coherence | Batch A: compose one shared shell, remove child page headers, relocate controls and distinguish the two resource workspaces | Two top-level workspace tabs; one page title; no note-only actions or raw storage header in Memory |
| P0: daily work | Batch B: arrange browser/search/document and memory list/detail according to the second-edition UX contract | Exact title/path/body search, stable query scope, independent drafts, content-first memory and version-checked deletion |
| P1: understandable assistance | Batch C: review candidates first; related information opt-in; graph entered from context | No background focus theft, stale decision or falsely complete graph; explicit return restores reading state |
| Safety throughout | Retain T002–T004 manual save/recovery and T006 reviewed rename; finish native quit/restart before autosave/preview replacement | No data-loss guard is removed to simplify a screen; native bytes and acknowledged revisions agree |
| After A–C | Batch D: finish T009/T013 tabs/history with exact restoration and dirty-close decisions | Clean preview never replaces a dirty note; overflow and history preserve scoped identity |
| Release gate | Batch E: T017–T020 and 018 T005 | Responsive/native task evidence, screenshots and measurements; component checks remain separate |

Batch B reuses the existing bounded query, versioned note-save and native-memory
owners. Resource groups replace the extra browser tab strip; query scopes are
explicit, filters/sort are disclosed with removable active criteria, and browse
folder/scroll state survives search. One document control row owns save status
and an explicit IME-guarded save. Office's existing note mode picker presents all
three editing modes. Memory puts content/type/time first and folds advanced
fields/storage; refresh reads the exact inspected ID, unavailable records are
withdrawn, and confirmed deletion selects an adjacent loaded record. The shared
confirmation owner now also respects a disabled action after its panel opens.
No new storage, query service, SDK contract or autosave was introduced.

Batch C makes the existing candidate comparison the primary maintenance view.
Source management, archived runs, create and audit remain reachable through the
house utility menu; their authority and pending-decision guards are unchanged.
The compact header/status leave space for the comparison. Outline and links
remain primary in Related Information; secondary metadata is disclosed together.
An incomplete graph explicitly names Currently Loaded Notes, and Back to Note
returns to the retained Office instance. D/E, whole-query relationship coverage
and native qualification remain open.

The [exit/recovery checkpoint](../../docs/evidence/desktop/2026-10-24-knowledge-exit-recovery/OUTCOME.md)
closes a further T004 owner gap: a clean source does not excuse an unresolved
recovery write/cleanup. The quit chain now waits for its acknowledgement and
retains an unresolved recovered candidate for the user's decision. The connected
457-case campaign and both tenant renderer builds pass. This remains a hermetic
composition proof, not actual application quit/relaunch.

The preceding checkpoints were blocked on GUI configuration. The retained
2026-10-23 A3S Test MCP launcher and background semantic guard have now been
recovered unchanged. No foreground input, coordinate action or broader policy
was introduced. A fresh isolated bundle needed the existing development-launch
marker; it now starts. Its first blank content capture remains a failed journey;
the next session paints after background Tab and passes a scoped native body
search/open/edit/save path with exact disk read-back. The full T004 gate remains
open. Do not bypass it to enable autosave or replaceable
preview.

The user's latest screenshot and four-state wireframe also exposed a remaining
composition gap. The implemented follow-up removes the unselected desktop's
empty document toolbar, moves document controls into the reader column, makes
Recent Modified top-aligned with exact paths/full dates, and groups Filter/More/
New Note below the browser search. Default scope is named Default Workspace;
an explicitly configured path remains exact. Small-screen creation retains its
existing filename flow. The [checkpoint](../../docs/evidence/desktop/2026-10-24-knowledge-wireframe-implementation/OUTCOME.md)
records 462 connected passes, both renderer builds and a full native build.
These owner/component/build checks do not replace D/E or native UI acceptance.

T006 needs the complete source-discovery/read portion of T007 before impact can
be authoritative. This backend prerequisite may precede the M2 UI without moving
search ahead of the M1 safety gates or starting a second index/watcher.

Review each connected slice at 1440/1024/768/320 CSS content widths. Complete the
keyboard path and keep screenshots alongside behavioral/disk evidence. Six native
journeys and the 5,000-note measurements remain release gates even when component
tests pass. The current macOS background accessibility observation issue is an
acceptance gap; do not replace that gate with foreground or coordinate input.

### Existing feature dependencies (presentation governed by R1–R4 above)

The table below retains the original feature dependencies. The A–E delivery
sequence above now governs presentation work. Completed owner contracts remain
valid; their component layouts are not acceptance of the current third-edition UX.
The safety dependencies in [tasks.md](tasks.md) remain enforced.

| Order / tasks | Visible delivery | Review and acceptance |
| --- | --- | --- |
| 1 / T002–T004, T011 | Reading shell: left navigation, central Office editor, named related-information toggle; title, relative path and save state remain visible. `新建笔记` is the primary page action; maintenance is a quiet destination. | Create → edit → switch → return → save works with two independent drafts. Failed persistence, conflict and external deletion keep text and offer a concrete next action. Manual save remains until quit/restart/IME recovery gates pass. |
| 2 / T005/T006 | Exact link choice and rename review: duplicate titles show paths; rename shows old/new path, affected notes and exact occurrence count before any write. | Cancel writes nothing; stale review refuses and preserves the proposed name; interrupted writes expose the journal. Published/pending references and unreviewed directory impact explain refusal. |
| 3 / T008/T012 | Full-text results replace the navigation list; each row has title/path, match context and source type. Folder/tag filters, clear-filter, retry, coverage and load-more remain next to the query. | Chinese composition issues no intermediate query; late A cannot replace B; body result 250 opens the version-checked range; returning restores query/filter/list position. First preparation leaves the editor usable. |
| 4 / T009/T013/T015 | One replaceable clean preview, pinned tabs, Back/Forward and quick open; related information collapses, graph opens as a destination with a list alternative. | Editing pins a preview. Dirty content survives tab/result/link navigation. Close decisions, overflow, note state and graph viewport restore by exact identity; inactive heavy editors are unmounted. |
| 5 / T010/T014 | Review presents reason, current/proposed content, frozen evidence and `保留当前版本` / `接受此修改`; `返回阅读` restores the previous note. | Accept addresses the displayed candidate revision; stale/failure keeps evidence and selection; success selects the next item without dismissing the final outcome. Read-only history exposes no accept action. |
| 6 / T016–T020 | Responsive, keyboard and locale acceptance on the connected product. | Review 1440/1024/768/320 CSS content widths, 400% zoom, Chinese/English and reduced motion; retain six desktop journeys and owner/UI measurements separately. |

Use existing root design tokens, Office modes, collection controls, command
registry and in-place dialogs. No new theme, editor or multi-split framework is
needed. T011's extraction should follow these responsibilities only when wiring
them makes the existing page simpler.

### Desktop acceptance scripts

Use an isolated synthetic instance and background semantic input. Observe the
actual workflow before authoring deterministic ACL; no real user vault or model
provider is required for these fixture-backed journeys. Screenshots prove layout;
native read-back proves writes. All owned test processes must be cleaned up.

| Journey | Steps | Required proof |
| --- | --- | --- |
| J1 Find and return | Search a Chinese body term unique to note 250; page results; open a hit; return to results. | Exact path/revision/range; retained query/filter/position; truthful coverage; no search/navigation model call. |
| J2 Links and rename | Choose between duplicate basenames; follow an anchor; review rename; cancel, refresh a stale intent, then complete or recover an interrupted operation. | Correct note and occurrence; cancel leaves native bytes unchanged; affected links and journal match the displayed outcome. |
| J3 Two drafts | Edit A and B; switch by link/tab/quick open; save while further editing; request normal quit and relaunch. | Neither draft is replaced by a late acknowledgement; native saved/recovery bytes agree with the acknowledged revisions. |
| J4 Conflict and recovery | Edit externally while dirty; compare; save a new copy; exercise a failed recovery write and restart after acknowledged persistence. | External original remains intact; copy is not an overwrite; failure cannot claim saved; restart restores only durable bytes. |
| J5 Review and return | Open a fixture-backed pending change while a note is dirty; inspect evidence; reject a stale accept; refresh and accept/keep; return to reading. | Exact candidate/source generations; publication bytes match the outcome; next-item focus and reading draft/context survive. |
| J6 Keyboard and width | Create, search, choose a link, switch tabs, save and review by keyboard; exercise Chinese IME and all named widths/zoom. | Focus/announcements/essential actions remain usable; surrounding page has no horizontal overflow; no invisible or ignored text input is counted. |

The existing macOS AX observation gap is a native acceptance task, not a reason
to weaken these scripts or infer their success from renderer tests.

## Constitution Check

`PASS` means the plan names an enforcing implementation/test; execution is pending.

| Principle | Status | Evidence in the plan |
| --- | --- | --- |
| I Fail closed | PASS | Versioned save, explicit ambiguity, owner scope, partial coverage and guarded rename; T002/T005–T008 |
| II Evidence retained | PASS | T018–T020 retain commands, scope, screenshots, disk results and limitations |
| III Hermetic tests | PASS | Every behavior task includes an owning test; no live PostgreSQL requirement |
| IV Closed sets | PASS | Native query/save/rename enums use parse/as_str, strict decoding and unknown refusal |
| V Workspace/public APIs | PASS | Root Cargo workspace; reuse/export Core APIs through owner, no private cross-crate imports |
| VI Schema owner | PASS | Files remain files; recovery/view data stay host-owned, not an invented DataApi database |
| VII Shrink-only ratchets | PASS | No allowlist growth; no new Radix/native-select exemptions |
| VIII Replaceable transports | PASS | Existing desktop boundary and Core interfaces; no provider transport in note contracts |
| IX ACL | PASS | No new configurable product switches; draft/query/operation records are data |
| X Commands/tests | PASS | New commands registered with main-window capability; native tests in siblings |
| XI/XII Judgment/review | PASS | Preserve 012; record unsupported published-reference rename and performance limitations |
| XIII One repository | PASS | Planning in parent; Studio sources remain solely in private submodule; no new repo |
| XIV Names | PASS | Exact stored paths/titles and explicit selection; no inferred entity names |
| XV Meaning | PASS | Literal search is mechanical; AI classifications remain model/typed declarations |
| XVI Pickers | PASS | Shared SearchSelect/Combobox and menus only |
| XVII Dialogs | PASS | Existing in-place dialogs; no desktop edge-slide motion |

## Project Structure and prior art

All source paths below already exist unless explicitly marked proposed. Proposed
files stay in existing categories; no new process-root directory or editor stack.

| Concern | Existing owner/pattern | Planned reach |
| --- | --- | --- |
| Reader shell | `apps/studio/src/renderer/pages/knowledge/KnowledgeNotesPage.tsx` | Keep route composition; extract proposed `KnowledgeWorkbench.tsx`, `KnowledgeSearch.tsx`, `KnowledgeNoteTabs.tsx` only as each responsibility is introduced |
| Paths/relations | `knowledgeLinks.ts`, `knowledgeWikiLink.ts`, `noteReading.ts`, `knowledgeGraph.ts` | Replace stem-only resolution once and update all callers, suggestions, backlinks and graph; preserve link syntax and source ranges |
| Editor | `packages/office/src/internal/features/work/editors/markdown-editor.tsx` | Preserve current visual/source/split modes; extend reveal/save lifecycle only where needed by the reader; test other consumers if touched |
| Draft state | `pages/knowledge/heldNoteDraft.ts`, `src-tauri/src/tenant.rs` profile_dir_under, `src-tauri/src/lib.rs` main CloseRequested/request_exit | Evolve to scoped per-note state, backed by proposed host `knowledge_draft.rs`; flush through existing quit lifecycle and preserve hide-on-close semantics |
| File mutations | `apps/studio/src-tauri/src/workspace_file.rs`, `local_documents.rs`; `crates/code/core/src/workspace/services.rs` read_for_edit/write_for_edit | Add bounded versioned note seam, reuse path/CAS guards; do not silently change all document callers to a new contract |
| Catalog/search | `crates/code/core/src/workspace/manifest.rs`, `services.rs`, `retrieval/lexical.rs` | Reuse scoped manifest/catalog and persistent lexical capabilities. Proposed Studio `knowledge_query.rs` adapts complete query/continuation/coverage; no new embedding engine |
| Invalidation | `pages/documents/useLocalDocuments.ts` subscribeLocalDocumentChanges; `windows/main/KnowledgeMaintenanceRuntime.tsx` | Share existing events; reconcile only affected note/query state. Do not create a second watcher or model scheduling owner |
| Maintenance | `knowledgeMaintenance.ts`, `KnowledgeMaintenanceWorkspace.tsx`, `crates/okf/src/personal/maintenance.rs` | Retain 012 source/candidate generations and policy; revise presentation and focus, not authority |
| Comparison | `pages/agents/taskConversation/TaskDiffView.tsx`, `components/chat/messages/tools/agent/AgentFileDiffView.tsx`; existing maintenance before/after Markdown | Reuse a genuinely shared primitive through its owning category; otherwise retain before/after. No upward import from knowledge into agents |
| Commands/layout | `hooks/command/useCommandShortcuts.ts`, `pages/agents/WorkbenchCommandPalette.tsx`, `components/chat/shell/splitterA11y.ts` | Reuse registry/dialog/splitter patterns; no global shortcut handler competing with editor/IME |
| UI/locales | `packages/ui`, root `DESIGN.md`, Studio `zh-cn.json`/`en-us.json` | Shared rows/search/pickers/feedback; no new theme or hidden operational controls |

`specs/015-knowledge-workbench-ux/` contains spec, plan, UX contract and tasks.
It is planning memory, not a second acceptance record.

## Owner contracts and implementation decisions

### Scope and authority

Resolve tenant/workspace/vault once at admission. Every read/write/query/draft
request is bound to that identity; a view change withdraws old asynchronous
responses. Source notes are editable. Generated pages are read-only here and
use the existing maintenance owner for publication. The same filesystem may
be observed by more than one view, but does not gain a second mutation authority.

### Saves, drafts and external changes

Reuse Core's public revisioned edit pattern and the existing host path guards.
Read returns text/base generation; save submits that base and new text, and
returns the actually saved generation. A stale base refuses rather than overwrites.
Do not confuse a content hash comparison with a cross-process filesystem lock:
prove the actual owner/atomic-replace guarantees and retain conflicts/recovery
for other writers. Never claim arbitrary external writers are transactionally locked.

Draft state keys include tenant/vault/path; retain base, local revision and last
acknowledged bytes. Host persistence lives in the existing private instance path,
outside source discovery, with bounded records and permissions. Failed persistence
cannot clear dirty state. Scoping removes the prior single-draft/global-path leak.
Serialization schema is recovery data, not a new JSON product configuration.
Normal quit must resolve dirty drafts before the existing exit sequence proceeds.
An abnormal kill can recover only acknowledged durable drafts. Native recovery
must not claim that in-memory keystrokes were persisted or change app hide behavior.

Only enable 1-second idle autosave after native conflicts and recovery are proved.
Coalesce at most one in-flight save plus the newest queued revision per note.
Let the existing maintenance runtime settle/coalesce source events and preserve
one active job; do not start an AI operation per local input/save acknowledgement.
The current runtime already coalesces invalidation for 1.2 seconds and schedules
pending work after 1.5 seconds. Reuse that owner; autosave must not add a second
model trigger or duplicate queue.

### Link resolution and rename

Resolve a qualified relative target before considering an unqualified stored
basename. Return a typed `resolved`, `ambiguous`, `missing` or `invalid` result.
No case-folding of path identity or first-match fallback. Display all ambiguous
candidates with parent paths and require explicit selection.

Preview rename/move with exact impacted link spans, immutable revisions and a
reviewed intent. Do not regex-rewrite prose, code fences or unrelated substrings.
Single-file writes use existing safe primitives; multiple writes require a
durable journal/recovery path and accurate partial outcomes, not a promise of
global atomicity. Sources cited by generated/pending Wiki content are a safe
refusal in the first slice. A later approved integration may produce 012 review
proposals; no direct rewrite bypasses that workflow.

### Search, coverage and continuation

The current `LexicalSearchRequest` is bounded Agent retrieval (Top-K and candidate
truncation), not a complete paged collection query. It cannot simply be relabelled
as all-vault search. T007/T008 must prove the owner adaptation; expose any needed
library API at Core's public boundary instead of importing internals.

Use literal title/body search with stored folder/tag filters. Chinese substring
matching must be tested; BM25 tokenization is not assumed equivalent. Use lexical
ranking only where it preserves the declared matching/coverage contract. If the
existing engine cannot meet it, add the smallest bounded owner-side literal-query
operation over its admitted snapshot, retaining explicit indexing progress.

Proposed request: `search` (≤512 Unicode scalars), `scope` (`notes` or `published`),
optional authorized folder/tag, `sortBy` (`relevance`, `path`, `modified`),
`sortOrder`, optional opaque cursor and `limit` (default 20, admitted maximum 100).
Response: typed unique note refs, source revisions, context/ranges, query/index
revision, coverage/skipped reasons and optional nextCursor. Exact total is optional
and supplied only for a complete snapshot. A cursor binds scope/query/sort/revision;
unknown values, repeated non-progress and stale/mismatched cursors refuse explicitly.

No fetch-all-then-slice in React. Partial discovery never implies zero real matches.
Cold indexing remains cancellable and bounded by existing owner budgets; do not
raise the UI's 200-note constant as the proposed fix. Keep a settled query snapshot
while paging; offer a clear refresh when indexing invalidates continuation.

The reading UI's source-note search and Agent `search_wiki` over published pages
have different scopes. Share machinery where appropriate, not their meaning or
publication checks. Drafts, staging and private host state are never search corpus.

## Verification Plan

- Existing and new sibling renderer tests assert user outcomes, stale identity,
  save races, ambiguity, query state, focus and review; no snapshot-only tests.
- Native temporary-vault cases prove CAS, confinement, journals/drafts, exact
  query scope, partial metadata and permission registration. Core additions get
  Core sibling tests; OKF behavior stays under existing maintenance tests.
- Reuse A3S Test agentic GUI exploration on an isolated instance first, then
  promote proved paths to ACL. Background semantic actions only; an adapter's
  completion report is not proof a rich editor received text. Assert native bytes.
- Representative fixture: 5,000 notes / ~50 MiB, Chinese/emoji, same-named files,
  deep directories, large/skipped notes, external changes and a pending proposal.
  Record cold/warm timing, host hardware, payload/reads and coverage separately.
- Targets: warm query P95 ≤300 ms, opening ≤100 KiB note P95 ≤250 ms, query-page
  payload ≤256 KiB. Any cap/exclusion is shown; measurements decide further work.
- Run focused renderer/native/Core tests, applicable no-native-select/permission
  ratchets, locale/design checks, Rust formatting and workspace dependency guards.
  Full UI/native production build is required before desktop acceptance.
- Record commands/counts/failed attempts in
  `docs/evidence/desktop/<date>-knowledge-workbench-ux/OUTCOME.md`.

## Risks and planned limits

Implementation discovery: conditional file writes already exist in Studio's
`workspace_snapshot.rs` and `/api/workspace-file/snapshot`, including descriptor
confinement and ETag/If-Match. Reuse that boundary for notes rather than replacing
it with Core's optional-extension fallback. Native authoring recovery is extracted
for reuse; note recovery remains private, scoped and revision-checked. These
refinements reduce new implementation without relaxing T002/T004's test gates.

1. Whole-vault search may need a public Core query API beyond Agent Top-K; T007
   is an upstream contract gate, not permission to add a parallel indexing engine.
2. Published-source rename remains a clear refusal until integration with 012 review
   is complete. Existing files are not bulk-migrated or silently disambiguated.
3. Native recovery/safe saves precede autosave and replaceable preview tabs.
   An interface-only milestone must keep truthful existing save semantics.
4. Existing dirty-worktree build/typecheck issues must be reported separately;
   no baseline/allowlist expansion or weakening is authorized by this plan.
5. Estimates exclude cloud/team services, arbitrary split layout, new themes,
   advanced graph physics and full Obsidian vault/plugin compatibility.

## Research basis

The source comparison used Opensidian commit
[`4fe24c9`](https://github.com/jpzk/opensidian/tree/4fe24c937532951a51f7c253817f2a05083d380e)
for workflow reference. Its navigation, full-text context, link-impact review and
graph discoverability inform the outcomes. Aster implementation uses the in-repo
patterns above; this plan neither copies its code nor assumes its basename
fallback or feature claims satisfy Aster's fail-closed requirements.

| Source workflow | Aster decision |
| --- | --- |
| [Search rows and stale-response guard](https://github.com/jpzk/opensidian/blob/4fe24c937532951a51f7c253817f2a05083d380e/ui/main.js#L1013) | Keep path/context visible and reject obsolete responses; add Aster's explicit coverage, bounded continuation and source-revision guard. |
| [Tabs/history](https://github.com/jpzk/opensidian/blob/4fe24c937532951a51f7c253817f2a05083d380e/ui/main.js#L5430) and [quick switcher](https://github.com/jpzk/opensidian/blob/4fe24c937532951a51f7c253817f2a05083d380e/ui/main.js#L6748) | Complete the find → open → follow → return workflow using Aster's command registry and per-note drafts; retain one mounted heavy editor. |
| [Rename link-impact count](https://github.com/jpzk/opensidian/blob/4fe24c937532951a51f7c253817f2a05083d380e/src-tauri/src/index.rs#L773) and [move-then-ask UI](https://github.com/jpzk/opensidian/blob/4fe24c937532951a51f7c253817f2a05083d380e/ui/main.js#L5035) | Reuse the idea of an understandable impact count. Aster reviews before writing, revalidates the approved revisions and journals partial changes; generated/pending references remain under 012. |
| [Backlinks](https://github.com/jpzk/opensidian/blob/4fe24c937532951a51f7c253817f2a05083d380e/ui/main.js#L330) and [global graph entry](https://github.com/jpzk/opensidian/blob/4fe24c937532951a51f7c253817f2a05083d380e/ui/main.js#L8627) | Make stored relationships easy to reach, with scope, keyboard/list access and return navigation. Preserve Aster's exact identities and existing layout engine. |

These are source-based workflow observations, not claims that either product's
desktop UX or performance has passed the other's acceptance tests.
