# DeepSeek effort selection through the shipped TUI

The supplied ACL was copied into an isolated private profile. Its SHA256 remains
`0d16cc3d54069e83202d242d85caf730d411ae50078d7538c76b12ec0db57ad7`.
The SDK ACL parser reports the selected provider protocol as `Openai`.
Only `deepseek/deepseek-flash` was selected; account importers were disabled.

The real fullscreen TUI, ACP and Core completed five task-body checks, in order:
`max`, `default`, `none`, `low`, `high`. Each task read one fresh random canary
through one original successful, complete read receipt. Its canonical Run
completed, its final answer contained that canary, and the actual terminal
viewport displayed it. The source ACL and canary files remained unchanged.
The prompt template, model and observation deadlines are in
[real-plan.json](real-plan.json); results are in [real-result.json](real-result.json).

The startup selection retained the configured `max`, separately from the
advertised default `high`. Each later switch had one correlated acknowledged
model RPC with the exact requested selection. `default` displayed the effective
`high`; other selections displayed their own value. See
[selection-trace.json](selection-trace.json) and
[viewport-observations.json](viewport-observations.json).
These RPC observations are not captures of the upstream HTTP request body.

Selected metadata reported a 1048576-token context window from the provider,
ready capabilities, and `none`, `low`, `high`, `max` controls with a
`provider+documented` reasoning source. Thought-chunk counts differed between
tasks. They are observations, not an oracle for hidden provider compute or a
reason to tune prompts or force reasoning text.

## Checker failures retained

The complete matrix still has overall status **FAIL**. After five completed
tasks and CLI exit 0, its outer checker required a Python relay finalizer marker
that was absent. All five prompt RPC responses were already present. The result
has not been rewritten into an overall pass.

An earlier attempt also remains **FAIL** in [initial-result.json](initial-result.json).
It removed ANSI sequences instead of interpreting cursor overwrites and missed
the completed answer. The repository-pinned vt100 0.15.2 parser independently
reconstructed the original viewport and found the exact canary. The original
request completed in 4.349569 seconds; this later analysis does not retroactively
certify its 120-second observation check. See
[initial-reader-assessment.json](initial-reader-assessment.json).

The corrected attempt used fresh random canaries with the same prompt template,
model and deadlines after the prior delivery was settled. The VT reader's
cursor-overwrite self-check distinguishes rendered `ABCD` from ANSI-stripped
`AXCDB`. Six copied relay tests passed. The relay only added scalar observations;
forwarded protocol packet bytes were unchanged.

A separate `/quit` startup probe passed in **4.697 seconds**, with zero prompt
admissions. It observed the original TUI, relay and ACP process identities before
quit and confirmed all three absent within its frozen 10-second deadline.
Its relay finalizer marker was also absent. This demonstrates that the marker is
not a valid process-cleanup oracle. It does not retroactively certify the
matrix's shutdown timing or qualify idle-after-discovery shutdown. See
[quit-plan.json](quit-plan.json) and [quit-result.json](quit-result.json).

## Controlled request-field regression

The new `switches_rebuild_native_controls` test exercises public ACP model
replacement and the real Core HTTP serializer. Its loopback model catalogue
returns context/output limits without reasoning metadata. Explicit typed
configured controls cover that partial-metadata fallback; they are not evidence
of discovering reasoning capabilities from that fixture.

For the startup `max` and replacements `default`, `none`, `low`, `high`, every
captured generation request uses the selected model and clamped output limit
393216, with a 1048576-token session window. `default` omits both native overrides;
`none` sends `reasoning_effort: none` and disabled thinking; the remaining explicit
levels send their exact value and enabled thinking. This verifies the outgoing
fields against a controlled endpoint, separately from the live provider tasks.

The first compile attempt used a private capability helper and the wrong send
argument type; no tests executed. The first HTTP fixture omitted the model-list
GET performed by real session admission. Its failure reproduced alone before
the fixture was corrected. The parameter assertions were preserved. Digests of
these failures and the final logs are in [artifact-digests.json](artifact-digests.json).
The first live attempt's stderr log was overwritten before archiving; no claim
of retaining that original stderr is made.

## Verification

Commands, counts and environment are retained in [checks.json](checks.json).
The host was macOS arm64 with Rust 1.97.1, the shared root target and incremental
compilation disabled.

- ACP model-discovery module: **2 passed, 0 failed, 0 ignored**
- Private trace relay: **6 passed, 0 failed**
- ACP all-target Clippy passed with existing warnings; no new-test finding
- Touched-file formatting passed; workspace formatting retains exactly the
  same 18 unrelated paths as the preceding preserved baseline
- CLI and ACP built together and independently matched current source hash
  `fbef44abd3546407304d313c4def1924a9aa09bc`; see
  [source-reconciliation.json](source-reconciliation.json)
- The live matrix and startup quit used the preceding source-matched pair
  `6802032282069feec5ac9e77371477a3f5a0896b`. The subsequent source change is the
  new test; these are separate build identities
- Evidence was reopened and compared with private originals and digests;
  `git diff --check` passed before committing

## Explicitly not claimed

These are actual PTY/provider task checks and a controlled wire regression,
not agent-desktop visual acceptance. macOS still reports a locked console;
native context and timeout-owner replays remain pending. No Windows/Linux
runtime, all 508 planned cases, every reasoning-control family, hidden provider
compute or upstream HTTP payload capture is claimed. Private provider reasoning,
credentials and raw session data are excluded from public evidence.

The candidate includes the separate uncommitted ACP cancellation work. This
change does not adopt that patch, establish a clean release candidate or publish
a release. Lefthook is unavailable locally; explicit checks provide validation.
