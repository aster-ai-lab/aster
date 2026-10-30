# Feature Specification: Integrated local model control

**Feature Branch**: current worktree; no branch or commit requested

**Created**: 2026-10-28

**Status**: Implementation

**Scope revision 2026-10-29**: The scope now requires the complete Aster local-model-control capability baseline, including features previously deferred. [full-replacement.md](full-replacement.md) and [reference-baseline.json](reference-baseline.json) extend this specification with a pinned reference inventory and acceptance obligations. This revision supersedes the former first-delivery exclusions; it does not mark any task complete.

**Input**: 根据开发规范按规划完成开发和测试。为 Aster 本地设置增加模型网关、路由、额度、用量统计，融合现有模型配置与用量页面，参考用户提供的界面设计。

**Gate context**: Product regime. The root Core library owns reusable model-call behavior, ACP transports its facts, and the private Studio submodule owns local settings and host lifecycle. The revised epic includes the full pinned replacement scope; earlier first-delivery deferrals no longer bound completion.

## User Scenarios & Testing

### User Story 1 - One source for model configuration (Priority: P0)

A local user configures providers, keys, model capabilities and pricing in the existing provider page, then binds the existing default/quick/translate uses to a model or an explicit routing plan in the model page.

**Independent Test**: Core policy/parser tests and Studio provider/default-model tests prove stable references, valid capabilities, reload, and named refusals for missing or disabled targets. Local multi-key execution uses the same identities as the existing key editor.

**Acceptance Scenarios**:

1. Given an existing configured local model, enabling this feature preserves its identity, credentials, and default selections.
2. Given a saved routing plan, selecting it for a supported use resolves its candidates without creating a fake model or changing `UniqueModelId` semantics.
3. Given a missing, disabled, or incompatible candidate, dispatch refuses it with its reason; it never chooses an unconfigured provider.

### User Story 2 - Observable local usage (Priority: P0)

The account-free local user opens the existing usage route and sees observed built-in and gateway calls with shared filters, totals, charts, heatmap, request details and CSV export.

**Independent Test**: Owning Rust metering/store tests and renderer interaction tests prove per-attempt accounting, unknown values, immutable pricing, matching filters/export, and no organization dependency.

**Acceptance Scenarios**:

1. One logical model request with three upstream attempts is one request and three attempts; every reported attempt contributes its usage once.
2. A failed, cancelled or interrupted attempt retains unknown usage when no usage was observed. Missing pricing is not free usage.
3. Updating current pricing does not reprice historical calls. Different currencies do not sum or sort together.
4. The transcript, usage view and export refer to the same recorded invocation facts, without separately billing cumulative turn totals.
5. The default view uses compact metrics, trend/ranking, heatmap and request rows; narrow layouts retain core columns and put details in the existing side panel.

### User Story 3 - Explicit routing and budgets (Priority: P0)

The user configures fixed or ordered-fallback routes and daily/monthly local budgets for the global scope, a provider or a gateway access key. Provider quota observations and local budgets remain distinct.

**Independent Test**: Hermetic real-dispatch tests exercise failures before/after output, capability refusals, authoritative retry metadata, concurrent reservation, cancellation, restart and unknown settlement.

**Acceptance Scenarios**:

1. Before output reaches the caller, an eligible failure can move to an explicitly configured compatible candidate. After output, no transparent replay occurs.
2. A global budget also constrains every fallback candidate. Concurrent calls cannot spend the same reserved allowance.
3. Reminder budgets allow calls and expose state. Stop budgets refuse new calls with a reason and reset information.
4. An enforceable money budget requires known pricing and a supported bounded accounting path; activation refuses missing prerequisites instead of silently weakening the budget.
5. Provider allowance cards expose source, observation time, reset time and stale/unsupported/error states. They never invent a zero remaining balance.

### User Story 4 - Local gateway clients (Priority: P0)

A user enables the existing gateway surface, creates a named access key, grants explicit models/routes, copies connection instructions, and observes a test request in usage.

**Independent Test**: Local loopback HTTP integration tests exercise authentication, grants, supported wire protocols, streaming/tool calls/cancellation and shared routing/budget/usage. Tauri permission and renderer tests prove lifecycle and connection UI.

**Acceptance Scenarios**:

1. The default listener is loopback, advertises only verified protocols, and runs with the desktop lifecycle.
2. Gateway keys identify callers and restrict model/route access without exposing supplier credentials.
3. Disabling external gateway access leaves built-in model routing and accounting active.
4. Local Aster data/configuration and managed ToYou data remain isolated.

### Edge Cases

- Missing/partial provider usage, overlapping cache fields, absent first-token timing, unpriced calls, subscription reference values and explicitly free prices.
- Retries, tool rounds, cancellation, client disconnect, process restart, duplicate facts and divergent replay.
- Disabled/deleted targets, stale quota readings, unsupported protocols and state tied to a particular account.
- Calendar boundaries, timezone changes, multiple currencies, narrow windows and empty/error/loading states.

