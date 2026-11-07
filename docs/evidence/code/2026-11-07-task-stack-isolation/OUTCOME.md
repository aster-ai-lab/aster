# Task stack isolation — OUTCOME

Date: 2026-11-07  
Host: macOS (darwin), `rustc 1.97.1 (8bab26f4f 2026-07-14)`  
Source: `349f7fe69` (worktree with this change)

## Problem

Interactive “task crash” mapped to ACP death. Local IPS reports
(`aster_core-*`, 2026-11-05/06) showed `stack_overflow` → `abort` while
parent `FactRun` awaited `ChildRun` on the same OS stack and
`jsonschema` / `serde_json::Value` clone recursed during
`LiveCompletion`.

`Box::pin` alone does not change the OS stack.

## Changes

1. **O1** — Foreground `ChildRun` in [`crates/code/core/src/tools/task.rs`](../../../../crates/code/core/src/tools/task.rs) runs on `tokio::spawn` and is always joined (panic/cancel mapped to typed task failure). Background path unchanged (already spawned).
2. **O2** — `LiveCompletion.catalog` is `Arc<[ToolDefinition]>`; each `complete` clones the Arc, not the schema trees ([`fact_control.rs`](../../../../crates/code/core/src/fact_control.rs)).
3. **O3** — `output_schema` depth/byte refuse (`MAX_TASK_OUTPUT_SCHEMA_DEPTH = 32`, 64 KiB) at task tool and `execute_with_task_id_scoped`; tool argument schemas get the same depth gate in [`registry.rs`](../../../../crates/code/core/src/tools/registry.rs).

## Layer A verification

Commands and counts (each `--exact`, exit 0):

| ID | Test | Result |
| --- | --- | --- |
| A-S1 | `tools::task::tests::foreground_child_run_survives_deep_legal_output_schema` | 1 passed |
| A-S2 | `tools::task::tests::isolated_child_still_honours_parent_cancellation` | 1 passed |
| A-S2b | `tools::task::tests::task_tool_parent_cancellation_reaches_child_llm_call` | 1 passed |
| A-S3 | `tools::task::tests::early_setup_failure_leaves_no_stale_canceller_entry` | 1 passed |
| A-S4 | `agent_api::tests::fact_log::child_policy::child_policy_actual_fact_run_restores_before_task_and_skill_delegation` | 1 passed |
| A-S5 | `tools::task::tests::output_schema_depth_helpers_match_generate_object_ceiling` | 1 passed |
| A-S5b | `tools::task::tests::task_tool_refuses_oversized_output_schema_without_running_child` | 1 passed |
| A-S6 | `fact_control::tests::live_completion_shares_tool_catalog_by_arc_across_completes` | 1 passed |

```bash
cargo test --locked -p aster-core --lib tools::task::tests::foreground_child_run_survives_deep_legal_output_schema -- --exact
cargo test --locked -p aster-core --lib tools::task::tests::isolated_child_still_honours_parent_cancellation -- --exact
cargo test --locked -p aster-core --lib tools::task::tests::task_tool_parent_cancellation_reaches_child_llm_call -- --exact
cargo test --locked -p aster-core --lib tools::task::tests::early_setup_failure_leaves_no_stale_canceller_entry -- --exact
cargo test --locked -p aster-core --lib agent_api::tests::fact_log::child_policy::child_policy_actual_fact_run_restores_before_task_and_skill_delegation -- --exact
cargo test --locked -p aster-core --lib tools::task::tests::output_schema_depth_helpers_match_generate_object_ceiling -- --exact
cargo test --locked -p aster-core --lib tools::task::tests::task_tool_refuses_oversized_output_schema_without_running_child -- --exact
cargo test --locked -p aster-core --lib fact_control::tests::live_completion_shares_tool_catalog_by_arc_across_completes -- --exact
```

**Clippy:** `cargo clippy --locked -p aster-core --all-targets --no-deps -- -D warnings` fails on **pre-existing** diagnostics outside this change (e.g. `windows_shell.rs`, tracing `let_unit_value`). No new findings in the edited `task.rs` / `fact_control.rs` / `registry.rs` paths.

## Explicitly not claimed

- Full 1,020-case TUI catalog or P0 platform matrix
- Layer C live-model paired `aster` / `aster-acp` campaign
- Studio desktop launch panics (`Aster-*.ips` / `tao`)
- Teardown-only `session_end.worker_join` / `ACP transport closed` races
- That raising `RUST_MIN_STACK` is required or sufficient
- Workspace-wide Clippy `-D warnings` green on this host
