# Recovery after a different tool is refused

The reported turn tried `background`, then the shell, then ended with `run.tool_refusal_cycle` naming `background`. The previous receipt fold treated every later tool wave without new evidence as a repeated refusal of the first tool. The regression uses a sandbox that advertises background execution but refuses commands during preflight. On the previous fold, the normal Session API stopped with the same `background` error before the model could see the shell's refusal.

The fold now gives the model one more decision when its first recovery wave receives a structured refusal from a different tool for the first time. A repeated denial of a previously refused tool still stops the run, including when arguments change. The extra decision also ends at the next unproductive tool wave. Existing execution permission checks apply to each call. The terminal keeps the same stop code and describes stalled recovery when the refused tools differ.

## Verification

macOS 27.2.1 arm64, Rust 1.97.1, repository root with the pinned Studio submodule initialized. The regression uses the normal Session entry point, the built-in `background`, `bash`, `ls` and `read` tools, local temporary files and a scripted model. It checks two pre-execution refusals and one successful read, plus two stopping cases. A changed `background` request and a third denied tool each stop after three model decisions. The successful read is followed by a fourth decision and a completed turn.

| Command | Result |
| --- | --- |
| `cargo test --locked -p aster-core --lib background_and_shell_refusals_leave_one_bounded_recovery_opportunity -- --test-threads=1` on the previous fold | 1 failed with `run.tool_refusal_cycle` naming `background`, before the permitted read. |
| `cargo test --locked -p aster-core --lib -- sandbox_availability refusal_cycle convergence agent_api::goal_facade::tests fact_control:: goal_control:: hitl:: tool_confirmation:: --test-threads=4` | 507 passed, including the new regression and the existing same-tool, child, compaction and workspace-state stopping cases. |
| `cargo test --locked -p aster-acp --test unattended_e2e -- convergence cycle original_refusals child_refusal --test-threads=1` | 6 passed against the Cargo-built ACP. |
| `cargo build --locked -p aster -p aster-acp --bin aster --bin aster-acp` | Both binaries built. `target/debug/aster --build-id` and `target/debug/aster-acp --build-id` both returned `aster-cli-pair-v1:3459c8228f499bfa917f17864b2c57802d767d02:aarch64-apple-darwin`. |
| `cargo test --locked -p aster-acp --test shipped_tui_e2e -- shipped_tui_shows_recovery_and_an_actionable_stop_in_the_same_session shipped_agent_recovers_from_a_refused_tool_and_bounds_repeated_denials shipped_agent_bash_denials_use_one_recovery_round --test-threads=1` | 3 passed with the freshly built pair. The real PTY completed an alternate read, displayed the stop after repeated denials, and accepted a further prompt in the same session. |
| `cargo fmt --all -- --check` | Passed. |
| `cargo clippy --locked -p aster-core --all-targets --no-deps --features ci-all,deep-live` | Passed with warnings in unchanged files. No diagnostics pointed to the edited Rust files. |
| `cargo check --locked --workspace --all-targets` | Passed on macOS arm64. Cargo reported future-incompatibility warnings for `block` 0.1.6 and `proc-macro-error2` 2.0.1. |
| `/opt/homebrew/bin/python3.12 tests/workspace_dependencies.py` and `/opt/homebrew/bin/python3.12 tests/musl_linker_flags.py` | 7 and 2 passed, respectively. The host `python3` lacked `tomllib`. |

Clippy identified an unnecessary clone when building a one-item slice in `fact_control/tool_history_tests.rs:1068`. If that fixture is edited, it can pass `std::slice::from_ref(&contract)` to `decide_with_contracts`. The default-feature Core build also reported unreachable fallback arms in `tools/builtin/bash.rs:236` and `:249`. Those matches can use the exhaustive `ToolStreamEvent::OutputDelta` arm alone under the tested configuration.

## Explicitly not claimed

- Native Windows or Linux execution, a live provider run, or a successful `just dev` launch in the reported Windows workspace. The screenshot did not expose the underlying tool refusal diagnostics.
- Release qualification or installation of a new CLI version.
