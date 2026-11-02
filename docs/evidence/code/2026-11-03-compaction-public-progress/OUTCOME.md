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

Build freeze passed: 78.684303 s, versus observation-recovery 63.288609 s (+24.33%, not task speed). All five embedded identities match aster-cli-pair-v1:c01b5aaa4910f4201129c171a157bba99d22de42:aarch64-apple-darwin. Source is unchanged before/after build. Original real GLM has finished: 0/2 passed. No accepted task speed/token benefit is claimed. Original Holdout then Feed run once each; only both passes admit the original seven supplementary cases.

Not qualified: actual TUI background completion, cancellation, restart and long-task/subagent/parallel composition; native Windows/Linux. No Release or installed-app replacement. Existing root-checkout changes preserved.

Actual frozen TUI startup/resize: 1 scenario, 11/11 steps; three exact viewports and no truncation/cleanup error. Process 5.090032 -> 4.941225 s (-2.92%), zero model calls. This is a single startup observation, not long-task speed or full TUI acceptance.

## Final original GLM result and explicit user stop

Both critical cases finish naturally before cleanup. The user explicitly stops further optimization and asks to commit/merge/push main; no further candidate, model test or unchanged retry is started. Four owned test/Driver PIDs are confirmed exited. No process is signaled and other optimizer sessions remain untouched. Original config and five frozen members are rehashed unchanged.

| Compared with stream-preview-1 | Holdout | Feed |
| --- | ---: | ---: |
| Strict pass | false -> false | false -> false |
| Task seconds | 227.986 -> 207.262 (-9.09%) | 496.938 -> 469.087 (-5.60%) |
| Complete native tokens | 160289 -> 161479 (+0.74%) | 348814 -> 247319 (-29.10%) |
| Test process seconds | 242.989726 -> 222.373085 (-8.48%) | 515.165660 -> 486.754534 (-5.51%) |
| Requests | 10 -> 10 (0%) | 20 -> 18 (-10%) |
| Source reads | 5 -> 5 (0%) | 7 -> 7 (0%) |
| Compactions | 2 -> 2 (0%) | 8 -> 7 (-12.50%) |

Both read every required source version and settle their tools, but neither produces a correct final artifact or bound passed verification. All 28 response usages are complete; no relay rejection or transport failure. Cohort wall 758.737408 -> 709.693548 s (-6.46%); complete native 509103 -> 408798 (-19.70%). These are failed-run resource reductions, not accepted task improvements. Native lifecycle/timing completeness, exclusive partitions and unknown/unattributed intervals are retained explicitly in qualification.json. Original seven supplementary cases remain NOT RUN.

A read-only audit confirms continuing input pressure in the completed Feed baseline: the first full 23-tool request consumes 15268 of the configured 16384 tokens (93.19%); post-summary requests still consume 18972-26379 tokens. This is evidence of insufficient headroom and repeated compaction, not proof that every model arithmetic error has the same cause. No additional optimization is implemented after the user's stop.

The generic public-progress fidelity fix is published as 405a70e72. This closes the current evaluation and pauses this thread's optimization work at the user's request. The overall performance objective is not complete. Actual TUI background completion/cancel/restart/parallel/subagent acceptance, native Windows/Linux, and full long-task qualification remain unproven. No Release or installed-app replacement.
