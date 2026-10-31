# Implementation Plan: Reliable workspace retrieval

**Branch**: Existing branch | **Spec**: [spec.md](spec.md) | **Input**: [spec.md](spec.md)

**Gate context**: Root-workspace product regime; Core retrieval, ACP integration, and the built Aster terminal.

## Summary

Repair ACP's stale local-provider compile gate, reuse the existing hybrid orchestration with optional semantic runtime, expose honest per-channel capabilities, and render useful bounded source windows. Extend existing tests and benchmarks for Aster and use measurements to decide P3/P4 promotion. Keep the source catalog, public APIs, permission checks, and ephemeral semantic lifecycle.

## Constitution Check

| Principle | Status | Evidence in the plan |
| --- | --- | --- |
| I. Fail closed | PASS | Named missing-asset/channel refusals; verified source and cancellation retained |
| II. Evidence retained | PASS | T007 writes commands/counts/non-claims under docs/evidence/code/2026-10-23-workspace-retrieval |
| III. Hermetic tests | PASS | Every behavior task includes Core/ACP tests; real entry-point fixtures |
| IV. Closed sets | PASS | Existing channel, phase, mode enums; no inferred intent/name classification |
| V. One workspace / published APIs | PASS | Root Cargo workspace, initialized Studio gitlink, preserved Rust API signatures |
| VI. Local schema changes | N-A | No durable schema migration planned |
| VII. Shrink-only ratchets | PASS | No allowlist changes |
| VIII. Replaceable transports | PASS | EmbeddingProvider stays host-injected; no model dependency in Core |
| IX. One configuration language | PASS | Existing trusted workspace_retrieval ACL |
| XIII. One repository | PASS | Existing checkout; no new repository |
| XIV. Names are not guessed | PASS | Only query text lexical ranking and explicit symbol metadata |
| XV. Meaning is not matched | PASS | Explicit search mode; no intent classifier |
| XVI. Pickers are drawn | N-A | No UI picker changes |
| XVII. Dialogs stay in place | N-A | No UI dialog changes |

## Project Structure

### Documentation (this feature)

`specs/011-workspace-retrieval/{spec,plan,tasks,research}.md`; acceptance lives only in `docs/evidence/code/2026-10-23-workspace-retrieval/`.

### Source Code

| Layer | Path | Planned work |
| --- | --- | --- |
| Core | crates/code/core/src/workspace/retrieval, workspace/services.rs | Reuse one hybrid implementation with optional semantic input and existing lifetime/budget controls |
| Tool/API | crates/code/core/src/tools/builtin, agent_api, permissions | Independent hybrid availability, relevant windows, output budget, callers/tests |
| ACP | crates/code/acp/src/agent, tests | Repair local provider gate; honest prompt/tool declaration and real stdio tests |
| Terminal | tests/e2e, existing ACP test harness | Aster Test regression over built Aster/ACP fixture flow |
| Qualification | crates/code/core/examples, tests/fixtures | Reuse existing benchmark/report patterns for fixed Aster corpus and strategy/resource comparison |
| Docs/CI | crates/code/manual, .github/workflows/ci.yml if needed | Current Aster commands, capabilities, focused admission checks |

## Research

- `workspace/retrieval/hybrid_query.rs` already owns four-channel fusion, revision fences, source verification and cancellation. Refactor it once rather than duplicating a no-provider implementation.
- `workspace/services.rs` is the public workspace boundary; retain its method signatures and select the existing optional semantic runtime there.
- `tools/builtin/search.rs` and `tools/builtin/mod.rs` currently conflate hybrid and semantic availability; separate those capability predicates, update permissions/prompt consumers and tests together.
- `tools/builtin/hybrid_search.rs` currently displays a fixed prefix. Use the existing canonical lexical tokenizer for ranking source lines; literal matching is ranking, never name/intent inference. Reuse bounded UTF-8 truncation and rendering conventions.
- `acp/src/agent/local_embedding.rs` already validates asset revision, output shape and bounds. The dependency is compiled already; remove the undeclared feature gate rather than add another optional surface.
- Existing `workspace_retrieval_benchmark.rs`, `workspace_persistent_index_production.rs`, and `test_workspace_retrieval_real_llm.rs` provide resource/strategy patterns. Extend/reuse them; keep deterministic and real-model evidence distinct.
- `tests/e2e` and ACP stdio tests provide real entry-point test patterns. Aster Test TUI ACL owns terminal execution and cleanup.
- P3/P4 are measured decisions: first compare Lines/Recursive and RRF/MMR, provider call amplification and lifetime memory; add AST/runtime/dependencies only when the evidence establishes a specific unmet requirement.

## Verification Plan

- Focused Core retrieval/search/session/permission tests; ACP local embedding, retrieval seam, workspace trust and stdio end-to-end tests.
- Existing release retrieval and persistent-index qualification; versioned Aster quality/strategy report and bounded late-match output proof.
- Aster Test ACL check/run against the actual built terminal; real ACP fixture server captures tool invocations and asserts evidence reaches the next model request.
- Root `cargo fmt --all -- --check`, focused Clippy, CI workspace scripts, paired `cargo build --locked -p aster -p aster-acp`.
- Existing installed local-model qualification when assets are available; never auto-download or claim deterministic providers establish real semantic quality.
- No live PostgreSQL, remote model billing, unrelated full-workspace test campaign, commit, or deployment.
