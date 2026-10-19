# One authorized DeepSeek Flash desktop assessment

The new CUA assessment is complete as a measured failure. AsterDream R2 reached
`error` with `Agent stopped before completion: max_turn_requests`; the same
assistant anchor and execution were checked in native storage and the visible
window. There was one new submission, no paid retry, no manual candidate repair,
and no oracle feedback to the model. The nine earlier attempts remain intact.

| Measure | Observed result |
| --- | --- |
| Model | `deepseek/deepseek-flash` through the native ACP execution |
| Workspace | `/Users/roylin/code/tasks/T01-t032-cua-01` |
| Native task start | `2026-10-19T22:03:25.868Z` |
| Native assistant terminal update | `2026-10-19T22:34:58.799Z` |
| Whole task wall time | **1892.931 seconds: 31 minutes 32.931 seconds** |
| Normal model completion | **No** |
| Independent original-submission acceptance | **8/9; failed** |
| Persisted tool results | 50: 35 succeeded, 9 denied, 6 failed |
| Program / grep / code intelligence use | No calls to these tools in this assessment |

This wall time includes provider work, model repairs and self-checks, runtime
failures, CUA command review and permission waits. Build, controlled fixtures and
the independent oracle are excluded. Pure provider time and complete permission
wait durations cannot be recovered from the retained timestamps. There is no
qualified end-to-end speed improvement or comparison with the earlier 40-minute
run. The 50 `model.turn` facts are persisted tool decisions, not a measurement of
provider round trips.

## Original artifacts and acceptance

- [Original pipeline.py](t032-original-submission/pipeline.py): 25088 bytes,
  SHA-256 `1a078218164e6a895b4000a0423bef1420e153d97bb1834f470f6b1ade082438`.
- [Original output/results.json](t032-original-submission/output/results.json):
  1439900 bytes, SHA-256
  `dfcc0b13bea1b177aa5af6e9967f2bae42e8bfebdc6046f665fabf94db24ecf7`.
- Both files also remain at those relative paths in the task workspace above.
  [Freeze manifest](t032-submission-freeze.json) proves all 12 original input and
  instruction files matched their pre-submission hashes before acceptance.

The unchanged independent oracle SHA is
`36c4db2fb452ef738a228428598e38d567b4b77bf803b925b621078a416496ac`.
Its 270 DP/brute-force self-checks passed. Eight cases passed: public data,
zero budget, changed policy, changed data/references, row exclusivity, lexical
tie, cost tie and non-greedy selection. `negative_zero` failed because both
reconciliation `reference` and `actual` were `0.00` instead of `-0.00`.
The evaluator did not fix the candidate. Oracle process exit 1 is the measured
acceptance failure, not an infrastructure failure; oracle wall time was 1.041 s.
See [case results](t032-oracle.json) and [process evidence](t032-oracle-process.json).

## Runtime observations

The original R2 runtime and its assets were preserved throughout the assessment.
Its initial default Bash operations failed an OCI attachment inventory check.
The model subsequently requested host escalation; each actual host operation
remained under the existing approvals. The independent recursive canonical JSON
digest repair is qualified in [runtime evidence](runtime-carrier-canonical/OUTCOME.md).
R3's actual Linux guest fixture passed, but R3 was not substituted into this R2
assessment and is not credited with its result.

Two inner tool confirmations expired at the old 30-second policy deadline.
Late answers fell into a legacy opener and produced the original-owner error.
The new deadline/stale-answer repair has its own RED/GREEN record in
`approval-timeout/`; it does not retrospectively change these receipts.
Seven later calls were refused before execution as
`tool_effect_effective_arguments_unapproved`; their model decisions did not
request confirmation. Exact cause and scope are tracked separately, without
assuming an argument rewrite or weakening the permission boundary.

The original ACP process had exited by the final resource snapshot; it had no
remaining child processes. The desktop host remains open on the failed result.
No native mouse coordinates, pointer movement, app activation or hidden WebView
bridge were used to perform the CUA interactions. The final visible screenshot
is retained in the tool conversation, not claimed as a local PNG artifact.

Evidence: [native terminal metadata](t032-native-metadata-final.json),
[tool-only projection](t032-tool-final.json),
[CUA action observations](t032-cua-approval-observations.json),
[resource snapshot](t032-resources-final.json), and
[evaluation record](t032-evaluation.json).
