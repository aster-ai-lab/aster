# TUI e2e invariants (first principles)

## Result

Delivered the decisive invariant gates from the first-principles e2e plan, plus an
agent-desktop visual DeepSeek shop run against repo `./.aster/config.acl`.

## Authority (A1)

- Hermetic: `cli_model_override_outranks_workspace_default_model` — ACL
  `default_model=mock/gpt`, launch `--model mock/alt`; mock request model is
  `alt` and footer shows `mock/alt`.
- Visual: host ACL default is `glm-01/glm-5.3`; launch
  `aster --model deepseek/deepseek-flash` with
  `ASTER_CONFIG=/Users/roylin/code/aster/.aster/config.acl`. Terminal history /
  footer showed `deepseek/DeepSeek Flash` (see `visual/01-deepseek-pin-hist.txt`).

## Mode exclusivity (B1)

- Hermetic: `effort_slash_esc_clears_draft_before_prose` — `/effort` then Esc then
  `TURN-MARKER` reaches the mock without `Unknown TUI command` / `/effort…`
  concatenation. Esc policy clears bare `/command` drafts on first Esc.

## Side-effect contract (C3/C4)

- Formal `answer_dialog` answers `ask_user` / `1/3:select` like the visual
  harness (`ask_user_reply_keys_match_visual_harness_policy`).
- Visual multiturn (agent-desktop focus + Terminal history + file oracles):

```text
runs: DUAL-HOLD, DUAL-GATE-7K, DUAL-GATE-7K
fail_before_pass: true
pass_count: 2
model: deepseek/deepseek-flash
```

Evidence: `visual/visual-multiturn-summary.json`, `visual/screens/`.

## Commands (actual)

```text
cargo test -p aster-acp --locked --test tui_913_hermetic \
  cli_model_override_outranks_workspace_default_model -- --test-threads=1
→ ok

cargo test -p aster-acp --locked --test tui_913_hermetic \
  effort_slash_esc_clears_draft_before_prose -- --test-threads=1
→ ok

cargo test -p aster --test tui_913_real_llm ask_user_reply_keys_match_visual_harness_policy
→ (unit in test binary)

# Visual (agent-desktop + Terminal)
# ASTER_CONFIG=./.aster/config.acl default_model=glm-01/glm-5.3
# aster --model deepseek/deepseek-flash --no-plan --fullscreen
→ visual-multiturn-summary.json pass
```

CI: `.github/workflows/ci.yml` runs hermetic A1 with `--test-threads=1`.

## Explicitly not claimed

- Full Layer C ignored matrix re-run in this OUTCOME (cancel/ksandbox/capabilities).
- Non-macOS visual / agent-desktop.
- AX or agent-desktop `screenshot`/`press --headed` as oracles.
- Product policy that suppresses model `ask_user` for checker runs.
- Exhaustive slash catalog e2e.
