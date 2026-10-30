# Implementation Plan: Integrated local model control

**Branch**: current worktree | **Spec**: [spec.md](spec.md)

**Gate context**: Product regime; root Core/ACP and the private Studio submodule.

## Summary

Reuse the existing LlmClient/usage facts, provider/model identities, settings routes and house components. Implement immutable invocation metering first, then one ACL policy and authoritative local accounting owner, then wire model dispatch and the existing UI. External HTTP is an adapter over the same policy and accounting, not a second agent runtime. Existing dirty changes are preserved.

The 2026-10-29 full-replacement revision adds the pinned families in `full-replacement.md`. T012–T018 implement these families; T011 becomes the final audit over the whole revised scope. Reference changes after the recorded revision do not silently move this baseline. A future baseline update must retain the difference and its new obligations.

## Constitution Check

| Principle | Status | Evidence in the plan |
| --- | --- | --- |
| I. Fail closed | PASS | Typed invalid/unknown/refused states; unavailable policy/store blocks governed dispatch |
| II. Evidence retained | PASS | T011 and the declared evidence directory |
| III. Hermetic tests | PASS | Each T001–T010 pairs behavior and its owner tests |
| IV. Closed sets | PASS | Rust parse/as_str; strict boundary DTOs for local policy, usage and gateway |
| V. Workspace / public APIs | PASS | Existing root workspace; additive APIs; no shell dependency in Core |
| VI. Local schema | PASS | Local invocation/budget storage stays with its owner; typed parameters |
| VII. Shrink-only ratchets | PASS | No native-control or architecture allowlist growth |
| VIII. Replaceable transports | PASS | Native/ACP/HTTP adapters retain one policy and facts |
| IX. Configuration | PASS | ACL for policy; legacy owned-field migration before cutover |
| XIII. One repository | PASS | Parent owns Core/specs/evidence; private submodule owns Studio source |
| XIV. Names | PASS | Existing stored IDs and explicit user-authored labels |
| XV. Meaning | PASS | Fixed/ordered policies are typed; intent routing uses a model with a validated declared category set, never word/regex classification |
| XVI. Pickers | PASS | Existing house picker and shared model selector |
| XVII. Dialogs | PASS | Existing stationary page-side-panel primitives |

## Project Structure

| Layer | Existing owner / extension |
| --- | --- |
| Model invocation/metering | `crates/code/core/src/llm/`, existing `TokenUsage` and response metadata |
| Invocation transport | `crates/code/acp/src/agent/`, existing usage events and host methods |
| Local product policy/host | `apps/studio/src-tauri/src/providers.rs`, `preferences.rs`, `agent_plane/`, local-model module with sibling tests |
| Gateway adapter | Existing Rust protocol/HTTP facilities and `apps/studio/src-tauri/`; verified conversions only |
| Shared UI contracts | `apps/studio/src/shared/data/types/aiUsageRecord.ts`, model/provider/API schemas |
| Settings | Existing provider/model/usage/api-gateway routes, hooks, menus, translations and shared panels |

## Research / decisions

- `TokenUsage` has optional cache counters; its Core diagnostic snapshots explicitly are not a billing ledger. New metering keeps missing data distinguishable from zero.
- `AiUsageRecordEntry` already has attribution, nullable usage, immutable prices and timings. Reuse these semantics rather than adding a competing renderer record.
- `useUsageData.ts` currently requires an organization. The local route needs a tenant-specific data source while sharing presentation primitives.
- Native `SavedProvider` currently has a single key while the renderer has multi-key contracts. Complete the native boundary rather than presenting controls with no execution effect.
- `providers.rs::commit_defaults` mirrored device preferences. Local default selections now write only those existing ACL preferences. The route policy file owns routes, not a second set of default bindings; T006 extends the existing selection boundary to explicit model/route targets.
- `BudgetGuard` checks around model calls and records successful usage; analytics alone cannot enforce concurrent/restart-safe limits. Reservations and unknown settlement are required by the requested stop-budget behavior, not speculative hardening.
- `translation.rs` exposes existing Chat Completions/Anthropic conversion; protocol claims must be proven independently, not inferred from a settings page.
- `aster-dream-theme.css` scopes graphite to Aster. Data colors stay local to analytics; use existing settings and side-panel components.

## Verification Plan

Run focused Core, ACP, local-host and renderer/shared suites as their slices land. Use loopback fixture servers for actual stream/fallback/cancellation behavior; no external model calls are required. Run applicable dependency and UI/authority ratchets. Rust commands run from the root on toolchain 1.97.1, after confirming Studio is initialized. Test native rendering using an isolated `just fork` instance per Studio instructions. Preserve logs, task coverage and all not-claimed scope in OUTCOME.md. Do not commit or stage unrelated edits.

Full-replacement integration keeps the same owners: Core holds reusable protocol/routing/redaction behavior, the native host holds credentials, listeners, files and accounting, and existing desktop/CLI/TUI surfaces expose that owner. Existing plugin, tool, library, session and updater facilities are reused after checking their contracts. Privacy data stays out of the invocation ledger. Subscription refresh remains with its authentication owner. Remote sync uses explicit destinations and owner-controlled encryption; tests use local fixtures. Optional local-model masking/intent classification fails with an explicit unavailable state when required and not ready.
