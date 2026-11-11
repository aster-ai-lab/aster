# Cancellation stays with its prompt

Two real TUI mouse Stop gestures targeted the same foreground task. A test
relay held its completed cancellation response, then delayed the second
cancel notification until the first queued message was running. The
original implementation cancelled that unrelated queued message. Core
retained statuses `cancelled, cancelled, completed` for the three distinct
prompts.

The TUI now captures the observed prompt ID in its cancel effect and
preserves it through manual retries and automatic resends. The metadata
builder also preserves rewind and headless acknowledgement behavior.
ACP stores the generation number and prompt ID together, shares that
identity with its outgoing notifications, validates a supplied cancel ID,
and ignores a cancel targeting a different or settled prompt. This lookup
keeps the cancellation wakeup independent of the session registry lock.

## End-to-end verification

The same isolated fullscreen PTY reproduction passed after the fix. Its
grader, relay and frozen plan digests matched the retained failing run.
The original real Bash process was gone before the queued provider request.
Both waiting messages completed once, with statuses
`cancelled, completed, completed`. The delayed cancel packet retained
the same bytes. There were three unique prompt and RPC identities, three
model generations and no delayed fixture write.

A freshly launched macOS Terminal window then used the authorized
`deepseek/deepseek-flash` configuration. Agent-desktop delivered two
physical Stop clicks at the current Terminal text bounds. The relay
delivered the original second cancel after the first queued Bash had
started. Its process stayed alive, completed with exit code 0 after release,
and returned the exact fixture marker. The final queued message completed.
Core retained three distinct Runs, one cancelled and two completed.
Both Bash receipts matched their original prepared invocation identities.

An earlier native pointer calibration missed the Stop row and removed an
owned queued fixture. It emitted zero cancel notifications and was retained
as an invalid actuator run. Its original task completed naturally. The
owned CLI, relay and ACP processes were confirmed gone before the next
native attempt. The accepted run used Terminal's `AXBoundsForRange`
through a private calibration helper and preserved the original test
prompts and outcomes. Text entry and captures used scoped native fallbacks
for agent-desktop's mutable-window identity failures. The source ACL was
unchanged and the campaign's input source was restored.

[Observations](observations.json) retains sanitized counts and digests of
the private original traces and captures. Both diagnostic executables
matched current source fingerprint
`32f72c23c32b5fb383cc82df4b9774eb2ceb5753`.

## Checks

Rust 1.97.1, macOS arm64, root target directory, incremental compilation
disabled. The TUI unit filters overlap.

| Command | Actual result |
| --- | --- |
| `cargo test --locked -p aster-acp --lib -- --test-threads=1` | 522 passed, 10 opt-in cases ignored |
| `cargo test --locked -p aster-pager --lib app::dispatch::tests::turn -- --nocapture --test-threads=1` | 75 passed |
| `cargo test --locked -p aster-pager --lib cancel -- --nocapture --test-threads=1` | 246 passed, including current resend identity assertions |
| `cargo clippy --locked -p aster-acp -p aster-pager --all-targets --no-deps` | Passed with existing warnings |
| `cargo build --locked -p aster -p aster-acp --bin aster --bin aster-acp` | Passed |
| `cargo test --locked -p aster-acp --test shipped_tui_e2e -- --nocapture --test-threads=1` with both built binaries selected | 26 passed, 1 opt-in case ignored |
| `cargo fmt --all -- --check` | Existing differences in the same 18 baseline files, zero added files |
| Owned patch `git diff --cached --check` | Passed |

## Explicitly not claimed

Older clients that omit a prompt ID retain session-wide cancellation.
Transport EOF and reconnection, queued admission saturation, minimal native
mode, and native Linux or Windows remain pending. The tested diagnostic
pair includes the separate pre-existing uncommitted ACP foreground cleanup
patch. Its five original source patches were preserved and excluded from
this commit.
