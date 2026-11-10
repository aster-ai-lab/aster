# Aster TUI visual end-to-end verification

Date: 2026-11-10 (Asia/Shanghai)

The native Terminal TUI was operated using agent-desktop. The real DeepSeek task created, read and independently verified a 20-byte file. A controlled HTTP provider supplied deterministic stalled requests and model metadata for cancellation, queue and capability checks.

## Build and source identity

- Repository: `/Users/roylin/code/aster`
- Build baseline: `b1be95c5b7d6`, with the existing task working tree
- Final paired binaries: `target/debug/aster` and `target/debug/aster-acp`
- Pair identity: `aster-cli-pair-v1:73b1d5b5ca5b9ca39f1496d3ac18fbf638bc47e9:aarch64-apple-darwin`
- Rust: 1.97.1; macOS arm64; Terminal grid: 191 × 40
- Earlier controls/discovery captures used pair `ca486c0790437b4843de05607191ba2f94109b13`. The final pair adds the brand and session-info label changes. Relevant behavior was not changed between these captures; Stop, Ctrl+C and send-now were also repeated on the final pair.
- HEAD advanced externally to `682e4b15d8411763eca37e3266335032e90b743d`. That change only advances `apps/studio`; it does not change the tested CLI sources. This run made no Git commit or push.

## Observed results

| Check | Result | Evidence |
| --- | --- | --- |
| Mouse Stop during stalled HTTP | The socket closed and the TUI returned to idle; another prompt completed | [Cancelled](screens/02-stop-cancelled.png) |
| Ctrl+C during stalled HTTP | The socket closed and the TUI left Cancelling/Working | [Cancelled](screens/03-ctrl-c-cancelled.png) |
| Queue then Enter to send now | The stalled request was interrupted, the next prompt completed exactly once, and the queue row disappeared | [Queued](screens/04-queued.png), [completed](screens/05-send-now-completed.png) |
| YOLO naming | Mode banner and footer use YOLO; the final session-info modal also uses YOLO | [Banner](screens/01-yolo.png), [session info](screens/15-session-info.png) |
| Reasoning choices | The provider's none/low/high list is shown; medium/xhigh/max are absent | [Menu](screens/06-effort-menu.png) |
| Explicit effort and reset | The low request contained `reasoning_effort: low`; Default omitted the override and showed the reported default high | [Default](screens/07-effort-default.png) |
| Asynchronous model detection | Switching to the alternate model first showed detecting and unknown context, then low and 131K | [Pending](screens/08-model-detecting.png), [resolved](screens/09-model-detected.png) |
| Context synchronization | Context details changed from the first model's 262144 to the alternate model's 131072 tokens | [Context](screens/10-context.png) |
| /resume | The command opened the real picker; selection restored session `0cee1e56-8ea4-4aa2-a442-1768cebe1205` after a different new session had been created | [Picker](screens/11-resume-picker.png), [restored ID](screens/12-resumed-session.png) |
| Theme and settings names | The picker shows aster-night/aster-day; settings show Aster Night/Aster Day and updated hint text | [Picker](screens/13-theme-menu.png), [settings](screens/14-settings.png) |
| Legacy theme configuration | An isolated profile seeded with the old stored night-theme name loaded the new aster-night menu row as active | [Picker](screens/13-theme-menu.png) |
| Real provider task | DeepSeek Flash created visual-proof.txt, read it through a tool, ran file/byte checks, and returned OK-VERIFIED | [Real completion](screens/16-real-model-completed.png) |

The final mouse Stop closed the fixture socket 0.054 seconds after the desktop click command started. This is one functional observation, not a performance benchmark. A prolonged automation pause also exercised the existing HTTP timeout/retry before queue interruption; the queued follow-up itself was admitted once, with no observed replay after completion.

The resume screenshot predates the session-info capitalization correction and therefore shows the old lowercase detail value. The final session-info screenshot records the corrected YOLO label. The tested resume implementation was unchanged by the label correction.

## Defects corrected during this run

