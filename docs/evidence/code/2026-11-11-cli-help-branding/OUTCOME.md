# CLI help branding

The source-matched binary still exposed `GROK_SANDBOX` and
`GROK_AGENT_DASHBOARD` in ordinary help. Dashboard guidance also named a retired
`config.toml` path. The original clean-environment output is retained losslessly in
[before-output.json](before-output.json) and [before.json](before.json).

The sandbox option now uses the pinned Clap 4.6.7 `hide_env` facility. Its
existing environment binding and parser behavior remain intact. Dashboard help
keeps its action and scope without stale configuration guidance.

## Verification

Commands ran at the Aster root with Rust 1.97.1,
`CARGO_TARGET_DIR=/Users/roylin/code/aster/target` and `CARGO_INCREMENTAL=0`.
[checks.json](checks.json) retains exact commands and log digests.

- Pager CLI tests: **32 passed, 0 failed**, including root/dashboard help and
  preservation of the sandbox environment binding
- Pager all-target Clippy passed with existing warnings
- CLI and ACP built together and independently matched source fingerprint
  `6802032282069feec5ac9e77371477a3f5a0896b`; see
  [source-reconciliation.json](source-reconciliation.json)
- Real binary `aster --help` and `aster dashboard --help` both passed the frozen
  checks in [plan.json](plan.json). These commands use a cleaned environment and
  make no model or tool request. See [after.json](after.json)
- Touched-file formatting and `git diff --check` passed. Workspace formatting
  retains the 18 unrelated failures recorded in the preceding context fix

## Explicitly not claimed

These results cover the two demonstrated help leaks, not every legacy internal
identifier, hidden command, model-supplied name or native TUI surface. Native
visual acceptance is still pending desktop availability. The build remains a
diagnostic dirty-tree candidate containing the separate uncommitted ACP patch;
this commit does not land that patch or publish a release. The local commit
environment lacks lefthook; the explicit checks above are the verification.
