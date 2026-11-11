# Ignore a delayed start for a settled prompt

The shipped TUI finished a loopback model turn and returned to idle. The test
relay then delivered the original ACP start notification again, byte for byte.
The original TUI showed `[stop]` and entered a new busy state for the settled
prompt. The relay still recorded one prompt request and the provider received
one streaming invocation. The reproduction established a display and turn
ownership failure under delayed notification delivery.

The session view now records confirmed terminal prompt IDs from live replies,
viewer terminals, and the existing lost-reply reconcile path alongside the
IDs already learned during replay. The queue handler rejects a stale running
snapshot for a known terminal ID before it can change the current queue or
adopt a turn. The guard uses the bound session and prompt ID. A failed
transport response alone does not record a terminal. The existing queue
redraw result and wake handling are preserved.

The first code validation run passed the new regression and failed an older
replay test because the early return changed the active-view redraw result.
The final implementation preserves that result. The original assertions
still check both the redraw and the idle state.

## Verification

The reproduction uses the paired macOS arm64 binaries with a private loopback
fixture. An initial harness setup and a readiness preflight stopped before
prompt delivery. The successful reproduction and replay after the change
use the same driver, relay and observation conditions.

On the rebuilt pair, the same delayed start left the terminal idle and the
original reply in place. The following new prompt completed. The trace
contains two distinct prompt IDs with one completed response per request.
See [before and after observations](observations.json).

| Command | Result |
| --- | --- |
| `cargo test --locked -p aster-pager --lib app::acp_handler::tests:: -- --test-threads=1` | 589 passed |
| `cargo test --locked -p aster-pager --lib app::dispatch::tests::turn:: -- --test-threads=1` | 75 passed |
| `cargo build --locked -p aster -p aster-acp --bin aster --bin aster-acp` | Passed |
| `cargo test --locked -p aster-acp --test shipped_tui_e2e session_controls:: -- --test-threads=1` | 3 passed |
| `cargo clippy --locked -p aster-pager --all-targets --no-deps` | Passed with existing warnings |
| `git diff --check` for the owned patch | Passed |

Both rebuilt binaries matched source fingerprint
`231f8384a785711f87cd25f0b7f172db278676d2`. The PTY control tests selected
that pair with `ASTER_TUI_BIN` and `ASTER_ACP_BIN`.

## Explicitly not claimed

This case qualifies a late copy of a genuine ACP start notification after
settlement. The source of notification timing in the earlier session
remains unverified. Reconnect fault combinations, forced insertion
with several queued items and native window input are separate open cases.
The diagnostic pairs contain the separate uncommitted ACP cancellation change.

The native YOLO timeout attempt opened an accessible Terminal window, but
the test process inherited the home directory as its workspace. Both owned
test processes were terminated and the timeout attempt was excluded from
acceptance. The private native launcher now passes the fixture workspace
explicitly and verifies the sidecar working directory before returning.
