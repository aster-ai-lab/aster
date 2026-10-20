# Agent runtime boundary

The product-entry switches landed after this note. See
`docs/evidence/development/2026-10-20-shared-acp-agent/OUTCOME.md`.
The counts below are the earlier boundary slice.

## Result

The executable slices of the runtime remediation route are in the
working tree. `a3s-code-acp` describes itself as the ACP stdio agent
for Aster desktop hosts. Startup no longer hides `GROK_NPM_REGISTRY`
behind `cryptify` or `obfstr`. Process-facing strings in
`apps/aster/src/main.rs` say Aster. SVG rendering and account import
are package features. The `aster` crate allow list is empty.

The terminal turn loop stays on the pager. The inline tests in
`event_loop.rs` stay in that file. No new product setting appeared,
so no ACL block was added.

## What landed

- `crates/code/acp/Cargo.toml` and `src/lib.rs` — description and crate
  docs name Aster desktop hosts and `a3s-code-core` 9.1.4. Package name
  and version stay `a3s-code-acp` 9.1.4.
- `apps/aster/src/main.rs` — `build_update_config` reads
  `GROK_NPM_REGISTRY` directly. Stderr prefixes are `Aster:`. The
  telemetry client is `aster`. Leader and update client names are
  `aster-leader-cli` and `aster-update`. The user message names
  `aster agent`. `$GROK_HOME`, `GROK_WORKER_THREADS`, and `bin/grok`
  stay.
- `apps/aster/Cargo.toml` and `Cargo.lock` — `cryptify` and the aster
  edge on `obfstr` are gone. The lock diff is 23 deletions. The
  `obfstr` package block stays because another crate still uses it.
- `crates/code/acp` features — `default = ["svg-png", "account-import"]`.
  The binary `required-features` are the same pair. Existing
  `cargo build -p a3s-code-acp` lines still produce a binary that can
  render PNG and honor host account opt-in. `a3s-code-mermaid`,
  `quick-xml`, and `rusqlite` are optional. `--no-default-features
  --lib` compiles without them. An empty `import_accounts` list still
  imports nothing. No fourth importer was added.
- `apps/aster/src/main.rs` allow list — `unreachable_code`,
  `unused_mut`, `unused_variables`, `unused_imports`, and `dead_code`
  are gone. Three source-build stub parameters are prefixed with `_`.
  `signal_leaders_to_relaunch` is deleted. The source-build update
  path returns before that function could run, and it had no caller.
  `#![deny(clippy::indexing_slicing)]` stays.
- `tests/agent_runtime_boundary.py` — a missing crate allow is an empty
  set. An allow attribute after `#![deny(...)]` still counts. Growth
  fails. The script has no switch that raises a ceiling.
- Pager bash — `native_refusal_tests.rs` names the command `true`.
  With `A3S_REFUSE_UNQUALIFIED_NATIVE_SHELL` set, the gate returns
  `native_process_lifetime_unqualified` before spawn. Unset, this gate
  returns none. The env flag was not copied into ACP. The two refusal
  mechanisms were not merged.

## Ceilings

- `main_allow`: `[]`
- Identity needles in `main.rs`: `cryptify::flow_stmt!` 0,
  `obfstr::obfstr!` 0, `grok:` 0, `grok-pager` 0, `GROK_NPM_REGISTRY` 1
- `event_loop.rs` inline tests: 2173 lines, one `mod tests {`
- Account modules: `cc_switch`, `cursor_account`, `grok_account`
- ACP terminal path: `a3s-code-mermaid` only
- Config filename constants: the existing seven

## Commands and counts

- `cargo test --locked --offline -p a3s-code-acp --lib artifact_image`
  — 4 passed, 337 filtered, 0.24s. Test profile finished in 8m 18s.
- `cargo test --locked --offline -p a3s-code-tools --lib true_is_`
  — 3 passed, 3322 filtered, 0.00s. Two are the new refusal tests.
  The third is the pre-existing skills test
  `bool_fields_only_literal_true_is_true`, matched by the filter.
  Test profile finished in 6m 42s.
- `cargo check --locked --offline -p a3s-code-acp --no-default-features --lib`
  — finished in 1m 47s.
- `cargo check --locked --offline -p a3s-code-acp --bin a3s-code-acp`
  — finished in 27.77s. Default features include both required
  features, so the binary is checked.
- `cargo check --locked --offline -p aster` — finished in 3m 42s
  before the allow list was emptied.
- `cargo check --locked --offline -p aster --message-format=short`
  — finished in 41.50s after deleting `signal_leaders_to_relaunch`.
  Exit 0. `aster` emitted no warning. Pre-existing `f32` fallback
  warnings in `a3s-code-tools` web search remain.
- `cargo fmt -p aster -- --check` — exit 0.
- `python3 -m unittest tests.agent_runtime_boundary -v` — 15 tests,
  OK, 0.003s. The interpreter is Python 3.9.6. This count is the
  run from before the shared-ACP client ratchet below.

## Shared ACP client ratchet

The interactive pager already spawns `a3s-code-acp` when the sidecar
binary is present. Headless, `models`, `worktree`, and `aster agent
stdio` still enter the in-process shell. This slice records that
split and corrects the stale `9.0.0` comment. No entry point was
switched.

- `crates/terminal/crates/codegen/a3s-code-pager/src/acp/mod.rs` —
  the product-path comment now says `a3s-code-core 9.1.4`. The file
  contains zero `9.0.0`.
- `tests/agent_runtime_boundary.py` — shell spawn call sites are the
  closed set `headless.rs`, `models.rs`, `worktree_cmd/mod.rs`.
  `acp/spawn.rs` is the definition and is not a call site.
  `agent_runtime.rs` still contains `fn run_stdio(` and
  `run_stdio_agent(`. Interactive connect still names
  `a3s_spawn::a3s_acp_enabled()` before `a3s_spawn::spawn_a3s_acp(`.

Commands:

- `python3 -m unittest tests.agent_runtime_boundary -v` — 20 tests,
  OK, 0.156s. Python 3.9.6. Five new tests reject an extra
  `spawn_grok_shell(` call site, dropping `run_stdio_agent(`,
  reordering the sidecar gate, and putting `9.0.0` back.
- `cargo check --locked --offline -p a3s-code-pager --message-format=short`
  — finished in 3m 45s, exit 0.

## Explicitly not claimed

- `cargo test -p aster` was not run.
- Core `both_native_apis_refuse_uniformly_without_effects_or_output`
  was not re-run. The route records that the core test already names
  `true` and stops there.
- The terminal turn loop was not moved onto `a3s-code-core`. That
  slice needs a spec under `docs/spec-driven-development.md`.
- The 2173-line inline test module was not moved.
- Version, edition, license, and `a3s-*` crate names were not unified.
- The TOML loader was not migrated. No new ACL setting was added.
- Pager directory layout and license notices were not rewritten.
- OCI remains the product default sandbox backend.
- Studio delivery-alignment files already in the working tree were
  not edited, staged, or reverted.
- Pre-existing warnings in `a3s-use-bge`, `a3s-code-core`, and
  `host_context.rs` `as_str` were not fixed.
- No GUI run and no live model run.
- Nothing was committed.
- Headless, `aster agent stdio`, `models`, and `worktree` were not
  switched onto `a3s-code-acp`. The leader cluster still holds an
  in-process `MvpAgent`. `specs/002-shared-acp-agent/` was not created.
