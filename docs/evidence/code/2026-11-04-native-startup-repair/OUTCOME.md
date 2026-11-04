# Native startup and independent 8k qualification follow-up

Status: the Studio test-fixture compile blocker is repaired and workspace
qualification passes. The same-source CLI/ACP pair and all eight frozen runtime
groups pass. The first independent 8k real-model critical cohort finishes 1/2;
seven supplementary cases remain NOT RUN. No native-startup production repair
or complete real-model acceptance is claimed.

The user requested continued repair after the status review. Existing authorization
for the independent 8,192-output cohort and current-checkout-only work remains in
`../2026-11-04-release-8k-qualification/AUTHORIZATION.json`. The original 4k failures,
model, tasks, inputs, graders, request/context limits and deadlines stay fixed.
No additional worktree, commit, installation or publication is part of this repair.

## Investigation

The original ACP full suite was rebuilt from current sources, using the pinned
Rust 1.97.1 toolchain and the existing qualification target cache. It passes
506 tests, with 0 failures and 10 ignored. Source preimages and hashes are
retained before any edit. The Studio submodule is initialized at
`90ba531b6f857636c4caf50a49b88529716396c0`.
Exact command, elapsed time and exit code are recorded in
`baseline-acp-full-status.json`; `baseline-acp-full.log` retains the full result.
A read-only process observer records descendants of the owned test invocation.
No executable fixture is pre-run, no startup timeout is extended and no assertion
is weakened.

The current ACL hash matches the authorized `a9d012ce…` configuration. No secret
configuration bytes are copied into evidence.

`independent-8k-selfchecks-status.json` records 51/0/1 ignored, 51/0/3 ignored,
and 62/0/7 ignored for the two critical runners and original long-horizon runner.
The original compiler-fixture seed test passes in the complete group. Historical
intermittent failures remain retained; no cause is established or declared fixed.

Current Core default/minimal/CLI JSON connected tests each pass 202/0. ACP
all-target Clippy passes. The new shared output-budget helper/field is unused in
some independently compiled test targets; the critical and supplementary targets
consume it. Existing unrelated findings and exact follow-ups are in
`lint-review.json`; no lint allowance is added.

## Workspace blocker repair

The first root format check fails on a pre-existing line wrap in Studio
`native/data-sqlite/src/main.rs`; the exact two-line rustfmt change passes the
next root format check. No behavior changes.

The first workspace all-target check fails with E0603/E0624 at
`authority_local_plugins_tests.rs:35–36`: a new sibling test uses the private
`InstalledPlugin` fixture and its private `new`/`caps` methods. All callers were
inspected. The fix adds `pub(crate)` only to those three declarations in the
existing `#[cfg(test)]` module. Fields, cleanup, fixtures and all assertions stay
unchanged; no normal-build API or authorization behavior changes. The actual
caller and original publication tests pass **13/0** (1.10 s test body; 271.28 s
including the dependency build). Owning Studio all-target Clippy, root formatting,
root workspace all-target check and both root/submodule diff checks exit 0.
`studio-fixture-status.json` retains exact commands and times. Original failed
logs and the submodule source preimages remain retained. All unrelated Studio
renderer changes are preserved. No GUI or installed-application run occurred.

## Independent 8k admission

`authorized-budget-patch-fidelity.json` confirms all six existing budget-related
Rust/Python changes exactly match the previously authorized patches. The nine
critical sources are either byte-identical to the prior frozen originals or
match those exact budget-only patches. The graders, tasks and inputs are not
edited. Three current Core profiles, owning ACP/Studio Clippy, root formatting,
dependency boundaries and the workspace check pass before the new pair build.
CLI pair identity 3/3, delivery 21/21, sandbox contracts 7/7, smoke-oracle
selfchecks 3/3, release governance 10/10, dependency boundaries 7/7 and release
workflow 6/6 pass. These are contract/selfchecks, not final-archive installation.

The new registration is
`../2026-11-02-summary-input-retention/independent8k-20261104-native-followup-1-qualification-preregistration.json`.
Its source namespace `60b0b21e6353e17026ee86454696167e145bea19` still matches the
pre-build admission snapshot. Only the separately authorized output ceiling
changes from 4,096 to 8,192; all historical 4k results remain separate.

The paired build/freeze exits 0 in 166.14 s. All nine members match the registered
source; read-only strict codesign verification passes 9/9 without changing any
digest. `runtime-gates-status.json` records the actual frozen executable results:
ACP full 506/0/10 ignored; critical selfchecks 51/0/1 and 51/0/3 ignored;
long-horizon selfchecks 62/0/7 ignored; Cursor mock TUI 4/0/3 ignored; ACP wire
14/0; shipped wire 1/0; shipped PTY launch/resume/visible commands 1/0. Counts
overlap. All original startup bounds and assertions remain unchanged. Both
current full ACP runs pass; neither establishes the historical startup cause.

`ADMISSION.json` rechecks the unchanged complete source namespace, authorized
ACL, exact budget-only patches, nine critical sources and all frozen digests
before the first live attempt. The two critical tasks run once each in their
original order using GLM 5.3 and the independent 8k ceiling. No comparison to an
older configuration is selected, and all seven supplementary cases remain gated
until both critical tasks pass on this same candidate.

The first independent 8k critical cohort finishes **1 passed / 1 failed** in
963.61 s. Holdout passes the original artifact oracle after all five source
versions, 17 upstream calls and three compactions. Feed reads all seven versions
with unchanged arguments and identities, but times out at the original 600 s
prompt bound after 21 upstream calls and eight compactions. It creates no final
report. Identity, source non-mutation, tool terminal and cleanup checks remain
intact, and neither task records a transport or protected-response failure.
`critical-verdicts.json` and the original live artifacts retain the actual
results; missing native usage is never treated as zero. Seven supplementary
cases are explicitly NOT RUN in `supplementary-status.json`.

Inspection finds that the feed's public intermediate results exist in its
original archive, but deterministic summary trimming removes earlier rows and
the model spends further requests recovering them. The first grep searches a
derived-summary notation and correctly finds no original match; the second grep
returns original public checkpoints. The archive is not declared broken. A new
actual-splitter regression and bounded public-prose retention repair are tracked
separately in `../2026-11-04-public-prose-retention/OUTCOME.md`; no unchanged
real-model reroll is performed for this candidate.

## Explicitly not claimed

No diagnosed or repaired native-startup cause, full current-source acceptance,
independent 8k live pass, original 4k acceptance, supplementary-case completion,
latency/token/billing improvement, native Windows/Linux runtime qualification,
final installed archive execution or release readiness.
