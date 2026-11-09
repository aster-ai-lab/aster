# Aster Core control-loop DeepSeek E2E

Date: 2026-11-09  
Scope: session/Run cybernetic analogues in `aster-core` (goal tick + harness completion), real DeepSeek from `.aster/config.acl`.

## Commands

```bash
ASTER_CONFIG_FILE=/Users/roylin/code/aster/.aster/config.acl \
  cargo test -p aster-core --test test_control_loop_deepseek_e2e --no-run --locked

ASTER_CONFIG_FILE=/Users/roylin/code/aster/.aster/config.acl \
  cargo test -p aster-core --test test_control_loop_deepseek_e2e \
  -- --ignored --test-threads=1 --nocapture
```

Pinned model (this host): `deepseek/deepseek-flash`  
(resolved by `resolve_deepseek_route`; ACL `default_model` was `glm-01/glm-5.3`)

## Inventory

| ID | Test / mount | Layer |
|---|---|---|
| R1d | `r1_deepseek_verified_write_completes` | this suite |
| R1a/b/c/e/f | `test_harness_loop_live_e2e` (`deepseek_flash_*`) | mount |
| R2a/b/e | `r2_deepseek_advance_reconcile_and_budget` | this suite |
| R2c/d/f | `r2_deepseek_wait_evidence_pause_resume` | this suite |
| R3 | `r3_deepseek_write_goal_across_reopen` | this suite |
| R4a | `test_meta_harness_compose_live_e2e::live_composed_harness_write_completes_when_host_attests` | mount |
| R5a | `test_harness_loop_live_e2e` isolated write / plan-mode denies | mount |
| R3-long | `test_durable_goal_real_llm` | optional long gold |

## Results

Final run (`run-final.log`), wall ≈ 24.8s:

| Test | Result | Notes |
|---|---|---|
| `r1_deepseek_verified_write_completes` | pass | write + host attest → `Verified` |
| `r2_deepseek_advance_reconcile_and_budget` | pass | Advance → Replayed/Reconcile → BudgetExhausted; exhausted tick adds 0 LLM calls |
| `r2_deepseek_wait_evidence_pause_resume` | pass | NeedsInput while waiting; evidence unblocks; Pause→Resume clears Paused |
| `r3_deepseek_write_goal_across_reopen` | pass | write token survives FileSessionStore reopen; reused op fail-closed; preview `Complete` |

Counts: **4 passed; 0 failed** (suite body). Mounted R1/R4/R5 suites not re-executed in this OUTCOME.

## Explicitly not claimed

- Mock LLM / hermetic control-loop CI green
- HarnessMM `decision_reuse` / CCS / RSI
- Repo-outer design-control-loop (lint sensors, scheduled PRs)
- Full-task token/billing parity with Spec 035
- Assistant wording stability
- Default (non-`--ignored`) CI coverage for this suite
- Re-run of mounted `test_harness_loop_live_e2e` / meta-harness / durable-goal in this evidence folder
