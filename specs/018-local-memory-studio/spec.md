# Feature Specification: Local memory in Knowledge

**Feature Branch**: no branch requested; working tree remains on main

**Created**: 2026-10-24

**Status**: Implementation

**Input**: 知识页面增加类似 ReMe Studio 的记忆 tab，查看和管理底层 a3s-code 本地记忆。

**Gate context**: User-requested extension of 015, owned by Core durable memory,
ACP host extensions and the private Studio desktop submodule. Product regime.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Inspect local memory (Priority: P1)

Open Knowledge → Memory without configuring a model. Browse stored records,
search their text, filter their stored type, and inspect content and metadata.
Return to Knowledge with the note draft, selection and scroll retained.

**Why this priority**: Makes agent memory observable without another conversation.

**Independent Test**: Core temporary-store browse/filter/detail tests and renderer
tab/draft continuity tests. Empty and failed stores must be distinct.

**Acceptance Scenarios**:

1. **Given** synthetic records of the four stored types, **When** filtering and
   paging, **Then** every matching record is reachable without recall Top-K loss.
2. **Given** no provider, **When** opening Memory, **Then** inspection works locally.
3. **Given** an edited note, **When** switching tabs twice, **Then** the same editor
   and draft remain and hidden views do not steal focus.

### User Story 2 - Forget one record safely (Priority: P1)

Inspect a record and confirm its deletion. Only the exact inspected version is
deleted, after the owner acknowledges it. Agent writes or an unavailable owner
must yield a visible conflict/failure rather than a false success.

**Why this priority**: Removes stale or unwanted memories with a bounded action.

**Independent Test**: Core stale-version, concurrent-write and persistence tests;
ACP and renderer tests prove exact-ID/version binding and cancelled confirmation.

**Acceptance Scenarios**:

1. **Given** an unchanged record, **When** confirmed, **Then** it disappears from
   the persistent store and subsequent recall, and the next row can be inspected.
2. **Given** a changed record, **When** deleting the old version, **Then** conflict
   preserves the current data and invites refresh.

### Edge Cases

- Unknown operations/types/fields, malformed cursors, oversized content/query,
  unavailable workspace, changed scope and expired pages refuse explicitly.
- Another process holding the collection lock produces busy; no shadow database.
- Stored data that cannot be decoded is an error, never relabelled or hidden.
- Switching tenant/workspace clears selections and rejects late responses.
- Old unscoped Studio sessions are not admitted as a new tenant memory owner.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: MUST add Knowledge/Memory tabs using house controls and preserve the
  mounted reading/editing view; MUST refuse native select controls.
- **FR-002**: MUST expose bounded, stable, revision-checked browse, literal text
  filtering and exact detail via Core's actual VecMemoryStore; MUST NOT present
  agent recall as exhaustive browsing or create another memory store.
- **FR-003**: MUST render stored content as inert text and actual metadata;
  MUST refuse unknown types instead of inventing provenance or taxonomy.
- **FR-004**: MUST delete only an exact ID plus inspected revision after user
  confirmation; MUST serialize against SDK writes and refuse stale versions.
- **FR-005**: MUST use a live admitted ACP owner when present, otherwise the same
  public SDK owner API without a model; MUST refuse locked or lost owners and
  MUST NOT retry an uncertain mutation against another process.
- **FR-006**: MUST derive memory storage in the native host from tenant profile
  and canonical workspace, pass it as trusted ACL to new ACP launches and show
  that scope. MUST NOT accept renderer-supplied memory paths, copy legacy state,
  or read another tenant's memory. Existing terminal SDK defaults stay intact.
- **FR-007**: MUST restrict commands to the main window, validate request/response
  envelopes and late scope changes, and expose loading/empty/error/conflict states.
- **FR-008**: MUST retain hermetic evidence and a runnable synthetic E2E journey;
  MUST NOT claim native GUI acceptance from mocks or production renderer builds.

### Key Entities

- Memory record: existing Core VecMemoryStore record and its stored metadata.
- Memory page: bounded projection, engine snapshot revision, continuation.
- Memory revision: digest of exact persisted fields, used for conditional delete.
- Memory scope: native tenant profile + canonical workspace + admitted ACP owner.

## House Checklist *(mandatory — from Agents.md)*

- [x] New behavior has a small hermetic test in the owning package (T001–T004)
- [x] Closed sets: every new enum has parse/as_str (T001–T002)
- [x] No production unwrap/expect without an infallibility comment (T005)
- [x] Internal a3s dependencies stay local; no new dependency (T005)
- [x] Trusted storage binding is ACL parsed by a3s-acl (T003)
- [x] Evidence: docs/evidence/desktop/2026-10-24-local-memory/OUTCOME.md (T005)

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: Focused Core, ACP, native host and renderer suites have zero failures.
- **SC-002**: Two scopes with identical record IDs cannot read/delete each other.
- **SC-003**: A stale page/delete preserves current data and reports conflict.
- **SC-004**: The synthetic end-to-end journey records exact source and results;
  native GUI status is reported separately if its adapter is unavailable.

## Assumptions

- Management in this bounded slice means browse/search/filter/detail/refresh and
  confirmed single-record deletion. No automatic memory generation is needed.
- ReMe supplies workflow reference; Aster uses its actual database records.
- Memory remains the local tenant/workspace view when an organization knowledge
  center is selected. It does not substitute that organization's cloud bank.
- Desktop tenant isolation requires explicit storage for new sessions. No legacy
  data is imported; terminal-created workspace memories are not a tenant store.

## Explicitly Not Claimed

- ReMe installation, cloud memory banks, bulk clear, export, manual creation/edit,
  graph inference, short-term conversation buffers or removal of chat history.
- Migration of old workspace memories or forced restart of the user's processes.
- Completion of the separate 015 workbench epic or its native GUI campaign.
