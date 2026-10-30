# Aster workspace retrieval operations

This is the current Aster deployment contract. Historical upstream qualification
is retained in `WORKSPACE_RETRIEVAL_QA.md` and `WORKSPACE_RETRIEVAL_BASELINE.md`;
those SDK/CLI environments and measurements are not Aster release evidence.
Aster uses the initialized root Rust workspace and preserves the public Rust API.

## Ownership and enablement

Core owns admission, deterministic chunks, IDs, digests, versions, rank fusion,
source verification and bounded output. `WorkspaceServices` combines exact,
lexical, optional Code Intelligence and optional semantic channels. A missing or
failed semantic channel does not disable hybrid search on a catalog-capable
workspace. Backends without reads or a catalog do not advertise hybrid.

Core accepts a host-owned embedding provider through its public Rust trait.
ACP semantic retrieval is default-off and requires this declaration in trusted
user/explicit ACL:

```acl
workspace_retrieval {
  enabled = true
  local_cpu {}
}
```

Automatically discovered workspace ACL cannot grant this capability. With the
trusted opt-in, the shipped stdio agent loads the in-tree light tier from embedded
assets. An internal Rust composition seam can install a factory that takes
precedence; it is not an ACP metadata or environment-variable loader. This default needs no separate
BGE installation, environment factory variable or model download. A registered
factory that returns no provider refuses explicitly; it does not fall back to
the light tier. Embedded-asset load failures also report a named setup error.
Grep, glob, BM25 and model-free hybrid remain usable. No remote provider is
implicitly activated. Provider identity and actual dimensions come from the
provider descriptor rather than an assumed model shape.

The existing Rust host contract remains
`SessionOptions::with_workspace_retrieval(WorkspaceRetrievalOptions::new(provider))`.
Provider or strategy changes require a new session. A local model client may be
reused by the host; each semantic projection remains owned by its session.

## Source and lifetime boundaries

Only admitted UTF-8 text enters chunking or embedding. Sensitive/control paths,
generated content, binary and oversized assets remain excluded. Non-text
knowledge compilation is a separate system. Each returned source file is
reread once per query and checked against the captured complete-file digest
and exact chunk bytes. Revision races return an explicit partial/retry reason.
Do not remove verification or reuse an unchecked source cache for latency.

Lexical generations may persist through the existing a3s-vec projection.
Existing v3 manifests retain their engine primary keys across incremental
edits and restart. Missing or duplicate keys refuse the projection until the
catalog rebuilds it; legacy v2 dense-key generations remain readable.
Semantic vectors remain in A3S Memory's session-local in-memory index. Closing
a session cancels its work, refuses later session queries and clears its
vectors. This release adds no semantic persistence or cross-tenant sharing.

Hybrid gives lazy catalog admission a bounded first-query opportunity (at most
one second, also limited by the workspace operation timeout). Session creation
never waits for a corpus. A still-unpublished catalog reports `building`;
semantic `disabled`, `building`, `ready`, `degraded` and `closed` remain distinct.
The semantic-only tool follows its live phase; hybrid retains usable channels.

## Ranking and output contract

Line chunking and RRF-only remain the compatibility defaults. The existing
recursive splitter and deterministic MMR are explicit Rust options. Promotion
requires a versioned comparison, including exact identifiers, CJK, docs,
test-code discovery, duplicate evidence and authoritative source verification.
Do not infer symbol definitions or query intent with name/verb heuristics.

Ranked semantic and hybrid output now selects a relevant contiguous window
using the canonical lexical terms and literal query, with a prefix fallback
when semantic evidence has no lexical overlap. Windows contain at most sixteen
lines and each displayed line has a 500-byte escaped-text bound. Long lines
retain the matching neighborhood. Rendered response text has a 24 KiB budget.

- `start_line` / `end_line` describe displayed source lines.
- `chunk_start_line` / `chunk_end_line` retain full verified chunk anchors.
- `snippet_truncated` identifies a shortened window or line.
- `output_budget_reached` identifies omitted results; narrow the query/path.
- `returned_results`, result metadata and source anchors describe the same
  displayed results. There is no fabricated continuation cursor.
- Hybrid's `lexical_engine` identifies the actual catalog query engine;
  semantic status describes its own configured projection. Per-channel and
  global fallback metadata describe partial coverage.

## Qualification and rollback

Run from the repository root after Studio is initialized:

```sh
cargo test --locked -p a3s-code-core --lib -- tools::builtin::search workspace::retrieval::tests::hybrid_query agent_api::retrieval_tests permissions:: --test-threads=4
cargo test --locked -p a3s-code-acp --lib -- local_embedding retrieval_seam session_setup execution_capabilities --test-threads=4
cargo build --locked -p aster -p a3s-code-acp
cargo test --locked -p a3s-code-acp --test shipped_tui_e2e retrieval::shipped_retrieval_stdio -- --test-threads=1
cargo run --locked --release -p a3s-code-core --example workspace_retrieval_aster
cargo run --locked --release -p a3s-code-core --example workspace_retrieval_benchmark
cargo run --locked --release -p a3s-code-core --example workspace_persistent_index_production
```

A3S Test terminal qualification uses the paired built binaries and a loopback
model fixture, without model billing or a Web driver:

```sh
a3s-test check tests/e2e/workspace-retrieval.acl --json
cargo test --locked -p a3s-code-acp --test shipped_tui_e2e retrieval::shipped_retrieval_a3s_test_terminal_round_trip -- --ignored --exact --nocapture
```

The fixed Aster matrix uses orthogonal document/query vectors to isolate
lexical/ranking behavior; it is not real semantic quality evidence. Keep fixture hashes, source/chunk
counts, model dimensions, response bytes, batching counters and timing scope
with every report. Never convert response characters into claimed actual
model tokens, or combine independent vector and hybrid workload latencies.
Keep existing release-profile thresholds unchanged. Deterministic Core/ACP
contracts run in CI; local CPU model qualification needs installed admitted
assets and must be reported separately.

To disable semantic retrieval, set `enabled = false` in trusted ACL (or use
`without_workspace_retrieval()`), close affected sessions, verify zero vector
records/bytes, and recreate sessions. Model-free search continues. To roll back
ranking/output behavior, use the last qualified paired binaries. Preserve the
redacted reports, error classes and fixture/version identities; never add
source, prompts, credentials or vectors to production telemetry.
