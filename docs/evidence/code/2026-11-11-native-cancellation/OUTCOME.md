# Cooperative native command cancellation

Aster now pins ksandbox commit
[`aa626654`](https://github.com/kense-labs/ksandbox/commit/aa626654a9f7bcc517c0917f61456322862e6f28)
and forwards cancellation to its original native command owner. The owner
terminates the process tree, waits for the original child, and returns its
observed exit and partial output. Existing execution APIs remain available.
Aster maps that typed terminal into its existing prepared-invocation receipt
path; it does not clear unknown effects or invent completion after a timer.

The user explicitly authorized publishing the reviewed six-file patch to the
existing `fix/cooperative-command-cancellation` branch. Main was unchanged.
Remote Git blob hashes were checked against every reviewed file. No PR,
upstream release, second sandbox, or language binding was created.

## Checks

Commands ran at the Aster root with Rust 1.97.1,
`CARGO_TARGET_DIR=/Users/roylin/code/aster/target`, and `CARGO_INCREMENTAL=0`:

```sh
cargo update -p ksandbox --precise aa626654a9f7bcc517c0917f61456322862e6f28
cargo test --locked -p aster-core --lib fact_control::receipts::tests::event_cancellation -- --test-threads=1
cargo clippy --locked -p aster-core --all-targets --no-deps
cargo fmt --all -- --check
cargo build --locked -p aster -p aster-acp --bins
```

- Receipt tests: **7 passed, 0 failed, 0 ignored**
- Core all-target Clippy passed with existing warnings
- Changed Rust was formatted; workspace formatting still fails in unrelated
  files retained in the private check log
- The generated lockfile changed the ksandbox source and reassigned 16 existing
  dependency edges; the package/version set is unchanged. No lockfile was
  manually edited. See [lock-resolution.json](lock-resolution.json)
- Both freshly built binaries reported
  `aster-cli-pair-v1:60bc0bc6cf24ea539a9b383c88aaf66701a35eb4:aarch64-apple-darwin`
- The exact upstream bytes were previously tested with a temporary Cargo CLI
  patch: eight process tests, all-target Clippy, native macOS cancellation and
  prepared-receipt probes, plus Linux arm64 and Windows MSVC library
  cross-compilation. [proposal.json](proposal.json) records that scope;
  [published.json](published.json) records the verified remote bytes

A concurrent merge changed CI and the Studio gitlink. Recomputing the pair's
source fingerprint after that merge still matched the tested binaries exactly.
See [source-reconciliation.json](source-reconciliation.json).

## Real native TUI checks

The native Terminal run used `deepseek/deepseek-flash`, Normal permission mode,
and a private copy of the supplied `../../config.acl`. The original ACL and the
frozen waiter script were unchanged. The commands were sandboxed; their model
calls requested no host escalation. Each waiter spawned a real child.

| Gesture | Original task | Process and recovery evidence |
| --- | --- | --- |
| Mouse Stop | Cancelled, original exit 137 | Parent and child exited; replacement admitted once and completed; exact `STOP-RECOVERED-83\n` |
| Ctrl+C | Cancelled, original exit 137 | Parent and child exited; replacement admitted once and completed; exact `CTRL-RECOVERED-61\n` |
| Queue then send now | Cancelled, original exit 137 | Parent and child exited; queued replacement admitted once and completed; exact `QUEUE-RECOVERED-47\n` |

All **17 prepared invocations** in the session have one matching original
receipt, complete observation, and no unresolved operations. Late-write traps
remain absent. The later task and process restart did not resurrect a queue.
See [native-controls.json](native-controls.json).

The actual Stop click reached an observed durable terminal in about 0.27 s.
The separately observed native Ctrl+C terminal took about 0.31 s. The full
queue/type/capture/send-now gesture took about 3.28 s; that is not a measurement
of the final Enter alone. These are observations, not a performance campaign.

`/resume` opened the real popup in a new CLI/ACP process. Searching the exact
session ID restored the same session and all three cancelled Runs. A new task
read the unseen canary with a real tool and completed once. Its effective
context remained **1,048,576 tokens**, and the selected model remained DeepSeek.
See [native-resume-result.json](native-resume-result.json). The effort menu also
retained explicit `max` separately from `Default (high)`.

## Retained infrastructure failures

agent-desktop repeatedly refused changing Terminal window identities, including
unknown-delivery results. Input was observed before any fallback; it was never
blindly replayed. Mouse Stop and captures used agent-desktop. Some approvals,
Ctrl+C, queue input, and continuation used recorded native System Events input;
those keyboard actions are not claimed as agent-desktop-delivered passes.

The first Ctrl+C setup missed its frozen start deadline because approval was
not delivered. The first queue attempt failed to enter a replacement before
the original sandbox command timed out. Both failures are retained. A native
input assertion also ran before the next frame rendered; the operator now
waits for the exact draft and can adopt an already exact unsent draft without
typing twice. Prompts, process/effect checks, admission counts, expected bytes,
and scenario deadlines were unchanged. No model-specific product branch or
grader relaxation was added.

Native Ctrl+C at a pending approval subsequently closed that card in about
0.20 s. Denying its verification left the earlier mutation unverified, and the
completion gate reported failure honestly. No waiver or ledger reset was used.
The observed native timeout also reported the requested 900000 ms after a
shorter effective sandbox deadline; effective timeout reporting needs separate
investigation.

## Explicitly not claimed

This completes the native cancellation integration, not the entire 508-case
TUI plan or the expanded full-function goal. Configuration CRUD, every form,
IME, boundary/fault matrices, and other remaining cases still require their
own evidence. This is not native Linux/Windows or iTerm2 acceptance, nor proof
that custom/process-host SDK runners cooperate with cancellation. Pre-existing
ACP source changes remain uncommitted; this dirty diagnostic candidate is not
a clean release qualification and no Aster product release was published.
