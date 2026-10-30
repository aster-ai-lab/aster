# Tasks: Dashboard console simplification and UX modernization

**Input**: Design documents from `specs/017-dashboard-console-simplification/`

**Prerequisites**: plan.md (including Constitution Check), spec.md, research.md

**Planning status**: These tasks are not started by this planning request. Each behavior-changing implementation task includes its hermetic and applicable real-owner/browser evidence.

## Phase 0: Baseline and contracts

- [ ] T001 [US1/US2/US3] Record the current navigation count/depth, Overview request/content inventory, primary-task paths, unsupported selection/actions, reference screenshots and representative loading/error behavior in a baseline evidence note. Include actual ordinary-org/operator cases and classify prominent features by frequency, practical value and owner readiness (retain core, demote advanced, remove duplicate/unavailable).
- [ ] T002 [US1/US2] Publish per-destination owner/capability/state contracts for Workspace, Organization & Settings and Platform Administration. Cover ordinary non-official orgs, directory admin and independent supplier/model/service grants; unknown/unauthorized destinations must have named refusals.
- [ ] T003 [US3] Publish page UX contracts for Overview, Applications, Operations, Usage and representative collections/forms: primary action, content budget, toolbar, detail model, loading/empty/no-match/error/conflict states, responsive/keyboard/locale requirements and acceptance evidence. Apply `apps/dashboard/CHARTS.md` to chart type, aggregation, Top N＋Other, time/quality semantics, restricted ChartProps, runtime and chart tests. Define semantic status/chart/feature-accent usage, non-color labels and any required root `DESIGN.md` token additions with contrast review; forbid rainbow navigation and page-local palettes.
- [ ] T004 [US1] Finalize exact scoped route and member/BFF DTO contracts with owning teams; validate PR1 and Application owner readiness without treating planning as an owner gate.

## Phase 1: Capability, route and shell foundation

- [ ] T005 [US1/US2] Implement one typed destination/capability registry and `ConsoleCapabilityProvider`, separating Workspace membership/Application capabilities from existing Administration grants. Add hermetic ordinary-org, directory-admin, independent-grant, unknown-capability and cross-scope refusal tests.
- [ ] T006 [US1/US2] Implement validated scoped routes, `ScopedRouteGuard`, closed tab/menu parsing and legacy view migration. Test malformed/stale/cross-scope IDs, Back/Forward, authorized legacy restoration and retired/unknown fallbacks.
- [ ] T007 [US1/US2] Compose the five-entry Workspace and Organization & Settings shell; relocate Administration as a grant-gated settings section. Remove Workspace search, enforce two-level navigation and keep Applications unavailable until its real owner contract. Run source/access/browser shell tests.
- [ ] T008 [US2] Migrate existing Administration destinations without changing API/grant semantics. Verify supplier read/write/recovery, models/services reads, audit, Operations read-only behavior, platform logs/system/sign-in and every formerly authorized destination.

## Phase 2: Overview and collection simplification

- [ ] T009 [US1/US3] Implement the next-action Overview with ≤1 primary action, ≤4 metrics and ≤2 main content sections. Use real bounded owner/BFF reads with independent loading/error states; test unavailable-vs-zero, empty Applications, unsupported create and failed-section isolation.
- [ ] T010 [US3] Remove checkbox selection/selection bars from collections without supported batch commands while retaining authorized row actions. Simplify toolbars, active filters, details and pagers to the page contracts; test selection, filtering, sorting, pagination and no-match behavior.
- [ ] T011 [US2/US3] Refine Administration Overview and high-density collections without moving platform-wide metrics into Workspace. Preserve supplier optimistic writes/revisions/intents and Operations owner-specific read behavior; run real permitted CRUD/read regression.
- [ ] T012 [US3] Complete the first accessibility/responsive/localization pass: keyboard/focus, contrast, reduced motion, supported 320px/400% reflow, zh-CN/en-US, long text and reference viewports. Retain screenshots and interaction evidence; fix rather than document avoidable failures.
- [ ] T013 [US1/US3] Measure Phase 1–2 results against the baseline: destination count/depth, primary-task clicks, control inventory, request counts and representative load/error behavior. Record remaining gaps and do not claim Application activation.

