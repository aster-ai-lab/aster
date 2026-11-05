# Bounded local and manifest grep: end-to-end verification

## Final implementation

Local fallback scans and manifest-backed searches both run on the blocking
worker pool. Each path has a 30-second / 20,000-file scan budget and returns
partial results with `truncated = true`. Local workspace services also have a
30-second outer operation timeout. The existing cancellation guard and
per-file size limits remain in place. Candidate selection and automatic trigram
index construction run on the same blocking worker; regexes that cannot use
trigram pruning skip construction entirely.

Changed production files: `workspace/local.rs`, `workspace/manifest.rs`,
`workspace/services.rs`, and `workspace/grep_candidates/trigram_index.rs`.
Changed sibling tests: `workspace/local/tests.rs` and
`workspace/manifest/tests.rs`. ACP protocol coverage was added to
`crates/code/acp/tests/unattended_e2e.rs`.

Environment: Darwin arm64, rustc 1.97.1. The initial CLI and ACP were rebuilt
from the checkout with `CARGO_BUILD_JOBS=8 just build`, including the macOS
sandbox companions. No installed binary was used.

## Unit and static verification

- Scoped rustfmt passed on all changed Rust files.
- Local workspace tests: 43 passed, 0 failed.
- Manifest grep tests: 5 passed, 0 failed. Total: 48 passed.
- `cargo clippy --locked -p aster-core --all-targets --no-deps`: passed with
  exit code 0. Only pre-existing warnings in unchanged files were reported.
- `git diff --check` on changed files passed.

## Real end-to-end runs

- Full read-only prompt, `分析下 aster 中存在哪些垃圾文件`, using the freshly
  built CLI/ACP and `cursor/auto`: exit code 0 in 62.514 seconds. The final
  batch's searches completed in under 0.1 second each. This was a different
  model/session state from the earlier 263.154-second run and is not a
  controlled latency comparison.
- Immediate root-grep probe with the incident's query and glob: exit code 0
  in 103.383 seconds. The model reported that the requested grep timed out
  after 30 seconds and then completed its response. The tool's wall duration
  was about 76 seconds, so the 30-second value is not an end-to-end wall-time
  guarantee for model/tool orchestration.
- Direct production-interface probe without a model: completed in about 2
  seconds. This covers the real workspace backend without depending on a
  model's choices.

## Controlled benchmark

The baseline was generated with `git archive HEAD`; the current implementation
was built from the working tree. Both probes used the same `needle` query,
`*.md` glob, and a fixture with 20,005 non-matching files plus one matching
file. Each implementation ran three fresh processes.

- Baseline elapsed times: 2671, 575, 571 ms; all results had
  `truncated = false` and one match.
- Current elapsed times: 459, 372, 387 ms; all results had
  `truncated = true` and one match.
- Warm-run median: 575 ms for baseline and 372 ms for current, a 1.55x ratio
  in this fixture. Cold-start timings include process startup and are not used
  for that ratio.
- Raw results are retained in `benchmark-results.json` and the six per-run logs.

Earlier immediate-probe attempts timed out before the manifest path was moved
onto the blocking pool. Those failures are preserved in
`immediate-result-before-timeout.json` and `immediate-grep-output-before-timeout.log`.

## ACP default-path verification and final incident fix

ACP interactive sessions use the default local workspace assembly; unattended
sessions use the default manifest/retrieval assembly. Both inherit the Core
operation timeout and scan budget without test-only production configuration.

The ACP integration test drives the shipped binary through initialize,
new_session, and prompt with a mock provider. It uses the incident query and
20,002 Markdown files. Both modes return a real match plus an explicit
truncation warning. The final run completed the whole mocked prompt in 713 ms
for interactive ACP and 1,538 ms for unattended ACP. The pre-existing
unattended BM25/grep protocol test also passed.

A subsequent real-model failure exposed work before the original blocking
boundary: `ensure_auto_trigram_candidate_index` synchronously read workspace
contents on the executor. The incident regex cannot use trigram pruning, so
this work was both unbounded from the caller's perspective and unnecessary.
Two pre-fix runs timed out at 120.009 and 120.006 seconds. `acp-sample.txt`
captured the synchronous construction stack.

After the fix, the same real-model incident probe exited 0 in 32.221 seconds
and returned five matches in three files with truncation explicitly reported.
The full repository-analysis prompt then exited 0 in 42.693 seconds. An
intervening 97.730-second run failed because Cursor requested an unavailable
tool or supplied invalid arguments; it did not fail in workspace search and
was retried successfully.

Final verification:

- Manifest suite: 40 passed, 0 failed, including no-auto-index for an
  unprunable regex and non-blocking slow candidate selection.
- ACP incident integration test: passed for both default modes.
- `cargo clippy --locked -p aster-core --all-targets --no-deps` and the same
  command for `aster-acp`: exit code 0, with existing warnings only.
- `cargo fmt --all -- --check` and scoped `git diff --check`: passed.
- Repeated default-target `just build` / ACP-only link commands exceeded the
  300-second command limit. Final real-model runs therefore used the ACP
  binary successfully built from the same sources and default features in
  `target/grep-budget-tests`, copied to `target/debug/aster-acp`; the macOS
  sandbox stage completed separately.

## Explicitly not claimed

- No controlled before/after performance benchmark is claimed outside this
  fixture. Model choice, index readiness, filesystem load, and model reasoning
  time varied between the other end-to-end runs.
- No claim is made that every local search completes within 30 seconds of wall
  time. Backend budget, scheduler delay, result rendering, and model time are
  separate. Candidate construction no longer blocks the executor, but it is
  not itself a hard 30-second cancellable operation; the incident regex now
  avoids that construction.
- No Windows or Linux runtime verification was performed.
- No indexing architecture change, new dependency, new configuration option,
  or cross-platform release qualification is claimed.
- The separate aster-vec duplicate-key/indexing failures are not fixed here.
