# Shared runtime repair plan

Objective: finish all 12 findings in the Core / ACP / TUI audit, with one
permission and receipt meaning for TUI and AsterDream consumers.

Baseline: `d4a8393f12a6d526a4d08693ac76e73e87414b14`.
Audit: `.tmp/three-layer-audit.xpvRdQ/audit.md`.
The audit's source and isolated probes are starting evidence, not acceptance.

## Scope and order

| Finding | Owning code and callers | Required acceptance |
| --- | --- | --- |
| 1 | ACP prompt, research cleanup, both permission setters | A mode or deny change during a research turn survives normal, error and cancellation cleanup. Queued research reads current permission state. |
| 2 | ACP model / effort replacement, asset deny gate | Replacement preserves posture, explicit denies and the session turn queue. Host ceilings remain enforced. |
| 3 | Core porcelain baseline, delta, receipt and run observation | An unchanged status for an over-budget file cannot hide a content edit as Complete(empty). Bounded reads and read-only recovery remain. |
| 4 | Core background terminal and receipt replay | Live and replay classify the same settled execution and observation gap identically. Unknown and identity conflicts remain blocked. |
| 5 | TUI mode-then-prompt effect and task result | Refused mode switch sends no prompt, clears pending state and retains user input. |
| 6 | ACP model / config-option admission | Unknown session is rejected without creating a session or changing global selection. |
| 7 | ACP mode / posture parsing | Closed-set parsing precedes mutation. Canonical whitespace and case have one meaning. |
| 8 | ACP mode publication, TUI permission requests, Dream bridge | Both hosts use acknowledged effective permission state. Advertised modes control menus. Host ceilings and explicit denies win. |
| 9 | TUI ACP spawn and stdio callers | Model, reasoning effort, cwd and ACL path use the same forwarding rule. |
| 10 | Core batch metadata and its evidence consumers | Body compaction preserves bounded lifecycle identity and uncertainty evidence, or reports a typed gap. |
| 11 | CLI packaging, both installers, companion validation | Fault during pair installation preserves the prior pair. Packaged companions share verifiable build identity. |
| 12 | Core completion error, ACP wire error, TUI / Dream display | Unresolved execution carries a typed reconciliation action and never recommends blind resend. |

Each item gets a small hermetic owning-package regression. Existing audit
probes will be adapted to assert the corrected behavior against fresh builds.
No real credentials or copied user sessions are fixtures.

## Constraints

- Use the root workspace and Rust 1.97.1. Keep local dependencies and APIs.
- Preserve sandbox, host / workspace trust, read caps and fail-closed errors.
- Keep raw facts immutable. Separate execution terminal from observation
  coverage and external obligations; do not waive real uncertainty.
- Use existing implementations. Add no service, SDK binding or configuration
  format. Preserve other sessions' in-flight changes. Do not commit.

## Verification and evidence

Run focused Core, ACP and TUI regressions, protocol / host contract tests,
paired CLI build, `cargo fmt --all -- --check`, runtime-boundary and workspace
checks. Record exact commands and counts in `OUTCOME.md`. Re-run distinct
failures alone and identify baseline failures without hiding them.

Known baseline workspace failures: `apps/work/.git` exists, and
`a3s-gateway` lacks `publish = false`. These are outside the 12 findings and
are not permission to edit another session's work.

## Completion ledger

- [x] 1 Research permission restoration
- [x] 2 Model and effort permission preservation
- [x] 3 Honest bounded file observation
- [x] 4 Background live / replay parity
- [x] 5 Refused mode switch stops prompt
- [x] 6 Unknown session admission
- [x] 7 Closed mode parsing
- [x] 8 Shared host permission contract
- [x] 9 Stdio option forwarding
- [x] 10 Batch lifecycle evidence retention
- [x] 11 Paired delivery and install recovery
- [x] 12 Typed reconciliation errors
- [x] Final adversarial review, build and required checks

Acceptance and expanded baseline failures are recorded in [OUTCOME.md](OUTCOME.md).
All 12 original findings are closed; [FOLLOWUPS.md](FOLLOWUPS.md) records the
separate pre-existing native contracts, Git attribution failure and instability.
