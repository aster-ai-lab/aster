# Deep usage coverage for memory and context compaction

Date: 2026-11-09

## Commands and counts

| Command | Result | Count |
| --- | --- | --- |
| `cargo test -p aster-acp --lib an_empty_core_catalog_contains_only_acp_commands` | ok | 1 |
| `cargo test -p aster-acp --lib every_host_method_parses_back_from_its_name` | ok | 1 |
| `cargo test -p aster-acp --lib memory_flush_and_dream_require_a_live_session_and_return_pager_shapes` | ok | 1 |
| `cargo test -p aster-acp --lib deep_memory_inspect_cycle_then_flush_and_dream_without_a_model` | ok | 1 |
| `cargo test -p aster-core --lib test_auto_compact_rearms_and_continues_across_cycles` | ok | 1 |
| `cargo test -p aster-core --lib compact_conversation_writes_a_summary_fact` | ok | 1 |
| `cargo test -p aster-pager --lib root_session_auto_compact_cycle_failure_and_double_complete_do_not_wedge` | ok | 1 |
| `cargo test -p aster-pager --lib aster_tui_function_coverage_is_at_least_95_percent` | ok | 1 (≥ ≥95% |
| `shipped_tui_e2e` binary `shipped_tui_launch_resume_and_visible_commands` (paired `aster`+`aster-acp`) | ok | 1 |
| `shipped_tui_e2e` binary `shipped_agent_session_wire` (paired `aster-acp`) | ok | 1 |

Pair rebuild required before PTY/wire binary runs:

```bash
cargo test -p aster-acp --test shipped_tui_e2e --no-run
touch apps/aster/build.rs crates/code/acp/build.rs
cargo build -p aster -p aster-acp
ASTER_TUI_BIN=$PWD/target/debug/aster ASTER_ACP_BIN=$PWD/target/debug/aster-acp \
  ./target/debug/deps/shipped_tui_e2e-<hash> <filter> --nocapture
```

## Product reachability

- `ASTER_EFFECTIVE_COMMANDS` includes `memory`, `flush`, `dream`.
- Local `/memory` modal stays in `ASTER_UNAVAILABLE_COMMANDS` so ACP inspection replaces it.
- ACP advertises `memory` / `flush` / `dream` (and Core `/history`); pager unhides shell-gated `/flush`/`/dream`.
- Host methods `aster/memory/flush` and `aster/memory/dream` return pager-compatible JSON.

## Deep usage inventory

See `crates/code/acp/tests/tui_913_inventory.md` § Deep usage (memory and context compaction).

## Explicitly not claimed

- 100% of all Aster TUI slash commands at this depth in one land
- Live-model qualification of dream/flush LLM quality (Aster Core dream returns `no_work`)
- Changing telemetry wire names or JWT aliases
- Studio GUI / llvm-cov % as the CI gate for this suite
- PTY auto-compact threshold crossing via a live mock that raises context usage (UI reaction covered by pager root-session oracles; engine cycles by Core tests)
