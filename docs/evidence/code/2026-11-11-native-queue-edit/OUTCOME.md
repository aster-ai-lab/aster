# Native queue editing

## Scope

Fresh native macOS Terminal, fullscreen, YOLO, only the authorized
`deepseek/deepseek-flash` provider. The current source fingerprint and both
paired build IDs were reconciled before and after the case. The unchanged
paired build is recorded in [ACP EOF settlement](../2026-11-11-eof-outcomes/OUTCOME.md).
No product source changed in this case.

The frozen fixture runs one foreground Bash process until an explicit
release file appears, with a 480-second fixture deadline and a requested
540000-millisecond tool timeout. Three local follow-ups are queued while it
owns that process. A separate unsent composer is retained through the operations.
The fixture, relay and plan retained their original digests.

## Observations

Agent-desktop operated the real queue with physical mouse and keyboard input.
Terminal text bounds qualified the target row for mouse selection. Keys used
the verified focused Terminal after explicit window-ID actions encountered
the adapter's mutable-title identity failure.

| Interaction | Actual result |
| --- | --- |
| Open/select a queue row with a nonempty composer | Draft retained |
| Edit and save the first row | Only its payload changed; composer restored |
| Shift+J, then Shift+K | Selected row moved down and back; exact FIFO order restored |
| Change that row again, then Escape | Prior saved payload retained; composer restored |
| Delete the second row | Only that row disappeared |
| Up and Down | Subsequent edit opened the exact intended row each time |
| Escape from the queue | Queue focus dismissed; composer retained |
| Explicitly release the foreground fixture | Original, edited first and retained last prompts completed once in order |

Only one prompt request was admitted during all mutations. Final ACP request
digests matched exactly the original, edited first and retained last inputs.
Their prompt IDs, RPC IDs and durable Run IDs were each distinct. Core retained
three completed Runs with exact results `NATIVE-QUEUE-ORIGINAL-DONE`,
`NATIVE-QUEUE-EDITED-DONE` and `NATIVE-QUEUE-LAST-DONE`.

The deleted row, discarded edit and unsent composer produced zero prompt
admissions. The composer still contained its original text after settlement.
The original Bash receipt matched its prepared invocation identity, reported
`succeeded`, exit code 0, complete observation and the exact fixture output.
Its process retired after release. The source ACL was unchanged and the input
source was restored.

## Observer corrections

The first preparation observer inserted a space at a Terminal soft wrap inside
a long word. It timed out before Return, with zero prompt requests. The same
already-typed composer was submitted once; its ACP digest matched the frozen
original input. The CLI and ACP were never relaunched.

An explicit window-ID shortcut attempt returned unknown delivery. Its state
was inspected without repeating it. Row selection then used its native text
bounds. The Ctrl+semicolon shortcut is not verified by this case.

The first keyboard-selection observer mistook hover action buttons for the
selected row. Editing after the already-delivered Up proved the actual first
selection. Down was verified the same way. Queue payload expectations stayed
fixed.

The first terminal receipt check confused the Run status `completed` with the
receipt outcome. It was corrected against the pinned
`ToolExecutionOutcome` closed set in `crates/effect/src/coding/tool_effects.rs`.
Read-only inspection of the same completed Runs additionally checked exit code,
exact output and complete observation. The failed observer logs are retained
privately alongside their corrected observations.

## Checks

| Check | Actual result |
| --- | --- |
| Current source and both paired build IDs | Matched `4e0fd46b3e91f3ebc895c66e9674c8fea1f6e290` |
| Native queue mutation case | Passed |
| Exact admission sequence and reply validation | 3 requests, 3 completed Runs, 3 exact replies |
| Original invocation-bound tool receipt | 1 prepared, 1 succeeded, exit 0 |
| Private harness syntax check | Passed |
| Frozen fixture, relay, plan and source ACL | Unchanged |
| Evidence `git diff --cached --check` | Passed |

[Observations](observations.json) records sanitized results and digests of
the original private traces, frames and screenshots. This evidence-only change
reuses the paired build; no Rust tests, build or Clippy were rerun.

## Explicitly not claimed

This case covers native fullscreen ASCII text rows and one unsent text
composer. Images, expanded skills, other input modes, multiline and Unicode
rows, reconnect, queue saturation, minimal mode, Linux and Windows require
their own evidence. Queue/draft loss across ACP EOF remains open. This is
partial campaign progress, not full TUI acceptance or a release candidate.
The diagnostic pair still includes the separate pre-existing uncommitted ACP
foreground cleanup patch, which is excluded from this commit.