1. Model discovery appended `/v1` twice when a Chat Completions base URL already ended in `/v1`. A regression first failed with `/v1/v1/models`; discovery now reuses the request protocol's endpoint normalization. The tightened fixture accepts only `/v1/models` and `/v1/chat/completions`.
2. Built-in theme canonical names, settings copy, version hints, copy hints and the account-import label were updated to Aster or neutral terms. Legacy theme aliases remain readable. The theming guide now documents the new names.
3. Session-info displayed raw `yolo`; it now displays YOLO while retaining the wire value.
4. A pre-existing copy-path test supplied a `.grok` path but expected `.aster`. Its input was corrected to the current default path; the assertion was retained. The sandbox warning test was updated for its revised wording.

## Verification commands

All Cargo commands ran with:

```sh
CARGO_TARGET_DIR=/Users/roylin/code/aster/target CARGO_INCREMENTAL=0
```

| Command | Result |
| --- | --- |
| `cargo test --locked -p aster-core --lib model_discovery -- --test-threads=1` | 8 passed |
| `cargo test --locked -p aster-pager-render --lib theme:: -- --test-threads=1` | 172 passed |
| `cargo test --locked -p aster-pager --lib settings -- --test-threads=1` | 386 passed |
| `cargo test --locked -p aster-pager --lib slash::commands::theme -- --test-threads=1` | 18 passed |
| `cargo test --locked -p aster-pager --lib format_session_info_displays_returned_permission_posture -- --test-threads=1` | 1 passed |
| `cargo test --locked -p aster-pager --lib account_import -- --test-threads=1` | 6 passed |
| `cargo test --locked -p aster-pager --lib version_mismatch -- --test-threads=1` | 16 passed |
| `cargo test --locked -p aster-pager-render --lib clipboard:: -- --test-threads=1` | Final run: 66 passed |
| `cargo test --locked -p aster-pager --lib sandbox_profile_conflict -- --test-threads=1` | Final run: 1 passed |
| `cargo test --locked -p aster-pager --lib docs::tests -- --test-threads=1` | 19 passed |
| `cargo clippy --locked -p aster-core --lib --no-deps` | Completed with warnings |
| `cargo clippy --locked -p aster-pager -p aster-pager-render --lib --no-deps` | Completed with warnings |
| `cargo build --locked -p aster -p aster-acp --bins` | Passed before native tests |
| `git diff --check` | Passed |

Counts are per command and should not be summed as a unique-test total. Exact summaries are retained in [results.json](results.json).

## Real file oracle

[visual-proof.txt](visual-proof.txt) contains exactly `ASTER-VISUAL-E2E-OK` followed by one newline.

- Length: 20 bytes
- SHA-256: `14eb3af38f788d77774e6490f7dd957c8fe00dbcd7faeeaf2975be192d0ad603`
- The model's native transcript includes creation, Read 1 file, and a shell check using cat, wc and od.
- An independent filesystem read asserted the exact bytes after the turn finished.

## Desktop automation and isolation

agent-desktop handled the task session, window management, native snapshots, screenshots, Unicode text input and mouse input. Read-only Terminal contents supplied a second text oracle because its accessibility field can lag the alternate screen. Some key delivery and animated-title lookups failed or had unverified effects. Those actions were checked against live state and were not counted as successful by tool return status alone.

The system's Simplified Pinyin input method could stage text in composition and alter punctuation under System Events keystroke input. Input calibration used the ABC source during test input. The repository's existing visual harness System Events fallback supplied some keys. Native agent-desktop Unicode text input was used for the real launch and task prompt. Later redundant resume attempts affected by automation/input composition are not claimed as additional passing runs.

Work used fixed isolated directories under `/tmp/aster-tui-visual-controls`. Existing user sessions and project files were not used as task workspaces. The copied real-provider configuration was private and deleted after the real CLI exited. The owned provider and Terminal window were closed. The original Simplified Pinyin input source was restored. No clipboard write was performed.

## Explicitly not claimed

- Universal reasoning/context discovery for every provider or model
- A live-provider reproduction of every cancellation or queue scenario; those scenarios used the controlled stalled HTTP service
- A maximum-context stress test, cross-platform acceptance, packaging or release acceptance
- A full workspace test suite, warning-free Clippy, or a performance benchmark
- Rewriting user history, upstream model names, legacy technical identifiers or all legacy reference documentation
- Installing the binaries, changing the user's configured profiles, committing or pushing this working tree
