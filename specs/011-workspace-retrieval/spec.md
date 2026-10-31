# Feature Specification: Reliable workspace retrieval

**Feature Branch**: Existing working branch; no branch or commit requested.

**Created**: 2026-10-23

**Status**: Implemented and locally qualified; independent baseline CI failures documented

**Input**: Implement the agreed architecture optimization and end-to-end tests according to the development standards.

**Gate context**: Product regime, `crates/code/core`, `crates/code/acp`, and the existing Aster terminal. This epic implements the P0–P4 retrieval plan: working local enablement, an Aster baseline, optional semantic evidence, useful bounded snippets, and measured strategy/resource decisions.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Honest local enablement (Priority: P1)

A trusted host can explicitly enable the existing local BGE provider. Sessions without installed assets keep working and describe the named unavailability. Workspace configuration cannot grant this authority.

**Why this priority**: The current ACP refers to an undeclared `local-bge` feature despite already depending on the BGE engine.

**Independent Test**: ACP local-provider hermetic tests, configuration trust tests, and a shipped stdio boundary test. Missing/invalid assets cause no installation or remote fallback.

**Acceptance Scenarios**:

1. Given a trusted opt-in and an admitted provider, a session exposes semantic retrieval.
2. Given missing or changed assets, setup names the refusal and retains model-free search.
3. Given only an automatically discovered workspace opt-in, semantic retrieval remains disabled.

### User Story 2 - Useful retrieval without a semantic provider (Priority: P1)

The existing hybrid search combines available exact, lexical, and symbol evidence even when semantic retrieval is disabled or degraded. It reports the actual channels, checks the current source, and retains cancellation and workspace isolation.

**Why this priority**: Optional semantic infrastructure must not prevent the other retrieval channels from serving.

**Independent Test**: Core workspace and session tests with real temporary files and deterministic/failing providers, plus ACP tool-definition and stdio end-to-end tests.

**Acceptance Scenarios**:

1. Without a provider, hybrid returns verified lexical/exact hits and explicitly unavailable semantic status.
2. With a degraded provider, hybrid remains advertised and serves available evidence; semantic-only capability follows readiness.
3. A deleted, unreadable, stale, denied, or out-of-workspace source never reaches the rendered result.
4. Closing a session cancels work, refuses subsequent calls, and releases semantic vectors.

### User Story 3 - Relevant bounded evidence (Priority: P1)

Search output includes the actual matching code, including matches beyond the first sixteen lines, with accurate rendered anchors and a bounded response size.

**Why this priority**: Returning only a chunk's prefix can hide evidence the index already found.

**Independent Test**: Tool rendering tests for late matches, CJK, UTF-8, long lines, empty results, response bounds, and a fixture-driven agent/tool round trip.

**Acceptance Scenarios**:

1. A literal/token match near the tail of a chunk is present in the displayed contiguous window.
2. Rendered start/end lines describe displayed source; full chunk anchors remain separately available in metadata.
3. Output truncation is explicit and cannot split UTF-8 or expose unverified source.

### User Story 4 - Measured quality and resource choices (Priority: P2)

Maintainers can reproduce a versioned Aster retrieval comparison and end-to-end qualification before promoting a chunking, reranking, or resource default.

**Why this priority**: Synthetic perfect embeddings and upstream measurements do not establish Aster performance.

**Independent Test**: Existing release benchmarks plus a focused versioned Rust/TS/TSX/CJK fixture and deterministic stdio/TUI tests. Report query and lifecycle timing separately, source/chunk/dimension counts, evidence quality, response bytes/tokens when measurable, and provider input/request counts.

**Acceptance Scenarios**:

1. Lines/Recursive and RRF/MMR are compared one variable at a time on fixed relevance labels.
2. Exact-identifier behavior and source/lifecycle invariants do not regress.
3. AST chunking, alternative embeddings, or changed CPU limits ship only if an observed failure or measured resource bottleneck justifies them; retain the measured decision when no change is warranted.
4. End-to-end tests execute the built ACP and Aster entry points, with hermetic model fixtures for repeatability and explicit reporting of real-model qualification availability.

