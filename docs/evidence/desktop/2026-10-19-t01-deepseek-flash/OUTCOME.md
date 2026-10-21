# AsterDream / DeepSeek Flash T01 evaluation and interrupted-turn repair

Status: completed. The repaired AsterDream / DeepSeek Flash run produced an unmodified submission that passed the initial eight independent scenarios; an additional legal negative-zero formatting case failed, for **8/9 local scenarios** overall.

## Requested outcome

Use visible, background CUA interaction to run AsterDream with DeepSeek Flash on `../tasks/T01`, preserve inputs, and independently assess the submission. During the attempt the user explicitly requested a fix for `Agent stopped before completion: max_turn_requests`.

## Repair

- Remove the unconditional twelve-read-only-result ceiling from both coding-loop paths. Distinct source reads can make progress without modifying files. Keep the configured overall tool budget, duplicate-call refusal, approval policy, cancellation, and workspace sandbox boundaries.
- Preserve the duplicate threshold when stock Harness composition overlays the base configuration. Retain historical forced-stop and checkpoint wire fields for compatibility.
- Snapshot public app-project inputs into the private task folder before binding it. Exclude registered task directories and owner-defined harness state. Bound file reads, refuse symbolic links, preserve existing task files, and roll back only this operation's newly published or moved entries on failure.
- Keep home project selection independent from hidden bound conversations. Give explicit/pending project selection priority and refuse an invalid explicit project instead of silently running in an empty system folder. Fail closed if the private project binding fails.

## Validation

The two new sixteen-source regressions failed on the original implementation and passed after the repair, including the production fact-log path. The fact-control group passed 162 tests. The dedicated budget-forced-stop regression passed. Effect composition inheritance passed its focused test. The complete shipped ACP wire scenario passed, proving duplicate calls produce typed denied/no-effect receipts and configured 16/128/4 tool budgets remain effective.

Project snapshot tests passed 7/7; catalog tests passed 14/14. The project selection and real bridge tests passed 52/52; AgentPage project/workspace/review-task tests passed 12/12. Focused formatting and diff checks passed. `just build-work` built the host, ACP, and Office together. See `checks/` and `shipped-wire-after-readonly-fix.log`.

The wider existing tree does not pass every gate: workspace dependency checks found the existing nested `apps/work/.git` and `a3s-gateway` publish metadata; workspace formatting reports other-tree differences; AgentPage has six unrelated sidebar assertions that also fail in isolation, and web type checking reports existing errors. An existing plan-heavy native Node test fails because its default test process has no qualified sandbox companion, also reproduced in isolation. These were not silently treated as passing checks.

## Runtime evidence and setup corrections

The original installed application and first development attempt were invalid model evaluations: the task inputs were unavailable in their effective directories. The initial development launch also selected an older ACP sibling beside its host. Both setup failures are retained under `runtime/`; they do not count as model coding failures.

The development host and executable built from the final source have matching Mach-O UUIDs; code signing changes their full-file hashes. The development app now has the current ACP and Office siblings. The actual live ACP process has SHA-256 `128588d3b0aee9e3700c497500a3bfb5b1ca75aee228c0f332184f8a04e49ad8`, captured by process identity. Its OCI descendant tree uses the existing qualified sandbox cache, without native execution fallback. No credentials or user session state were copied into this repository.

The valid run is `01a154b3-fd4c-70c3-ad72-48c14a36e920`, starting 2026-10-19 23:07:24.907 +08:00, on `/Users/roylin/code/tasks/T01-workspace`. Its eleven input files and instruction are byte-identical to T01. The model launch ACL selects `deepseek/deepseek-flash` at `https://api.deepseek.com`. The same original prompt was submitted through CUA. Successful instruction/input reads and real Python execution are directly observed. Independent oracle files stay outside the model's workspace and are not supplied to the model.

## Evaluation

The model finished at 23:15:27.763 +08:00: **482.856 seconds**, 26 model calls and 26 tool calls (23 succeeded, three model-program or self-test calls exited nonzero). It repaired its initial `KeyError: active` itself. Host status is `done`, assistant status `success`, and the core terminal is `verified`. Runtime stats do not contain usable token/cost totals, so none are estimated.

The independent evaluator passed **8/8 scenarios**, each run twice from an unrelated working directory with input/output paths containing spaces: public data, zero budget, changed policy, changed data/reference tables/provenance, row exclusivity and event edge cases, lexicographic ties, minimum-cost ties, and a greedy counterexample. Each comparison checks the entire structured result with strict types and exact list order. Every output was byte-identical across its two executions and input files were unchanged. See [submission-evaluation.json](submission-evaluation.json).

The actual JSON generated inside AsterDream also exactly matches the independent public oracle, beyond the isolated reruns: 1000 input rows, 987 effective rows, 13 cancellations, 252 candidates, 16 selected actions, cost 64/64, optimum gain **3,165,191 milli**. The eleven original inputs and instruction remain byte-identical to T01. See [public-output-verification.json](public-output-verification.json) and [input-and-delivery-integrity.json](input-and-delivery-integrity.json).

Frozen submission: [submission/pipeline.py](submission/pipeline.py), SHA-256 `a0f468639ddac231ba38fb8452ab7946b4651a2ee62ead69341bac042d5b7b4b`. User-facing files remain `/Users/roylin/code/tasks/T01-workspace/pipeline.py` and `output/results.json` (1,439,900 bytes; SHA-256 `dfcc0b13bea1b177aa5af6e9967f2bae42e8bfebdc6046f665fabf94db24ecf7`). Only standard-library static imports were found. The evaluator isolates the single file, so it cannot borrow neighboring self-written modules or answer artifacts. The model received no oracle values or repair instructions during the valid attempt.

CUA accessibility inspection directly shows the final completed response, exit code zero, metrics, and artifact buttons. Its final inline links display `[blocked]`, while artifact buttons and on-disk deliverables exist; this link presentation was not changed by the interruption repair.

A source review found no answer hardcoding, dynamic imports, network/subprocess use, or dependency on external answer files. Directly testing the submitted optimizer against 270 brute-force instances also passed. See [oracle/source-review.md](oracle/source-review.md).

An additional legal minimal case exposes signed-zero formatting: one Consumer order has Sales `1.000`, Profit `-0.001`, and Shipping Cost `0`. Decimal `ROUND_HALF_UP` at two places gives `-0.00`; the submission writes `0.00` for both `reconciliation.actual` and `reconciliation.reference` on Consumer / Total_Profit. The command exits zero and every other structured field matches the independent oracle. The original submission is preserved. See [oracle/negative-zero-evaluation.json](oracle/negative-zero-evaluation.json). This is **8/9 local scenarios** overall, not an official score.

Assessment: this single repaired-harness run completes the public workload and all seven initial generalization scenarios independently, with self-correction and exact optimization, but is not perfect on strict decimal string formatting. It does not establish a general success rate for DeepSeek Flash or a score on unavailable official hidden cases. Oracle self-checks are not counted as model results.

## Explicitly not claimed

No official hidden-test score, repeated-run statistical estimate, Windows runtime certification, release packaging, commit, or publication. The active app is a development bundle using the owned 4318 renderer server; `/Applications/AsterDream.app` remains the previous release. No broad unrelated formatting, type errors, sidebar behavior, or other Harness overlay settings were changed.
