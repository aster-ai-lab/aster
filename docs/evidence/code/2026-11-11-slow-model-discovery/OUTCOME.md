# Draft and effort during delayed model discovery

The shipped TUI and ACP used two loopback providers that expose the same
raw model ID. The second provider held its catalog response on a test latch.
While that response was pending, the terminal displayed `detecting`, consumed
`/effort none`, and accepted a draft. Neither provider received a generation
request during the hold.

After the catalog advertised `high` as its default, the terminal displayed
`slow/gpt (none)` and retained the draft. Submitting that draft sent an actual
request to the second provider with `model: gpt` and `reasoning_effort: none`.
The first provider received zero generation requests. The second provider
received one catalog request. The regression reuses the existing PTY and
HTTP fixture with a catalog latch.

The first harness attempt sent the text and Enter without observing the
rendered command. The command remained in the composer. The corrected
actuator waits for the command text, presses Enter, and asserts that it was
consumed before typing the draft or releasing the response. The corrected
case passed with the original product implementation.

## Verification

macOS arm64 and Rust 1.97.1, with the shared root target directory and
incremental compilation disabled. The two binaries independently matched
source fingerprint `fe62b8f577fc6cee53d5bd55fde451892ec46783`.

| Command | Result |
| --- | --- |
| `cargo build --locked -p aster -p aster-acp --bin aster --bin aster-acp` | Passed |
| `cargo test --locked -p aster-acp --test shipped_tui_e2e reasoning::delayed_catalog_keeps_draft_and_explicit_effort -- --exact --nocapture --test-threads=1` | 1 passed after the actuator correction |
| `cargo test --locked -p aster-acp --test shipped_tui_e2e -- --test-threads=1` | 25 passed, 1 live Goal case ignored by default |
| `cargo clippy --locked -p aster-acp --all-targets --no-deps` | Passed with existing warnings |
| `cargo fmt --all -- --check` | Same 18 baseline files outside this change |
| `git diff --check` for the owned patch | Passed |

The PTY runs set `ASTER_TUI_BIN` and `ASTER_ACP_BIN` to the built pair.
The ignored Goal case ran independently with [real DeepSeek](../2026-11-11-bound-goal-live/OUTCOME.md).

## Explicitly not claimed

This case verifies the typed override and draft while one catalog response
is pending. It leaves conflicting overrides, out-of-order responses,
timeouts and failures, other provider protocols and native window input
outside its scope. The diagnostic pair contains the separate uncommitted
ACP cancellation change.
