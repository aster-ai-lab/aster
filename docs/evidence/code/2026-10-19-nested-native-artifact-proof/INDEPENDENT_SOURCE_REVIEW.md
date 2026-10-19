# Independent source safety review

Reviewer: `/root/native_execution`, acting read-only, with no source changes or
duplicate test runs. This records that reviewer's conclusions communicated
after reviewing the frozen five-source patch, its Program/Batch/governed-tool
callers and the new test sources. It does not claim a second independent test
execution.

The reviewer found no required safety changes and confirmed that production
sources could freeze. The reviewed hashes are the five `afterSha256` entries in
[source-sha256.json](source-sha256.json).

| Boundary | Reviewed conclusion |
| --- | --- |
| Prepared-call isolation | Each prepared executor clone receives a fresh private ledger; it does not inherit a previous owner or share the Turn's proof map. Nested calls create separate child ledgers and merge completed observations into their parent. |
| Authority | Only the registered native tool hook, invoked through the governed path, creates native proof. Program results, nested metadata and serialized checkpoint fields cannot populate the private proof map. Public tool APIs remain unchanged. |
| Actual effect binding | A child check transfers only onto an outer writer record with the same path and full final identity. Required kind, digest and mode comparisons remain, and the aggregate report re-observes the current workspace. |
| Failure and tampering | Failed enclosing calls do not transfer positive evidence. Unverified paths remain uncovered; a later writer, including identical-byte rewriting, invalidates earlier proof. Existing incomplete-observation and read-budget refusals remain. |
| Nested callers | The shared governed invoker covers Program and Batch without granting either orchestrator special metadata authority. The tests exercise Program → Batch → native calls and concurrent nested calls. |
| Lock ownership | Scope locks follow parent → child; sibling merges serialize through the parent. No reverse ownership cycle was identified. |

The reviewer also inspected the new Core isolation case: it preloads an earlier
owner, then actually runs two prepared calls concurrently; each observes only
its own marker. The ACP cases verify committed original receipts and aggregate
digests, and include five refusal scenarios after an already-verified top-level
artifact. Final observed results were supplied to the reviewer: ACP Office
group 19/19; Core native-proof group 5/5. See
[test-observations.json](test-observations.json) and the linked logs in
[OUTCOME.md](OUTCOME.md).

Limits: declared read-only tool contracts retain the existing observation
opt-out; undeclared tools receive conservative bounded scanning. This review
does not establish live GUI/task success, visual PNG quality or performance at
load. No permission boundary, completion waiver or terminal-state override was
introduced to make the tests pass.
