# Aster TUI function coverage — analysis and anti-overfit hardening

## Goal and result

Keep the authoritative Aster TUI function-coverage gate ≥ 95%, and close
**real product inconsistencies** that the previous single-row polish left open
(especially Plan entry paths and list dual-oracles). Do **not** inflate % by
shrinking the denominator or weakening `!effects.is_empty()` style oracles.

**Gate result: still 95/95 = 100% (PASS)** after strengthening
`plan-mode-excluded` to cover dashboard staging as well as agent Shift+Tab.

## Coverage definition (unchanged)

`aster_tui_function_coverage_is_at_least_95_percent` in
`crates/terminal/crates/codegen/aster-pager/src/app/dispatch/tests/prompt.rs`:
observed effect / product list rows over `ASTER_*` commands + settings +
`effort-menu` + `plan-mode-excluded` (95 rows). Not llvm-cov. Not the 1,020
PLANNED catalog.

## Analysis (overfit / vacuous risk)

| Finding | Severity | Disposition |
| --- | --- | --- |
| ~59/95 rows are absence / static membership | Structural weakness | Kept as inventory contract; not deleted to inflate % |
| `command_changes_the_screen` accepts bare `!effects.is_empty()` | Weak oracle | Documented; not “fixed” by deleting rows |
| Agent Shift+Tab Plan filter alone left dashboard / `--plan` / enter-plan open | Real inconsistency | Closed in this land |
| e2e `UNAVAILABLE`∩`FULLSCREEN` for `memory`/`tasks` with different descriptions | Dual false-oracle | Removed from UNAVAILABLE; kept ACP FULLSCREEN + effects |
| Inventory marked `fork` / ACP `memory`/`tasks` absent incorrectly | Doc drift | Inventory updated |
| ACP session “coverage” gate is helper mapping (19/19) | Synthetic | Explicitly not claimed as PTY coverage |
| Live pin expects 42 unavailable names; list is 41 | Pre-existing drift | Not claimed fixed |

## Product changes (anti-overfit)

Under `aster_active()` / `apply_aster_slash_gate()`:

1. Agent Shift+Tab: Plan filtered from `mode_choices`; nudge jump disabled; pre-session Normal skips Plan (prior land).
2. Dashboard Shift+Tab: `DashboardDispatchMode::cycle(..., exclude_plan)` never stages Plan; gate clears a staged Plan.
3. `dispatch_enter_plan_mode` and `set_plan_mode(On)` refuse with toast; Off still allowed to leave Plan.
4. Aster handshake clears launch `plan_mode` and `contextual_hints.plan_mode`.

## List / oracle hygiene

- `shipped_tui_e2e`: drop dual `memory`/`tasks` UNAVAILABLE rows; drop `history` from FULLSCREEN (local unavailable); keep ACP `memory`/`tasks` effects; stop claiming `/history` as an offered effect.
- `tui_913_inventory.md`: document fork / expand / fullscreen / ACP memory·tasks; correct absent table for plan/cd/fork/memory/tasks.

## Commands and counts

```text
cargo test -p aster-pager --lib --locked aster_tui_function_coverage_is_at_least_95_percent -- --nocapture
# PASS

cargo test -p aster-pager --lib --locked dashboard_cycle_mode_skips_plan_under_aster -- --nocapture
# PASS

cargo test -p aster-pager --lib --locked code_shift_tab_never_selects_published_plan -- --nocapture
# PASS

cargo test -p aster-pager --lib --locked code_plan_yolo_exit_preserves_backend_posture -- --nocapture
# PASS

cargo fmt -p aster-pager -- --check
# PASS
```

## Explicitly not claimed

- llvm-cov / F-table ≥ 95%
- Studio Gate A/B
- Full `shipped_tui_e2e` PTY green in this land (list edits only; not re-run here)
- Live e2e unavailable-count pin 41→42 reconciliation
- Moving `cd` off EFFECTIVE (still dashboard-only toast; dead-end with `/dashboard` hidden remains open)
- Filtering Plan out of ACP `SessionModeState` wire catalog
- Replacing ACP `tui_session_function_coverage_*` synthetics with PTY oracles
- Strengthening every `command_changes_the_screen` row to command-specific observables
