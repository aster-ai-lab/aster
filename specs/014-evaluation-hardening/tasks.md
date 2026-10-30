# Tasks: Evaluation-driven delivery hardening

**Input**: [spec.md](spec.md), [plan.md](plan.md)
**Prerequisites**: Constitution check and read-only consistency analysis pass before source edits.

## Phase 1: Shared delivery behavior and tests

- [x] T001 [US1] Record source/submodule/dirty-file baseline under docs/evidence/code/2026-10-23-evaluation-hardening; inspect all importer call paths. Fix numeric display in packages/office with an import/display regression covering 12000, zero, negatives, decimals, cached formula, date and explicit hidden formats; assert values stay typed.
- [x] T002 [US1] Preserve complete attachment metadata names in Studio AgentComposer.tsx and turns.rs staging, with Chinese/space/long/multi-dot/duplicate-name/unsafe-path tests and unchanged byte digest.
- [x] T003 [US1] Replace prose/shell-derived products in taskConversation/turn.ts with explicit tool/publication candidates; wire capability-scoped current file metadata in TaskConversationMessage.tsx and validate on open/download. Tests cover missing/deleted/renamed/inaccessible/outside-workspace files, tracebacks and verified products during input waits. Preserve concurrent UI changes.

## Phase 2: Native reads, execution facts and tool contracts

- [x] T004 [US2] Trace a3s_office public APIs, then add/reuse bounded source reads through ACP document_read/plugin_tools. Fixture tests prove DOCX paragraph/table locations, XLSX cells/types/formats/formulas, PDF text/page-image capability and explicit unavailable OCR; reject over-limit, malformed, unauthorized and symlink-escape inputs.
- [x] T005 [US2] Extend existing OCI/context facts and qualified Studio companion startup checks. Hermetic tests distinguish actual guest shell, persistent workspace, per-call temp files, missing companions and unavailable optional tools without permission/network expansion.
- [x] T006 [US3] Reproduce the observed heredoc permission refusals through the governed call path; verify projected patch schema against execution. Add exact-approved/effective-rewrite/deny/invalid-diff tests and retained bounded diagnostic evidence. Change production admission only for a demonstrated defect.

## Phase 3: Supported conclusions and connected callers

- [x] T007 [US3] Extend ACP output_check with closed date-span and sequence-change operations and sibling tests for endpoint convention, all decreases, missing/zero/invalid values and limits. Update existing session tool guidance for source readback, final claim review and model-handled conflicting requirements; no example phrase matcher. Test caller registration/schema and alternate phrasings with the deterministic model fixture.

## Phase 4: Build and end-to-end acceptance

- [ ] T008 [US1-US3] Run focused/connected tests and applicable CI gates; build root terminal+ACP and Studio+Office with pinned toolchain. Record frozen source/binary/renderer/sandbox/model identities. Preserve original input hashes and private-submodule boundaries.
- [ ] T009 [US1-US3] Background CUA proves A07 numeric grid and formula bar, full attachment names and actual product open/download; retain screenshots, AX, source values and file hashes. Promote the stable path to a deterministic regression using an admitted existing test engine.
- [ ] T010 [US1-US3] Run three fresh candidate attempts each of A07/A01/A02/list-13 with independent content/execution/presentation verdicts; regress seven earlier passing cases; finish the 17 unfinished cases. Preserve the original paused A08 attempt as baseline. Compare A02/A07 median tool counts to 56/28 and separate approval wait where evidence permits.
- [ ] T011 [US1-US3] Record OUTCOME.md with Result, What landed, commands/counts, remaining model failures, Explicitly not claimed and requirement-by-requirement acceptance audit. Do not mark unfinished gates passed; do not stage/commit/push without a request.

## Final Verification Gate

- [ ] Every behavior change has its focused owning-package test and connected caller coverage.
- [ ] cargo fmt --all -- --check and applicable workspace CI checks are verified.
- [ ] No new crates.io A3S dependency, SDK binding, workspace, lockfile or native select.
- [ ] Product configuration remains ACL; sandbox/trust/read bounds remain enforced.
- [ ] Private Studio source remains in its submodule; unrelated dirty changes remain intact.
- [ ] Candidate CUA artifacts prove actual presentation and open/download, not only file existence.
- [ ] All required live attempts have independent verdicts; full scope and residual failures are reported honestly.
- [ ] OUTCOME.md links authoritative evidence for FR-001 through FR-009 and SC-001 through SC-004.

## Recovery note — 2026-10-23

The user authorized restoration after external root restore/clean removed implementation and evidence. This document was recovered from the original task patch. Prior baseline screenshots/logs are currently missing; historical summaries are not current proof. All implementation gates require new verification. Retained Studio changes and original evaluation inputs are preserved.

## V3 evidence-driven additions

- T003 additionally carries validated Office publication metadata separately from clipped ACP text; a large workbook must remain a real current product alongside its PNGs.
- T004 additionally exposes a bounded exact worksheet-path outline before cell detail, so output projection cannot silently hide the middle worksheet in ordinary multi-sheet reads.
- V2 diagnostics remain retained. T010 restarts all 36 required fresh attempts on a separately frozen V3; no mixed-version median or complete V3 acceptance is claimed.

## V8 execution-semantic follow-up

- V7 A01 repeat 3 exposed an incorrect successful comparison over process substitutions. Exact-sandbox controls retain distinct pipe contents and a failing read through an open descriptor after unlink in guest /tmp; see process-substitution-metadata-probe.log under the evidence directory.
- T005 is reopened: provide bounded guest-private /tmp storage, refuse conflicting template mounts, retain workspace restrictions and verify actual unlinked-file/diff behavior before freezing a successor candidate. This stays inside the existing per-call temporary-storage contract; no new public API, dependency or permission mode is added.
- V7 measurements remain diagnostic evidence. A successor cohort must use its own frozen identities and fresh ordinary inputs; medians never combine versions.

V8 follow-up qualification: owning tests 3 passed; connected OCI tests 11 passed/4 explicitly enabled separately; all four signed-HVF tests pass; seven real-guest controls verify 64MiB private /tmp, unlinked descriptor reads, different-file pipe diff, workspace persistence and credential refusal. See private-tmp-native-qualification.log, private-tmp-live-tests-results.json and candidate-v8-build-check-results.json. Final cohort and deterministic GUI gate remain open.
