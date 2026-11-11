# Native DeepSeek timeout in YOLO

The source-reconciled CLI and ACP ran the unchanged
[host timeout case](../2026-11-11-command-timeouts/timeout-host-plan.json) in
an owned macOS Terminal window. The launcher passed the fixture path with
`--cwd` and verified the sidecar process directory before submitting a
prompt. The footer showed DeepSeek Flash, its effective `max` effort, and
`YOLO`. Agent-desktop sent Return and captured the ready and settled windows.
The verified native text fallback filled the composer after the earlier
agent-desktop focus failure.

One Bash invocation timed out at the requested 1500 ms. The original
receipt retained the invocation identity, captured output and exit metadata,
and complete observation. The Bash tool returned failure on timeout and the
parent Run completed. No approval was entered, no retry was admitted, and no
matching fixture process survived. The source ACL digest was unchanged and
the input source returned to its pre-test selection.

## Verification

The paired macOS arm64 binaries matched source fingerprint
`231f8384a785711f87cd25f0b7f172db278676d2`. The launcher used the equivalent
of `aster --cwd <fixture> --model deepseek/deepseek-flash --no-plan --yolo --no-subagents --fullscreen`.
See [observations](observations.json) for the checked receipt fields and
the digest of the private, unmodified Terminal capture.

An earlier startup stopped during configuration loading. A subsequent
attempt entered the home directory instead of the fixture workspace. Both
owned processes from that attempt were terminated. The accepted run used
the explicit workspace argument, verified process binding, and a fresh
prompt admission under the original case plan.

## Explicitly not claimed

The evidence covers the frozen YOLO host timeout on macOS. Other permission
modes, detached execution, native context restoration and other platforms
remain outside this result. The diagnostic pair contains the separate
uncommitted ACP cancellation change.
