# Systemic Tool Refusal-Cycle Root-Cause Analysis & Fix Verification

Date: 2026-11-28
Platform / Toolchain: `Darwin 27.2.0 arm64`, `rustc 1.97.1 (8bab26f4f 2026-07-14)`

## Root-Cause Gaps Addressed (Systemic Layer)

Following earlier per-tool fixes (`read`, `bash`, `background`), remaining systemic coupling points could still trigger spurious `Tool progress blocked: tool '...' repeated a refusal without fresh original progress; change the approach or obtain the required host input` (`run.tool_refusal_cycle`) failures or opaque denials:

1. **Same-`model.turn` multi-cycle refusal overcounting (`crates/code/core/src/fact_control/receipts/refusal.rs`)**:
   - When a single `model.turn` emitted multiple tool calls that split across scheduler `cycle`s (for instance, on `needs_confirmation`) and were both refused without an intervening model recovery turn, `fold_execution_control` keyed on `(turn, cycle)`, counting one model response wave as multiple refusal turns.
   - `fold_execution_control` now resets on `"user.message" | "question.answered"`, tracks the model emission wave via `model.turn` facts (`PreparedAction { tool, args, wave }`), and folds calls split from the same `model.turn` wave under `(turn, wave)` when `model.turn` facts exist (preserving synthetic `(turn, cycle)` behavior when folding logs without `model.turn` facts). `RefusedArg` keys on `(round, batch_index, call_id)` so multi-call waves remain distinguishable.

2. **Scheduler duplicate counters across `background.output` & `question.answered` (`crates/effect/src/coding.rs`)**:
   - `on_background_output` injected live async output from running background tasks without clearing stale `"background"` entries in `state.duplicate_calls`, risking false duplicate blocks on follow-up `background` calls after async output arrived.
   - `"question.answered"` represents new external host guidance via `ask_user`, but previously did not reset `state.duplicate_calls` like `"user.message"`.
   - `on_background_output` now runs `state.duplicate_calls.retain(|(tool, _), _| tool != "background")`, and `"question.answered"` now clears `state.duplicate_calls` (with `duplicate_reset_pending = false`).

3. **Read-only tool in-workspace symlink parity (`crates/code/core/src/permissions/interactive.rs`)**:
   - After `bash` and `read` migrated to allow in-workspace symlinks (`follow_terminal_symlink = false`), read-only inspection tools (`office_read`, `search`, `ls`, `code_symbols`, `code_navigation`, `code_diagnostics`) remained routed through `local_path_crosses_symlink` (`follow_terminal_symlink = true`), rejecting valid in-workspace symlinks as `symlink_boundary_escape`.
   - Those read-only inspection tools now use `read_path_crosses_boundary` (`path_crosses_symlink_boundary(root, path, false)`), while mutating/file-writing tools (`write`, `edit`, `patch`, `download`, `office_file`) keep strict `local_path_crosses_symlink` (`follow_terminal_symlink = true`), and symlinks targeting outside the workspace remain denied for all tools.

4. **Plan-mode refusal diagnostics (`crates/code/core/src/permissions/interactive.rs`, `crates/code/core/src/fact_control/receipts.rs`)**:
   - `InteractiveToolGuardrail::denial_code` previously returned `None` when `InteractiveApprovalMode::Plan` denied bounded/high-risk tools, losing structured denial specificity in host recovery messages.
   - `denial_code` now returns `Some("plan_mode_read_only")` after critical boundary/catastrophic checks, and `permission_refusal_hint` explains read-only Plan mode constraints and non-mutating alternatives (or asking the user / switching modes).

## Modified Files

- `crates/effect/src/coding.rs`
- `crates/effect/src/coding/revalidation_tests.rs`
- `crates/code/core/src/fact_control/receipts.rs`
- `crates/code/core/src/fact_control/receipts/refusal.rs`
- `crates/code/core/src/fact_control/receipts/tests/refusal_cycle.rs`
- `crates/code/core/src/fact_control/receipts/tests/permission_diagnostics.rs`
- `crates/code/core/src/permissions/interactive.rs`
- `crates/code/core/src/permissions/tests.rs`
- `crates/code/core/src/agent_api/tests/fact_log/convergence.rs`
- `crates/code/acp/tests/shipped_tui_e2e.rs`

## Built Test Binaries

- `cargo build --locked -p aster -p aster-acp --bin aster --bin aster-acp`
  - `./target/debug/aster --build-id`: `aster-cli-pair-v1:9374ad38bfa82b3301eb0b20cd4d62a93eb70b61:aarch64-apple-darwin`
    - SHA-256: `09bf59bc24baeaf14e13e71bedf215d1b288f65cf96a237219162e4d27119ba1`
  - `./target/debug/aster-acp --build-id`: `aster-cli-pair-v1:9374ad38bfa82b3301eb0b20cd4d62a93eb70b61:aarch64-apple-darwin`
    - SHA-256: `5e6a48c74c38da0c4d67c85c1207400c344da73d0764cf8b4096f94527bfd254`

