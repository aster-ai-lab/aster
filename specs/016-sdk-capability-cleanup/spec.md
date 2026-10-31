# Feature Specification: Rust SDK and explicit capability activation

**Feature Branch**: Existing working branch; no branch requested.
**Created**: 2026-10-24
**Status**: Implementation authorized
**Input**: Clean other-language SDK surfaces and optimize the reviewed capability boundaries.
**Gate context**: Product regime; Rust Core, ACP and the private Studio host. This closes the four findings from the capability review.

## User Scenarios & Testing

### User Story 1 — Supported integration guidance (Priority: P1)

A developer finds only the supported local Rust SDK and ACP integration paths in current documentation.
**Independent Test**: Core manual index and a retired-SDK instruction guard; root workspace-dependency guard.
**Acceptance Scenarios**:
1. Current guides contain no Node/Python/Go SDK installation, bridge-build or publishing instructions.
2. Programming-language examples, desktop TypeScript tooling and historical provenance remain intact.

### User Story 2 — Explicit Office capability (Priority: P1)

A generic ACP coding session receives baseline tools. Studio explicitly requests its Office tools without changing permissions.
**Independent Test**: ACP metadata/session setup tests and Studio host-metadata tests.
**Acceptance Scenarios**:
1. Missing or false `officeTools` does not register Office tools; true registers both native Office tools.
2. Invalid metadata types refuse session creation. Source-review mode remains tool-free even with Office requested.
3. Studio declares Office support through its existing host metadata; request text cannot grant it.

### User Story 3 — Optional and lazy browser search (Priority: P2)

Standalone ACP builds need no browser-search dependency. Desktop builds can explicitly select it. Search prepares the browser only when that tier is awaited.
**Independent Test**: Cargo feature/dependency checks and Core web-search lazy-runtime tests.
**Acceptance Scenarios**:
1. Default standalone ACP excludes the headless feature; desktop build/development/package paths explicitly opt in.
2. Constructing a search plan performs no provisioning or task spawn. Awaiting its headless runtime preserves existing resolution and time limits.
3. API/HTTP-only searches never prepare Moli; cancellation drops owned pending work.

### User Story 4 — Truthful embedding contract (Priority: P2)

A host can distinguish its registered embedding factory from the embedded light backend.
**Independent Test**: Existing ACP embedding factory/light-tier tests and current documentation checks.
**Acceptance Scenarios**:
1. Documentation describes the actual factory-first, explicitly opted-in light-tier behavior.
2. Invalid factories and failed light-tier loads remain named failures; no implicit remote route is introduced.

### Edge Cases

- Office visibility never widens workspace permissions or source-review restrictions.
- Feature unification is checked on the actual paired terminal/ACP build graph, not inferred from a single manifest.
- Retired SDK source and publishing jobs are removed if found; retained evidence and source-provenance records are not erased.

## Requirements

- **FR-001**: Current integration material MUST describe local Rust/ACP and MUST NOT advertise retired SDK delivery.
- **FR-002**: Office registration MUST require an explicit host boolean and MUST refuse malformed values.
- **FR-003**: Browser search MUST be an explicit Cargo capability; disabled builds MUST retain typed refusal for browser-only engines.
- **FR-004**: Provisioning MUST be deferred until the headless tier is used and MUST preserve deadlines and cancellation ownership.
- **FR-005**: Embedding guidance MUST identify actual backend selection; missing or broken resources MUST NOT silently activate remote services.
- **FR-006**: Existing public Rust APIs, local Aster dependencies, tenant isolation and unrelated working changes MUST be preserved.

## House Checklist

- [x] New behavior has small owning-package hermetic tests.
- [x] Closed metadata booleans and existing Cargo features; no guessed intent.
- [x] No new unexplained production unwrap/expect.
- [x] Aster dependencies remain local and unpublishable.
- [x] Persistent product configuration remains ACL.
- [x] Evidence is recorded in `docs/evidence/code/2026-10-24-sdk-capability-cleanup/OUTCOME.md`.

## Success Criteria

- **SC-001**: No current SDK installers or package publishing instructions remain for retired language bindings; manual links/index pass.
- **SC-002**: Office missing/false/true/invalid/source-review cases pass through actual session setup; Studio metadata preserves Office behavior.
- **SC-003**: Default and browser-enabled feature checks pass; lazy runtime construction needs no executor and awaiting resolves the configured runtime.
- **SC-004**: Focused/connected tests and applicable build checks identify the current source and report every residual failure honestly.

## Assumptions

- Cleanup concerns aster SDK delivery, not JavaScript/Python tools used by the product or by coding fixtures.
- Studio remains an Office-capable host; generic ACP clients explicitly opt in.

## Explicitly Not Claimed

- No removal of supported Rust APIs, production deployment, new GUI flow, model benchmark, or resolution of unrelated pre-existing test failures.
