# Read-only Git query stalls

Status: implementation, regression, focused lint, and paired native build
verification complete. Existing check failures are recorded below.

Follow-up: the check blockers below were subsequently repaired in
[Windows check blockers](../2026-10-30-remaining-checks/OUTCOME.md). The measurements
and binary identities in this record describe the earlier build.

## Confirmed cause and change

`GitTool::execute` unconditionally took a full mutation snapshot before every
Git operation, then another snapshot after success. On the successful
repository path exercised by the regression, each snapshot ran six host Git
queries and read dirty-file content for mutation stamps. The regression's
read-only `status`, `branch`, `remote` batch therefore started 36 extra Git
processes and performed six unnecessary mutation observations.

The real tool path was materially slower than the standalone Git provider in
the reported Windows workspace. This is the confirmed workload amplification
fixed here. The historical error `Git command timed out after 30000ms` was
recorded, but that exact failing subprocess was not reproduced in this run;
the evidence does not establish which of the status helper's four internal
Git commands originally exceeded its deadline.

The tool now uses its existing read-only capability declaration to omit the
mutation baseline and delta for read-only operations. Mutating operations
retain both observations and their path-ownership bookkeeping. There is no
timeout increase, fallback executable, retry, new dependency, or published
Rust API change. Existing process timeout/cancellation handling is unchanged.

The new regression and the preceding Codex discovery regression are included
in the existing Linux/macOS CI workflow.

## Regression proof

`cargo test --locked -p a3s-code-core --test git_read_queries -- --nocapture`
was run before the production change. It failed: 0 passed, 1 failed. Git's
Trace2 events showed 36 extra mutation-observation processes for the three
read-only operations.

After the change, the same regression passed: 1 passed, 0 failed. Its final
run completed in 2.40 seconds. The query batch uses the real registered Git
tool and real system Git in a current-thread Tokio runtime, matching ACP's
runtime flavor. The extra process count is now zero; the original dirty file
remains intact. Trace instrumentation lives only in an isolated test child.
The test uses a temporary repository with spaces and non-ASCII characters.

## Same-host workspace spot checks

A temporary Cargo example invoked the real `ToolExecutor` against the user's
workspace, first with a standalone `status`, then two batches of `status`,
`branch`, and `remote`. The example was removed after diagnosis. No model
request or remote Git operation was used.

| Invocation | Before | After |
| --- | ---: | ---: |
| Standalone status | 20.532 s | 4.434 s |
| First batch, status / full batch completion | 34.516 s | 3.551 s |
| First batch, branch | 28.541 s | 0.472 s |
| First batch, remote | 28.388 s | 0.403 s |
| Second batch, status / full batch completion | 25.070 s | 2.623 s |
| Second batch, branch | 19.758 s | 0.376 s |
| Second batch, remote | 19.645 s | 0.406 s |

All spot-check tool results had exit code 0. These are observed comparisons
on one changing development worktree, not a statistically controlled benchmark
or a guarantee that an arbitrarily slow Git repository cannot time out.

## Checks and existing failures

- Native Windows x64, Windows build 22631; pinned Rust
  `1.97.1-x86_64-pc-windows-msvc`; default package features.
- Studio was initialized before Cargo commands.
- `cargo test --locked -p a3s-code-core --test git_read_queries`: 1 passed.
- `cargo test --locked -p a3s-code-acp --test codex_account_stdio`: 2 passed,
  covering 8 directory-discovery scenarios with synthetic login data.
- `cargo test --locked -p a3s-code-core --lib tools::builtin::git::tests -- --test-threads=1`:
  16 passed, 1 failed, 5273 filtered out. Checkout path receipts/ownership,
  stash ownership, worktree operations, and remote credential redaction passed.
- The failure, `worktree_cleanup_effect_fails_closed_on_a_dirty_worktree`, was
  rerun alone and failed identically. It bypasses `GitTool::execute`, so the
  changed path is not involved. Git/provider worktree listings use forward
  slashes on Windows, while `created.path` uses backslashes; the existing
  assertion compares their rendered strings. The runtime correctly refused
  dirty-worktree removal. A separate read confirmed that the synthetic
  `uncommitted.txt` survived with its original contents. The exact follow-up
  is to compare the diagnostic
  against the provider's returned path representation, while retaining the
  dirty-worktree survival assertion. This assertion was not changed here.
- Changed Rust files passed direct `rustfmt --edition 2021 --check` and
  `git diff --check`.
- `cargo fmt --all -- --check` remains blocked by Windows OS error 206 before
  rustfmt launches. Package format checks report pre-existing differences in
  ACP `src/agent.rs` and Core `src/porcelain/windows_tests.rs`; those unrelated
  working-tree edits were not reformatted.
- The dependency guard's existing host limitation is retained in the
  [Codex fix record](../2026-10-30-codex-home-resolution/OUTCOME.md): host
  Python cannot import its `encodings` standard-library module.
- `cargo clippy --locked -p a3s-code-core -p a3s-code-acp --all-targets --no-deps`
  failed in unchanged existing targets: `examples/workspace_retrieval_benchmark.rs:312`
  borrows `rerank` after its earlier move; Windows `tests/test_mcp_stdio_soak.rs:82`
  lacks its `c_void` import; `src/llm/http/redirect_tests.rs:29,41` ignores I/O
  read amounts, which Clippy denies. Exact follow-ups are to retain a clone
  before the benchmark builder consumes its rerank options, import `c_void`
  in the Windows test, and handle bounded partial HTTP-header reads in the
  redirect fixture. None of these files was changed here.
- `cargo clippy --locked -p a3s-code-core --lib --test git_read_queries --no-deps`
  passed with existing warnings.
- `cargo clippy --locked -p a3s-code-acp --all-targets --no-deps` passed with
  existing warnings. No new finding was reported for the changed code.

## Paired build and binary smoke

`cargo build --locked -p aster -p a3s-code-acp` passed in the default debug
profile. Both binaries reported the identical source/target identity:

```text
aster-cli-pair-v1:f8a0f8243317428c3ac0cdb9f0ca6cabf9fa4557:x86_64-pc-windows-msvc
```

- `target/debug/aster.exe --version`: `aster 0.1.4 (2dd0dc2f973b)`.
- `target/debug/a3s-code-acp.exe --version`: `9.1.6`.
- Aster `--help` exited successfully.
- The newly built ACP was initialized with `HOME`, `CODEX_HOME`, and
  `A3S_CODEX_HOME` removed from its child environment. With the existing
  host opt-in, it discovered all 8 local Codex models through `USERPROFILE`.
  No model request was sent and no credential data was retained here.

SHA-256 of the reopened build outputs:

```text
03107DF97E2114A4D1F82413ABBC7EA854623A126D127CF92FD118BC3730E60D  target/debug/aster.exe
E3F1F0B43E857C34EE4A4441CC1F131395B684F1ADC22AEE681CF462EDE3AF68  target/debug/a3s-code-acp.exe
```

These are local source builds, including the working tree's other existing
edits. The previously installed official release was not overwritten. Run
`./target/debug/aster.exe` from the repository root to use this verified pair.

## Explicitly not claimed

- Reproduction of the historical 30-second failing Git subprocess or removal
  of legitimate Git deadlines.
- Native Linux/macOS execution, a fully passing workspace check, or a clean
  pre-existing working tree.
- A change to Git write authorization, source provenance, or another session's
  edits.
- A new official stable release or replacement of the installed stable pair.
