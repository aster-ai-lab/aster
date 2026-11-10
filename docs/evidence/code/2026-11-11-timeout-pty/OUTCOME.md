# Real command timeout settlement in Normal mode

The shipped CLI and ACP pair ran both original DeepSeek timeout cases in a
fullscreen PTY on macOS arm64. The [plan](plan.json) retains the exact prompts,
requested timeouts and original observation deadlines from the
[native timeout plan](../2026-11-11-command-timeouts/timeout-plan.json).
The driver selected the one-time approval only after the active request's
arguments matched the frozen command and the terminal displayed the Execute
approval. Each command ran once in an isolated workspace.

- The 1500 ms request produced its original complete timeout receipt with
  captured output, the fixture start marker and an observed exit status.
- The 900000 ms request reached the 120000 ms policy ceiling. Its receipt
  retained the requested budget, the effective timeout, captured output and
  the observed exit status.

Each prompt settled once with the original tool call identity. Each receipt
reports complete observation and the actual captured output shape. The two
fixture processes were observed while running and their original process
identities were absent after settlement. The CLI exited normally. The source
ACL digest matched before and after the test. See [result](result.json) and
[observations](observations.json) for commands, counts and digests.

The runner pinned byte-identical copies of both source-reconciled binaries at
fingerprint `87f313b79042fb5ee64806794fc7b2300fbdd298` while subsequent CLI
branding checks ran. The [pair record](../2026-11-11-resume-effort/source-reconciliation.json)
identifies the candidate. The first harness preflight stopped because the relay
assembly omitted an import. No prompt was delivered. The corrected setup kept
the original test inputs and deadlines. See
[preflight assessment](preflight-assessment.json).

## Explicitly not claimed

This replay covers Normal timeout ownership through the real terminal program
and provider. The native window interaction, YOLO host replay, detached process
trees and Windows/Linux runtime remain outside the evidence. The separate
uncommitted ACP work is present in this diagnostic pair.
