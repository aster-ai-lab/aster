# Visual loop + retrieval — agent-desktop + DeepSeek

## Result

- Pair: staged `aster` 0.3.1 / `aster-acp` 9.2.1 from this checkout.
- Model: `ASTER_CONFIG` from repo `.aster/config.acl` with CLI
  `--model deepseek/deepseek-flash` (footer `deepseek/DeepSeek Flash`).
- **AL-V1** (autonomous loop + `/compact`): **pass** — HOLD before GATE;
  compact mid-run; resume repair; `pass_count=2`.
- **SR semantic** (`workspace_retrieval { enabled; local_cpu }`): **pass** —
  `SEMANTIC-GATE-9c2e`; decoy untouched; history shows search/invoice evidence.
- **DeLoop** changing-feed visual: **not claimed** (Terminal companion cannot
  reliably advance an external feed mid-tool-turn; keep `deloop_real_llm` ACP).

## Commands

```text
python3 apps/aster/tests/visual_tui_harness/drive_al_compact.py \
  --bin-dir /tmp/aster-visual-lh/bin \
  --work-dir /tmp/aster-visual-lh/al-v1 \
  --model deepseek/deepseek-flash \
  --config $PWD/.aster/config.acl \
  --evidence docs/evidence/code/2026-11-09-visual-loop-retrieval-deepseek/al-v1
→ pass=true fail_before_pass=true compact_hist_mentions_compact=true

python3 apps/aster/tests/visual_tui_harness/drive_sr_semantic.py \
  --bin-dir /tmp/aster-visual-lh/bin \
  --work-dir /tmp/aster-visual-lh/sr-sem \
  --model deepseek/deepseek-flash \
  --config $PWD/.aster/config.acl \
  --evidence docs/evidence/code/2026-11-09-visual-loop-retrieval-deepseek/sr-semantic
→ pass=true SEMANTIC-GATE-9c2e
```

## Evidence

| Gate | Summary | Screens |
| --- | --- | --- |
| AL-V1 compact | `al-v1/visual-al-v1-summary.json` | `al-v1/screens/01–06-*.png` |
| SR semantic | `sr-semantic/visual-sr-semantic-summary.json` | `sr-semantic/screens/` |

## Explicitly not claimed

- DeLoop 7-version changing-feed survival under compaction (ACP live only).
- Hard proof that search used `mode=semantic` vs grep/hybrid (history tokens only).
- Hermetic DL-H0 / SR-H3 CI gates (separate from this visual companion).
