# Aster TUI e2e — hermetic form restore and non-regression gates

## Verdict

**pass** — hermetic `shipped_tui_e2e` stays green with a restored native-form PTY case; function-coverage gate remains 95/95; one Layer C gold turn (assistant text) passes against the operator ACL. Live CLI-model-override gold timed out waiting for a DeepSeek footer and is not claimed.

## Changes

1. Added `shipped_native_form_submit_cancel_and_reject` (mock `FORM-ORACLE` / `FORM-ORACLE-CANCEL`): required field, `max 5` range, submit (`Ada`/`4`/`true`), Ctrl+C cancel.
2. Mock keeps cancel on a separate oracle prefix so a prior submit’s values cannot short-circuit a later form turn.
3. Inventory: form rows point at the real hermetic fn; schema refusal stays Core/live (no hermetic card).

## Commands

```bash
cargo test -p aster-pager --lib --locked aster_tui_function_coverage_is_at_least_95_percent -- --nocapture
# ok · 1 passed

cargo test -p aster-acp --test shipped_tui_e2e --locked -- --nocapture --test-threads=1
# ok · 20 passed; 0 failed; 1 ignored (`shipped_bound_goal_real_llm`) · ~167s

cargo test -p aster-acp --test shipped_tui_e2e --locked shipped_native_form_submit_cancel_and_reject -- --nocapture --test-threads=1
# ok · 1 passed · ~9.6s · form_e2e.log

ASTER_CONFIG_FILE=$PWD/.aster/config.acl \
  cargo test -p aster --test tui_913_real_llm --locked real_llm_tui_turn_shows_assistant_text \
  -- --ignored --nocapture --test-threads=1
# ok · 1 passed · ~13s
```

## Counts

| Gate | Result |
| --- | --- |
| Function coverage | **95/95 pass** |
| Hermetic shipped TUI e2e | **20 pass / 1 ignore** (was 19/1) |
| New form oracles | `empty-field`, `range`, `FORM-SUBMITTED`, `FORM-CANCELLED` |
| Layer C single turn | **pass** |
| Layer C `…cli_model_outranks…` | **fail** — timed out waiting for `deepseek/deepseek-flash` (~57s); ACL `default_model` is `glm-01/glm-5.3` |

## Explicitly not claimed

- `shipped_bound_goal_real_llm` (still ignored)
- Dual-defect / long-task Layer C refresh
- Hermetic PTY schema-refusal / `FORM-REJECTED`
- Full `aster_tui_live_e2e` kimi waves
- Live A1 model-override gold (failed this run)
- Tokenizer-level compact savings (`token-count=unmeasured` unchanged)
