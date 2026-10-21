# Root workspace, one Flow engine

The root workspace resolves one `a3s_flow` target. The package is
`a3s-flow` 2.0.0 and the manifest is `crates/flow/Cargo.toml`.
`crates/flow-v2` is absent from disk and from the root member list.
Every file in the committed `crates/flow-v2` tree is present under
`crates/flow`. The gitignored `packages/ui/node_modules` tree was
removed. The select-timer and scope-cancel checksum constants are gated with
the store feature that reads them. `aster-dream` and `a3s-desktop`
depend on this package with default features off and only `native-ts`.
`a3s-code-core` keeps the dependency optional, at `crates/flow`, and
links it through `dynamic-workflow`.

This record does not replace
`docs/evidence/flow/2026-10-18-single-engine/OUTCOME.md`.

## Validation

Commands ran from the repository root. The Python interpreter was
`/opt/homebrew/bin/python3.13`.

- `cargo metadata --locked --offline --no-deps --format-version 1`:
  one package with target `a3s_flow`, name `a3s-flow`, version
  `2.0.0`, manifest `crates/flow/Cargo.toml`. `aster-dream` and
  `a3s-desktop` use path `crates/flow`, default features off, feature
  `native-ts`. `a3s-code-core` uses the same path, optional, default
  features off, no direct features.
- `python3.13 -m unittest tests.workspace_dependencies.WorkspaceDependenciesTest.test_products_share_one_flow_engine`:
  passed. The same process also ran the `tests.crate_drift_report`
  unit tests. The combined result was 5 passed.
- `cargo test -p a3s-flow --all-targets --features sqlite -- --test-threads=8`:
  108 suites, 556 passed, 0 failed, 1 ignored. The ignored test is
  `native_ts_compiler_executes_real_bun_workflow` (`requires
  A3S_FLOW_NATIVE_TS_COMPILER and Bun`). Exit 0. Log:
  `/tmp/a3s-flow-sqlite.log`.
- `cargo check -p a3s-code-core -p aster-dream -p a3s-desktop --offline --locked`:
  exit 0, dev profile finished in 3m 00s. The log records
  `Compiling aster-dream` and `Compiling a3s-desktop`.
- `cargo test -p a3s-code-core --offline --locked --features dynamic-workflow --lib flow -- --test-threads=8`:
  120 passed, 2 failed, 5179 filtered out. Exit 101.
  `test_dynamic_workflow_parallel_deep_research_inherits_parent_permissions`
  failed with `tool_effect_unresolved`. Run alone with the same
  feature and `--test-threads=1`, it failed the same way.
  `git_parsers_bound_paths_and_do_not_accept_overflow_as_complete`
  was included because its name contains `overflow`. Run alone on the
  default features with `--test-threads=1`, it passed: 1 passed.
- `python3.13 tests/crate_drift_report.py --baseline tests/crate_drift_baseline.json`:
  exit 0. Drifted set remains 11 names. `a3s-flow` remains
  `CONTENT-DRIFT`. The baseline file was not rewritten.

## Explicitly not claimed

No live PostgreSQL run. No edit to published migration SQL
bytes. No claim that a Flow 1.1 database has been upgraded. No merge
of `apps/api/crates/flow` or `apps/cloud/crates/flow` into the root
package. No restore or further deletion of `apps/science`,
`apps/web`, `apps/official`, `docs/evidence` outside this new
directory, or `crates/event`. The real-Bun native TypeScript test
stayed ignored. The parallel deep-research permission failure remains
the baseline failure recorded on 2026-10-18; this landing does not
clear it. No outside-workspace copy of the 1.1 engine was located or
retained by this run.
