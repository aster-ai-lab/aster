# Workspace retrieval decisions

## Starting evidence

- ACP's BGE dependency already enables `bge-m3`, but `agent.rs` and `session_open.rs` require an undeclared `local-bge` cfg. The existing validated adapter is unreachable in normal builds.
- `WorkspaceServices::hybrid_search` refuses without a semantic runtime. `SearchTool` advertises/removes semantic and hybrid together. The existing exact/BM25/symbol implementation can work without an embedding function.
- Hybrid rendering always selects the first sixteen lines, even when the matching source lies later in the verified chunk.
- The existing semantic in-memory lifecycle is intentional. Durable lexical indexing already exists. No semantic persistence or second index authority is justified by these findings.
- Upstream manual references to other SDKs, old standalone commands, and legacy benchmark environments are provenance, not Aster qualification evidence. Aster commands run from the initialized root workspace.

## Measurement decisions

The versioned reports and final P3/P4 decisions are retained below. Defaults stay Lines/RRF; no dependency or model was selected without evidence.

## Isolation and intermediate qualification

The original checkout contains concurrent changes outside this epic. A root
format check observed a transient parse error in the other goal-control work;
the initial paired build was canceled by its exact owned process identifiers.
Other tasks and their Cargo processes were not stopped. An independent detached
worktree inside this repository, `.codex-worktrees/retrieval`, isolates the 35
owned files on base `ce0f7b14c`, with Studio initialized at its tracked
`08e4e90657b37cb32b74a0455b15e4d4b7fb71e4` gitlink. No branch/commit was created.
Original-file digests are retained outside the tracked tree so final sync refuses
to overwrite any concurrent edits. Qualification uses `CARGO_INCREMENTAL=0`.

- Core focused verification (full workspace retrieval, search, session and
  permission filters): 307 passed, 0 failed; 26.48 seconds test execution.
- Earlier root ACP focused verification: 26 passed, including the ten formerly
  cfg-disabled local adapter cases, actual configuration trust, setup and
  degraded-provider hybrid execution.
- All 21 owned Rust files pass their rustfmt check. Root formatting still names
  existing goal-control/fact-control/unattended files outside this epic, which
  are unchanged relative to the isolated base.
- Original-root internal dependency guard: 6 passed using installed Python 3.13
  (the system Python lacks tomllib). CLI paired-delivery fault injection:
  10 passed.

## Read-only consistency review

| Finding | Severity | Requirement/task | Resolution |
| --- | --- | --- | --- |
| No constitution violation or uncovered functional requirement found | note | FR-001–FR-008 / T001–T007 | Continue implementation and qualification; unchecked tasks are not completion evidence |
| Real semantic quality cannot be inferred from the matrix orthogonal document/query vectors | note | FR-006 / T005–T006 | Keep explicit control labeling and separately inspect installed model readiness |
| Global format issues belong to the base/concurrent work | warning | SC-004 / T006 | Retain full and owned-file checks; do not change unrelated files or claim a green full-tree gate |

Verdict: ready for remaining qualification; not a completion verdict.

## T005 measured persistent-index defect

The unchanged release production example failed twice after a single edit and
restart: `aster-vec lexical result returned an unknown primary key`. The v3
manifest already persists sparse engine keys, but `load_current` opens the
adapter with regenerated dense keys and only restores the manifest keys into
the separate state afterwards. A second edit can trigger a full rebuild and
hide the fault, which is why the existing multi-edit test missed it.

T005 therefore includes restoring the existing exact persisted mapping into
the adapter, rejecting missing/duplicate v3 keys, preserving v2 dense-key
compatibility, and a single-edit reopen assertion in the existing regression.
No new schema or semantic persistence is introduced.

## P3 strategy decision

| Chunking | Ranking | Chunks | Recall@5 | MRR | Document requests | Accounted vector bytes |
| --- | --- | ---: | ---: | ---: | ---: | ---: |
| lines_80 | rrf_only | 7 | 1.0000 | 1.0000 | 1 | 826 |
| lines_80 | deterministic_mmr | 7 | 1.0000 | 1.0000 | 1 | 826 |
| recursive_512_64 | rrf_only | 9 | 1.0000 | 0.9375 | 1 | 1032 |
| recursive_512_64 | deterministic_mmr | 9 | 1.0000 | 0.9375 | 1 | 1032 |

All four profiles retain the eight labeled relevant sources, three explicit
exact-identifier first-place gates, and the empty negative control. Recursive
places the split-name query second, which remains visible in the report rather
than being relabeled as an exact identifier. First-place gates are authored
explicitly; no query/name heuristic selects them. MMR adds no quality gain on
this small control corpus. Keep Lines/RRF; the measurements justify no AST
dependency, global MMR promotion or new embedding model.

The legacy-prefix display ablation loses the line-64 source in all profiles;
the current rendered window retains it. The corpus is a controlled regression
set, not a representative real-model benchmark or a Semble comparison.

## P4 resource decision

The adapter readiness command reports local BGE assets absent. No weights were
downloaded, and no real BGE quality/CPU result is inferred from the orthogonal
control. Each profile uses one document provider request, at the batching lower
bound; close returns vector records/bytes to zero. Keep the current CPU/batch
limits and ephemeral projection. Final release resource gates and RSS results are retained separately. The
persistent restart fault was repaired and passes both the single-edit regression
and the unchanged 512-file production example.

## Final release verification

The unchanged v5 release gates pass: exact 25,000×384 P95 4.734625 ms
(30 ms ceiling); 4-file/512-chunk hybrid RRF P95 3.330917 ms and MMR
3.821708 ms (100 ms ceilings); added rerank P95 0.490791 ms (10 ms ceiling).
Physical document requests are 8 for 512 inputs, matching the batching lower
bound (amplification 1.0). The first contended run's RRF P95 140.074625 ms
remains in `retrieval-contended.json`; it is not a before/after code-speedup
comparison. For the final sequential direct-binary measurements, only this
task's compiler group was paused and then resumed in a `finally` block.

The persistent example passes with 512 files/chunks, 128 concurrent queries,
same-byte generation reuse, changed-byte publication, ten restart hits and one
retained generation. Peak process RSS is retained in macOS bytes: 23,379,968
(matrix), 187,023,360 (v5 retrieval), and 33,980,416 (persistent). These include
the whole deterministic process, not BGE model weights or a vector-only budget.

The matrix fixes the Portable lexical engine; native aster-vec is separately
covered by the locked v5 resource, persistence, and Core contracts. The matrix
does not establish representative native-engine or real semantic quality.

## Final acceptance review

Core: 308 passed (19.80s); ACP: 26 passed (0.57s); actual stdio: two passed
(5.94s); final paired Aster terminal: one passed (21.47s, eight ACL steps,
no cleanup error). Scoped rustfmt of all 25 Rust edits, focused Clippy, optional
harness check, release examples and paired native build pass. Both native
executables have the same source identity recorded in native-pair.json.

FR-001–FR-008 and SC-001–SC-004 are covered by the final OUTCOME audit. Full
workspace fmt/Studio failures and native-macOS Kylin fixture failures remain
explicit, independently reproduced baseline limits. This is no waiver or green
full-tree CI claim; no commit or release was requested. No scoped blocker or
new unresolved requirement remains. Original-file hash checks protect every
owned file during sync. The working change remains unstaged for review.
