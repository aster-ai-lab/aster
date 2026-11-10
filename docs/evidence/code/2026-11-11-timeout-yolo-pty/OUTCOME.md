# Real command timeout settlement in YOLO mode

The shipped CLI and ACP pair ran the original
[host timeout case](../2026-11-11-command-timeouts/timeout-host-plan.json)
with `deepseek/deepseek-flash` in an isolated fullscreen PTY on macOS arm64.
The runner used the supplied ACL through a private copy and verified the ACL
digest after the run. It observed the selected DeepSeek model and YOLO
footer before sending the frozen prompt once.

The exact Bash invocation requested 1500 ms. Its one original timeout receipt
reported 1500 ms, captured the fixture start marker, included output and exit
metadata and reported complete observation. The tool failed on timeout and the
parent Run completed. The runner observed the fixture process while it ran
and checked that its original identity had disappeared after settlement.
The ACP relay observed one prompt request, one response and zero permission
requests. The CLI exited with code zero.

## Verification

The source-reconciled binaries had fingerprint
`c00dde195b0f55e43b6c211bf5ccb9bf50985ad9`. The private runner pinned
byte-identical copies and executed the equivalent of
`aster --model deepseek/deepseek-flash --no-plan --no-subagents --yolo --fullscreen`
with the original case deadlines. See [observations](observations.json).

The first setup stopped during hashing because the system Python lacks
`hashlib.file_digest`. A second preflight stopped when argument parsing
rejected `yolo` as a value for `--permission-mode`. Neither attempt delivered
a prompt. The successful launch used `--yolo` and the original prompt and
deadlines.

## Explicitly not claimed

This result covers the original YOLO host timeout case through the real
terminal program and provider. Native window interaction, other platforms
and the full cancellation matrix remain outside this result. The diagnostic
pair contains the separate uncommitted ACP cancellation change.
