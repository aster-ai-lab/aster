# Host command cancellation and native TUI recovery

Date: 2026-11-10. Platform: macOS arm64, Terminal, Rust 1.97.1.

## Result and scope

The source-built `aster` / `aster-acp` pair passed three real DeepSeek Flash
cancellation-and-recovery cases in **YOLO host execution**: mouse Stop, Ctrl+C,
and send-now for a queued prompt. Each original parent and child was running
before cancellation and absent afterward. Each replacement was admitted once,
wrote the exact frozen bytes, read them back, and reached durable `completed`.
A later no-tool answer completed without the queue returning. After the pair
exited, a new TUI process loaded the same session through `/resume` and completed
another real write-and-read task.

The canonical log contains **19 preparations, 19 original results, and five
turn-complete facts**. Every preparation has exactly one identity-matched result;
there are no incomplete or unresolved receipts in this new test session. The
three cancelled command receipts retain the original invocation identities,
exit code **137**, and observed partial file changes. Cancelled runs retain
`cancelled`; their subsequent successful runs retain `completed`.

This supersedes the YOLO recovery failure on the earlier candidate recorded in
[the previous visual run](../2026-11-10-deepseek-visual-e2e/OUTCOME.md). Its failed
sessions and evidence remain preserved. Historical unknown invocations are not
erased or converted into successful outcomes.

## Implementation

Two connected defects caused the observed recovery failure:

1. Dropping a Bash future cancelled capture without letting its original host
   command return a terminal result. Cancellation now reaches the existing
   workspace command owner. It kills the owned process group, waits for the
   original child, finishes the command's workspace observation, and returns
   the actual cancellation result. The fact adapter accepts this result only
   with private evidence bound to that prepared invocation and exit code.
   Tool metadata alone cannot grant this authority.
2. An installed Hook executor wrapper was counted as an executed input Hook
   even when its dispatcher matched no handlers. A later permission refusal
   then created another false unresolved operation. Dispatch now returns its
   policy result and actual activity together. Empty built-in dispatchers
   report no activity; matching, opaque, and channel observers remain
   conservative. Real Hook effects still retain uncertainty after refusal.

The existing 500 ms tool settlement grace, tool timeouts, canonical completion
guard, original receipt identity checks, and refusal rules were not relaxed.
An experimental ledger-watermark projection was discarded because it could not
establish the missing original execution terminal. No model name, prompt,
fixture filename, or expected answer was added to production decision logic.

The additive workspace/sandbox cancellation methods preserve existing SDK
implementations through their ordinary execution default. A backend that cannot
return its original cancellation terminal remains uncertain. The native
ksandbox adapter and process-host adapter still use that default; they are not
qualified by this host-runner result.

## Candidate and frozen inputs

- Baseline: `4ced2b135225`, plus the owned Core changes and the pre-existing
  five-file ACP cancellation patch in the shared checkout
- Pair BUILD_ID:
  `aster-cli-pair-v1:9039f5f46a7c6c9ecb6bc21e3922675ed94d8847:aarch64-apple-darwin`
- Model: `deepseek/deepseek-flash`; effective context: 1048576 tokens
- Test session: `55269b30-ce83-4850-a346-e88b1dd18b6e`
- Configuration: a private copy of the supplied `../../config.acl`, resolved
  from ToWork as `/Users/roylin/code/config.acl`
- [Candidate hashes](candidate.json), [frozen plan](frozen-plan.json),
  [original receipts and run results](results.json), [verification](verification.json)

The original configuration hash stayed unchanged. The private copy was removed
after both owned processes exited, and Simplified Pinyin was restored. The
frozen `waiter.py` and independent expected outputs were not edited to pass the
run. Only the two new API contract comments and this evidence were added after
the native run; executable implementation stayed unchanged.

The foreign ACP patch was included in the source build but is not adopted or
committed as this task's code. Parallel Web and Studio changes were preserved.
Release remains pending the ACP owner's commit and clean release qualification.

## Native observations

| Case | Observed terminal and recovery | Native evidence |
| --- | --- | --- |
| Mouse Stop | Original `cancelled`, original exit 137, replacement `completed` once | [Running](screens/native-stop-v2-running.png), [recovered](screens/native-stop-v2-recovered.png) |
| Ctrl+C | Original `cancelled`, exit 137, replacement `completed` once | [Recovered](screens/native-ctrlc-v2-recovered.png), [injection refusal](keyboard-injection.json) |
| Queued send-now | Queued prompt was promoted once, original `cancelled`, replacement `completed` | [Queued](screens/native-queue-v2-queued.png), [recovered](screens/native-queue-v2-recovered.png) |
| No-tool follow-up | `READY-AFTER-CANCEL`, durable `completed`, queue absent | [Native frame](screens/recovery-02-no-tools-followup.png), [run](no-tools.json) |
| `/resume` and cold restore | Popup opened; new process loaded the exact session ID, preserved three cancellations, and completed a new write/read | [Popup](screens/recovery-03-resume-popup.png), [ID filter](screens/recovery-07-history-id-filter.png), [new task](screens/recovery-09-resumed-task-complete.png) |

