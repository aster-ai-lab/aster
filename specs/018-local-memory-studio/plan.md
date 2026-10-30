# Implementation Plan: Local memory in Knowledge

**Branch**: main (no branch requested) | **Spec**: [spec.md](spec.md) | **Input**: [spec.md](spec.md)

**Gate context**: Core/ACP/private Studio, product regime; extension of 015.

## Summary

Extend the existing memory owner with bounded browse/detail and conditional
single deletion. Bind new Studio ACP sessions to a tenant/workspace memory root
through trusted ACL. The main-window command routes through an admitted live
ACP client or opens the same SDK store only when no owner is admitted; the engine
lock remains authoritative. Add a lazy Memory tab alongside the retained notes
view. No new dependency or database.

## Constitution Check

| Principle | Status | Evidence in the plan |
| --- | --- | --- |
| I. Fail closed | PASS | Strict DTOs, bounds, stale cursor/version and lock refusal |
| II. Evidence retained | PASS | docs/evidence/desktop/2026-10-24-local-memory/OUTCOME.md |
| III. Hermetic tests | PASS | Core temp stores, ACP loopback, host and renderer tests |
| IV. Closed sets | PASS | Operations/types/errors parse/as_str, unknown fields refused |
| V. One workspace / published APIs | PASS | Public Core API, root lockfile, private Studio gitlink |
| VI. Local schema changes | N-A | Existing collection, no schema change |
| VII. Shrink-only ratchets | PASS | No allowlist growth |
| VIII. Replaceable transports | PASS | Owner DTO independent of Tauri/ACP |
| IX. One configuration language | PASS | memory_dir in trusted ACL launch config |
| XIII. One repository | PASS | All files in this tree/private submodule |
| XIV. Names are not guessed | PASS | Exact stored IDs, types and metadata |
| XV. Meaning is not matched | PASS | Literal text filter, declared typed operations |
| XVI. Pickers are drawn | PASS | House SegmentedControl/Picker |
| XVII. Dialogs stay in place | PASS | House confirmation component, no custom motion |

## Project Structure

### Documentation (this feature)

specs/018-local-memory-studio/{spec,plan,research,data-model,tasks}.md

### Source Code

| Layer | Path | New files |
| --- | --- | --- |
| SDK | crates/code/core/src/durable_memory_vec/ | management.rs and management_tests.rs |
| ACP | crates/code/acp/src/agent/ | memory_management.rs and tests |
| Desktop host | apps/studio/src-tauri/src/ | local_memory.rs and tests; owner routing in agent_plane |
| Renderer | apps/studio/src/renderer/pages/knowledge/ | KnowledgeMemoryPage.tsx, localMemory.ts, tests |
| Shared data | apps/studio/src/shared/types/ | localMemory.ts |

## Research

See [research.md](research.md). Existing LazyFileMemoryStore same-process registry,
Collection snapshot iterator, fetch/delete/flush and SDK memory mutation paths
are the owner patterns. ACP host_methods.rs is the closed host method set;
agent_plane/acp.rs already admits live wiki clients. knowledge_query.rs shows
main-window admission and strict bounded envelopes. KnowledgeWorkbench.tsx and
KnowledgeNotesPage.tsx show hidden mounted-view continuity. House DESIGN.md wins
over ReMe styling. No direct collection access from the renderer or desktop host.

## Verification Plan

- Core focused management tests: paging completeness, strict decoding, bounds,
  stale version, concurrent update, persistence, recall deletion and dedup hygiene.
- ACP tests: no-model browse, exact launch workspace/root and unknown-field refusal.
- Native tests: canonical workspace/tenant separation, locked owner, ACL binding,
  response identity, no retry of uncertain mutations and main-window capability.
- Renderer tests: filters/detail/delete/cancel/conflict, stale scope, keyboard,
  note draft continuity; no-native-select ratchet; production renderer builds.
- Root cargo fmt and local dependency check; focused lint/i18n/typecheck delta.
- Synthetic E2E through real ACP and store; GUI adapter attempt reported honestly.
- No live cloud/PostgreSQL, real user-memory mutation or full workspace suite.

## Implementation checkpoint

The user's 2026-10-24 whole-page replan now owns presentation in
[015's second-edition UX contract](../015-knowledge-workbench-ux/ux.md). Its
Library/Memory shell, content-first details and collapsed storage information
are planned changes under 015 T011/T016; the SDK/ACP/host contracts here remain
authoritative. The completed first-edition UI tests below are regression
baselines, not acceptance of the redesigned screens.

T001–T004 are implemented with hermetic owner/ACP/native/renderer tests. Real
stdio ACP E2E and both renderer bundles pass. See the evidence OUTCOME for exact
counts and retained failures. T005 is open for full root-format cleanliness and
native GUI acceptance; a production renderer build is not visual acceptance.
