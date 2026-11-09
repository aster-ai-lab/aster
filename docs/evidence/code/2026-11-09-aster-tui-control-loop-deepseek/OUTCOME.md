# Aster TUI visual control-loop — agent-desktop + DeepSeek

Date: 2026-11-09  
Companion to `crates/code/core/tests/test_control_loop_deepseek_e2e.rs` and
`docs/evidence/code/2026-11-09-aster-core-control-loop-deepseek/`.

## Result

**pass** on staged `aster` 0.3.1 / `aster-acp` 9.2.1 from this checkout.

| Visual ID | Core map | Oracle |
| --- | --- | --- |
| V-R1 | R1d write sensor | `loop_out.txt` exact token; screenshot `03-r1-write` |
| V-R2 | R2 pause/resume controller | `/goal` progress → pause copy → resume → bound status |
| V-R3 | R3 reopen feedback | `--continue` keeps token bytes + bound `/goal status` |

Model pin: `ASTER_CONFIG=$PWD/.aster/config.acl` + CLI `--model deepseek/deepseek-flash`  
(footer `deepseek/DeepSeek Flash`; welcome `deepseek_visible_on_welcome=true`).

Summary (`visual-control-loop-summary.json`):

```json
{
  "pass": true,
  "r1_loop_out_exact": true,
  "r2_paused_visible": true,
  "r2_status_bound": true,
  "r3_loop_out_survived": true,
  "r3_goal_still_bound": true,
  "goal_out_written": true,
  "goal_out_exact": true
}
```

## Commands

```bash
cargo build --locked -p aster -p aster-acp
mkdir -p /tmp/aster-visual-cl/bin
cp target/debug/aster target/debug/aster-acp /tmp/aster-visual-cl/bin/

# Terminal.app must be running (agent-desktop app name: 终端)
python3 apps/aster/tests/visual_tui_harness/drive_control_loop.py \
  --bin-dir /tmp/aster-visual-cl/bin \
  --work-dir /tmp/aster-visual-cl/work \
  --model deepseek/deepseek-flash \
  --config $PWD/.aster/config.acl \
  --evidence docs/evidence/code/2026-11-09-aster-tui-control-loop-deepseek
```

Wall clock (final driver): ≈105s (`driver.log`).

## Evidence

| Step | Screen | Hist |
| --- | --- | --- |
| Welcome / DeepSeek pin | `screens/01-welcome.png` | `01-welcome-hist.txt` |
| Empty `/goal status` | `screens/02-goal-empty.png` | `02-goal-empty-hist.txt` |
| V-R1 write | `screens/03-r1-write.png` | `03-r1-write-hist.txt` |
| V-R2 goal active | `screens/04-r2-goal-active.png` | `04-r2-goal-active-hist.txt` |
| V-R2 paused | `screens/05-r2-paused.png` | `05-r2-paused-hist.txt` |
| V-R2 resumed | `screens/06-r2-resumed.png` | `06-r2-resumed-hist.txt` |
| V-R2 status | `screens/07-r2-status.png` | `07-r2-status-hist.txt` |
| V-R3 reopen | `screens/08-r3-reopen.png` | `08-r3-reopen-hist.txt` |
| V-R3 goal status | `screens/09-r3-goal-status.png` | `09-r3-goal-status-hist.txt` |

Harness: `apps/aster/tests/visual_tui_harness/drive_control_loop.py`  
Signals: `agent-desktop` focus/list-windows, `screencapture -R` bounds, Terminal history, workspace files.

## Explicitly not claimed

- Mock LLM / hermetic CI green for this visual path
- HarnessMM `decision_reuse`
- Repo-outer design-control-loop
- Non-macOS / non-Terminal visual paths
- AX-only oracles without Terminal history
- Re-run of mounted `test_harness_loop_live_e2e` / durable-goal in this folder
