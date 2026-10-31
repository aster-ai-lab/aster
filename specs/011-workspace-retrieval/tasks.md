# Tasks: Reliable workspace retrieval

**Input**: [spec.md](spec.md), [plan.md](plan.md)

**House rule**: Each behavior-changing task includes its test. Tasks execute sequentially because the retrieval and capability changes share contracts.

## Phase 1: Behavior and tests

- [x] T001 [US1] Repair stale local-provider gating in `crates/code/acp/src/agent.rs` and `agent/session_open.rs`; extend sibling local-provider/setup tests for actual availability, missing assets and no fallback. Preserve `unattended.rs` in-flight edits.
- [x] T002 [US2] Refactor `crates/code/core/src/workspace/retrieval/hybrid_query.rs` and `workspace/services.rs` so semantic runtime is optional, with no copied fusion pipeline. Include no-provider, degraded-provider, invalid/denied path, stale-source, cancellation and closed-session tests; retain source verification and current channel status.
- [x] T003 [US3] Implement bounded relevant source windows and explicit output-budget metadata in `crates/code/core/src/tools/builtin/hybrid_search.rs` (and existing shared renderer only when needed), with sibling tests for late match, CJK/UTF-8, long lines, full-chunk vs display anchors, and response bound.

## Phase 2: Wiring and qualification

- [x] T004 [US2] Update `tools/builtin/{search,mod}.rs`, session/API tests, permissions/tool-contract consumers, ACP retrieval prompt/seam tests and capability expectations. Preserve the tool name and per-mode authorization. Add real built-ACP stdio model/tool round-trip cases proving hybrid without semantic and named local-asset refusal.
- [x] T005 [US4] Add a fixed Aster Rust/TS/TSX/CJK relevance/late-match fixture; reuse Rust report/benchmark conventions for Lines/Recursive × RRF/MMR and measured lifecycle/provider/response costs. Run existing release qualification and decide/document P3/P4 promotion using observed failures rather than synthetic perfect recall. Address any demonstrated quality/resource defect and its regression test before completion, including the measured v3 sparse-key reopen fault in `workspace/retrieval/{persistent,aster_vec}.rs`.
- [x] T006 [US1–US4] Add/check/run deterministic Aster Test terminal regression against the paired built Aster/ACP with the existing loopback model fixture pattern; run focused Clippy, root formatting, CI workspace checks and paired build. Reproduce unrelated failures separately, retain ownership/cleanup and sanitized reports. Check local real-model availability and qualify if admitted assets exist.

## Phase 3: Evidence

- [x] T007 [US1–US4] Update current Aster retrieval runbook/commands; write `research.md` decisions and `docs/evidence/code/2026-10-23-workspace-retrieval/OUTCOME.md` containing Result, What landed, exact commands/counts, baseline/after measurements, adversarial review, and Explicitly not claimed. Audit every FR/SC against current authoritative evidence.
- [x] T008 Initial qualification left changes reviewable and unstaged. The user's follow-up `提交代码` authorizes a scoped local commit; preserve unrelated pre-existing edits.

## Final Verification Gate

- [x] T001–T004 owning-package focused tests pass; counts and command logs retained.
- [x] T005 versioned quality/resource report, existing release gates, exact-identifier and stale-source invariants pass; conditional P3/P4 decisions recorded.
- [x] T006 real ACP stdio and Aster Test terminal flows pass and clean up owned processes.
- [x] All 25 owned Rust files pass scoped rustfmt and focused Clippy passes; full `cargo fmt --all -- --check` has independently reproduced base-only failures.
- [x] Relevant root CI workspace checks and paired Aster/ACP build pass, with any independent pre-existing failure explicitly reproduced rather than hidden.
- [x] No new external Aster dependency, native select, product configuration format, language binding, vector persistence, implicit download, or semantic opt-in.
- [x] T007 acceptance audit proves all requirements; real-model availability/non-claims are explicit.
