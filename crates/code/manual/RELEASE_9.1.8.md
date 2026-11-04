# Core / ACP 9.1.8 and CLI 0.1.8

## User-visible changes

- Goal and ordinary task summaries follow the user's language, lead with useful progress and verification, and distinguish a finished execution from an accepted goal. Internal run identities and plan decision codes stay out of the closeout.
- Restored public tool progress reaches the Core/Effect boundary once, including batched calls and compaction. Original facts, retained results and private continuation transport are preserved.
- Compaction summarizes public observations instead of internal receipt envelopes. Main-prompt fixed overhead no longer bypasses the separate semantic-summary request; fitting prefixes remain lossless and failed summaries keep the bounded fallback and original public archive.
- Retained-context budgets count the decoded tool observations sent to the provider. Receipt metadata no longer displaces public checkpoints; errors, exit codes, images and original private continuation remain accounted for.
- Generated summaries are fallible working notes. Literal identifiers/text and record coverage require original observations; writing and reading back a file establishes persistence rather than source fidelity.

## Qualification and delivery

Core remains a private/local Rust SDK. Core/ACP source and ACP platform archives use `aster-acp-v9.1.8`; paired CLI archives use `aster-cli-v0.1.8`, both with `--latest=false`.

The user selected a separately frozen 8,192-output release qualification with the original GLM 5.3 tasks, data, graders, request/context limits and timeouts. The earlier 4,096-output failure cohort remains retained and is not counted as passing under the new configuration. Before publication, require the two critical cases and their seven supplementary cases, clean-commit branch qualification, the clean-SHA Kimi TUI gate, applicable platform lanes and final installed archive execution in auto Linux-guest plus YOLO Darwin-host modes.

CLI and ACP are built and distributed together with required sandbox companions, BUILD_ID and SHA256SUMS. Versioned releases and channel manifests follow [release governance](../../../docs/release-governance.md).

Actual commands, counts, failures and qualification limits are retained in:

- [Goal summary UX](../../../docs/evidence/code/2026-11-03-goal-summary-ux/OUTCOME.md)
- [Public progress restoration](../../../docs/evidence/code/2026-11-03-restore-core-optimizations/OUTCOME.md)
- [Retained observation budget](../../../docs/evidence/code/2026-11-04-retained-observation-budget/OUTCOME.md)
- [Independent 8k release qualification](../../../docs/evidence/code/2026-11-04-release-8k-qualification/OUTCOME.md)

## Known limits

Native plugin start/echo and host fixture execution timeouts have occurred in full local suites; selected isolated cases pass without changing source or deadlines. Their startup/order cause remains unproven. Qualification does not establish universal native cold-start reliability, every-model accuracy, original Kimi session replay, or native Windows/Linux runtime beyond the actually retained checks. No latency/token/billing gain is inferred from single model runs.
