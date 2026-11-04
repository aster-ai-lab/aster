# Retain fitting public progress across a compacted tool result

Status: focused pre-fix regression fails 0/1; the repaired Effect package passes
169 tests with 1 ignored. Connected Core default/minimal/CLI JSON each pass
203/0; owning Clippy, root format and workspace checks pass. The new paired
build/freeze and all frozen runtime controls pass. Its first independent 8k
critical cohort fails 0/2 at the unchanged prompt deadline. No real-model
acceptance or performance benefit is claimed.

## Observed failure

The preceding independent 8k candidate at source
`60b0b21e6353e17026ee86454696167e145bea19` passes holdout but times out on feed at
the original 600-second prompt bound. Feed reads all seven versions, publicly
states the cumulative intermediate results, performs eight compactions and
never writes the final artifact. Compactions occupy 218.12 seconds. The last
summary's deterministic middle trim removes earlier intermediate values; the
model explicitly searches the original public archive to recover them. The
second search correctly returns those original public results. No transport or
protected-response failure occurs. Full results remain in
`../2026-11-04-native-startup-repair/critical-verdicts.json`; all seven
supplementary cases remain NOT RUN. This does not identify every cause of the
whole-task timeout or establish an isolated performance comparison.

## Change

Effect's existing `split_compact_tail` stops at a completed tool batch that
cannot fit in the recent-history budget. That also excludes earlier short public
assistant text even when the unused budget could retain it. The new hermetic
test reproduces this with an opaque public checkpoint, a large old observation
and a small subsequent tool exchange. It calls the actual splitter, requires the
original checkpoint in the retained view, checks exact original partition bytes
and the existing token allowance, and fails before the production edit.

The same splitter now uses unused tail allowance for recent public assistant
text, retaining original order. Completed tool calls/results still form the
same suffix; original strings and facts remain unchanged. The zero-budget path
and mandatory most-recent batch behavior are unchanged. No new ledger, store,
controller, tool, configuration, dependency, permission or public API is added.
Selection uses only the existing scheduler's role/record representation and
recency, never task vocabulary, field names, values, formulas, seeds or models.

A connected Core provider-boundary test verifies that retained public text
survives a lossy summary without reopening its old tool call or private
continuation. Separate original responses with identical public text stay
separate; the current call's private continuation is restored only in its
original channel. Original facts must remain byte-identical.

## Actual verification

Darwin arm64, pinned rustc 1.97.1, initialized Studio, root workspace and the
existing qualification cache. Exact commands, elapsed times and exit codes are
retained in `red-status.json`, `effect-green-status.json` and `checks-status.json`.
The Effect full package green is 169 passed / 1 ignored across its targets.
Core's Rust 2021 edition is retained; unrelated formatting from an intermediate
direct Rust 2024 formatter invocation was restored to the captured preimage
before the repository checks.

The focused positive/negative budget controls pass 1/1. A direct format check
initially reports one wrapped binding added with the negative control; it is
corrected before the final root format gate. Each Core profile passes the new
provider-boundary case and all 202 prior connected cases. Effect/Core all-target
Clippy, `cargo fmt --all -- --check`, root workspace all-target check and
`git diff --check` exit 0. Existing unrelated warnings and an unchanged synthetic
contract clone finding are recorded with an exact follow-up in `lint-review.json`.
There is no full-guideline-compliance claim.

`candidate.json` registers source
`3b78de62a5032c928ec4e58e9ce5a6aad16cb614` before a new live attempt. The complete
namespace differs from the preceding 8k candidate only in the splitter and its
two owning test files. Original critical/supplementary tasks, prompts, inputs,
graders, GLM 5.3, authorized ACL, independent 8k output ceiling, original
context/request caps and deadlines stay fixed. The meaningful production fix
has a retained failing regression; this is not an unchanged failure reroll.

The paired build/freeze exits 0 in 185.51 s with the same complete namespace
before/after. All nine frozen identities/digests match; read-only strict
codesign passes 9/9 without changing any digest. Frozen ACP full is 506/0/10
ignored; critical selfchecks 51/0/1 and 51/0/3 ignored; original long-horizon
selfchecks 62/0/7 ignored; Cursor mock TUI 4/0/3 ignored; wire 14/0; shipped wire
1/0; shipped PTY 1/0 (87.21 s). Counts overlap. `runtime-gates-status.json`
retains original commands, bounds and all results.

`ADMISSION.json` rechecks the complete source namespace, authorized ACL, all
nine frozen digests, unchanged nine critical source files and six authorized
budget-only helpers before the two original tasks run once each in order.
This source has no selected performance baseline. The first candidate's 8k
1/2 result and every original 4k failure remain retained and distinct. Seven
supplementary cases remain gated on both current critical cases passing.

The actual first-attempt results in `critical-verdicts.json` are holdout 0/1
(600074 ms, five versions/read events, six compactions, 22 upstream calls) and
feed 0/1 (600072 ms, seven versions/read events, eight compactions, 19 upstream
calls). Neither writes the requested final artifact. Neither has a transport or
protected-response failure. Source and configuration stay unchanged and cleanup
completes. All seven supplementary cases remain NOT RUN. These failures cannot
be combined with the preceding candidate's holdout success.

Retaining original public prose is locally verified, but does not establish
whole-task convergence. A large mandatory tool tail can still leave no spare
tail allowance; the separate summary body also falls to its 512-token floor
when fixed context exceeds the old post-compaction target. The next bounded
budget repair is tracked in `../2026-11-04-fixed-prefix-summary-budget/OUTCOME.md`.

## Explicitly not claimed

No new independent 8k live acceptance, original 4k acceptance, supplementary
completion, proof that all intermediate results always fit, isolated token or
latency improvement, diagnosed native-startup cause, native Windows/Linux
runtime, GUI e2e, final installed archive, commit, installation or publication.
