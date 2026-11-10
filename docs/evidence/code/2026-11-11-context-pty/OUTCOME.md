# Context occupancy on a restored DeepSeek session

The shipped CLI and ACP pair loaded the same six-turn DeepSeek session through
fullscreen and minimal PTYs. `/context` displayed `21.2k` reported input in
both views. The correlated ACP replies report 21177 used, 1048576 total and
1027399 free tokens, with usage available and the category breakdown unavailable.
The canonical Run contained two input reports, 21076 and 21177. Session
cumulative usage was 249178 tokens.

Both views displayed a free-context row and the 1.0m window. They omitted the
unreported system, overhead, tool and compaction rows. The fullscreen modal
closed with Escape and both launches exited normally. The relay observed zero
prompt admissions. The supplied ACL and original session snapshot digests
matched before and after the run.

The frozen [plan](plan.json), [result](result.json) and
[observations](observations.json) retain the commands, exact wire values and
selected viewport text. The six existing relay checks passed. The relay adds
only scalar observations to the original packet forwarding path. The pinned
vt100 reader interprets the 45 by 140 terminal byte stream.

The run used macOS arm64 and the source-reconciled CLI and ACP fingerprint
`87f313b79042fb5ee64806794fc7b2300fbdd298`, verified before the subsequent
branding edits. The [pair record](../2026-11-11-resume-effort/source-reconciliation.json)
identifies the exact candidate. The earlier native reproduction and the frozen
native replay remain in the [context fix evidence](../2026-11-11-context-occupancy/OUTCOME.md).

## Explicitly not claimed

This run covers the measured context display for a restored session in both
terminal modes. Native pointer input and the separate native context replay
remain pending desktop availability. Unmeasured and zero-usage regressions
have focused tests in the context fix. The full catalogue, other providers,
Windows and Linux are outside this run's evidence.
