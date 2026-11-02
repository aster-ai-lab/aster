# Issue #295: provider failures during unattended work

Issue: https://github.com/aster-ai-lab/aster/issues/295

The initial GitHub query returned one open issue. This change addresses #295.
The process-level ACP suite passes all 27 tests. Three new tests cover six
HTTP failure/recovery cases, also repeated through A3S Test's TUI runner.

## Changes

- Core retains typed transient provider exhaustion (status and HTTP attempt
  count) for the exact Run before the actor renders the error as text. The
  provider child sends a private frame; the original parent records it in the
  existing bounded Run store. No public event variant or existing schema changes.
- Unattended freeform work has a separate allowance of three consecutive
  provider cycles, with 1 s and 2 s waits between them. Core's own HTTP retries
  and Retry-After handling still run first. Model idle/non-terminal budgets are
  not charged for those provider failures. Cancellation and the original job
  deadline still bound the waits.
- An exhausted provider failure writes `reason=retryable_external`,
  `outcome=failed`, `exitCode=1`, and `providerStatus` in the result JSON.
  The last error describes the provider failure instead of an earlier gate error.
  ACP returns its normal terminal response so the wrapper can read the result.
- A failed exact Goal Run is not replayed. After the provider's HTTP retry
  policy stops, the Goal launch returns the same retryable-external result.
  Checkpoint/resume and keeping child processes alive across failed Runs are
  outside #295 and are not implemented here.
- Permanent account/auth/quota refusals retain their existing handling.
  Completion guidance, bound verification requirements and interactive retry
  behavior remain on their existing paths. A genuine worker join failure is
  not replaced by a retained provider diagnosis.

## End-to-end evidence

The tests launch the real `aster-acp` executable, communicate over ACP stdio,
use a loopback OpenAI-compatible HTTP fixture, and execute workspace tools.
They use separate temporary state/home directories and a workspace name with
spaces and non-ASCII characters. They do not access a real model account.

| Case | HTTP status | Observed result |
| --- | --- | --- |
| Persistent outage, freeform | 429 / 503 | 33 failed HTTP attempts across three provider cycles; retryable-external JSON, no successful checker and no verified completion |
| Recovery, freeform | 429 / 503 | 13 failed HTTP attempts, then the actual Bash checker exits 0; original guidance is observed, answer.txt contains exactly `42\n`, and a verified turn.complete is retained |
| Failed exact Goal Run | 429 / 503 | 11 failed HTTP attempts; retryable-external JSON without replaying the failed exact Run or waiving verification |

The checker is `test -f answer.txt && test "$(cat answer.txt)" = 42`.
Assertions inspect the committed tool receipt, its exit code, and
`metadata.verification_shell_command`; model text alone cannot pass the test.
The native host execution request follows the existing fixture's admitted
`sandbox_permissions=require_escalated` path after macOS refuses unqualified
native process lifetime. Permission enforcement was not changed.

- [Complete ACP E2E log](unattended-all-e2e.log): 27 passed, 0 failed.
- [A3S Test report](a3s-run.json): one wrapper scenario passed; all three native
  test functions and six cases ran, and the owned process exited 0.
- [Terminal recording](terminal/provider-results.vt): exact bytes copied from
  this run's A3S artifact directory; no unrelated sessions were copied.
- [A3S replay after the paired rebuild](a3s-run-pair.json): all six cases
  passed again; process exit 0 and no output truncation. The exact terminal
  bytes are in [the paired replay recording](terminal/provider-results-pair.vt).
- [A3S ACL admission](a3s-check.json) and the reusable suite
  [unattended-provider-retry.acl](../../../../tests/e2e/unattended-provider-retry.acl).

The A3S pass repeats the six cases; it is not six additional distinct cases.

## Checks

Host: macOS, aarch64-apple-darwin, pinned rustc 1.97.1 (8bab26f4f).
Cargo used default owning-package features, `--locked`, and
`CARGO_TARGET_DIR=target-goal-issues`. Test runs set `RUST_MIN_STACK=16777216`.

