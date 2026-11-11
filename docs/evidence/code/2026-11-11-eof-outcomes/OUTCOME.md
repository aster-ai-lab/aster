# ACP EOF settlement

## Change

Closing the real ACP stdin while a foreground Bash invocation was running
retired its process, but the original Run and invocation result were absent
from the session store. The CLI exited successfully and printed no connection
failure. ACP returned from its I/O task without awaiting Core shutdown. The
TUI treated the closed message channel as an ordinary successful quit.

ACP now awaits the existing Core close operation before dropping its runtime.
The TUI preserves its exit information when that channel closes, prints the
existing resume command, reports `Agent connection closed`, and exits with
code 1. Ordinary quits retain their existing behavior. The new lifecycle test
also verifies that closed sessions and their owner reject later admission.

## Observations

The controlled fullscreen case used the shipped CLI and ACP pair, a loopback
provider, one real foreground process, two local waiting messages and an
unsent draft. A byte-preserving relay closed only ACP stdin at a complete
packet boundary and retained stdout until EOF. Its original producer, relay
and plan were unchanged between the failing and fixed observations.

The outcome grader was added after inspecting the missing terminal facts.
It was applied to the retained original observations before running the
fixed pair. It requires a Run, an invocation-bound terminal receipt, process
retirement, one original model request, an error explanation and a failing
exit code. The final grader and all case inputs retained their frozen hashes.

| Observation | Original pair | Fixed pair |
| --- | --- | --- |
| Original Run | Absent | One, cancelled |
| Prepared tool invocation | One | One |
| Bound result receipt | Absent | One, cancelled |
| Foreground process after shutdown | Gone | Gone |
| Model generations / prompt requests | 1 / 1 | 1 / 1 |
| CLI exit code | 0 | 1 |
| Visible connection failure | Absent | Present |

The process did not reach its delayed write and required no harness
termination. The observation deadline was 40.5 seconds, based on the current
35-second cancellation settle grace and 50-millisecond abort grace. The
fixture's natural completion was 120 seconds. This is functional evidence,
not a latency measurement.

A freshly launched native macOS Terminal case used only the authorized
`deepseek/deepseek-flash` provider with the same foreground/queue/draft fault
shape. Its original Run and exact prepared invocation retained a cancelled
result. CLI, ACP and tool owners exited, and the restored Terminal showed
the connection error and matching resume command. Explicitly resuming the
same session loaded the same cancelled Run, produced zero prompt requests,
and remained idle. The source ACL digest was unchanged.

Agent-desktop drove the native case with scoped native input and capture
fallbacks for its mutable-window identity failures. The first resume
observer selected an empty Terminal startup window. It was rebound using
the live ACP parent CLI's actual TTY. The successful inspection used that
same running resume process without relaunching it or sending another prompt.
The original input source was restored. The native test observed owner exit
and visible error; only the controlled PTY directly measured CLI exit code 1.

[Observations](observations.json) retains sanitized counts and digests of
private original artifacts. Both diagnostic executables matched current
source fingerprint `4e0fd46b3e91f3ebc895c66e9674c8fea1f6e290`.

## Checks

Rust 1.97.1, macOS arm64, root target directory, incremental compilation
disabled. The freshly paired binaries were selected explicitly for shipped
end-to-end tests and used for both controlled and native observations.

| Command | Actual result |
| --- | --- |
| `cargo test --locked -p aster-acp --lib lifecycle_tests -- --nocapture --test-threads=1` | 1 passed |
| `cargo test --locked -p aster-pager --lib app::tests -- --test-threads=1` | 71 passed |
| `cargo build --locked -p aster -p aster-acp --bin aster --bin aster-acp` | Passed |
| `cargo test --locked -p aster-acp --test shipped_tui_e2e -- --nocapture --test-threads=1` with both built binaries selected | 26 passed, 1 opt-in case ignored |
| `cargo clippy --locked -p aster-acp -p aster-pager --all-targets --no-deps` | Passed with existing warnings |
| `cargo fmt --all -- --check` | Existing differences in the same 18 baseline files, zero added files |
| Owned patch `git diff --cached --check` | Passed |

## Explicitly not claimed

The two unsubmitted waiting messages and unsent draft are still absent after
restart in both cases. Queue/draft recovery is an open defect. This evidence
does not establish complete EOF recovery or complete TUI acceptance. Abrupt
process death, other blocking phases, native minimal mode, Linux and Windows
remain unverified here.

The tested diagnostic pair includes the separate pre-existing uncommitted
ACP foreground cleanup patch. Its five original source patches are preserved
and excluded from this commit. No release candidate or publication is claimed.
