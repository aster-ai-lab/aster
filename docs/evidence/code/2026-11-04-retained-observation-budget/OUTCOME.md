# Retained observation budget

Status: budget repair verified in three current-source Core profiles; local runtime qualification finished with one intermittent native startup failure. No real-model acceptance or publication claim.

## Reproduction and change

`SessionCompact::compact_with_tail` reserved the retained tool result's entire stored JSON envelope. The provider path instead decodes it through `message_from_folded_tool`, including typed error/exit/image content and excluding internal receipt metadata. Adding receipt-only metadata therefore reduced the semantic handoff allowance even though the provider-visible observation was identical.

The focused regression fails before the product edit with `receipt metadata changed the handoff` (0 passed / 1 failed, exit 101), then passes in the connected Core suites. It executes the actual compactor with text, error and image receipts, compares summary and reported prompt estimates across metadata variants, checks that images still consume context, and preserves original facts and retained lines. Existing private-continuation tests remain in the connected filter.

The production change uses the existing decoder and shared token estimator for retained tool/tool-error entries. Other folded messages and original private-continuation accounting remain on their existing paths. There is no new API, dependency, option, model prompt, output limit, classifier or continuation loop. `ResponseTruncated` and verification debt remain intact.

## Actual checks so far

macOS arm64 / Rust 1.97.1, initialized pinned Studio `013b087cc16b498a0d7e9fd8ee1c5580984fa3fe`, root workspace. Exact commands, counts, elapsed times and exits are in the status receipts and logs.

- Initial connected Core default/minimal/CLI JSON: each **202 passed / 0 failed**; overlapping counts. `core-status.json`.
- Root `cargo fmt --all -- --check`: exit 0. `format-status.json`.
- Owning Core Clippy, all targets, default plus CLI JSON: exit 0, no match in the two changed paths. Existing warning sites and coverage limits remain in `lint-review.json`.
- First workspace `cargo check --locked --offline --workspace --all-targets`: exit 101, lockfile update refused. Original log retained; this was not suppressed.

The first regression draft included a trailing newline in its expected summary; it failed at the existing trim boundary before the intended comparison. The synthetic body was corrected to end with an explicit checkpoint. Exact metadata-invariance assertions were retained. Both that draft result and the actual pre-fix metadata failure remain recorded. A local registration preparation also encountered a historical nullable field; the existing nine-entry critical-source fidelity record was revalidated instead. No paid attempt occurred in either preparation.

## Concurrent source and lock change

The initial source namespace was the prior accepted local candidate's `472342bb80916a39cc42841eb949cf65a94e9db9`. The two budget edits yield `1ee6e529444be5f3493752238dafd7793cf5a0ff`. During subsequent checks, the lockfile changed from `446e151e...` to `6f48e2c4...`; search runtime/config/provider sources and Core's manifest also changed. `external-source-drift.json` records the exact path delta; no writer is attributed. Those changes were preserved.

Cargo metadata resolves the current workspace successfully and leaves the current lockfile byte-identical. Both lockfile snapshots and package-identity comparison are retained. A locked workspace recheck is running against the current file. The pre-drift Core/Clippy records must not be presented as frozen current-source qualification; a new source snapshot and rerun will bind the final local build if it remains stable.

## Original real-model boundary

ACL hash `a9d012ce...`, GLM 5.3, original task/data/graders, 4,096 output, 40 requests and original contexts/timeouts remain unchanged. All nine critical harness source files match their frozen hashes. The earlier R10a cohort remains 1 passed / 1 failed (feed's typed provider-output truncation); its successes are not mixed with older candidates. No paid-model reroll occurred in this repair. The observed receipt overhead is a budget defect, not proof that it caused the earlier provider `max_tokens` stop.

## Explicitly not claimed

No new original real-model acceptance, seven-case supplementary success, latency/token/billing benefit, native cold-start repair, original Kimi session replay, clean release commit, cross-platform native runtime, final installed archive sandbox execution or publication. No worktree, staging, commit, tag, upload, channel change or installed-product replacement was performed.

## Final local snapshot before independent 8k harness work

Current-source R12a namespace is `fe5f4fd2f41c06b0e9f6e1619cc259b4b0b20454`. The namespace before current-source checks equals the post-build namespace. Core default/minimal/CLI JSON each 202/0; owning Clippy, root fmt, workspace all-target and diff check all exit 0. Nine frozen members have matching source identities/digests and 9/9 read-only signature verification. Exact commands and exits are in `current-qualification-status.json` and `current-signature-status.json`.

Full ACP finishes **505 passed / 1 failed / 10 ignored**. The failing native plugin cancellation case times out before its `started` marker at its unchanged 5-second deadline. Same frozen executable, selected case alone: **1 passed**, 2.50 s test body / 2.60 s outer, no fixture pre-run or timeout change; the external read-only observer captures owned processes and triggers no stack sample. This is intermittent and not proven repaired. Full failure and isolated pass remain separate.

The remaining runtime groups pass: Cursor mock TUI 4/0/3 ignored; both critical selfchecks 50/0 (1 and 3 ignored); ACP protocol 14/0; shipped wire 1/0; shipped PTY launch/resume/visible commands 1/0. These use scripted local models and do not constitute real-model task acceptance. No paid attempt occurred in R12/R12a.

`counterexample.json` derives exact pre-fix assertion values: same public observation, receipt-only metadata changes handoff size from 7,436 to 2,057 UTF-8 bytes. The fixed test preserves equality for text/error/image cases. This measures the synthetic budget defect only.

The user subsequently selected independent 8k qualification as the release gate and explicitly selected current checkout only. That authorization is retained in `../2026-11-04-release-8k-qualification/AUTHORIZATION.json`; no additional worktree may be created. Original 4k results remain failed and distinct.
