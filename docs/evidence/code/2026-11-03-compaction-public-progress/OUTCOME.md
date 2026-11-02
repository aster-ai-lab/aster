# Public tool progress retained for compaction

Normal provider history already contains ToolCall.text, but both automatic and manual compaction dropped it. The real Effect fold and Core projection each fail a new 0/1 red test before the fix. This is a transcript fidelity defect, independent of any benchmark values.

The two production changes preserve distinct public prose once per batch. Prose precedes adjacent tool-call lines so the existing compaction tail grouping remains intact. Tool order/IDs/args, receipts and scheduling remain unchanged. Provider reasoning remains excluded. No new API, option, store, budget, dependency or task-specific calculation.

The unpublished observation-recovery variant is withdrawn from the worktree after pre-hash checks on all nine owned paths; its exact tree, patch, frozen binaries and 1/2 GLM evidence remain available in the previous evidence record. The new variant starts from published main and changes only four source/test paths.

## Red/green and qualification

| Check | Outcome | Process seconds |
| --- | --- | ---: |
| format | exit 0 | 0.040182 |
| effect_green | test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 92 filtered out; finished in 0.00s | 2.707507 |
| core_green | test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 5697 filtered out; finished in 0.00s | 75.206410 |
| effect_all | test result: ok. 92 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 6.26s; test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 11.79s; test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s; test result: ok. 15 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.06s; test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.09s; test result: ok. 18 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.08s; test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s; test result: ok. 14 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.66s; test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s; test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.84s | 30.112427 |
| core_default | test result: ok. 407 passed; 0 failed; 1 ignored; 0 measured; 5290 filtered out; finished in 13.71s | 14.153834 |
| core_cli_json | test result: ok. 407 passed; 0 failed; 1 ignored; 0 measured; 5290 filtered out; finished in 24.77s | 99.856860 |
| core_minimal | test result: ok. 407 passed; 0 failed; 1 ignored; 0 measured; 5175 filtered out; finished in 16.89s | 92.344473 |
| core_clippy | exit 0 | 75.416423 |
| effect_clippy | exit 0 | 2.847442 |
| acp_controls | test result: ok. 23 passed; 0 failed; 0 ignored; 0 measured; 466 filtered out; finished in 3.24s | 72.796435 |
| workspace_format | exit 1 | 22.528163 |

Both targeted regressions move 0/1 -> 1/1 (+100 percentage points). Effect process 5.336430 -> 2.707507 s (-49.26%); Core 82.414012 -> 75.206410 s (-8.75%). These include compilation/cache and are not task-speed gains. All qualification tests make zero real model requests.

Effect all targets: 164 passed, 1 ignored. Each Core profile: 407 passed, 1 ignored. Core coverage explicitly includes background settlement, subagents, long-horizon capability composition, history concurrency and artifact persistence. The three profiles overlap. ACP controls: 23 passed. Both owning-package all-target Clippy commands exit 0, with no warnings in the four changed paths; pre-existing other warnings remain. Workspace format still fails on 783 pre-existing paths, zero changed-path intersection.

Complete CLI namespace audit examines 16182 paths: only the four owned paths differ from the prior qualified main namespace. Original GLM tasks, prompts, input registrations, model configuration, options, caps and frozen graders stay unchanged.

Build freeze passed: 78.684303 s, versus observation-recovery 63.288609 s (+24.33%, not task speed). All five embedded identities match aster-cli-pair-v1:c01b5aaa4910f4201129c171a157bba99d22de42:aarch64-apple-darwin. Source is unchanged before/after build. Original real GLM is now IN PROGRESS. No accepted task speed/token benefit is claimed. Original Holdout then Feed run once each; only both passes admit the original seven supplementary cases.

Not qualified: actual TUI background completion, cancellation, restart and long-task/subagent/parallel composition; native Windows/Linux. No Release or installed-app replacement. Existing root-checkout changes preserved.

Actual frozen TUI startup/resize: 1 scenario, 11/11 steps; three exact viewports and no truncation/cleanup error. Process 5.090032 -> 4.941225 s (-2.92%), zero model calls. This is a single startup observation, not long-task speed or full TUI acceptance.
