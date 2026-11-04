# Local main integration and remaining work

Status: source integration on the existing local `main` branch, as requested
by the user. Long-running qualification remains paused. This is source integration,
not an acceptance or release declaration.

## Scope

Include the previously staged Core/ACP optimization, CLI pairing, owning
dependency changes and qualification fixtures recorded in
`../2026-11-04-release-8k-qualification/release-selected-source.json`, the
public-prose retention fix and the fixed-prefix summary budget repair. Retain
the current regression tests and evidence. The two Studio check repairs are
already present in the initialized Studio `main` history and need no new
submodule commit or pointer change.

Preserve the independently staged Box SDK provenance changes in
`docs/migration.md` and `docs/source-manifest.json`. Preserve unrelated gateway,
Studio and other working changes. Do not resume model tasks, install, publish,
push, create a branch or create a worktree for this integration.

The working `Cargo.lock` changed independently after frozen qualification. Use
its already-verified staged snapshot for this commit and preserve the newer
working bytes as an unstaged change. An isolated Git index excludes unrelated
staged documents without unstaging them in the user's index. Recheck every
committed CLI-source member against the latest frozen source manifest.

## Actual verification already retained

Darwin arm64, pinned rustc 1.97.1, initialized Studio, root workspace. The latest
candidate is `55ad0e75dfc6f70f2b5f373390711c7926ef3c28`; full arguments, feature
sets, elapsed times and actual results are in the adjacent evidence folders.

- Core default/minimal/CLI JSON: each 205 passed, zero failed; three additional
  fixed-context fallback controls pass. Counts overlap.
- Effect: 169 passed, one ignored; Studio authority/capability tests: 13 passed.
- Owning Clippy, root format and workspace all-target checks pass.
- Latest paired build and freeze pass; strict read-only signatures and digests
  pass 9/9. Frozen ACP full: 506 passed, ten ignored; the two critical harness
  selfcheck groups: 51 passed each; original long-horizon selfchecks: 62 passed.
- Seven frozen runtime groups pass. The final shipped PTY group was stopped at
  the user's explicit pause; it is neither a pass nor a diagnosed failure.
- Prior independent 8k candidates score 1/2 and 0/2 on real critical tasks.
  Both fail the complete gate; their successes cannot be combined.

The original commands/results remain in
`../2026-11-04-native-startup-repair/OUTCOME.md`,
`../2026-11-04-public-prose-retention/OUTCOME.md` and
`../2026-11-04-fixed-prefix-summary-budget/OUTCOME.md`. Committing does not turn
mixed-working-tree verification into clean-revision qualification.

## Remaining work, in order

1. Recheck source/configuration identity and finish the interrupted shipped
   PTY gate. Diagnose any real native-startup failure at the existing bound;
   historical native-startup flakiness still has no demonstrated root fix.
2. Measure the fixed system/tool-schema cost in the 16k feed scenario. The new
   allocation cannot reclaim space when fixed context already exceeds the
   original trigger; whole-task convergence is not proved by the local budget
   regression. Then admit the unchanged original holdout and feed tasks once
   each on the same frozen candidate. Preserve all previous failed cohorts.
3. Only after both current critical tasks pass, run the original seven
   supplementary cases, with the same prompts, inputs, seeds, model, limits
   and graders. They remain NOT RUN for the latest candidate.
4. Complete clean-revision Kimi/TUI and required platform qualification. Keep
   untested native Windows/Linux targets explicit. A GUI e2e run requires a
   fresh Studio build from this checkout before launch.
5. Before release, verify the final CLI/ACP/sandbox and desktop companion
   archives, install into an isolated prefix and pass actual installed-command
   smoke gates. Retain latency/token evidence without claiming an isolated
   improvement from failed tasks or different source cohorts.

## Explicitly not claimed

No latest-candidate real-model pass, original 4k acceptance, seven-case
completion, proved latency/token/billing gain, native-startup root-cause fix,
native Windows/Linux or GUI e2e result, final archive qualification,
installation, remote push or publication.
