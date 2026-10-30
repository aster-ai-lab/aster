# Tasks: Local memory in Knowledge

## Phase 1: Behavior and tests

- [x] T001 [US1,US2] Core management DTOs, bounded snapshot browse/read and
  conditional delete; serialize writes and clean caches. Temporary-store tests
  prove completeness, strict refusal, races, persistence and subsequent recall.
- [x] T002 [US1,US2] ACP host method using exact admitted launch workspace and
  effective memory root without model/session creation. Tests cover no provider,
  unknown fields, mismatched workspace and exact-ID/version deletion.

## Phase 2: Wiring

- [x] T003 [US1,US2] Native tenant/workspace root, trusted ACL binding, live owner
  routing and provider-free SDK fallback. Register main-only command/permission.
  Hermetic tests prove tenant isolation, lock refusal and uncertain-write handling.
- [x] T004 [US1,US2] Retained Knowledge/Memory tabs and list/detail/search/type
  filter/refresh/confirmed delete UI. Strict bridge DTOs, i18n and focus behavior.
  Renderer tests prove draft continuity, stale scope, failures and confirmation.

## Phase 3: Evidence

- [ ] T005 [US1,US2] Run focused Core/ACP/native/renderer checks, formatting,
  local-dependency and native-control ratchets, lint/i18n and production builds.
  Run synthetic real-ACP E2E and attempt GUI acceptance with bounded evidence.
  Record commands/counts/source hashes and explicit gaps in OUTCOME.md.
- Commit/staging omitted: user did not request either.

T005 checkpoint: focused tests, real ACP E2E, both renderer builds, ToYou native
check, owned formatting, lint, i18n and dependency guard pass. Full Web typecheck
retains its 396 baseline diagnostics; root Rust fmt reports two concurrent files.
GUI capability probe refuses the installed browser adapter, and no GUI MCP tools
are exposed. Native GUI acceptance remains unclaimed. See
`docs/evidence/desktop/2026-10-24-local-memory/OUTCOME.md` for counts and commands.

The preceding GUI-connection gap is superseded: the retained background-only MCP
bridge is restored and Knowledge native search/open/edit/save and Memory's empty
state have been observed. A provider-free native fixture then reopened T003:
the Agent-plane factory appended an already resolved instance suffix again,
persisting its record outside the profile used by Memory. The qualified failing
oracle is in `docs/evidence/desktop/2026-10-24-knowledge-native-profile/`.
The redundant resolution is removed. Seven owner cases, one native factory case
and the real profile persistence oracle pass. Actual GUI record inspection,
cancel/confirm deletion and fresh ACP read-back also pass; see that directory's
OUTCOME.md. The later Library switch during human review does not establish
post-delete UI continuity.
No existing profile/state is copied or migrated. T005 stays open for actual
memory-record GUI journeys and the remaining global verification limits.

## Final Verification Gate

The latest full testing request adds the shared
[E2E plan](../015-knowledge-workbench-ux/e2e-plan.md) and
[scenario catalogue](../015-knowledge-workbench-ux/e2e-cases.md).
Memory acceptance includes KM01–KM18, KA02's actual Agent extraction, KJ07's
cancel/delete/recall journey, and the cross-scope/GUI variants; historical SDK
seeding proves management only. T005 is not complete until those required
current-layout gates and cleanup pass. This is planning, not a new GUI result.

- [x] Owning packages' focused tests pass (T005)
- [ ] cargo fmt --all -- --check passes (T005)
- [x] No new crates.io a3s dependency (T005)
- [x] No native select added on product surfaces (T005)
- [x] Product storage configuration is ACL (T003/T005)
- [x] Evidence distinguishes hermetic/real ACP/GUI results (T005)
