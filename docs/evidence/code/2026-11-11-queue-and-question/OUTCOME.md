# Exact admission order and question answers

The fullscreen TUI queued three separately marked follow-ups while the
loopback provider held the first turn. Before release, the provider observed
only the first streaming request. After release, it observed exactly four
streaming requests in the expected order. The shipped Core store contained
four distinct completed Runs with the exact four prompts and their matching
results. The terminal showed the last reply and no settled queue rows.

The existing byte-preserving ACP relay independently recorded four distinct
prompt IDs and RPC IDs in one session. Each ID had one start and one
`end_turn` response, in the same order. It recorded zero extra prompt
requests. See [wire observations](observations.json).

The first FIFO harness run waited for the first reply in the active viewport
after the queue had already scrolled to its last reply. The corrected check
observes the last visible reply, then checks the entire sequence against the
provider and reads each result from the Core store API. The product was
unchanged.

The question test now checks the exact value returned by the TUI. Selecting
`beta` delivered a structured answer containing `beta` and
`permission_grant: false` to the provider. The test requires one persisted
`question.answered` fact whose parsed content equals the captured answer.
The following edit still changed only the owned fixture file.

## Verification

The macOS arm64 build used Rust 1.97.1 and the shared target directory with
incremental compilation disabled. The two binaries independently matched
source fingerprint `ca84b308b9b3eb590dddf0dae8ee963da60764a7`. The tests
used `ASTER_TUI_BIN` and `ASTER_ACP_BIN` to select the built pair. In the
traced run, the latter selected the existing ACP relay around that binary.

| Command | Result |
| --- | --- |
| `cargo build --locked -p aster -p aster-acp --bin aster --bin aster-acp` | Passed |
| `cargo test --locked -p aster-acp --test shipped_tui_e2e session_controls::queue_drains_three_follow_ups_in_order_once -- --exact --nocapture --test-threads=1` | 1 passed with the ACP relay |
| `cargo test --locked -p aster-acp --test shipped_tui_e2e shipped_tui_ask_user_and_edit_tool_oracles -- --exact --nocapture --test-threads=1` | 1 passed |
| `cargo test --locked -p aster-acp --test shipped_tui_e2e -- --nocapture --test-threads=1` | 26 passed, 1 opt-in live Goal case skipped |
| `cargo clippy --locked -p aster-acp --all-targets --no-deps` | Passed with existing warnings |
| `cargo fmt --all -- --check` | Same 18 baseline files outside this change |
| `git diff --check` for the owned patch | Passed |

## Explicitly not claimed

This covers natural FIFO settlement in a fullscreen PTY and exact single
choice delivery. Forced insertion with multiple waiting items, a delayed
or repeated terminal notification, free-text or multi-select question
variants and native window input remain outside the result. The diagnostic
pair contains the separate uncommitted ACP cancellation change.
