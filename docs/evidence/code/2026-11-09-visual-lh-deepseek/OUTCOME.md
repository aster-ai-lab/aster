# Visual LH e2e — agent-desktop + DeepSeek (config.acl)

## Result

- Launched Aster TUI via `open -a Terminal` + `agent-desktop` focus/capture from
  this checkout’s staged `aster` 0.3.1 / `aster-acp` 9.2.1 pair.
- Model pin: `ASTER_CONFIG=./.aster/config.acl` (host default `glm-01/glm-5.3`) with
  CLI `--model deepseek/deepseek-flash`. Footer history shows
  `deepseek/DeepSeek Flash`.

| Gate | Result | Oracle |
| --- | --- | --- |
| Multiturn shop (C4) | **pass** | `DUAL-HOLD` then `DUAL-GATE-7K`×2 |
| LH-H1 soft parallel intent | **soft pass** | GATE only (no mid HOLD) |
| LH-H1 hard ordered repair | **pass** | HOLD before GATE in `runs.txt` |
| LH-H3 retrieval needle | **pass** | `RETRIEVE-GATE-9c2e`; decoy untouched; hist found `rate_impl.py` |

## Commands

```text
cargo build --locked -p aster -p aster-acp
cp target/debug/aster target/debug/aster-acp /tmp/aster-visual-lh/bin/

python3 apps/aster/tests/visual_tui_harness/drive_shop_multiturn.py \
  --bin-dir /tmp/aster-visual-lh/bin \
  --work-dir /tmp/aster-visual-lh/multiturn \
  --model deepseek/deepseek-flash \
  --config $PWD/.aster/config.acl \
  --evidence docs/evidence/code/2026-11-09-visual-lh-deepseek/multiturn
→ fail_before_pass=true pass_count=2

python3 apps/aster/tests/visual_tui_harness/drive_lh_parallel.py ...
→ soft pass (GATE without HOLD)

python3 apps/aster/tests/visual_tui_harness/drive_lh_parallel_hard.py \
  --bin-dir /tmp/aster-visual-lh/bin \
  --work-dir /tmp/aster-visual-lh/lh-h1-hard \
  --model deepseek/deepseek-flash \
  --config $PWD/.aster/config.acl \
  --evidence docs/evidence/code/2026-11-09-visual-lh-deepseek/lh-h1-hard
→ fail_before_pass=true pass_count=1

python3 apps/aster/tests/visual_tui_harness/drive_lh_retrieval.py \
  --bin-dir /tmp/aster-visual-lh/bin \
  --work-dir /tmp/aster-visual-lh/lh-h3 \
  --model deepseek/deepseek-flash \
  --config $PWD/.aster/config.acl \
  --evidence docs/evidence/code/2026-11-09-visual-lh-deepseek/lh-h3
→ RETRIEVE-GATE-9c2e; decoy_unchanged; search hist cites inventory/backstock/rate_impl.py
```

Harness notes: ensure Terminal.app is running before `list-windows --app 终端`;
bind new windows via pre-launch id snapshot when titles omit cwd.

## Evidence

| Scenario | Summary | Screens |
| --- | --- | --- |
| Multiturn | `multiturn/visual-multiturn-summary.json` | `multiturn/screens/03–06-*.png` |
| LH-H1 soft | `lh-h1/visual-lh-h1-summary.json` | `lh-h1/screens/01–02-*.png` |
| LH-H1 hard | `lh-h1-hard/visual-lh-h1-hard-summary.json` | `lh-h1-hard/screens/` |
| LH-H3 retrieval | `lh-h3/visual-lh-h3-summary.json` | `lh-h3/screens/01–02-*.png` |

## Explicitly not claimed

- Hard proof of product parallel/subagent *tool* invocation (ordered edits ≠ parallel overlap).
- Code-intelligence visual gate (LH-H4).
- Hermetic scripted parallel-overlap or ACP `subagent_spawned` wire oracles.
- Non-macOS / non-Terminal visual paths.
