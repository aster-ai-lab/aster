# `/init` slash command for Aster TUI

Date: 2026-11-14. Domain: `code` (TUI / `aster-pager`).

## Goal

Add an `/init` command to the shipped TUI that generates a short `AGENTS.md`
project overview for future AI agents, matching the "analyze this project and
write an agent entry file" behaviour of Claude Code's `/init`.

Decisions taken with the user before implementation:

| Choice | Decision |
| --- | --- |
| Output file | `AGENTS.md` |
| When it already exists | refuse with a message, never overwrite (fail closed) |
| How the overview is produced | model analyzes the working tree via an injected structured prompt |

## Changes

| File | Change |
| --- | --- |
| `crates/terminal/crates/codegen/aster-pager/src/slash/commands/init.rs` | new `InitCommand` + 4 unit tests |
| `crates/terminal/crates/codegen/aster-pager/src/slash/command.rs` | new default trait hook `pre_run_refusal(&Path) -> Option<String>` |
| `crates/terminal/crates/codegen/aster-pager/src/slash/commands/mod.rs` | module declaration + registration in `builtin_commands()` |
| `crates/terminal/crates/codegen/aster-pager/src/app/dispatch/prompt.rs` | call the hook before `run()`; a refusal renders as a system message |
| `crates/terminal/crates/codegen/aster-pager/src/app/dispatch/queue.rs` | mirror the refusal in `EditedCommandGate` so an edited queued row is not dropped early |

`CommandExecCtx` was deliberately left unchanged: it has no cwd, and adding one
would have required editing ~25 test construction sites. `app.cwd` is already
available at both dispatch call sites, so the hook keeps the change surgical.
No tool name was declared in `required_tools` because the file-writing tool name
could not be confirmed from source; an unverified name would hide the command.

## Commands and results

| Command | Result |
| --- | --- |
| `cargo check --locked -p aster-pager --all-targets` | EXIT 0 |
| `cargo test --locked -p aster-pager --lib init -- --nocapture` | EXIT 0, 63 passed / 0 failed; all 4 new `slash::commands::init::tests` passed |
| `cargo clippy --locked -p aster-pager --all-targets --no-deps` | EXIT 0 (81 warning lines, none referencing a changed file) |
| `rustfmt --edition 2024 --check .../commands/init.rs` | EXIT 0 |
| `cargo fmt --all -- --check` | EXIT 1, pre-existing diffs in 18 unrelated files; none in the changed files |
| `CARGO_TARGET_DIR=… CARGO_INCREMENTAL=0 cargo build --locked -p aster -p aster-acp --bin aster --bin aster-acp` | EXIT 0 |
| `python3 .runtime/tui-full-e2e/source_pair.py` | EXIT 0 after the rebuild |

Rebuilt pair (expected change, because the source hash legitimately changed):

- `aster` 899,194,296 -> **899,221,736** bytes
- `aster-acp` 308,713,768 -> **308,713,768** bytes (unchanged)
- BUILD_ID `b4e19738…` -> **`917bd35213a706a6dbe5656b86c0c3258b8577e4`** for both
- `source_files` 13,466 -> **13,467**

## End-to-end verification (PTY, shipped binary)

Two isolated cwd/state/trace fixtures via `launch-init-e2e.sh`:

| Scenario | Model prompts | Rendered refusal | Observation |
| --- | --- | --- | --- |
| cwd contains `AGENTS.md` | 0 | yes (`will not overwrite`) | command stopped before `run()` |
| cwd has no `AGENTS.md` | 1 | no | injected prompt reached the model; it began with `Run pwd && ls -la` |

The relay is the local deterministic test fixture; no live provider was called.

## Explicitly not claimed

- Not committed or pushed (no user request to commit).
- The model-written `AGENTS.md` content was not verified end to end: the PTY
  fixture uses a deterministic mock ACP relay, so no real file was generated.
- No live-provider, native-terminal (N lane), or agent-desktop visual E2E run.
- `cargo fmt --all -- --check` failure and clippy warnings are pre-existing and
  were not fixed; they are outside this change's scope.
- The 508-case TUI E2E campaign counters were not changed
  (7 PASS / 203 PARTIAL / 298 NOT_RUN).
