# Feature Specification: Workspace-retrieval benchmark suite (Semble-class)

**Created**: 2026-10-23 · **Status**: Draft

**Input**: User directive — build an evaluation mechanism for aster's
retrieval capability modeled on MinishLab/semble's `benchmarks/` design.

**Gate context**: Product regime. Root python harness (`tests/retrieval_bench/`)
driving the built aster Core via the existing `workspace_retrieval_aster`
example binary (real engine, no model in the loop for retrieval itself).

## What Semble's design gives us (measured, not copied)

| Semble mechanism | This suite's counterpart |
| --- | --- |
| ~1,250 queries / 63 repos / 19 languages, pinned by revision | A pinned-repo corpus starting with THIS repository + 2–5 external pinned repos (Rust, TS, Python, Go, CJK-bearing), extensible by `repos.json` |
| 3 query categories: semantic (behavior), architecture (design), symbol (lookup) | Same taxonomy; generated per-repo from the codebase's own identifiers + docs, frozen as fixtures with labels |
| NDCG@10 primary; per-language/category cuts | Same metric family; computed offline by the harness over frozen graded labels |
| Token efficiency: tokens-at-first-hit vs grep+read, recall@budget | Same comparison: aster search_snippet vs ripgrep+read on the same queries, tokenized by chars/4 (no tiktoken dep) |
| Speed: cold index time, warm p50 latency | Same, measured against the built binary (the engine is Rust; CPU-only parity) |
| Baselines: ripgrep, probe, cs, CodeRankEmbed, ablations | Baselines: ripgrep (PATH), pure-BM25 ablation (engine's lexical-only mode), hybrid ablation, hybrid+semantic |
| Results keyed by commit SHA | Same: `results/<suite>-<sha12>.json`, resume-on-rerun |

## User Stories & Testing

### US1 — Frozen graded corpus (P1)
The harness ships a `repos.json` (pinned revisions), a frozen
`queries.jsonl` (id, repo, category, query, graded relevance: chunk-id →
grade 0–3), and a sync step that materializes the corpus at the pinned
revisions. Labels are LLM-generated ONCE (Claude judgment, Semble's
method) and then frozen — the benchmark never calls a model at run time.

**Independent test**: corpus integrity — every query's graded chunks
exist in the pinned corpus digests.

### US2 — Quality benchmark (P1)
`run_quality.py` drives the built `workspace_retrieval_aster` binary per
query (hermetic engine, batch mode), parses hits, and computes NDCG@10,
MRR, Recall@5, per-category and per-repo cuts, for each baseline
(ripgrep, lexical-only, hybrid, hybrid+semantic when a provider is
admitted).

**Independent test**: a 3-query synthetic mini-corpus with known ranks —
the metric code itself is unit-tested (NDCG of a perfect ranking = 1.0,
a reversed ranking scores below chance, ties handled).

### US3 — Token efficiency (P1)
`token_efficiency.py` replays each query through two workflows —
(a) aster snippet path, (b) simulated ripgrep+read (grep -n + read
whole matching files) — and reports tokens-at-first-relevant-hit
(chars/4, 32k miss penalty) and recall at 500/2k/8k/32k budgets.

**Independent test**: a fixture where the answer sits in line 900 of a
long file — snippet path must hit under budget while read-file must not.

### US4 — Speed benchmark (P2)
`speed.py` measures cold index (first query per repo) and warm p50/p95
latency (next 20 queries), CPU-pinned via `taskpolicy` on macOS /
`taskset` when available.

### US5 — CI gate + regression posture (P2)
Quality runs hermetically in CI on the mini-corpus with a floor
(lexical-only NDCG@10 ≥ floor for symbol queries — exact identifier
hits must never regress). Full-corpus runs stay nightly/manual
(`--output` resume).

## Requirements (fail-closed)

- **FR-001**: The retrieval engine under test is the BUILT Core binary
  (no Python reimplementation); the harness only feeds queries and
  parses JSON.
- **FR-002**: Labels, repos, and revisions are frozen fixtures; a label
  drift is a deliberate commit. A missing repo/label refuses the run
  with the named gap.
- **FR-003**: Baselines run the same queries; ripgrep absence skips that
  baseline loudly, never silently.
- **FR-004**: No model calls at benchmark time; the one-time label
  generation is recorded as evidence with its prompt and judge.
- **FR-005**: Results are keyed by engine commit SHA and suite name; a
  rerun resumes and never clobbers foreign results.

## Explicitly not claimed

- No 19-language parity on day one (the corpus grows by `repos.json`).
- No LLM-as-judge at run time.
- No new embedding model selection (specs/011's measured decision —
  Lines/RRF defaults — stands until this benchmark measures otherwise).
