# Cleanup agent-desktop e2e (post @aster-ai-lab rename)

Date: 2026-11-03. Host: macOS arm64. Tool: `agent-desktop` 1.0.2 / protocol 2.5.

## Result

**Not green.** Renderer + native Studio for the cleanup tree were rebuilt and an isolated instance was exercised, but both NAV-01 and WIKI-01 failed before a passing pair was recorded. Later launches lost AX exposure (`accessible=false` / `window_without_accessibility_element`), which blocked further retries.

## Build

| Step | Result |
| --- | --- |
| `ASTER_TENANT_SLUG=aster STUDIO_RENDERER_DIST=cleanup-e2e pnpm exec rsbuild build --mode production` | exit 0; `.studio/dist/cleanup-e2e/` |
| Studio / companions under `target/studio/aster` (`Studio`, `aster-acp`, staged launcher) | present; Studio mtime 19:34+ |
| Isolated instance | `ASTER_STUDIO_INSTANCE=cleanup-e2e-1103`, preview `127.0.0.1:4368`, dist `cleanup-e2e` |

## Script adjustments (test harness, not product)

`apps/studio/tests/e2e/agent-desktop/nav01_work.py`:

- Leave/return target is **设置** (not **插件**). ActivityBar hides `plugins` via `TEMPORARILY_HIDDEN_ACTIVITY_BAR_APP_IDS`.
- Away-page marker is button `搜索设置` (Settings search is an AX button, not a textfield).
- Composer key delivery uses `--headed` to reduce `STALE_REF` on `press`.

## Observed agent-desktop runs

Evidence dir: `docs/evidence/desktop/2026-11-03-cleanup-agent-desktop-e2e/`.

| Case | Best observed | Failure |
| --- | --- | --- |
| NAV-01 | Clicked 工作; focused composer; typed/cleared draft path partially | Earlier: missing **插件**; then settings wait mismatch; then `STALE_REF` on digit `press` when focus could not be confirmed |
| WIKI-01 | Bound isolated window | `no Wiki affordance in AX on the Work page` (no project/task row with `查看…百科`) |

When AX worked, `list-windows` reported `accessible: true` and snapshots were `complete` with the 工作 rail. After subsequent relaunches (and an `agent-desktop` restart), the same instance window stayed `accessible: false` for >30s while Studio logged `accessibility publish initialized=1 enabled=1`. Snapshots then returned `ACTION_NOT_SUPPORTED` / `window_without_accessibility_element`.

## Explicitly not claimed

- NAV-01 or WIKI-01 exit 0 on this cleanup build.
- That `@aster-ai-lab/*` rename caused the Wiki/nav failures (nav target change is the hidden plugins rail; Wiki needs a task/project row fixture).
- Stable AX for every isolated `Aster.app` under `instances/cleanup-e2e-1103` after agent-desktop restart.
- User primary Aster window health (it was stopped earlier by other e2e launchers / cleanup of competing instances).
