# Command latency investigation and candidate optimization

Status: candidate implementation and initial qualification only. A complete
T01 model rerun and product deployment remain pending.

## Measured result

The same three sequential `printf phase-probe` commands, each in a fresh,
isolated workspace utility VM, averaged 13,977.603 ms with the original cached
companions and 3,894.375 ms with independently staged candidate companions
(72.14% lower in this small sample). This is not an end-to-end T01 speed claim.
See `baseline.json`, `candidate.json`, and their corresponding logs.

Baseline mean phases were host connect 2,460.785 ms, bundle prepare 375.643 ms,
VM create 10,841.226 ms, process start 61.487 ms, collect/wait 71.671 ms,
VM delete 162.393 ms, host shutdown 0.932 ms, and scratch removal 1.556 ms.
The candidate host connect and VM create means were 447.637 and 2,717.766 ms.

Runtime inspection found repeated SHA-256 verification of libraries, firmware,
kernel and system image before VM entry. CPU sampling of the original shim's
non-booting context smoke showed software SHA compression on the main thread.
The candidate enables the existing SHA dependency's Apple Silicon assembly
backend and retains the original verification and CPU fallback behavior.

## Implementation scope

- Add metadata-only core tracing for connect, bundle preparation, create,
  start, collect/wait, delete, shutdown and scratch removal.
- Add an explicitly ignored real-HVF timing probe, with checked output,
  status and timeout results.
- Bring the required runtime build closure into the root Cargo workspace,
  retaining local unpublished A3S dependencies and the core/SDK public APIs.
  Restore the host/agent/shim version contract matching the existing 0.3.6
  guest image; the imported core/SDK remain 0.3.2.
- Restore the host's opt-in stdin-EOF lifetime contract expected by Code.
  EOF, signal and errors use the existing VM shutdown path. An independent
  standard thread avoids an uninterruptible Tokio stdin read.
- Repair source-import compile gaps in console identity transfer, confinement
  reports and owner monitoring without removing the underlying checks.

The original cache at
`/Users/roylin/Library/Caches/aster/sandbox/dev-20261018-core-bash` was not
replaced. Candidate binaries were independently copied and signed under
`target/sandbox-latency-assets`; the current application's sandbox selection
has not been changed. No Box daemon or process-host fallback was introduced.

## Validation performed

From the repository root, using the pinned toolchain:

- `cargo test --locked -p a3s-code-core --lib real_command_phase_timing -- --ignored --nocapture --test-threads=1`
  with `ASTER_TEST_OCI_ASSETS` pointing first at the original cache and then
  at `target/sandbox-latency-assets`: 1 test and 3 real VM samples passed in
  each run. Raw logs include the completed phase timings.
- `cargo build --locked --offline -p a3s-oci-cli -p a3s-oci-krun --bins`:
  passed from the root workspace.
- `cargo test --locked -p a3s-code-core --lib sandbox::oci::tests::real_ -- --ignored --nocapture --test-threads=1 --skip real_command_phase_timing`
  against candidate assets: 3 passed in 12.84 seconds. Cases verify streaming
  and exit receipts, credential/hardlink and Git write protections, timeout
  of a `setsid` descendant, cancellation, and no subsequent counter writes.
- Focused runtime qualification: 39 checks passed, covering EOF (5), CLI
  lifecycle (2), reports (1), owner monitoring including real parent death
  and neighboring-group survival (3), pinned console transfer (12), Seatbelt
  (1), macOS asset rejection (7), manifests (7), and changed-runtime refusal
  (1). See `integration-checks.json` for exact commands and source identities.
- Focused `git diff --check`: passed.
- Focused formatting on the 12 runtime Rust files passed. The full workspace
  formatting gate still reports 66 unrelated/imported-tree differences.
- `/opt/homebrew/bin/python3.13 tests/workspace_dependencies.py`: 4 passed,
  2 existing failures (`apps/work/.git` and gateway publish metadata).

## Outstanding plan

First finish runtime qualification and expand the repeated command benchmark.
Then optimize task-independent tool orchestration, preserving required
verification and model-driven tool selection. Independently repair the
configured workspace retrieval declaration/provider path; plain grep is
already available through `search(mode="grep")` and does not need embeddings.
Finally repeat T01 and related input variants, report distributions and exact
result correctness, and only then update the development application.

## Explicitly not claimed

- No complete optimized T01 model execution, latency distribution over 19
  commands, or statistical end-to-end improvement has been established yet.
- No current application or installed release was switched to the candidate.
- No Windows/Linux runtime qualification or clean full-workspace gate is
  claimed. Source-build integration was necessary because the original cached
  runtime did not match the imported CLI's lifecycle interface.
- No integrity check was removed; no VM reuse, reduced isolation, privileged
  host execution, answer cache or task-specific filename branch was added.
- Workspace semantic/hybrid retrieval and code intelligence were investigated
  but not changed or newly provisioned in this optimization slice.