### Edge Cases

- Index building/empty catalog: report partial evidence; never claim semantic readiness.
- Concurrent edit/rename/delete or lagged watcher: reject stale ranges or return a revision-changed result.
- Invalid query, limit, include, path, canceled/closed session: preserve typed named refusals.
- Large/secret/generated/binary source and budget exhaustion: preserve source admission and bounded work.
- Poisoned provider output, changed model identity, failed inference: isolate the semantic channel and preserve cleanup.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: ACP MUST use its existing local embedding adapter after trusted opt-in and MUST refuse missing/invalid assets without downloads or remote fallback.
- **FR-002**: Hybrid MUST combine available channels without requiring an embedding provider and MUST disclose unavailable/degraded channels.
- **FR-003**: Public search names, Rust APIs, path normalization, per-mode permission checks, source verification, revision fencing, and cancellation MUST remain intact; invalid/unauthorized calls MUST be refused.
- **FR-004**: Rendering MUST select bounded relevant source windows, preserve exact line anchors and UTF-8, and explicitly report truncation; unverified source MUST NOT be rendered.
- **FR-005**: Semantic vectors MUST remain session-local and be released on close; the epic MUST NOT add implicit persistence or cross-tenant sharing.
- **FR-006**: Aster-specific versioned quality/resource reports MUST compare existing strategies before defaults change; unsupported superiority claims MUST be refused.
- **FR-007**: Tests MUST exercise real Core/session/ACP/terminal paths, including disabled/degraded/canceled cases; deterministic providers MUST NOT be represented as real-model quality evidence.
- **FR-008**: Build/packaging/CI/docs MUST agree with shipped capabilities. Builds MUST use the initialized Studio submodule and pinned root workspace; internal Aster dependencies MUST stay local.

### Key Entities

- `WorkspaceChunkCatalog`: Core's sole owner of source ranges, IDs, digests, and revisions.
- `WorkspaceServices`: Existing workspace capability boundary and hybrid orchestration entry point.
- `WorkspaceRetrievalRuntime`: Optional session-owned semantic projection and provider lifetime.
- Rendered source window: A verified contiguous subset with its own accurate display anchors.

## House Checklist *(mandatory — from Agents.md)*

- [x] New behavior has a small hermetic test in the owning package (T001–T005).
- [x] Closed sets retain existing enums; any new enum gets parse/as_str (T001–T005).
- [x] No new unexplained production unwrap/expect (T006).
- [x] Internal dependencies remain local (T006).
- [x] Product configuration remains ACL parsed by aster-acl (T001, T004).
- [x] Evidence target: `docs/evidence/code/2026-10-23-workspace-retrieval/OUTCOME.md` (T007).

## Success Criteria *(mandatory)*

- **SC-001**: Focused Core retrieval, search, permissions, session, and ACP seam tests pass with exact counts recorded.
- **SC-002**: Built ACP stdio and Aster terminal end-to-end cases pass without a live external model dependency.
- **SC-003**: Versioned quality/strategy and release resource reports are retained; exact-identifier and stale-source gates pass.
- **SC-004**: `cargo fmt --all -- --check`, focused Clippy, relevant root CI workspace checks, and paired Aster/ACP build pass, or any pre-existing failure is reproduced independently and explicitly recorded without claiming the failing gate passed.

## Assumptions

- Changes remain in the existing checkout and preserve unrelated in-flight edits.
- Existing line chunking and RRF-only compatibility defaults remain until measurements justify a change.
- Real local-model qualification is attempted only with already installed/admitted assets; startup does not install them.

## Explicitly Not Claimed

- No vector database service, ANN index, implicit vector persistence, multi-repository search, automatic remote clone, or new language binding.
- No automatic semantic opt-in or remote embedding route.
- No Semble speed/token equivalence or universal quality claim from synthetic fixtures.
- No commit, push, deployment, or edits to other tasks' dirty files.
