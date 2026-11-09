# Aster brand cleanup — scoped end-to-end verification

## Goal and result

Re-planned e2e against the brand-cleanup surface (Grok / xAI / SpaceX / SuperGrok
product chrome → Aster). All gates in the scoped plan passed after pairing
`aster` + `aster-acp` build identities.

Plan: `PLAN.md`. Raw log: `RESULTS.txt`.

## Suite (scoped to brand change)

| Gate | Package / entry | Result | Count |
| --- | --- | --- | --- |
| Terminal title renders Aster | `aster-pager` `notifications::title::` | PASS | 35/35 |
| Upsell URL `referrer=aster-build` | `aster-pager` `upsell_non_max_upgrade_url` | PASS | 1/1 |
| Plugin CTA marketplace | `aster-pager` `cta_e2e` | PASS | 59/59 |
| Home path helpers | `aster-pager` `--test grok_home_paths` | PASS | 2/2 |
| Winget quit hint Aster | `aster-update` `winget` | PASS | 5/5 |
| Rate-limit / free-usage copy | `aster-shell` `format_rate_limited` | PASS | 5/5 |
| OAuth referrer `aster-build` | `aster-login` `oidc::protocol::` | PASS | 18/18 |
| Prefer `.aster` project config | `aster-workspace` `find_project_configs` | PASS | 1/1 |
| ACP model identity “You are Aster” | `aster-acp` `shipped_agent_sends_aster_identity_to_the_model` | PASS | 1/1 |
| Shipped CLI oracles (product Aster, completions) | `aster` `cli_p0` `shipped_cli_p0_oracles` | PASS | 1/1 |
| TUI one-agent + refuse `x.ai` release base | `aster` `tui_one_agent` | PASS | 1/1 |
| Installer refuse `https://x.ai/cli` | `scripts/install.sh` + `ASTER_RELEASE_BASE` | PASS | refuse exit 1 |

## Commands

```text
cargo test -p aster-pager --lib --locked notifications::title:: -- --nocapture
cargo test -p aster-pager --lib --locked upsell_non_max_upgrade_url -- --nocapture
cargo test -p aster-pager --lib --locked cta_e2e -- --nocapture
cargo test -p aster-pager --test grok_home_paths --locked -- --nocapture
cargo test -p aster-update --lib --locked winget -- --nocapture
cargo test -p aster-shell --lib --locked format_rate_limited -- --nocapture
cargo test -p aster-login --lib --locked oidc::protocol:: -- --nocapture
cargo test -p aster-workspace --lib --locked find_project_configs -- --nocapture

cargo build --locked -p aster -p aster-acp
# pair ids must match: aster --build-id == aster-acp --build-id
ASTER_ACP_AGENT_BIN=$PWD/target/debug/aster-acp \
  cargo test -p aster --test cli_p0 --locked shipped_cli_p0_oracles -- --nocapture
cargo build --locked -p aster -p aster-acp
cargo test -p aster --test tui_one_agent --locked \
  shipped_tui_one_agent_connects_acp_and_keeps_serve_in_process -- --nocapture
cargo test -p aster-acp --test shipped_tui_e2e --locked \
  shipped_agent_sends_aster_identity_to_the_model -- --nocapture

ASTER_RELEASE_BASE=https://x.ai/cli bash scripts/install.sh --prefix /tmp/aster-refuse-prefix
# expect: exit 1, stderr contains "does not download that binary"
```

## Brand oracles observed

- `cli_p0` version JSON: `"product":"Aster"`
- `cli_p0` completions register `aster`
- `tui_one_agent` CAPTURE: remote `x.ai` / invalid release overrides refused
- `acp_identity`: system prompt contains `You are Aster, an expert AI coding agent`
- Installer: `error: this installer does not download that binary` for `ASTER_RELEASE_BASE=https://x.ai/cli`

## First-pass failures (fixed before claim)

- `cli_p0` / `tui_one_agent` initially failed with mismatched `ASTER_PAIR_BUILD_ID`
  when `aster` was rebuilt without a matching `aster-acp`. Rebuilt both together
  and re-ran; both PASS.
- Wrong install script path/args for refuse check; corrected to root
  `scripts/install.sh` with `ASTER_RELEASE_BASE=https://x.ai/cli`.

## Explicitly not claimed

- Headed visual Terminal suite (DeepSeek / agent-desktop screenshots)
- Full `shipped_tui_e2e` feature-row matrix / 95% function-coverage pack
- Studio GUI e2e for Aster CLI provider rename / live `pnpm generate`
- Full `apps/aster/tests/install_release.sh` (pins package version `0.1.2`)
- Renaming internal wire aliases (`OpenSupergrokUrl`, JWT `supergrok_*`, `GROK_*` env)
