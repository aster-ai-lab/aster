# Aster workspace search qualification

Run commands from the initialized root Cargo workspace. The current operations
contract and focused commands are in [WORKSPACE_RETRIEVAL_OPERATIONS.md](WORKSPACE_RETRIEVAL_OPERATIONS.md).

The existing `workspace_persistent_index_production` example exercises real
manifest discovery, deferred catalog admission, durable aster-vec generations,
concurrent verified readers, identical-byte reuse, changed-byte publication,
generation collection and reopen after restart:

```sh
cargo run --locked --release -p aster-core --example workspace_persistent_index_production
```

The existing `workspace_retrieval_benchmark` retains the locked separate exact
vector and hybrid profiles, provider batching, rerank resource and close gates:

```sh
cargo run --locked --release -p aster-core --example workspace_retrieval_benchmark
```

The Aster matrix adds fixed Rust/TS/TSX/Chinese/doc/test cases, a late line-64
match, and a negative control. It compares Lines/Recursive × RRF/MMR through
public session APIs and actual tool rendering, checks provider request counts
and zero vectors after close, and exits nonzero when an invariant fails:

```sh
cargo run --locked --release -p aster-core --example workspace_retrieval_aster
```

Its orthogonal document/query control cannot create semantic recall. Response bytes and the
legacy-prefix display ablation are reported without claiming actual model
token savings. Profile timing and machine identity accompany results.

The stdio and Aster Test terminal regressions execute the actual built ACP/Aster
against a deterministic loopback model. The model fixture confirms that the
verified source evidence reaches its next request before returning the visible
completion marker. The missing-asset stdio case also proves that trusted local
opt-in fails with the actual asset reason and retains model-free hybrid.

Evidence for this epic belongs to
`docs/evidence/code/2026-10-23-workspace-retrieval/`, separately from its
`specs/011-workspace-retrieval/` planning artifacts. Historical upstream reports
remain provenance and do not establish performance for this Aster build.
