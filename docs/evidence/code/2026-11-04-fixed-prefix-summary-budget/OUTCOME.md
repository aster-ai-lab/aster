# Reserve summary space after fixed context

Status: the real-compactor regression fails before the fix (0 passed / 1
failed). Final-source Core default/minimal/CLI JSON each pass 205/0; owning
Clippy, root format, workspace all-target and diff checks pass. Paired build and
freeze pass; seven frozen runtime groups pass. The user paused work during the
last shipped PTY group; its owned processes were stopped. No real-model task
was started for this candidate. No live acceptance or performance improvement
is claimed.

## Observed failure and change

The preceding public-prose candidate fails both critical tasks at their
unchanged 600-second prompt bound after reading every version. Fixed prompt and
mandatory recent context can exceed the old 51%-of-window post-compaction
target. Subtracting this fixed cost from that target collapses the summary to
its 512-token floor despite unused space below the original 85% trigger.

The production budget now applies the existing 60% fraction to the headroom
remaining after fixed context. The original context limit, trigger, summary
ceiling and minimum allowance remain. There is no model output cap change,
new tool, configuration, dependency or task-specific selection. A private
trigger field permits repeated fixed-context reservations without cumulative
subtraction. Both native and fact-backed compaction use this shared budget.

The regression calls the real fact-backed compactor with a 14000-token fixed
prefix and opaque Unicode checkpoints. It requires one actual summary call,
all original checkpoints, unchanged retained input and a reported total below
the original trigger. Another test exhausts all fixed costs from 0 through
24576: reserving more fixed context cannot increase the body allowance. Existing
budget assertions follow the changed allocation and retain the complete-prompt
trigger bound; no model task, input or grader is changed.

An initial conditional implementation passed 57 focused cases, but review found
that its threshold switch could increase the allowance when fixed cost grew.
It was replaced before live qualification. Its source and result are retained
as `compaction.conditional-intermediate.rs` and
`conditional-intermediate-green-status.json`; that result does not verify the
final implementation.

## Actual verification

Darwin arm64, pinned rustc 1.97.1, initialized Studio, root Cargo workspace.
`red-status.json` records the exact failing command, exit 101 and 0/1 result.
`checks-status.json` records every final-source command, elapsed time and actual
result. The three Core profiles each pass 205 tests (overlapping coverage).
Core all-target Clippy with CLI JSON features, `cargo fmt --all -- --check`,
`cargo check --locked --workspace --all-targets` and `git diff --check` exit 0.
`lint-and-diff-review.json` records reviewed candidates, the unchanged fixture
clone follow-up and coverage limits. Captured preimages retain unrelated edits.

`candidate.json` registers source
`55ad0e75dfc6f70f2b5f373390711c7926ef3c28` before any live attempt. Its entire
CLI source namespace differs from the preceding candidate only in the shared
budget implementation and its two owning test files. The two original tasks
remain ordered holdout then feed, one attempt each, with the independent 8192
output ceiling and all other original inputs, model, guards and bounds fixed.

The paired build commands exit 0 (93.19 s and 93.30 s). Complete source
namespaces match registration before and after build. Strict read-only codesign
and digest checks pass 9/9. Frozen ACP full is 506/0/10 ignored; critical
selfchecks are 51/0/1 and 51/0/3 ignored; long-horizon selfchecks 62/0/7 ignored;
Cursor mock TUI 4/0/3 ignored; wire 14/0; shipped wire 1/0. Counts overlap.
Three additional irreducible-fixed-context fallback controls pass 3/0.
`runtime-gates-status.json` retains the seven completed groups. `PAUSED.json`
records the explicit user interruption during shipped PTY and the owned
processes stopped. That group is neither a pass nor a diagnosed test failure.
Both critical model tasks and all seven supplementary tasks remain NOT RUN
for this source. Continuing requires completing the interrupted runtime gate
and rechecking source/configuration identity before any live admission.

## Explicitly not claimed

No independent 8k or original 4k live acceptance, supplementary completion,
isolated latency/token gain, native-startup diagnosis, native Windows/Linux
runtime, GUI e2e, final installed archive, commit, installation or publication.
