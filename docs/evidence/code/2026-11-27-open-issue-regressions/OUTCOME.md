# ACP/Core/TUI issue regression evidence — 2026-11-27

## Scope

Covers #322 and #329–#334, plus the ACP half of #328. GitHub bodies, not the earlier shifted history-summary numbering, define the scope.

## Focused checks actually run

Each command below ran with `--locked -p <package> --lib <filter>` and finished with **1 passed, 0 failed**:

```text
aster-pager         timeout_rejects_submillisecond_zero_rounding_and_roundtrips_valid_values
aster-pager         demote_code_backend_refuses_the_missing_migration_with_an_actionable_message
aster-pager         switch_hint_code_backend_is_resume_even_with_dashboard_enabled
aster-pager         thinking_body_uses_ui_width_limit_and_terminal_boundary
aster-pager         set_max_thoughts_width_clamps_out_of_range
aster-pager-render  prime_seeds_all_caches
aster-core          cancellation_description_declares_the_independent_process_group_limit
aster-acp           reconciliation_wire_error_carries_an_action_and_refuses_retry
aster-acp           gc_duration_is_bounded_and_rejects_invalid_values
aster-acp           every_host_method_parses_back_from_its_name
aster-acp           office_file_schema_uses_single_string_types_for_compatible_gateways
```

Results by issue:

- **#322:** `0.0001` seconds is rejected before it can round to 0 ms; valid millisecond values survive ACL reload and a subsequent save.
- **#329:** Code-backend session hints use `/resume`, not unsupported `/dashboard`.
- **#330:** the pager sends object params, ACP exposes list/gc/db-path, and the golden wire fixture includes all three methods. A real `target/debug/aster-acp` stdio session initialized with protocol 1, returned an empty worktree list, completed `gc` with `dryRun=true`, and returned the isolated `$ASTER_HOME/worktrees.db` path. No `-32601` occurred. See `acp-worktree.json`.
- **#331:** `kill_task` explicitly documents that cancellation can precede cleanup and that independent `setsid/setpgid` groups can survive. This is an intentional scope clarification, not a claim that those groups are now killed.
- **#332:** unresolved effects return `manual_review`, concrete read/new-session instructions, and explicitly state that no `reconcile_tool_effects` RPC exists.
- **#333:** Code backend Ctrl+B now returns no unsupported migration effect and tells the user to let the command finish or use Ctrl+C.
- **#334:** the saved width is primed into the render cache, changes invalidate current scrollback heights, terminal width remains an upper bound, and the body rail subtracts its own columns.
- **#328:** both model-facing and internal `office_file` schemas recursively reject array-valued `type`; the two former scalar arrays are represented by nine single-type branches. The actual loopback task request is recorded in `docs/evidence/desktop/2026-11-27-open-issue-regressions/`.

Additional checks:

- `just build` — passed.
- `cargo test --locked -p aster-studio --lib agent_plane::launcher::tests` — **24 passed**.
- `cargo test --locked -p aster-studio --lib providers::local_tenant_tests` — **11 passed, 1 ignored**.
- `cargo clippy --locked -p <package> --all-targets --no-deps` for `aster-acp`, `aster-studio`, `aster-pager`, `aster-pager-render`, and `aster-core` — all exited 0 with existing warnings.
- Root and Studio `git diff --check` — passed.

## Full-suite results and failures

- `cargo test --locked -p aster-acp --lib` after the golden update: **558 passed, 1 failed, 10 ignored**. The remaining failure is `agent::tests::budget_line_states_a_maximum`, asserting the old text `Stop when the user's outcome is done.`; current unrelated prompt edits removed that sentence. The worktree golden failure was fixed, and the full-run resume `AlreadyExists` and MCP rollback failures each passed when rerun individually. The full ACP suite is not claimed green.
- `cargo test --locked -p aster-pager --lib`: **10350 passed, 58 failed, 4 ignored**. Failures span existing turn-completion, plan-mode, settings, navigation and help tests. The issue-focused tests above pass; the full Pager suite is not claimed green.
- `cargo fmt --all -- --check` failed on many existing unrelated Rust formatting differences. No blanket format/reformat was applied. Whitespace/error checks through `git diff --check` passed.

## Explicitly not claimed

- No independent-process-group cancellation implementation was added or measured beyond the documented contract.
- No live external model, third-party gateway, or real destructive worktree GC was used.
- No full workspace `just test` pass, full Studio suite pass, full typecheck pass, or `cargo fmt --all -- --check` pass is claimed.
- No commit, push, submodule landing or parent gitlink bump was performed.
