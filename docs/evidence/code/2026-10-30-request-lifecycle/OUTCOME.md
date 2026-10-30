# Request lifecycle and PR #9

Reviewed PR #9 at `9d015a60614fcde645c1a38e8a757c97f4339667` against main
`e2beb284d3fe99a1fd8b4ad78f13a9988187c9f9`. All 62 proposed paths are accounted
for in `pr9-scope-audit.json`: 10 runtime/probe paths are integrated, 16 paths
are superseded by current delivery/configuration workflows, and 36 historical
evidence files remain on the original PR branch. The branch is retained.

## Result

- Cancellation now covers the complete non-streaming HTTP request and response
  body. JSON and raw streaming error-body reads share the existing timeout and
  cancellation policy. Known 401/403 responses return before a model deadline
  can hide the authorization status; an immediately available complete body
  retains the provider diagnostic.
- TUI/headless initialization, authentication, session operations and model
  selection reuse the existing bounded session RPC. Failed initialization
  cancels the agent token. Session-load errors retain the actual cause.
- Full prompt replacement remains explicitly refused; the CLI help says so.
  Appended rules, permission metadata, current `a3s/` methods, single-hop
  clients, redirect policy and Windows transport handling are preserved.
- Full-output review found an additional first-run defect: the generated ACL's
  literal `替换为你的密钥` was used as a real key. The ACL entry boundary now
  represents that exact placeholder as missing credentials. The configuration
  file and model catalog remain intact; a configured key still works. This
  fix is in `crates/code/acp/src/config.rs`, beyond the old PR's file list.

## Verification

Native macOS arm64, pinned Rust 1.97.1. Commands run from the root workspace:

| Check | Observed result |
| --- | --- |
| HTTP lifecycle cases, each rerun alone before the fix | 5 failed, 1 passed |
| Startup cases, each rerun alone before the fix | 4 failed |
| First-run placeholder regression before the fix | 1 failed |
| `cargo test --locked -p a3s-code-core --lib llm::http:: -- --test-threads=1` | 37 passed |
| `cargo test --locked -p a3s-code-pager --lib -- startup_deadline_tests headless::tests acp::tests acp::permission_rules_tests headless::permission_rules_tests --test-threads=1` | 74 passed |
| `cargo test --locked -p a3s-code-acp --lib config::tests -- --test-threads=1` | 26 passed |
| `cargo build --locked -p aster -p a3s-code-acp` | Both built from the same source |
| Original `tests/cli_acp_lifecycle.py`, `--phase after` | All 6 assertions passed; full replies also audited |
| `tests/cli_pair_delivery.py` | 10 passed |
| `tests/release_governance.py` | 9 passed |
| `tests/cli_release_workflow.py` | 5 passed |
| `tests/workspace_dependencies.py` with Python 3.13 | 6 passed |
| `cargo fmt --all -- --check`; `git diff --check` | Passed |
| `cargo clippy --locked -p a3s-code-core --all-targets --no-deps` | Passed, existing warnings retained |
| Same Clippy command for `a3s-code-acp` | Passed, existing warnings retained |
| Combined Clippy including `a3s-code-pager` | Failed: 30 indexing/slicing diagnostics in byte-identical main files |

The initial grouped HTTP run failed all six cases because its first failure
poisoned the shared test environment lock. Independent runs established the
five real failures and the already-passing diagnostic case. Every original HTTP
fixture byte is preserved except four item-scoped lint explanations. Those
tests each own a separate Tokio runtime and serialize process-global proxy
settings; no production mutex policy or lint configuration is changed.

The original native script, its prompt, model, timeouts and assertions are
unchanged. Its initial six-case exit status was a false positive for missing
ACL: it checked only its local provider's call count, while the full reply
showed an attempted DeepSeek request. That receipt is preserved in
`native-initial.tar.gz`; `pr9-native-initial-fidelity.json` records the gap.
After the configuration fix, the same case fails locally with the SDK's
session-configuration error before LLM client construction, in 0.163 seconds.
The unfinished 401 case finishes in 0.712 seconds after exactly one streamed
model request (following the fixture's separate pre-analysis). These timings
describe individual runs, not a performance benchmark.

Both native archives preserve all 13 original filenames and bytes per run:
six stdout/stderr pairs and the ordered result receipt. Full JSON replies,
byte counts, binary SHA-256 values and final source hashes were re-opened and
compared. No fixture process matching either owned binary directory remains.
The test pair's matching `--build-id` was independently reproduced by executing
the actual `scripts/cli_pair_build.rs::stamp`; it fingerprints source bytes,
including uncommitted changes, rather than identifying a Git commit.

## Explicitly not claimed

No full-workspace Clippy pass, green global CI, native Linux/Windows execution,
real LLM/provider validation, packet-level egress trace, new release, installer
publication or installed desktop verification is claimed. The 30 pre-existing
Pager Clippy errors and their exact unchanged source hashes are retained in
`pr9-clippy-existing-errors.json`; no rule was weakened. Current-main CI still
has separate backend failures. Historical PR artifacts are not substituted
for this run's binaries, and the old v0.1.2 delivery proposal is not restored.