| Check | Result | Log |
| --- | --- | --- |
| New typed classification / Run retention unit tests | 2 passed | `core-provider-exact.log` |
| Core original model governance | 10 passed | `core-model-governance.log` |
| ACP unattended / result classification | 39 passed | `acp-unattended-unit.log` |
| ACP attempt delivery / cancellation | 5 passed | `acp-attempt-unit.log` |
| Existing Core provider retry authority | 1 passed | `core-retry-authority.log` |
| Real ACP process E2E | 27 passed | `unattended-all-e2e.log` |
| A3S Test native process replay | Passed; exit 0, no output truncation | `a3s-run.json`, `a3s-run-pair.json` |
| Core + ACP all-target Clippy | Exit 0; existing warnings | `clippy.log` |
| Workspace formatting | Exit 0 | `workspace-format-complete.log` |
| Workspace dependency checks | 7 passed | `workspace-dependencies.log` |
| Governance checks | 4 passed | `governance.log` |
| Additional package test selected by the broad provider filter | Fixture fails with InvalidBinding; isolated rerun also fails | `core-provider-unit.log`, `package-failure-isolated.log` |
| CLI + ACP paired build | Recheck exit 0 after shared lockfile update | `build-pair.log`, `build-pair-recheck.log` |

Main commands:

```sh
CARGO_TARGET_DIR=target-goal-issues RUST_MIN_STACK=16777216 cargo test --locked -p aster-acp --test unattended_e2e -- --test-threads=1 --nocapture
CARGO_TARGET_DIR=target-goal-issues RUST_MIN_STACK=16777216 cargo clippy --locked -p aster-core -p aster-acp --all-targets --no-deps
cargo fmt --all -- --check
python3.13 tests/workspace_dependencies.py
python3.13 tests/governance_regime.py
```

Exact Cargo commands and exits are retained in [checks.json](checks.json).
The separately selected package test fails in `agent_api/tests/fixtures.rs:479`
while constructing a package capability binding, before executing provider
logic. Its fixture and package-binding rules were not edited for this task.
The initial paired build refused an inconsistent shared Cargo.lock. No manifest
or lockfile was edited by this change; the second build passed after another
session updated main and the shared lockfile. The successful rebuild took
3 min 07 s and built both binaries with `--locked`.

## Retained test setup failures

- The first baseline probe used `Retry-After: 0`, which the existing parser
  treats as absent. It hit the job deadline under the normal exponential
  backoff; [red-e2e.log](red-e2e.log) is not claimed as a reproduction of exactly
  sixteen host retries. The final fixture uses the supported positive value
  `Retry-After: 0.001` to exercise exhaustion inside the same fixed deadlines.
- The first recovery probe attempted Bash without the native host-execution
  request; macOS refused it. The test failed, and its assertion was retained.
  The final case uses the existing approved fixture execution path and checks
  the actual successful receipt. See `provider-e2e-first.log` and
  `provider-e2e-second.log`.
- Goal planning now requires a structured pre-analysis response. The loopback
  fixture previously returned plain text, so startup failed before any write.
  The shared fixture now answers that planning contract. Original task prompts,
  deadlines, verification requirements and acceptance assertions were preserved.
  Failed attempts remain in `provider-e2e-final.log` and `provider-e2e-complete.log`.

## Source and publication

[source.json](source.json) records the captured before and final source digests.
The checkout contains substantial concurrent work. Only the task delta is
selected for publication by merging those captured sources against current
HEAD; other uncommitted changes are left intact. Recorded tests ran in the
shared working checkout, not a clean clone of a partially staged tree.

## Explicitly not claimed

- Full-workspace test success: the additional package fixture failure is retained.
- Windows/Linux native qualification, GUI tests, live GLM/provider reliability
  or performance results from the loopback failure fixture.
- Checkpoint/resume, persistence of the typed diagnostic across restarts, or
  keeping child tools alive across failed Core Runs. Metadata follows Run retention.
- A release, archive qualification, or passing remote CI before it actually runs.