## Phase 3: Application activation (PR2–PR3 after PR1)

- [ ] T014 [US1] PR2: implement typed Application collection/create/detail with default context and ≤3 Chatbot inputs through real owner/BFF contracts. Test full scope, typed decoding, unsupported capability, create intent and continuity; record `application_id/workload_id/route_id` with real healthy HTTPS evidence.
- [ ] T015 [US1] PR3: add real conversation test, external Application keys, one-time secret and HTTP example on the same Application. Test exact grants, hidden runtime credential, content persistence, negative cross-app/tenant/management/shared-token/direct paths, replay/cancel accounting and real external call; link PR2 identity continuity.

## Phase 4: Continuous delivery and recovery (PR4–PR6)

- [ ] T016 [US1/US4] PR4: add OCI update and deployment timeline with owner-derived Converging/Ready/Degraded state on the same Application page. Verify unchanged Application/Workload/Route/URL/key/grant and failure/death semantics.
- [ ] T017 [US1/US4] PR5: add public-Git source binding, visible behavior change and successful/failed/cancelled terminal logs in one Application story. Test immutable commit/digest, real logs after cleanup, no failed serving activation and replay idempotence.
- [ ] T018 [US1/US4] PR6: add owner-supported rollback/cancel/recovery UX with immutable successors and honest partial-convergence state. Verify unchanged URL/access, no duplicate deployment, post-rollback real answer and operation recovery.

## Phase 5: Observability and final acceptance (PR7–PR8)

- [ ] T019 [US4] PR7: deliver Application activity/usage/cost views with typed correlation, request/token facts, unknown/estimated quality markers and no second ledger or duplicate counting. Run cross-scope and replay tests.
- [ ] T020 [US3/US4] Complete final visual/content/accessibility review across all changed pages and actual states, including degraded, unresolved writes, missing receipts, empty/no-match and long translations; retain reference-viewport and keyboard evidence.
- [ ] T021 [US1] PR8: run the clean-environment full journey and second-operator runbook, plus required Console/CLI/REST/Rust SDK/Management MCP surfaces. Retain at least five target-developer UAT records, no-assistance count and login-to-first-answer median.
- [ ] T022 [US1/US2/US3/US4] Write each slice's `docs/evidence/dashboard/<date>-console-simplification/OUTCOME.md` with exact commands/counts, baseline comparisons, identity continuity and Explicitly not claimed. Update the owning ROADMAP only after its actual gate passes.
- [ ] T023 Stage only an explicitly requested implementation slice, inspect `git diff --cached --name-only`, and never mix planning-only artifacts or another session's edits into a commit.

## Final Verification Gate

- [ ] Owning package focused tests and full standalone dashboard Vitest pass without timeout/allowlist weakening
- [ ] `npm run typecheck` and `npm run build` pass, including CSS contract
- [ ] Workspace has ≤5 primary destinations and ≤2 levels; Administration retains authorized workflows
- [ ] No selection/actions without owner-supported workflows and no demo acceptance
- [ ] Ordinary-org Workspace and independent Administration grants pass negative/positive scope tests
- [ ] PR2–PR7 identity continuity and PR8 M1–M6/UAT evidence are retained before product completion
- [ ] Keyboard, contrast, reduced-motion, supported reflow, locale and reference-viewport reviews pass
- [ ] `cargo fmt --all -- --check` runs for any future Rust edit; N-A for dashboard-only slices
- [ ] No new crates.io `a3s-*` dependency, native `<select>`, second design system, second authority or non-ACL product configuration
- [ ] No commit unless separately requested
