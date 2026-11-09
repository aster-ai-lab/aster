# Replace remaining Grok / xAI / SpaceX product brand with Aster

## Goal and result

User-facing Aster product chrome that still said Grok, SuperGrok, xAI, or
SpaceX now presents as Aster. Wire aliases, third-party provider labels for
BYOK-to-xAI, API model IDs, and telemetry service names stay intact.

## Changed (high signal)

- OAuth / billing referrer default `aster-build` (login device + OIDC + upsell URL)
- Rate-limit / upsell copy and Studio i18n already Aster-neutral; scrubbed
  SuperGrok / docs.x.ai product links on the Aster CLI subscription path
- Studio `grok-cli` provider display name → **Aster CLI**; website docs/official
  → Aster GitHub (wire `baseUrl` `cli-chat-proxy.grok.com` unchanged)
- User guide + aster-shell README: CLI binary examples `aster`, home `.aster`
- Terminal title / help / doctor prefixes from earlier pass retained

## Compatibility kept

- Env `GROK_*` still read (docs still name `GROK_LOG_FILE`, `GROK_CLI_CHAT_PROXY_BASE_URL`, …)
- Serde / permission aliases `grok_tui`, title `"grok"`, theme `groknight`/`grokday`
- Third-party Studio provider `grok` (api.x.ai BYOK) keeps name **Grok** and docs.x.ai
- API model IDs (`grok-4.*`, `grok-build`, `spacexai/*`) and JWT tier aliases (`supergrok_heavy`)
- Telemetry wire: `service.name=grok-cli`, meter `ai.aster.grok_code`, MDM `ai.x.grok`
- Internal symbols (`OpenSupergrokUrl`, `grok_home`, …) not renamed

## Verification

```text
cargo test -p aster-login --lib --locked oidc::protocol:: -- --nocapture
# PASS (18/18)

cargo test -p aster-pager --lib --locked dispatch::tests::billing::upsell_non_max_upgrade_url -- --nocapture
# PASS (1/1)

cargo test -p aster-pager --lib --locked notifications::title:: -- --nocapture
# PASS (35/35)
```

## Explicitly not claimed

- Full workspace / shipped_tui_e2e green after the string pass
- Renaming internal modules or Action enums (`OpenSupergrokUrl`)
- Regenerating provider-registry via live `pnpm generate` (source + matching
  `providers.json` website/name fields edited together)
- Removing the third-party xAI Grok BYOK provider or mini-app bookmarks to grok.com
- Migrating every on-disk `.grok` directory automatically

## Follow-on e2e

Scoped brand e2e (title, billing referrer, OIDC, CLI oracles, refuse x.ai,
ACP identity) is recorded in
`docs/evidence/code/2026-11-09-aster-brand-e2e/OUTCOME.md` (all gates PASS).
