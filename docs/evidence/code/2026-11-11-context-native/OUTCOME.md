# Native context accounting after session resume

Agent-desktop drove an owned fullscreen macOS Terminal window running the
paired CLI and ACP. The test opened `/resume`, searched by the entire saved
session ID, selected the matching row and confirmed the same ID through
`/session-info`. The session retained its three completed Runs, DeepSeek Flash
and its 1,048,576-token context window.

The `/context` view showed `18.5k` reported input against `1.0m` before the
follow-up. The latest persisted model usage report contained 18,515 input
tokens. A single new prompt read an unseen fixture. The original prepared
call and succeeded receipt shared one invocation identity, and the receipt
and the final answer contained the exact fixture value. After settlement the
view showed `18.9k` against `1.0m`, matching the latest report of 18,886
input tokens. Each modal inspection preserved the Run identities, model input
and report counts. The view omitted unreported breakdown categories.

## Verification

The macOS arm64 pair was built with
`cargo build --locked -p aster -p aster-acp --bin aster --bin aster-acp`.
The independent source check matched both binaries to
`231f8384a785711f87cd25f0b7f172db278676d2`. The verified launch used
`aster --cwd <fixture> --model deepseek/deepseek-flash --no-plan --yolo --no-subagents --fullscreen`.
Agent-desktop submitted the verified composer content and captured the picker,
session identity and both context views. Scoped native keystrokes filled
the composer and closed the overlays when the desktop actuator did not
change the view. Unsent input attempts produced zero admissions. The
observer was corrected to read the actual receipt enum, `succeeded`, from
the original Run. The model prompt was admitted once.

[Observations](observations.json) records the accounting and digests of the
unchanged native captures. The fixture and source ACL digests were unchanged,
and the input source was restored.

## Explicitly not claimed

This covers one fullscreen native resume and one context update on macOS with
DeepSeek Flash. Other providers, a minimal native window and delayed usage
reports are outside this result. The diagnostic pair contains the separate
uncommitted ACP cancellation change.
