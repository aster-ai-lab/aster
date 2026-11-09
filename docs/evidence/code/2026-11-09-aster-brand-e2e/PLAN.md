# Brand-change e2e plan

## Scope (from brand cleanup diff)

Surfaces that must stay Aster-branded end-to-end:

| Surface | Why | Gate |
| --- | --- | --- |
| Terminal title | `TitleItem::Aster` → `"aster"` | `aster-pager` title lib tests |
| Billing upsell URL | `referrer=aster-build` | billing lib + assertion |
| OAuth referrer | device/OIDC default `aster-build` | `aster-login` oidc protocol |
| Rate-limit / free-usage copy | no `docs.x.ai` / SuperGrok | `aster-shell` sampling error |
| Project config discovery | prefer `.aster`, fallback `.grok` | `aster-workspace` find_project_configs |
| Winget quit hint | "Aster" / `aster leader kill` | `aster-update` winget_tests |
| Plugin CTA marketplace | official Aster marketplace URLs | `cta_e2e` |
| Home path helpers | `GROK_HOME` / Aster home | pager `grok_home_paths` |
| Shipped CLI oracles | `version` product Aster, completions `aster`, no public x.ai host | `aster` `cli_p0` |
| Release base refuse | `https://x.ai/cli` / releases refused | `tui_one_agent` + install refuse |
| ACP model identity | system prompt "You are Aster…" | `shipped_tui_e2e` identity |

## Explicitly out of this e2e pass

- Headed visual Terminal suite (DeepSeek) — brand already covered by title + CLI oracles
- Studio GUI / provider-registry live `pnpm generate` catalog e2e
- Full `shipped_tui_e2e` feature-row matrix (function-coverage pack)
- Renaming internal `OpenSupergrokUrl` / JWT `supergrok_*` wire aliases
