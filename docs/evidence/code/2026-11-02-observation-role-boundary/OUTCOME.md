# Compaction obligation role boundary

Tool observations and ordinary tool arguments could overwrite the original user's
constraints and open/completed steps when their data contained reserved Markdown
headings. Plain tool messages containing a standing-document example could also
be omitted from the summary input. The existing typed/written role boundaries now
keep those values as data. Explicit `update_plan`, original user requirements and
prior sealed handoffs remain admitted. No new completion authority is introduced.

The implementation changes two source paths. The separate lint prerequisite
removes the idempotent `.aster || .aster` check in porcelain and its two fixture
mirrors; `.aster` remains excluded and no test assertion or input is weakened.

## Actual verification

| Check | Result | Process wall |
| --- | --- | ---: |
| Valid focused red2 | 0 passed, 4 failed | 109.271263s |
| Focused green, identical four tests | 4 passed, 0 failed | 200.219376s |
| Current checkout Core default | 325 passed, 1 ignored | 28.794873s |
| Current checkout Core CLI JSON | 325 passed, 1 ignored | 14.675266s |
| Current checkout Core minimal | 325 passed, 1 ignored | 146.513015s |
| Current checkout ACP connected controls | 33 passed | 86.364590s |
| Exact main plus only reviewed patch/prerequisite | 6 carry tests passed | 196.883517s |
| Exact main final Core all-target Clippy | passed | 9.374802s |
| Exact main final CLI + ACP paired build | passed | 14.930412s |

Focused failures: **4→0 (−100%)**; pass rate: **0%→100% (+100 percentage points)**.
The focused command wall increased **83.23%**,
including compilation/native loading. The test bodies reported0.04s and0.00s
(rounded output, not a zero-time assertion). This is not a task-speed benchmark.
Real LLM calls: **0**. Task-time/token deltas: **N/A**.

Commands ran from repository root with pinned Rust1.97.1, macOS arm64, initialized
Studio and `CARGO_INCREMENTAL=0`. The approved isolated checkout started at remote
main5a9bded4d7136c11ad0957e052f1e7c17170f9c4 with exact Studio43bb70e39670955731e546027844f3caca0808f9.
Its earlier qualified source tree and raw preimages were archived before reuse;
the original checkout and its unrelated changes were preserved.

```sh
cargo test --locked --offline --target-dir target/qualification/capacity-build-464359e5-e38f-44f5-81d2-b02797d264c5 --config profile.test.package.sha2.opt-level=3 -p aster-core --lib compaction_carry::role_tests -- --test-threads=1
cargo test --locked --offline --target-dir target/qualification/capacity-build-464359e5-e38f-44f5-81d2-b02797d264c5 --config profile.test.package.sha2.opt-level=3 -p aster-core --lib -- compaction:: compaction_carry:: fact_control:: agent::conversation_compaction_soak:: agent::turn_context:: agent_api::tests::background_subagent:: tools::task::tests::provider_fairness:: --test-threads=1
```

The connected Core command also ran with `--features serde_json/preserve_order`
and with `--no-default-features --features minimal`. ACP controls used
`cargo test --locked --offline -p aster-acp --lib -- compaction agent::extensions::plan_replay_tests:: agent::emit::tests:: agent::host_methods::tests:: agent::turn_terminal_tests:: --test-threads=1`
with the same target directory/test SHA2 profile configuration.

```sh
cargo clippy --locked --offline --target-dir target/qualification/capacity-build-464359e5-e38f-44f5-81d2-b02797d264c5 --config profile.dev.package.sha2.opt-level=3 -p aster-core --all-targets --no-deps
cargo build --locked --offline --target-dir target/qualification/capacity-build-464359e5-e38f-44f5-81d2-b02797d264c5 --config profile.dev.package.sha2.opt-level=3 -p aster -p aster-acp --bins
rustfmt --edition 2021 --check --config skip_children=true crates/code/core/src/compaction_carry.rs crates/code/core/src/compaction_carry_tests.rs
cargo fmt --all -- --check
```

Current integrated Core/ACP all-target Clippy and workspace formatting pass. Exact
main scoped formatting passes; its workspace formatting still reports **783 other
source files**. Neither role-change file is reported. Existing Clippy blockers
were verified and resolved by the separate, behavior-preserving prerequisite.
All original failures and per-process monotonic phase logs remain in the local
evidence directory; qualification.json retains exact commands and durations.

## Explicitly not claimed

- No real-LLM speed/token gain, full TUI qualification, or Windows/Linux runtime proof.
- The previous changing-feed failure is not attributed to this role fault: that
  numeric input lacks these headings. Prior critical cohort remains1/2 and all7
  supplementary cases remain NOT RUN.
- Existing full-main workspace formatting/CI is not green.
- No frozen task inputs, prompts, models, budgets, timeouts or acceptance rules
  changed. This is a general data/instruction boundary correction, not a task-
  or provider-specific answer or heuristic.
- The first red run's setup expected a constraint without its preserved label;
  that test fixture was corrected before the valid four-test red/green pair.
- Follow-up public metadata shows4/8 summaries hit the unchanged4096 output cap,
  57967 fixed system/schema bytes, and repeated recovery artifact reads. That
  evidence guides further investigation; it is not yet a verified optimization.
