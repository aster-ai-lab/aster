# Aster TUI Task Crash Self-Healing & Long-Horizon Uninterrupted Execution

## Root-Cause Analysis (First Principles)

1. **In-turn resilience gaps (`crates/effect`, `crates/code/core`, `crates/code/acp`)**:
   - Provider `400`/`413`/`422` context-length-exceeded errors (`context_length_exceeded`, `maximum context length`, `prompt is too long`, etc.) were collapsed into opaque `NonRetryableLlmError` / `ActorError::Defect`, immediately failing the turn without triggering reactive compaction.
   - Reported `prompt_tokens` became stale during multi-step tool bursts unless intra-step tokens (`user.message`, `tool.result`, `question.answered`, background arrivals) accumulated into `tokens_since_model` so `effective_prompt_tokens = prompt_tokens + tokens_since_model` triggers proactive compaction cleanly.
   - `CodingPhase::Infer` used fixed-delay retries with `DEFAULT_MODEL_ATTEMPTS = 2`, giving only 1 retry on transient mid-stream disconnects before aborting long-horizon turns.
2. **Durable fact-log crash resume gap (`crates/effect`, `crates/code/core`, `crates/code/acp`)**:
   - Interrupted turns persisted on disk in `FileLog` lacked a read-only projection (`inspect_coding_phase` / `inspect_interrupted_turn`) to separate safe restartable checkpoints (`Idle`, `Resumable`) from ambiguous non-replay-safe prepared tool invocations (`UnresolvedTool`), and had no dedicated resume path that avoids appending duplicate `user.message` facts.
3. **Sidecar lifecycle & TUI auto-reconnect gap (`crates/terminal/crates/codegen/aster-pager`)**:
   - `spawn_required_sidecar` / `spawn_bridge` ran a single OS child process per TUI lifetime; an unexpected crash of `aster-acp` killed the TUI connection permanently with no supervisor loop, backoff, crash-loop guard, or `ConnectionStatus` broadcast.
   - TUI reconnect `InitializeRequest` and `LoadSessionRequest` dropped `fs_read`, `fs_write`, `terminal`, permission posture, and session model selection, painted noisy `TurnFailed` markers during disconnect windows, and lacked automatic continuation when the reloaded session reported `interruptedTurnResumable` or `activeGoalResumable`.

## Architecture Changes