## Verification Commands & Observed Counts

1. `aster-effect` unit suite (after final edit to `crates/effect/*`):
   - Command: `cargo test --locked -p aster-effect --lib -- --skip long_fresh_observation_trajectories_deliver_every_observation`
   - Result: `test result: ok. 102 passed; 0 failed; 1 ignored; 0 measured; 1 filtered out`
   - Includes new test `coding::revalidation_tests::question_answered_and_background_output_reset_relevant_duplicate_counts`.

2. `aster-core` focused unit & integration suite:
   - Command: `cargo test --locked -p aster-core --lib -- background host_access permission_ shell::tests governed_tests::runtime_permission_preserves_the_winning_denial_reason refusal_cycle convergence argument_validation agent_api::goal_facade::tests::execution::refusal sandbox_availability --test-threads=4`
   - Result: `test result: ok. 237 passed; 0 failed; 0 ignored; 0 measured; 5874 filtered out`
   - Includes new tests:
     - `fact_control::receipts::tests::refusal_cycle::split_confirmation_batch_within_one_model_turn_allows_recovery_and_question_answered_resets`
     - `permissions::tests::read_only_tools_allow_in_workspace_symlinks_while_writes_and_escapes_deny_and_plan_names_code`
     - `fact_control::receipts::tests::permission_diagnostics::plan_mode_denial_explains_read_only_plan_mode`
     - `agent_api::tests::fact_log::convergence::same_model_turn_split_confirm_denials_and_plan_mode_recovery_complete_without_refusal_cycle`

3. Real PTY `aster` + `aster-acp` TUI recovery E2E:
   - Command: `ASTER_TUI_BIN="$PWD/target/debug/aster" ASTER_ACP_BIN="$PWD/target/debug/aster-acp" cargo test --locked -p aster-acp --test shipped_tui_e2e -- recovery_e2e --nocapture`
   - Result: `test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 43 filtered out`
   - Passed tests:
     - `shipped_tui_read_recovery_e2e_completes_productive_rereads_twin_batch_and_bad_batch_schema`
     - `shipped_tui_bash_symlink_and_recovery_e2e_recovers_and_completes`
     - `shipped_tui_background_yolo_recovery_e2e_completes_without_refusal_cycle`
     - `shipped_tui_split_batch_and_readonly_symlink_recovery_e2e_completes`

4. ACP unattended refusal E2E:
   - Command: `ASTER_TUI_BIN="$PWD/target/debug/aster" ASTER_ACP_BIN="$PWD/target/debug/aster-acp" cargo test --locked -p aster-acp --test unattended_e2e -- refusal --nocapture`
   - Result: `test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 32 filtered out`
   - Passed tests:
     - `an_interactive_run_stops_generating_after_repeated_original_refusals`
     - `a_delegated_child_refusal_returns_to_the_actual_acp_parent`
     - `provider_failures::exhausted_provider_after_refusal_retains_the_original_denial`

5. Formatting, whitespace/diff hygiene, and Clippy:
   - `rustfmt --edition 2021 --config skip_children=true --check <all 10 modified .rs files>` -> exit `0`
   - `git diff --check` -> exit `0`
   - `cargo clippy --locked -p aster-effect -p aster-core -p aster-acp --all-targets --no-deps` -> exit `0` (`0` warnings in modified code; `1` pre-existing warning on untouched line `crates/code/acp/tests/shipped_tui_e2e.rs:3355:44`)

## Explicitly not claimed

- Full workspace test sweep (`cargo test --locked --workspace`) and the remaining filtered tests outside the targeted suites above.
- `coding::revalidation_tests::long_fresh_observation_trajectories_deliver_every_observation` in `aster-effect` (explicitly skipped).
- Pre-existing dirty-tree `DiskContentAttestor` behavior in `crates/code/acp/src/agent/session_open.rs` affecting unrelated `shipped_tui_questions_before_approving_a_batched_write` / `boundary_e2e` auto-verification tests.
- Windows and Linux native runtime execution (only `Darwin 27.2.0 arm64` was executed).
- `cargo fmt --all -- --check` across unrelated pre-existing dirty files in the repository.
- Live external LLM provider behavior beyond deterministic mock HTTP/scripted clients, and updating `/Users/roylin/.local/bin/aster` or `/Users/roylin/.local/bin/aster-acp` (not overwritten).
