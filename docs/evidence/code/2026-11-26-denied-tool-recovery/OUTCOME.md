# Tool refusal recovery

## Result

MacOS arm64 with Rust `1.97.1` at source base `d75f49664724` and this worktree's changes. The recovery branch uses the original Run, tool authorization and fact log. One refusal gives the next normal model request an opportunity to use an allowed alternative. A later tool round without fresh evidence stops with `run.tool_refusal_cycle`, including when the model changes the denied arguments.

## What landed

- Aster Core records a host-owned refusal classification on compatible tool result envelopes. The existing observation and authorization checks remain the source of execution authority. Identical failures with different parameters yield one information identity.
- Core parks a batch on `ask_user` before dispatching its other calls and drives successive questions and confirmations in the same Run. The focused test confirms that the write happens once after the question and approval.
- ACP preserves the difference between an explicit rejection, no answer before the deadline, cancellation and an unavailable approval client. CLI classifies stable tool progress stop codes and displays a recovery action without a generic resubmit prompt.

## Reproduction and focused checks

| Command or case | Observed result |
| --- | --- |
| Baseline `cargo test --locked -p aster-core --lib denied_bash_gives_one_bounded_opportunity_to_change_approach` | Failed as expected with 0 host recovery instructions instead of 1 |
| `cargo test --locked -p aster-core --lib -- denied_bash_gives_one_bounded_opportunity the_same_run_can_wait_for_a_question refusal_cycle permission_diagnostics convergence novel_information a_question_parks execution::refusal execution::waits recovery_limits` | 63 passed, 0 failed |
| `cargo test --locked -p aster-core --lib -- agent_api::goal_facade::tests::execution` | 117 passed, 0 failed. Covers existing Goal wait, completion, original-run settlement, cancellation, stale approvals, and budget retention on reopen. Some cases overlap the focused Core run above |
| `cargo test --locked -p aster-acp --lib -- permission_elicitation unanswered_asks permission_response turn_terminal_tests` | 20 passed, 0 failed |
| `cargo test --locked -p aster-acp --test acp_conformance_stdio --test approval_rules_stdio -- --test-threads=1` | 15 ACP conformance and 2 approval rule tests passed |
| `cargo build --locked -p aster -p aster-acp --bin aster --bin aster-acp` | Built the current CLI and ACP from the same checkout |
| `cargo test --locked -p aster-acp --test shipped_tui_e2e -- shipped_agent_recovers_from_a_refused_tool shipped_agent_refuses_repeated_reads shipped_tui_shows_recovery_and_an_actionable_stop --test-threads=1` with the paired binary overrides | 3 passed. Covers authorized recovery, bounded denials with changed arguments, the retained duplicate guard, and further input in the same CLI session after the stop notice |
| `cargo test --locked -p aster-acp --test live_interactive_dialogue_real_llm -- real_agent_recovers_from_an_outside_workspace_refusal_in_the_same_turn --ignored --test-threads=1` with `ASTER_CONFIG_FILE` | 1 passed in 9.09 s using `deepseek/deepseek-flash`. Actual receipt sequence includes a typed refusal followed by the permitted file read |
| `cargo test --locked -p aster-acp --test unattended_e2e -- convergence original_refusals child_refusal --test-threads=1` | 4 passed. The run record does not establish use of the staged copy. This suite selects Cargo's ACP binary unless it receives a verified `ASTER_LH_CANDIDATE_MANIFEST` |
| `cargo test --locked -p aster-pager --lib -- error_display turn_completion` | 173 passed and 4 failed. The new stop-message case passed in isolation. All four failures reproduce individually in both the pre-change and current Pager binaries |
| `cargo clippy --locked -p aster-core -p aster-acp -p aster-pager --all-targets --no-deps` | Exit 0 with warnings |
| `cargo check --locked --workspace --all-targets` | Exit 0 |
| `cargo check --locked -p aster-core --features ci-all,deep-live` | Exit 0 with warnings |
| `python3.12 tests/workspace_dependencies.py` | 7 passed |
| `cargo fmt --all -- --check` and `git diff --check` | Exit 0 |

The four Pager baseline failures are `driver_prompt_complete_without_prompt_id_arms_on_current`, `driver_prompt_complete_without_prompt_id_arms_reconcile_not_finish`, `driver_missing_prompt_id_arms_against_current_when_idle_in_turn`, and `hook_denied_finalize_requeues_blocked_prompt_and_opens_card`. Each was rerun with `--exact` using the pre-change binary `aster_pager-8ffa902e7b42e9a0` and the current binary `aster_pager-3bdcc8df77ee80ba`. Both sets reproduced all four failures.

The initial PTY fixture matched successful tool output from earlier turns. Scoping its scripted responses to the current user turn exposed both paths in the same session. The initial live harness could not load its default account model with an isolated home. Passing the frozen model as a launch argument selected the declared provider before session initialization.

## Native terminal run

The matched `aster 0.4.1` and `aster-acp 9.3.1` pair was built with the command above and copied to a temporary staging directory. The SHA-256 of `aster` was `9af5999a8cfe3688b5a26416d49d0cc4b22006f9551af0773222bbd6650bcf33`. The SHA-256 of `aster-acp` was `82c4a9d36f542bae6936e9ee2197efce43a3813fc210039518fb21cb2500d0cd`.

The launcher used `open -a Terminal <fixture>/Aster-Recovery-Verification.command`. It set `HOME` to an isolated fixture directory, supplied the paired helper with `ASTER_ACP_AGENT_BIN`, and ran `<built-pair>/aster --cwd <fixture>/workspace --fullscreen --no-plan --disable-web-search --model deepseek/deepseek-flash <isolation prompt>`. The prompt asked for an outside-workspace read first and an in-workspace read if the host refused. The existing provider config was referenced through `ASTER_CONFIG`.

The actual session log contained the following observations.

| Sequence | Original fact | Observation |
| --- | --- | --- |
| 7 | `tool.prepared` | First `read` |
| 8 | `tool.result` | `denied`, `permission_denied`, exit code absent |
| 10 | `tool.prepared` | Second `read` |
| 11 | `tool.result` | `succeeded`, exit code 0, result included the in-workspace marker |
| 13 | `turn.complete` | `narrative` |

`agent-desktop list-windows --app Terminal` identified the running fixture as `w-356`. Both `focus-window` and `screenshot` failed with `global_window_inventory_unstable` / `frontmost_window_join`. macOS `screencapture` also failed to capture the window or its bounds. The original tool facts establish the live terminal behavior. Desktop screenshot evidence is unavailable.

## Explicitly not claimed

- Automatic reconciliation of a command whose side effects lack a trusted status source, or persistence of a newly generated Goal blocker across a process restart.
- The existing Goal wait observation can reopen a step but does not authorize the original tool invocation. Continuation after process loss would require a trusted answer bound to that invocation. The same-process test uses the original confirmation authority.
- Automatic restart of a failed local ACP process, a new provider failover strategy, or a new wall-clock budget for unrelated long-running turns.
- Studio GUI verification, Windows or Linux runtime qualification, or release publication.