- **Pillar 1 — Context-overflow self-healing & exponential retries**:
  - Added [`ActorError::ContextOverflow`](../../../../crates/effect/src/error.rs#L27-L29) and [`Effect::retry_exponential`](../../../../crates/effect/src/effect.rs) (cancellation-aware doubling capped at `max_delay`).
  - Updated [`SchedulerState`](../../../../crates/effect/src/coding.rs) (`tokens_since_model`, `overflow_recovered`) for cumulative post-model token tracking and added bounded single-shot reactive compaction in `CodingPhase::Infer` (`compaction-overflow:{turn}:{cycle}:{kept}` + `"infer-overflow:{turn}:{cycle}"` transition key to bypass `actor.rs` cause-dedup), plus `.retry_exponential(..., 30s)` backoff.
  - Classified HTTP `400 | 413 | 422` context-limit errors in [`llm/error.rs`](../../../../crates/code/core/src/llm/error.rs), mapped them to `ActorError::ContextOverflow` in [`fact_control/model_call.rs`](../../../../crates/code/core/src/fact_control/model_call.rs) (skipping duplicate `client.complete` fallback) and [`fact_reads.rs`](../../../../crates/code/core/src/fact_control/fact_reads.rs).
  - Raised `DEFAULT_MODEL_ATTEMPTS` to `5` across [`fact_control.rs`](../../../../crates/code/core/src/fact_control.rs) and [`turn.rs`](../../../../crates/code/acp/src/agent/turn.rs).
- **Pillar 2 — Durable fact-log inspection & non-duplicating resume**:
  - Implemented [`inspect_coding_phase`](../../../../crates/effect/src/coding.rs#L2160-L2187) (fixed budget ceiling to `u32::MAX` so `ModelDecision::Tool` is not misclassified as `Deny`) and [`inspect_interrupted_turn`](../../../../crates/code/core/src/fact_control.rs) (`InterruptedTurnState::{Idle, Resumable, UnresolvedTool}`).
  - Added [`AgentSession::interrupted_turn_state`](../../../../crates/code/core/src/agent_api/session_facade.rs) and [`AgentSession::stream_resume_interrupted_turn`](../../../../crates/code/core/src/agent_api/session_facade.rs) (wired via `OwnedConversationRequest::ResumeInterruptedTurn` in [`conversation_runtime.rs`](../../../../crates/code/core/src/agent_api/conversation_runtime.rs), [`owned_launch.rs`](../../../../crates/code/core/src/agent_api/conversation_runtime/owned_launch.rs), [`owned_worker.rs`](../../../../crates/code/core/src/agent_api/conversation_runtime/owned_worker.rs)), refusing `Idle` and `UnresolvedTool`.
  - Exposed `interruptedTurnResumable`, `interruptedTurnUnresolvedTool`, and `activeGoalResumable` in `LoadSessionResponse._meta` and handled `aster/resumeInterruptedTurn` on `session/prompt` in [`protocol.rs`](../../../../crates/code/acp/src/agent/protocol.rs) and [`turn.rs`](../../../../crates/code/acp/src/agent/turn.rs) (updating caller in [`goal_ext.rs`](../../../../crates/code/acp/src/agent/goal_ext.rs)).
- **Pillar 3 — Supervised self-healing `aster-acp` sidecar & TUI auto-resume**:
  - Added `status_rx` on [`SpawnedAgent`](../../../../crates/terminal/crates/codegen/aster-pager/src/acp/spawn.rs#L97-L111) and implemented [`spawn_supervised_sidecar` / `spawn_supervised_bridge`](../../../../crates/terminal/crates/codegen/aster-pager/src/acp/aster_spawn.rs) with shared `run_child_generation`, generation-0 spawn-readiness wait (`Notify`), immediate local RPC task cleanup (`gen_tasks.abort()` + `yield_now()`), fast fail across all 10 `AcpAgentMessage` variants during restart windows, exponential backoff, crash-loop cap (`5` restarts / `60s`), and fail-closed non-restart on oversized frame-limit errors.
  - Preserved `InitializeRequest` capabilities/meta across reconnects in [`acp/mod.rs`](../../../../crates/terminal/crates/codegen/aster-pager/src/acp/mod.rs) and [`event_loop.rs`](../../../../crates/terminal/crates/codegen/aster-pager/src/app/event_loop.rs), preserved `permissionPosture: "plan"` and `aster/sessionModelId` in `plan_reconnect_load`, suppressed transient `TurnFailed` blocks/notifications during reconnect windows in [`dispatch/prompt.rs`](../../../../crates/terminal/crates/codegen/aster-pager/src/app/dispatch/prompt.rs), and added bounded automatic continuation (`crash_auto_resume_count < 3`) via `Effect::ResumeInterruptedTurn` or `/goal resume` while surfacing non-resumable `UnresolvedTool` boundaries cleanly.
- **P0 Self-Healing Follow-ups (Read-only Safe Replay, Sidecar Heartbeat Watchdog, Mid-Turn `SIGKILL` E2E)**:
  - **Read-only idempotent tool safe replay across crashes**: Added `ToolRecoveryPolicy::ReplaySafe` (`"replay_safe"`) in [`tool_effects.rs`](../../../../crates/effect/src/coding/tool_effects.rs) (`round-trip` test in [`tool_effects/tests.rs`](../../../../crates/effect/src/coding/tool_effects/tests.rs)). Updated [`coding.rs`](../../../../crates/effect/src/coding.rs) (`is_replay_safe_batch`, `replay_tool_batch`, `inspect_coding_phase`) so durable `tool.prepared` batches whose recovery is `ReplaySafe` report `has_unresolved_preparation == false` and replay execution cleanly via `tool-replay:{turn}:{cycle}:{ids}` without duplicating `tool.prepared` (re-validating tool name, original/effective argument digests, and policy, then rebinding receipts to `original_prepared.identity` / `original_prepared.digest()`). Updated [`receipts.rs`](../../../../crates/code/core/src/fact_control/receipts.rs) to mark `read_only && idempotent` tools as `ReplaySafe` (keeping interactive `read_only` tools like `ask_user` as `NeverReplay`) and avoid treating `ReplaySafe` preparations as unresolved debt in `fold_receipts`.
  - **Sidecar hang / liveness heartbeat watchdog**: Added closed-set [`HostMethod::Health`](../../../../crates/code/acp/src/agent/host_methods.rs) (`"aster/health"` -> `{"ok":true}`) and updated the wire fixture [`acp_wire_surface_v1.json`](../../../../crates/code/acp/tests/fixtures/acp_wire_surface_v1.json). Added `heartbeat_interval: Some(15s)` / `heartbeat_timeout: 10s` on `SidecarRestartPolicy`, outbound activity timestamp tracking via atomic monotonic elapsed millis, and [`sidecar_heartbeat_watchdog`](../../../../crates/terminal/crates/codegen/aster-pager/src/acp/aster_spawn.rs) (`MethodNotFound` treated as responsive for older binaries) so silent/deadlocked `aster-acp` children are killed and respawned by `spawn_supervised_bridge`.
  - **Mid-turn `SIGKILL` auto-resume real PTY E2E**: Added [`shipped_tui_mid_turn_sidecar_crash_auto_resumes_without_duplicating_prompt_e2e`](../../../../crates/code/acp/tests/shipped_tui_e2e.rs#L4914) exercising live `SIGKILL` of `aster-acp` gen 1 while an in-flight model HTTP request is pending, verifying supervised respawn, `TurnFailed` suppression, automatic `ResumeInterruptedTurn`, clean completion (`SELF-HEAL-MIDTURN-RESUMED-OK`), and exact count `1` of `"type":"user.message"` in the durable log.
  - **Live `kimi-01/k3.1-pro` PTY Crash Self-Healing E2E**: Added [`LiveTui::wrap_sidecar_pids`](../../../../crates/code/acp/tests/aster_tui_live_e2e/driver.rs) and [`kimi_01_tui_read_only_replay_safe_and_sidecar_crash_self_heal`](../../../../crates/code/acp/tests/aster_tui_live_e2e.rs) verifying real `kimi-01/k3.1-pro` read tool preparation with `"recovery": "replay_safe"`, live `SIGKILL` gen 1 respawn + history recall, and live mid-turn `SIGKILL` gen 2 auto-respawn (gen 3) + auto-resume completion without `"Turn failed"` or duplicate `user.message`.

## Build Identity & Environment

- Host: `Darwin 27.2.0 arm64`, `rustc 1.97.1 (8bab26f4f 2026-07-14)`
- `cargo build --locked -p aster -p aster-acp --bin aster --bin aster-acp`
  - `./target/debug/aster --build-id`: `aster-cli-pair-v1:04ec9ee98a02cda70caacdb9957bc14c4037bf5d:aarch64-apple-darwin`
  - `./target/debug/aster-acp --build-id`: `aster-cli-pair-v1:04ec9ee98a02cda70caacdb9957bc14c4037bf5d:aarch64-apple-darwin`
  - SHA-256 (`./target/debug/aster`): `5c68534e409746858b8de4b7c29592b12153da645eb7f1363cea7e1713cb6307`
  - SHA-256 (`./target/debug/aster-acp`): `96465f6a09af497224084ea644c52c9ad6ff0dc3d1e1c3cbd7ab0ecae1cc1299`

## Verification Results

All checks below ran after the final edit to each owning crate:

1. `cargo test --locked -p aster-effect --lib -- --skip long_fresh_observation_trajectories_deliver_every_observation`
   - Result: **106 passed; 0 failed; 1 ignored; 1 filtered out** (`1.50s`)
   - Verified `replay_safe_preparation_replays_cleanly_on_resume_without_unresolved_debt`, `closed_sets_refuse_unknown_values_and_round_trip`, `large_post_model_tool_burst_triggers_proactive_compaction_with_reported_usage`, `context_overflow_triggers_bounded_emergency_compaction_and_recovers`, and `inspect_coding_phase_distinguishes_idle_resumable_and_unresolved_tool`.
2. `cargo test --locked -p aster-effect --test effect_ops`
   - Result: **16 passed; 0 failed** (`0.09s`)
   - Verified `retry_exponential_doubles_delay_up_to_cap_and_cancels_cleanly`.
3. `cargo test --locked -p aster-core --lib -- llm::error context_overflow convergence::interrupted_ --test-threads=4`
   - Result: **10 passed; 0 failed** (`6106 filtered out`, `0.19s`)
   - Verified `interrupted_turn_resumes_from_durable_fact_log_without_duplicating_user_message_and_refuses_unresolved_tool` and `interrupted_read_only_tool_replays_safely_on_resume_without_duplicating_preparation`.
   - Prior baseline `aster-core` checks (`fact_control_tests::effort_tool_rounds`, `fact_control::no_count_limit_tests`, `fact_control::receipts::tests::refusal_cycle`, `mid_stream_failures_retry_beyond_two_attempts_and_complete`): verified in prior baseline pass (`27 passed; 0 failed`).
4. `cargo test --locked -p aster-acp --lib -- host_methods`
   - Result: **16 passed; 0 failed** (`556 filtered out`, `1.53s`)
   - Verified `host_method_wire_names_match_the_golden_surface` and `every_host_method_parses_back_from_its_name` including `HostMethod::Health` (`"aster/health"`).
5. `cargo test --locked -p aster-pager --lib -- aster_spawn event_loop::tests::plan_reconnect event_loop::tests::reconnect dispatch::tests::prompt::handle_prompt_response_suppresses_turn_failed_during_sidecar_reconnect --test-threads=4`
   - Result: **29 passed; 0 failed** (`10391 filtered out`, `8.10s`)
   - Verified `supervised_sidecar_respawns_after_crash_and_restores_rpc`, `supervised_sidecar_restarts_hung_child_on_heartbeat_timeout`, `supervised_sidecar_fails_closed_without_restart_on_oversized_frame`, `supervised_sidecar_stops_after_restart_budget_exhausted`, `plan_reconnect_load_preserves_plan_posture_and_model_when_populated`, and `handle_prompt_response_suppresses_turn_failed_during_sidecar_reconnect`.
6. `ASTER_TUI_BIN="$PWD/target/debug/aster" ASTER_ACP_BIN="$PWD/target/debug/aster-acp" cargo test --locked -p aster-acp --test shipped_tui_e2e -- recovery_e2e self_heal mid_turn_sidecar_crash --nocapture`
   - Result: **7 passed; 0 failed** (`43 filtered out`, `4.38s`)
   - Verified `shipped_tui_sidecar_crash_self_heals_and_continues_e2e`, `shipped_tui_long_horizon_stream_drop_and_context_overflow_self_heal_e2e`, and `shipped_tui_mid_turn_sidecar_crash_auto_resumes_without_duplicating_prompt_e2e` (live mid-turn `SIGKILL`, supervised respawn, `TurnFailed` suppression, automatic `ResumeInterruptedTurn`, single `user.message` fact).
7. `rustfmt --edition 2024 --config skip_children=true --check <all 10 P0 modified .rs files + 2 live E2E .rs files> && git diff --check`
   - Result: **0 errors, exit code 0**
8. `cargo clippy --locked -p aster-effect -p aster-core -p aster-acp -p aster-pager --all-targets --no-deps` (plus targeted `--test aster_tui_live_e2e`)
   - Result: **exit code 0**; no new Clippy errors or warnings in edited lines.
9. Real `kimi-01/k3.1-pro` E2E against operator ACL (`ASTER_CONFIG_FILE=<repo>/../config.acl`, no credentials printed/copied):
   - `ASTER_CONFIG_FILE=<repo>/../config.acl ASTER_TUI_BIN="$PWD/target/debug/aster" ASTER_ACP_BIN="$PWD/target/debug/aster-acp" cargo test --locked -p aster-acp --test aster_tui_live_e2e -- kimi_01_ --ignored --test-threads=1 --nocapture`
     - Result: **2 passed; 0 failed** (`115 filtered out`, `141.16s`)
     - Verified `kimi_01_tui_compacts_and_plans_then_reopens_the_same_goal` (compaction recall, `/goal` planning, `--resume` goal reopen) and `kimi_01_tui_read_only_replay_safe_and_sidecar_crash_self_heal` (Turn 1 real `read` with `recovery="replay_safe"`; Turn 2 post-`SIGKILL` gen 2 respawn + recall; Turn 3 mid-turn `SIGKILL` gen 3 auto-respawn + auto-resume completion without `Turn failed` or duplicate `user.message`).
   - `ASTER_CONFIG_FILE=<repo>/../config.acl ASTER_LIVE_MODEL=kimi-01/k3.1-pro ASTER_ACP_BIN="$PWD/target/debug/aster-acp" cargo test --locked -p aster-acp --test live_interactive_dialogue_real_llm -- real_agent_answers_then_approves_a_single_write_and_reopens --ignored --test-threads=1 --nocapture`
     - Result: **1 passed; 0 failed** (`10 filtered out`, `36.52s`)
     - Verified real `kimi-01/k3.1-pro` `ask_user` elicitation + `allow-once` `write` approval + cold reopen + `read` verification.
   - Pre-existing `live_tui_prompt` harness check (`cargo test --locked -p aster-acp --test live_tui_prompt -- --ignored --test-threads=1`): failed on `default_model cursor/auto` without `ASTER_DEFAULT_MODEL`, and with `ASTER_DEFAULT_MODEL=kimi-01/k3.1-pro` panicked inside untouched `live_tui_prompt.rs` (`spawn_local` called outside `tokio::task::LocalSet`).

## Long-Horizon Evaluation (`kimi-01/k3.1-pro`)

All runs below used the matching binary pair (`pair_build_id=aster-cli-pair-v1:04ec9ee98a02cda70caacdb9957bc14c4037bf5d:aarch64-apple-darwin`) against `../config.acl` (`ASTER_LIVE_MODEL=kimi-01/k3.1-pro`) with no edits to frozen prompts, inputs, timeouts, or graders:

1. **6-Turn Multi-File Dynamic Reconciliation (`long_horizon_real_llm`)**
   - Command: `ASTER_CONFIG_FILE=/Volumes/GXF-Q-8T/config.acl ASTER_LIVE_MODEL=kimi-01/k3.1-pro ASTER_LH_EVIDENCE_DIR="$PWD/docs/evidence/code/2026-11-28-tui-crash-self-healing-and-long-horizon/long-horizon-eval" cargo test --locked -p aster-acp --test long_horizon_real_llm -- real_native_file_workflow_multiple_user_turns --ignored --test-threads=1 --nocapture`
   - Result: **PASS** (`1 passed; 0 failed`, `337.79s`, `elapsed_ms=250301`)
   - Metrics: `turns_passed=6/6`, `tool_calls=18`, `permission_requests=6`, `upstream_calls=30` (`200: 30`, `transport_failures=0`, `rejected=0`), `tokens=237716`, `identity=true`, `oracle=true`, `terminal=true`, `cleanup=true`, `current_artifact=true`, `side_effects=true`.
   - Evidence: [`result.json`](./long-horizon-eval/native-multiturn-diagnostic-82d32a52-bee2-4be3-9e62-e9aa840f65b5/result.json)

2. **Single-Run 7-Version Changing Feed + Forced Compaction (`deloop_real_llm`, 4096 output-token qualification budget)**
   - Command: `ASTER_CONFIG_FILE=/Volumes/GXF-Q-8T/config.acl ASTER_LIVE_MODEL=kimi-01/k3.1-pro ASTER_LH_EVIDENCE_DIR="$PWD/docs/evidence/code/2026-11-28-tui-crash-self-healing-and-long-horizon/long-horizon-eval" cargo test --locked -p aster-acp --test deloop_real_llm -- changing_external_batches_survive_compaction_in_one_real_acp_run --ignored --test-threads=1 --nocapture`
   - Result: **FAIL** (`0 passed; 1 failed`, `626.67s`, `elapsed_ms=513380`; `workflow_pass=false`, `mechanism_exercised=true`)
   - Metrics & Root Cause: `published_versions=7`, `completed_feed_reads=7`, `same_argument_information_identities=7`, `compactions=6`, `upstream_calls=16` (`200: 16`, `transport_failures=0`), `overflow=false`, `tools_terminal=true`, `cleanup=Ok`. After reading all 7 batches cleanly across 6 compactions, model call 16 hit the 4096 output-token cap (`completion_tokens=4096`, `reasoning_tokens=4093`), causing `forced="response_truncated"` before writing `report.json`.
   - Evidence: [`result.json`](./long-horizon-eval/deloop-changing-feed-61bf86de-055d-45a3-bd4b-28249953fe0c/result.json)

3. **Single-Run 7-Version Changing Feed + Forced Compaction (`deloop_real_llm`, 8192 output-token capacity diagnostic)**
   - Command: `ASTER_CONFIG_FILE=/Volumes/GXF-Q-8T/config.acl ASTER_LIVE_MODEL=kimi-01/k3.1-pro ASTER_LH_EVIDENCE_DIR="$PWD/docs/evidence/code/2026-11-28-tui-crash-self-healing-and-long-horizon/long-horizon-eval" cargo test --locked -p aster-acp --test deloop_real_llm -- changing_external_batches_capacity_diagnostic_with_8192_output --ignored --test-threads=1 --nocapture`
   - Result: **PASS** (`1 passed; 0 failed`, `690.33s`, `elapsed_ms=579816`; `workflow_pass=true`, `mechanism_exercised=true`)
   - Metrics: `published_versions=7`, `completed_feed_reads=7`, `same_argument_information_identities=7`, `compactions=6`, `oracle_correct=true` (`total_cents=999024`), `bound_verified_outcome=true`, `overflow=false`, `tools_terminal=true`, `cleanup=Ok`, `upstream_calls=19` (`200: 18`, `transport_failures=1` — recovered mid-run without failing the turn).
   - Evidence: [`result.json`](./long-horizon-eval/deloop-changing-feed-8k-66abbb0a-307e-4c7b-a138-8517b5d410ba/result.json)

## Pre-existing Unrelated Dirtiness Observed (Untouched)

- `agent_api::tests::fact_log::no_count_limit::explicit_positive_tool_count_still_refuses_the_next_call` failed with `model returned a tool invocation twice for a request with no tools` due to an unrelated pre-existing uncommitted diff in `crates/code/core/src/fact_control.rs`.
- `recovery::shipped_tui_questions_before_approving_a_batched_write` and 3 `boundary_e2e` tests were excluded from the `shipped_tui_e2e` filter due to pre-existing uncommitted `DiskContentAttestor` changes in `crates/code/acp/src/agent/session_open.rs`.
- `crates/code/acp/tests/live_tui_prompt.rs` uses `#[tokio::test(flavor = "multi_thread")]` without `tokio::task::LocalSet`, causing `spawn_local` to panic when running `AsterCodeAgent::new` in-process (the CLI/PTY suites `aster_tui_live_e2e` and `live_interactive_dialogue_real_llm` exercise the full binary pair and passed).

## Explicitly Not Claimed

- Installed binaries at `~/.local/bin/aster` and `~/.local/bin/aster-acp` were **not** overwritten (only checkout binaries under `target/debug/` were built and verified).
- Linux targets and Windows targets were not executed in this verification run (all unit, mock, and live `kimi-01/k3.1-pro` E2E runs executed on macOS `aarch64-apple-darwin`).
- Unattended `live_tui_prompt` in-process test failed due to the untouched `LocalSet` test bug noted above, and non-`kimi_01_` `#[ignore]` live tests in `aster_tui_live_e2e` were filtered out.
