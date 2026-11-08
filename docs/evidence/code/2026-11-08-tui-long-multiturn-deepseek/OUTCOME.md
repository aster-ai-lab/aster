# Aster TUI long-horizon + multi-turn real LLM (DeepSeek)

## Result

Layer C dual-defect coding gates passed on a fresh `aster` 0.3.0 + `aster-acp` 9.2.0 pair with `deepseek/deepseek-flash`. Visual Terminal/agent-desktop runs of the same shop fixture also reached `DUAL-GATE-7K` after failing checker lines.

## Stack

| Piece | Value |
| --- | --- |
| TUI | `/tmp/aster-tui-long-e2e/bin/aster` → `0.3.0 (668c7bda60ec)` |
| ACP | `/tmp/aster-tui-long-e2e/bin/aster-acp` → `9.2.0` |
| Model | `deepseek/deepseek-flash` from `./.aster/config.acl` (pinned via `ASTER_PINNED_MODEL` + `default_model`) |
| Fixture | `rate.py` (`* 3`), `fee.py` (`+ 11`), immutable `gate.py` → `DUAL-HOLD` / `DUAL-GATE-7K` |

## Formal gates (authoritative)

Invoked through `acl_env_config run` so coding tests refuse to copy credentialed ACL bytes:

```bash
ASTER_TUI_BIN=…/aster ASTER_ACP_BIN=…/aster-acp ASTER_PINNED_MODEL=deepseek/deepseek-flash \
  target/debug/examples/acl_env_config run /tmp/aster-tui-long-e2e/config.acl \
  cargo test -p aster --test tui_913_real_llm --locked \
    real_llm_tui_turns_fix_the_diagnosed_bug \
    real_llm_tui_long_task_repairs_and_quotes_the_check \
    -- --ignored --test-threads=1 --nocapture
```

| Test | Oracle | Time | Log |
| --- | --- | --- | --- |
| `real_llm_tui_turns_fix_the_diagnosed_bug` | 4 prompts / 4 Runs; terminals narrative → unverified → verified → verified; `runs=DUAL-HOLD \| DUAL-HOLD \| DUAL-GATE-7K \| DUAL-GATE-7K` | 69.71s | `logs/multiturn-cargo.log` |
| `real_llm_tui_long_task_repairs_and_quotes_the_check` | Single long prompt; partial fail while one defect remains; ≥2 `DUAL-HOLD` before `DUAL-GATE-7K`; assistant quotes pass | 35.65s | `logs/longtask-cargo.log` |

Both: `test result: ok. 1 passed`.

## Visual companion (agent-desktop window ops)

Same fixture under `/tmp/aster-tui-long-e2e/ws`, Terminal launch with `--model deepseek/deepseek-flash --no-plan --fullscreen`.

| Scenario | Result | Artifacts |
| --- | --- | --- |
| Long task (`LONG_PROMPT`) | `runs.txt`: 4×`DUAL-HOLD` then `DUAL-GATE-7K` | `screens/02-long-task.png`, `visual-long-runs.txt` |
| Multi-turn (diag → partial → repair → rerun) | Final `runs`: `HOLD,HOLD,GATE,GATE`; files `* 8` / `* 5` | `screens/03–06-*.png`, `visual-multiturn-summary.json` |

Visual multi-turn notes (does not weaken formal pass): turn 2 opened `ask_user` instead of appending `DUAL-HOLD`; a follow-up forced `python3 gate.py` with no edits; turn 4 needed an extra Enter because the prompt stayed in the composer.

## Explicitly not claimed

- ACP `long_horizon_real_llm` Rust guest / OCI repair campaign (separate LH01 assets).
- Every other ignored `tui_913_real_llm` row beyond the two Layer C coding gates above.
- That visual ask_user handling matches the formal PTY `answer_dialog` automation without operator follow-ups.
- Non-macOS hosts.