Mouse Stop reached the observed durable terminal about 0.18 seconds after the
click sequence started. Timing in `results.json` is observational, not a
performance qualification. Send-now's full sequence includes typing, queueing,
and capturing the queued state before the promotion action.

agent-desktop 1.0.2 (envelope 2.5) drove the native window. Its Ctrl+C injection
returned `APP_UNRESPONSIVE` / `menu_modifier_value_invalid` with explicit
`not_delivered`; macOS System Events then delivered the key to the verified
owned window. This is a native Ctrl+C result, not a successful agent-desktop
Ctrl+C injection. Window identity/focus errors and screenshot fallbacks remain
in the private raw automation logs.

A cold-resume attempt received unrelated desktop input while focus moved to
other applications. That attempt was not accepted as a product result. The
operator paused other desktop work, the query was cleared, and the exact session
ID was selected before the successful restore. Selecting the already loaded
session earlier only focused it and was not counted as disk replay. ACP's later
`restore=true` session build and the new completed run establish the cold load.

The private harness initially looked for an active Run in a session-store
snapshot before that snapshot was published. It then read the same original
admission from the canonical prepared fact; it did not resubmit the input.
Canonical user-message counts confirm one admission for every control prompt
and replacement.

## Verification commands and results

Commands ran at the repository root with:

```sh
export CARGO_TARGET_DIR=/Users/roylin/code/aster/target
export CARGO_INCREMENTAL=0
```

```sh
cargo fmt -p aster-core -- --check
cargo clippy --locked -p aster-core --all-targets --no-deps
cargo test --locked -p aster-core --lib -- \
  fact_control::receipts::tests hooks:: tools::process:: tools::types::tests \
  sandbox::host_access::tests agent::hook_runtime:: \
  agent_api::conversation_runtime::receipt_restore_tests \
  agent_api::direct_tools direct_tool_timeout_cancels_and_settles \
  queued_tool_timeout_cancels_and_settles \
  --skip detached_binding_uses_the_pinned_capability_package_and_control_identity \
  --test-threads=1
cargo test --locked -p aster-core --lib \
  agent_api::capability_runtime_tests::hook_projection -- --test-threads=1
cargo build --locked -p aster -p aster-acp --bins
cargo fmt --all -- --check
```

- Core format: passed. Core all-target Clippy: passed with existing warnings
- Focused regression: **282 passed, zero failed**
- Composed/supervised Hook regression: **10 passed, zero failed**
- Paired source build: passed; matching BUILD_ID and binary hashes recorded
- Workspace format: failed on **18 unrelated, pre-existing paths**, listed in
  `verification.json`; no bulk formatting was applied
- Initial focused run: 281 passed and two failed. The new escalation test was
  refused before spawn because it lacked exact-call approval; the test was
  corrected to supply that approval. The other failure reproduces alone in
  `detached_binding_uses_the_pinned_capability_package_and_control_identity`:
  the untouched package fixture's capability snapshot digest does not bind its
  package/generation/Knowledge surface. It remains excluded from the scoped
  passing batch and is not reported as passing
- An initial build command selected the nonexistent package `aster-cli` and
  exited before building. The corrected `aster` + `aster-acp` command above
  passed before any native run

The new regressions exercise real host processes through direct authorized
execution and the YOLO sandbox adapter, preserve partial writes, prevent a late
child write, and admit a subsequent tool. Counterexamples retain unknown
execution for forged cancellation metadata and real input-Hook effects. Existing
unknown-receipt, canonical completion, timeout, cancellation, cache provenance,
and Hook lifecycle assertions remain in the passing batch.

## Explicitly not claimed

- Full execution of the 508-case TUI catalogue, every model/effort combination,
  or all providers
- Cancellation recovery for native ksandbox, process-host, remote/custom SDK
  runners, daemonized descendants outside the existing owner boundary, or
  native Windows/Linux execution
- Automatic reconciliation of historical unknown receipts from the earlier
  failed sessions
- Workspace-wide green CI, zero existing lint warnings, performance acceptance,
  a clean release candidate, publication, tags, or updated release manifests
