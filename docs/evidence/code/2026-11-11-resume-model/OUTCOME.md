# Restore the session model after the ACL default changes

The shipped TUI reopened an existing session and displayed its original
transcript. Its first new HTTP request used `mock/alt` after the ACL default
changed from `mock/gpt`. The regression failed on the observed request model.

Cold TUI recovery now supplies the qualified model ID from the session summary.
ACP checks it against the current configuration before opening the session.
It resolves that model's current capabilities and applies a saved effort only
when the effort belongs to that model. A session without a recorded effort
uses the provider default when restoring a model different from the current
default. Live sessions, explicit launch options and unattended launches retain
their precedence. A removed saved model returns a load error.

The regression passed through the rebuilt CLI, ACP and Core with a loopback
provider. Opening the session sent zero model requests. The first new request
used `gpt`, omitted the unrelated ACL effort and contained each of the two
user messages once, in order. The session ID was stable. Reopening with
`--model mock/alt` showed the explicit override in session information.
The ACP regression for a removed model verified that no session or Core
agent was opened.

## Verification

The checks ran on macOS arm64 with Rust 1.97.1, the shared root target
directory and incremental compilation disabled. The first focused PTY run
failed on `alt` versus `gpt`. The same routing assertion passed after the fix.

| Command | Result |
| --- | --- |
| `cargo build --locked -p aster -p aster-acp --bin aster --bin aster-acp` | Passed |
| `cargo test --locked -p aster-acp --test shipped_tui_e2e reasoning:: -- --test-threads=1` | 2 passed |
| `cargo test --locked -p aster-acp --test shipped_tui_e2e -- --test-threads=1` | 24 passed, 1 live Goal case ignored |
| `cargo test --locked -p aster-acp --lib resume_tests:: -- --test-threads=1` | 11 passed |
| `cargo test --locked -p aster-pager --lib acp::model_preferences::` | 2 passed |
| `cargo clippy --locked -p aster-acp -p aster-pager --all-targets --no-deps` | Passed with existing warnings |
| `cargo fmt --all -- --check` | 18 baseline files outside this change |
| `git diff --check` for the owned patch | Passed |

Both shipped binaries matched the independently computed source fingerprint
`c00dde195b0f55e43b6c211bf5ccb9bf50985ad9`. The PTY tests set
`ASTER_TUI_BIN` and `ASTER_ACP_BIN` to those built executables.

## Explicitly not claimed

The cross-model test uses two loopback models and checks the actual HTTP
payload. Native window input and real provider switching are outside this
evidence. macOS reported a locked console and agent-desktop listed zero
accessible Terminal windows. The diagnostic pair contains the separate
uncommitted ACP cancellation change.