## Requirements

### Functional Requirements

- **FR-001**: Extend existing provider/model/usage/gateway surfaces. Each setting has one authoritative writer; references reuse existing provider/model/key identities.
- **FR-002**: Product behavior configuration is ACL parsed/generated by `a3s-acl`. Owned legacy configuration migrates without long-lived dual writes or silent unreadable-file defaults.
- **FR-003**: Fixed and ordered-fallback routing use typed candidates, explicit compatible capabilities and named refusal reasons. No intent matching with words/regexes.
- **FR-004**: Persist logical requests and individual attempts with stable identities, source/model/key attribution, status, usage, timing and pricing snapshots. Secret values and prompt/response bodies are not usage metadata.
- **FR-005**: Reuse existing usage field semantics and UI components; add local queries, common filters, request details and CSV. Unknown and zero remain distinct.
- **FR-006**: Separate cash estimates, subscription allowances/reference value, and local model usage; retain currencies and immutable price snapshots.
- **FR-007**: Enforce local global/provider/gateway-key budgets with reservation and durable settlement where the selected mode promises stopping; refuse unsupported guarantees.
- **FR-008**: Surface supported provider quota observations with provenance and freshness; unsupported sources remain explicitly unsupported.
- **FR-009**: Provide loopback gateway lifecycle, named scoped keys, supported protocol connection examples and a real test-call flow. Built-in calls share policy/accounting without depending on the external-listener switch.
- **FR-010**: Preserve tenant isolation, account-free Aster behavior, existing Core/ACP execution authority, public model identifiers, trust boundaries and cancellation.
- **FR-011**: Keep Aster's graphite navigation/controls and use the reference's neutral cards and purple/blue data colors. Use house pickers/panels, accessible labels, responsive core columns and semantic unknown/error states.
- **FR-012**: Satisfy MP-01–MP-23 in `full-replacement.md`, including the pinned reference's protocols, account/intent scheduling, subscription reuse, client adapters, privacy, shared library/tools, remote/headless operation, profiles/sync, observability and management interfaces. Each family requires implementation and owning evidence; existing Aster behavior may satisfy it only after contract verification.

### Key Entities

- **Model target / route**: typed local product configuration referencing existing model identities.
- **Observed request / attempt**: immutable reusable model-call facts and local persistent projections.
- **Price snapshot**: per-invocation immutable per-unit rates/currency; absence remains unknown.
- **Budget / reservation**: local policy and durable consumption state, separate from analytical snapshots.
- **Quota observation**: provider-reported windows, timestamp and support/failure state.
- **Gateway client**: local access-key identity and model/route grants, distinct from upstream secrets.

## House Checklist

- [ ] New behavior has hermetic owning-package tests (T001–T010).
- [ ] Closed sets have parse/as_str and reject unknown values (T001–T007).
- [ ] No new unexplained production unwrap/expect (T011).
- [ ] Internal dependencies stay local and root-workspace-owned (T011).
- [ ] Product configuration uses ACL (T002).
- [ ] Evidence: `docs/evidence/local-model-control/2026-10-28/OUTCOME.md` (T011).

## Success Criteria

- **SC-001**: The hermetic model-call campaign proves one source of consumption across successful, failed, retried, cancelled and replayed requests.
- **SC-002**: Local UI tests prove configuration, filtering, details, export, budget refusals and gateway test-call behavior without login or a live external provider.
- **SC-003**: Targeted Rust/renderer suites, applicable workspace/architecture checks and root `cargo fmt --all -- --check` pass, or independently reproduced pre-existing failures are retained and reported.
- **SC-004**: Rendered local settings are inspected at narrow and wide sizes with the supplied screenshot as a visual reference.
- **SC-005**: Every FR and task has inspected evidence before this epic is called complete.

## Assumptions

- Existing dirty edits belong to their authors and are preserved. Work is incremental in the current worktree.
- Supported protocol claims follow actual implementation and loopback test evidence; the gateway does not imply universal protocol conversion.
- The first delivery covers observed Aster calls and explicitly connected clients. Unsupported vendor allowance APIs display unsupported.

## Completion boundary

- Intent classification, third-party observations, subscription reuse, opt-in LAN sharing and remote operation are required under the revised full-replacement goal. They are no longer deferred.
- Aster retains its own product identity, ACL configuration, permission boundaries, account-free local host and updater. External-client file formats are interoperability boundaries, not additional Aster configuration languages. Existing subscription authentication owners are reused without copying credentials/session state.
- New foreign-language SDK bindings and a separate cloud billing product are not alternative implementations of the requested local product.
- Real-provider reconciliation and platform qualification require actual evidence; synthetic campaign results cannot be described as live bills or hardware acceptance.
