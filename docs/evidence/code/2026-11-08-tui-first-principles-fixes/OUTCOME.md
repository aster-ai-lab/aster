# TUI first-principles fixes (model authority + slash dismiss)

## Result

Implemented the P0 authority/state fixes from the first-principles plan:

1. Interactive ACP spawn now forwards CLI `--model` to `aster-acp`.
2. Esc dismiss of the slash palette clears a single-line `/…` draft so prose cannot concatenate into `/effortReply`.
3. Visual companion harness documents region-capture + file oracles and answers `ask_user` cards.

## Code

| Change | Path |
| --- | --- |
| `ConnectFlags.model` + `connect()` spawn forward + log | `crates/terminal/.../acp/mod.rs` |
| Wire `args.model` into connect flags | `crates/terminal/.../app/mod.rs` |
| Spawn argv contract test | `crates/terminal/.../acp/aster_spawn/argument_tests.rs` |
| `ASTER_DEFAULT_MODEL` outranks workspace `default_model` | `crates/code/acp/src/config.rs` |
| `slash_dismiss` / `clear_incomplete_slash_draft` | `crates/terminal/.../views/prompt_widget/mod.rs` |
| Esc uses `slash_dismiss` | `agent_view/prompt.rs`, `app_view.rs`, `dashboard/state.rs` |
| Visual harness + ask_user driver | `apps/aster/tests/visual_tui_harness/` |

## Verification (actual)

```text
cargo test -p aster-acp --locked aster_default_model_env_outranks_workspace_default_model --lib
→ ok

cargo test -p aster-pager --locked interactive_connect_spawn_args_include_cli_model_override
→ ok

cargo test -p aster-pager --locked incomplete_slash_draft
→ esc_clears_incomplete_slash_draft_so_prose_cannot_concatenate ok
→ clear_incomplete_slash_draft_leaves_multiline_prose ok
```

Restored `specs/031-cli-mise-replacement/contracts/mise-upgrade-reference.json` so
`aster-pager` lib tests compile (`include_str!` path was dangling after backend
migration).

## Explicitly not claimed

- Live re-run of ignored `tui_913_real_llm` Layer C gates after this patch (hermetic
  unit/contract tests only in this OUTCOME).
- Full visual multiturn re-drive on a headed machine in this session.
- Product policy changes that suppress model `ask_user` for checker runs (harness
  answers the card instead).
- agent-desktop `screenshot` / `press --headed` / `clipboard-set` reliability.
