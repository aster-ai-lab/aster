---
description: >-
  AsterDream implementation mapping for the root DESIGN.md collection standard,
  including shared components, transport boundaries and acceptance evidence
sources:
  - src/renderer/components/collection/CollectionPagination.tsx
  - src/renderer/components/collection/CollectionSelectionBar.tsx
  - src/renderer/components/collection/CollectionToolbar.tsx
  - src/renderer/components/collection/useCollectionPage.ts
  - src/renderer/components/collection/optimisticRows.ts
  - src/renderer/components/LocalBackupManager.tsx
  - src/renderer/pages/settings/McpSettings/McpTool.tsx
  - src/renderer/pages/platform/PlatformPage.tsx
  - src/renderer/data/platformCatalog.ts
  - src/renderer/data/platformSuppliers.ts
  - src/renderer/pages/platform/SupplierEditor.tsx
  - src/renderer/pages/platform/SupplierCreateRecovery.tsx
  - src/renderer/pages/platform/SupplierRelations.tsx
  - src/renderer/pages/org/OrgMemberTable.tsx
  - packages/ui/src/components/console/Pagination.tsx
  - packages/ui/src/lib/use-confirmation-action.ts
  - packages/ui/src/lib/rowMotion.ts
---

# Collection UX implementation guide

The normative product contract is [DESIGN.md section 14](../../../../../DESIGN.md#14-collection-development-standard).
This guide maps that standard to AsterDream implementations. It does not define a
second design specification or certify existing screens. The
[pagination guide](./data-pagination-guide.md) and
[API guidelines](./api-design-guidelines.md) own wire types, hooks and transport
mechanics; they must implement the root interaction contract.

## Implementation map

| Requirement | AsterDream implementation / development obligation |
|---|---|
| Resource capabilities and sidebar coverage (§14.1–2) | Declare the actual owner, scope, lifecycle and operations before adding controls; `PlatformPage.tsx` exposes current views, not universal CRUD support |
| Query and authority identity (§14.3) | Scope list memory and every SWR key to the exact account/team/resource/parent and committed query; validate shareable route state |
| Search, filters and sorting (§14.4–6) | Use shared SearchInput, SegmentedControl and Combobox; query the owner, respect IME and ignore obsolete responses |
| Numbered pagination (§14.7) | Use `CollectionPagination` over `ConsolePagination`; local `useCollectionPage` is for complete bounded lists, not a replacement for remote pagination |
| Cursor histories (§14.7) | Use shared `CursorPager` with localized labels and pending state; expose only navigation/counts supported by the owner |
| Row and batch actions (§14.8) | Bind exact IDs and explicit selection scope; add batch controls only with per-record authority and a real owner batch contract |
| Forms and revision conflicts (§14.9) | `SupplierEditor` demonstrates associated labels, linked error summary, dirty guard, reviewed revision and explicit secret keep/replace/clear; preserve safe input on failure |
| Confirmation ownership (§14.9) | Both shared confirmation variants use `useConfirmationAction`; pass an `operationKey` with target/revision, localized `failureText`, and current permission/pending conditions |
| Reconciliation and denied reads (§14.10) | Invalidate affected page/detail/count/facet/badge keys in the same authority scope; distinguish confirmed write success from a failed refresh |
| Boundaries and accessibility (§14.11–12) | Keep local DataApi database-only and member requests on the member plane; use shared focus/dialog controls and qualify real keyboard/narrow-window behavior |
| Completion (§14.13) | Retain the applicable acceptance matrix, memory/live PostgreSQL campaigns, actual changed transport/GUI evidence and the owning ROADMAP gate |

## Table presentation and performance

The visual rules live in [DESIGN.md §6 Table](../../../../../DESIGN.md#table). AsterDream implements them as follows:

| Rule | AsterDream implementation |
|---|---|
| Result count in the control row | Pass the count as `CollectionToolbar` `summary`; it renders in the right group before refresh/create, never as its own row |
| 16px checkbox, 24px target | `RecordTable` uses the default `Checkbox` size with an `after:-inset-1` pseudo-element; do not pass `size="lg"`. The `data-selection-cell` cell toggles selection and is excluded from the row's expand click |
| Default page size | Tables open at 20 rows per page (`PAGE_SIZE` in `PlatformPage.tsx`); a chosen size is kept when the reader returns to that catalog |
| Pickers | Filters, the page-size picker and every other dropdown are `SearchSelect` (`size="sm"`, `searchable={false}` for ≤ 8 options, `allowClear` only on optional filters) per [DESIGN.md §6 Picker](../../../../../DESIGN.md#picker-dropdown); never a native `<select>` (`noNativeSelect.test.ts`) |
| Floating selection bar | Pass `CollectionSelectionBar` as the `PlatformFrame` `overlay`, never inside the list. The frame renders it in a non-scrolling layer centered 16px above the list viewport's bottom and adds 80px end padding while it shows. Do not key the bar by its IDs: remounting replays the entrance animation on every toggle; the bar resets its copy status when the selection changes |
| Selection during refresh | Server-paged lists never disable checkboxes or the selection bar for a same-query refresh; `RecordPage` gates them only for rows kept from a previous query (`stale`) |
| Row actions | Visible `详情` ghost button plus `RowActionMenu` (24px `更多`, edit then delete). The menu runs the chosen action from `onCloseAutoFocus` after refocusing its trigger, so the opened dialog restores focus to that row |
| Memoized rows | `RecordTable` renders `RecordRow` (`memo`) and hands it stable callbacks that read the latest handlers from a ref. Callers keep `columns`, `detailExtra` and `rowActions` referentially stable (`useMemo`, module constants) |
| Search draft isolation | `CatalogSearch` owns the typed draft and its 300ms debounce; the list re-renders only on a committed query |
| Same-query refresh | Server-paged lists keep rows readable and clickable during revalidation; only `RecordPage` dims rows kept from a previous query |
| Detail/editor dialog | `SupplierEditor` anchors `DialogContent` at 10dvh, shows `SummarySkeleton` while loading, and dismisses by setting `open: false` with `closing`; the provider drops the editor from `onCloseAutoFocus` after the exit animation. A closing editor is never resumed |
| Stable context | `SupplierEditorProvider` memoizes its actions value and `open`, so row actions re-render only when access, pending or busy state changes |

## Optimistic list writes

[DESIGN.md §14.10](../../../../../DESIGN.md#1410-loading-errors-feedback-and-reconciliation) makes list edits
and removals optimistic, and the §6 table rules define the row motion. AsterDream implements them as follows:

| Rule | AsterDream implementation |
|---|---|
| Overlay store | `beginOptimistic(collection, { remove, patch })` in `optimisticRows.ts` returns a handle. Call `confirm()` on acceptance or `rollback()` on rejection, then `release()` after the list reconciles. A confirmed overlay releases itself after 8s. A later change to the same row supersedes the earlier one; a rollback after a confirm does nothing |
| Reading the overlay | `useOptimisticRows(collection, ownerRows, idOf)` returns the projected `rows`, a `removed` count for totals, and `motion(id)`. Pass a module-level `idOf`. Use the same collection string at the writer and the reader, scoped like the list's SWR key (`supplierCollection(scope)`, `org/<orgId>/members`, `backup/local/<dir>`) |
| Row motion | `ResourceTable` and `DataTable` take `rowMotion={(row) => motion(id)}`. Custom rows (`RecordRow`, `OrgMemberTable`) spread `data-motion`, `aria-busy`, `rowMotionStyle(index, motion)` and `rowMotionClassName(motion)` from `@a3s-lab/ui` |
| Reconcile without a flash | `RemoteCatalogPage` holds the last page for the same key while `SupplierEditor` clears and revalidates the cache. Local lists refetch quietly (`fetchBackupFiles({ quiet: true })`) without the loading overlay |
| Form lifecycle | `SupplierEditor` closes on submit (`onWriting`), keeps the editor mounted while the write runs, and on rejection reopens with the draft and error (`onWritten(false)`). Closing the dialog during the write drops the editor once the write settles |
| Confirmations | `ConfirmDialog` actions return `undefined` and run the write with `void`, so the dialog closes at once. The row shows the pending or leaving state, and a failure restores it with a toast |
| Shared-list writes | `McpTool` overrides a switch at once but locks the sibling switches until the owner answers, because each toggle saves the whole disabled-tool list |

## Current migration status

Supplier/model main tables use owner-backed server pages with exact totals and
facets, full request keys, account/team isolation and last-page correction.
Other platform collections, overview reads and nested model-route histories still
require owner pagination where they grow. Shared local pagination and confirmation
improvements do not complete owner-authorized CRUD or the whole-application goal.

New member supplier command contracts separately qualify explicit resource grants,
reviewed CAS and commit-time authority/dependency fencing. Durable create-intent
receipts now qualify exact replay, changed-input refusal, concurrent create,
atomic rollback and no resurrection on memory/live PostgreSQL
([receipt proof](../../../../../docs/evidence/studio/2026-10-12-platform-supplier-intents/OUTCOME.md)).
Strict member HTTP CRUD now passes the same actual-session memory/live PostgreSQL
loopback campaign; native resource-path admission passes separately
([HTTP proof](../../../../../docs/evidence/studio/2026-10-12-platform-supplier-crud/OUTCOME.md)).
The typed renderer client and supplier editor expose independently granted writes,
full-field forms, explicit conflict review and bounded pending state. Component
checks use the actual shared controls and client over a network fixture; they do
not qualify the live native/GUI chain
([editor proof](../../../../../docs/evidence/studio/2026-10-12-platform-supplier-editor/OUTCOME.md)).
Do not expose additional controls based only on a service method's existence.

Supplier create consumers must retain one non-nil intent UUID and the exact
submitted input while reconciling an uncertain result. A replay returns the
original created ID/revision, even after a later edit or removal; fetch the current
record separately. Changed input requires a distinct reviewed intent after the
previous outcome is resolved. Receipts have no automatic expiry. Their keyed
fingerprints depend on the persisted supplier sealing key; losing/changing that
key refuses prior replay instead of silently creating a replacement record.

The editor retains the exact uncertain submission only in its mounted instance.
Local storage contains only the intent UUID. After a renderer restart, the
`SupplierCreateRecovery` confirmation resolves that request through the member
owner: `POST /v1/api/org/members/platform/suppliers/intents/{intent}/resolve`
with an empty body. Current create authority is required. The result is either
`created` with the original ID/revision, or `canceled` after a durable cancellation
fences any late creation under that actor's intent. Existing records are preserved;
the current detail is read separately. Cancellations have no automatic expiry.

Keep the marker until an owner outcome is confirmed and local cleanup succeeds.
Resolution failure, revoked authority or a replaced account must not clear it.
A failed refresh after confirmed recovery offers only a read retry. Memory/live
PostgreSQL, actual-session HTTP, native path admission and renderer component
proofs are retained in the
[recovery outcome](../../../../../docs/evidence/studio/2026-10-12-platform-supplier-recovery/OUTCOME.md).
Supplier relations now use a narrow member-authorized owner query, described below.
Probing and the complete native/GUI persistence workflow remain unqualified.

## Supplier models and removal dependencies

`GET /v1/api/org/members/platform/suppliers/{id}/models/page` lists declared
upstream model IDs with their individual dependency counts.
`GET /v1/api/org/members/platform/suppliers/{id}/dependencies/page` lists model
offerings that route through the supplier, including undeclared upstream IDs.
Both require current supplier read authority; they grant no full model-management
capability. The old admin-only array endpoint remains a compatibility surface;
AsterDream no longer calls it.

The strict query DTO accepts only `page`, `limit`, `search` and `model`. Defaults
are page 1 and limit 20; page bounds are 1–1,000,000 and limits are 10/20/50/100.
Search is a case-insensitive literal substring of at most 128 characters. `model`
is an exact stored upstream ID of at most 200 UTF-8 bytes and is accepted only for
dependencies. Stable order is name then exact ID; the owner returns the effective
valid page, including page 1 for zero matches.

Rows, exact matching total, supplier revision and `dependencyTotal` share one
repeatable-read snapshot. The latter counts all blocking offerings independently
of search or exact-model filtering. Counts identify offerings, not route entries.
Dependency rows expose only offering ID/name/state and this supplier's upstream
IDs; prices, credentials and other suppliers' routes are excluded. Each page
contains at most 100 rows and each dependency row at most eight upstream IDs.
Migration 113 adds a JSONB containment index. The retained PostgreSQL qualification
uses 5,120 offerings and a local two-second budget for three bounded reads.

`SupplierRelations` provides owner paging, IME-aware search, exact-model drilldown,
retry and account/team/parent-scoped cache keys. Read/delete dialogs isolate each
review generation and revision. Removal requires a successful dependency read,
the reviewed supplier revision and zero global dependencies; a failed/stale check
or remaining dependencies disables confirmation with a reason. The owner's
transaction still checks current authority, revision and dependencies atomically.
See the [related-page outcome](../../../../../docs/evidence/studio/2026-10-12-platform-supplier-relations/OUTCOME.md).

Track the full migration in
[the coverage matrix](../../../../../docs/evidence/studio/2026-10-12-platform-collections/coverage.md).
Feature/PR notes should use the review template in DESIGN.md §14.13 and name
remaining gaps explicitly. Historical cursor fallbacks and transport aliases are
compatibility behavior to migrate, not patterns for new owner contracts.
