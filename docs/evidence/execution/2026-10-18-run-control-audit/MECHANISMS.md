# Read-only delegation, budget and context audit

Date: 2026-10-18. No mechanism or UI change was made for this audit.

Each filter below was executed as
`cargo test -p a3s-code-core --lib <filter> -- --nocapture` from the root
with Rust 1.97.1. They can also be passed together after `--` as libtest
filters to reuse one Cargo build.

| Filter | Passed | Test time | Direct evidence |
| --- | ---: | ---: | --- |
| `tools::task::tests::child_policy` | 3 | 0.70 s | Normal/structured children inherit the exact current policy and budget; no recursive delegation tools |
| `tools::task::tests::parallel_task_executor_` | 4 | 1.89 s | Concurrent children, stable input order, configured concurrency, independent child failures and terminal events |
| `tools::task::tests::task_child_and_schema_coercion_share_the_scoped_budget_boundary` | 1 | 0.30 s | Every provider attempt is checked; only successful calls report usage, including structured-output coercion |
| `tools::task::tests::task_tool_parent_cancellation_reaches_child_llm_call` | 1 | 0.42 s | Parent cancellation reaches child generation |
| `tools::task::tests::delegated_child_skips_redundant_pre_analysis_model_call` | 1 | 0.28 s | Redundant child pre-analysis call count is zero |
| `tools::task::tests::task_child_run_` | 4 | 0.52 s | Allow, deny, confirmation and step-budget boundaries; an allowed unverified write still fails completion |
| `compaction::` | 35 | 0.20 s | Safe tool-call/result pairing, bounded recent output, UTF-8 boundaries, rolling summaries, active goals/constraints/open steps and target token watermarks |
| `orchestration::workflow_budget::tests` | 4 | 0.01 s | Shared accumulation, cap refusal, remaining tokens and inner host accounting |
| `agent_api::conversation_runtime::typed_interruption_tests::budget_denial_tests` | 1 | 0.84 s | One denied admission and no provider call on four send/stream surfaces; same session resumes after host allocation replenishment |
| `agent_api::direct_tools::tests::host_direct_calls_still_obey_budget_before_side_effects` | 1 | 0.04 s | Direct calls cannot bypass the pre-effect host budget |
| `agent_api::tests::compact` | 8 | 1.13 s | Real session summary facts, source carry, invalid source/writer refusal, budget refusal before provider and close cancellation |
| `fact_control::tests::auto_compact` | 2 | 0.29 s | Actual fact engine compacts on context/provider-token thresholds and continues with recent tool pairs |
| `concurrent_parallel_batches_share_eight_slots_and_execute_each_branch_once` | 1 | 1.34 s | Four concurrent batches × seven branches: maximum active generations is exactly eight and successful provider calls equal exactly 28 |
| `source_digest_excludes_harness_state_and_tracks_source_edits` and `soak_parent_fanout_cancel_leaves_no_child_or_source_change` with `--include-ignored` | 2 | 8.86 s | Source digest ignores only the existing official harness-state boundary; real source edits and ordinary hidden files still change it; 20 fan-out/cancel cycles leave no child or source mutation |

Total: **68 passed, 0 failed** in the final filters above. Test times exclude Cargo
build/cache-lock time and are hermetic fixture measurements, not live-provider
throughput or latency benchmarks.

## Ignored soak failure, reproduced and corrected

Command:

`cargo test -p a3s-code-core --lib soak_parent_fanout_cancel_leaves_no_child_or_source_change -- --ignored --nocapture`

The test ran all 20 parent fan-out/cancellation cycles. Every cycle's
maximum-eight-admissions and no-pending-child assertions passed. Its final
`source_digest` comparison failed (7.83 s). An isolated rerun with a
repository-local `TMPDIR` failed at the same assertion (8.73 s).

A read-only 50 ms filesystem observer saw exactly 320 created files, all
under `.a3s/child-runs/<child-id>/`: each of 160 admitted children retained
its `writer.lock` and fact-log `.jsonl`. No observed path was outside this
harness-state subtree. The digest helper currently walks every file in the
temporary workspace, so it labels this legitimate durable child state as a
source mutation. This is a confirmed test observation-domain defect; it is
not evidence that source files were edited or that a child stayed live.

After the isolated reproduction, the root task authorized a test-only
correction. `source_digest` now uses the existing
`porcelain::is_harness_path` boundary. A new small test proves that an actual
source-file edit and an ordinary hidden source file still change the digest.
No production code or allowlist changed for this correction.

The final command rebuilt the current test source and passed both checks:

`cargo test -p a3s-code-core --lib -- source_digest_excludes_harness_state_and_tracks_source_edits soak_parent_fanout_cancel_leaves_no_child_or_source_change --include-ignored --nocapture`

Result: **2 passed, 0 failed**, 8.86 s test time. The original 20-cycle soak
is now included in the successful 68-test count above. Its two earlier
failed attempts remain recorded here and are not counted as passing runs.

## Confirmed limits and not claimed

- `WorkflowBudget` is documented as a soft aggregate cost ceiling: usage is
  recorded after a provider call returns, so parallel calls already in flight
  may overshoot it. These tests do not prove a hard per-token reservation cap.
- No live model, retrieval service, production-size document corpus or
  long-running desktop restart campaign was used.
- No claim that every delegation/background/recovery combination is covered.
- The soak's lifecycle and source-isolation claims apply to its hermetic
  20-cycle fixture, not arbitrary real providers or unbounded task graphs.
