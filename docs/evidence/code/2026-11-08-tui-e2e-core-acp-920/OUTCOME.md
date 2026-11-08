# Aster TUI e2e — Core/ACP 9.2.0 + CLI 0.3.0 (DeepSeek)

## Result

Visual end-to-end TUI suite passed against a fresh checkout build of `aster` 0.3.0 paired with `aster-acp` 9.2.0, driven through Terminal with `agent-desktop` window ops, using `deepseek/deepseek-flash` from `./.aster/config.acl`.

## Stack under test

| Component | Version | Evidence |
| --- | --- | --- |
| `aster` (TUI) | `0.3.0 (668c7bda60ec)` | `version.txt`, welcome screenshot |
| `aster-acp` | `9.2.0` | `version.txt`, ACP log `session ready … model=deepseek/deepseek-flash` |
| Model | `deepseek/deepseek-flash` (`DeepSeek Flash`) | footer on screenshots; ACP session lines |
| Config source | `./.aster/config.acl` DeepSeek provider | user/home config synced for the run |

Launch (fresh binaries staged under `/tmp/aster-tui-e2e-030/bin`):

```bash
export PATH=/tmp/aster-tui-e2e-030/bin:$PATH
cd /tmp/aster-tui-e2e-030/ws
aster --model deepseek/deepseek-flash --fullscreen
```

Window discovery / focus / AX snapshot: `agent-desktop list-windows|focus-window|snapshot`.
Region screenshots: `screencapture -R` from `agent-desktop` bounds (AD `screenshot`/`press --headed` hit macOS `ps` inventory denials in this environment).
Key delivery: `pbcopy` + System Events paste/return into the focused Terminal tab.

## Suite

| Step | Action | Result | Artifact |
| --- | --- | --- | --- |
| 01 | Welcome / versions | Aster 0.3.0 + DeepSeek Flash footer | `screens/01-welcome.png` |
| 02 | `/help` | Slash palette / help UI | `screens/02-help.png` |
| 03 | `/model` | Model picker shows DeepSeek | `screens/03-model.png` |
| 04 | `/config` | Config UI opens | `screens/04-config.png` |
| 05 | `/settings` | Settings UI opens | `screens/05-settings.png` |
| 06 | `/effort` | Effort UI opens | `screens/06-effort.png` |
| 07 | Chat token reply | Assistant returned `E2E_OK_DEEPSEEK_920` (retest after overlay leak) | `screens/07b-chat-done.png`, `hist/07b-chat-hist.txt` |
| 08 | Tool read `readme.txt` | Quoted fixture contents via DeepSeek tool path | `screens/08-tool-read.png` |
| 09 | `/goal …` | Goal set | `screens/09-goal-set.png` |
| 10 | `/goal status` | Goal status | `screens/10-goal-status.png` |
| 11 | `/compact` | Compact ran | `screens/11-compact.png` |
| 12 | `/new` | New session | `screens/12-new.png` |

Machine-readable rollup: `SUMMARY.json` (`all_ok: true` after chat retest).

## Commands / counts

- Binaries: `/tmp/aster-tui-e2e-030/bin/{aster,aster-acp}` copied from this checkout’s fresh build.
- Driver: `/tmp/aster-tui-e2e-030/drive_e2e.py` → `drive.log`.
- Chat retest token appearances: 2 (user prompt + assistant line).
- Tool-read hist contains `Aster TUI e2e fixture readme` and `line-2 hello deepseek`.
- ACP log: `acp session ready … model=deepseek/deepseek-flash` (session `e0c8740b-…`, later `/new` `75eca1ba-…`).

## Explicitly not claimed

- `agent-desktop screenshot` / `press --headed` / `clipboard-set` success in this sandbox (helper/`ps` inventory denials); visual proof used window-bounds `screencapture` and Terminal history instead.
- Exhaustive coverage of every slash command beyond the suite table (e.g. `/yolo`, `/feedback`, worktree/resume).
- Non-macOS hosts; Linux/Windows TUI visual e2e.
- Publishing or tagging; this is a verification evidence pack only.
- That project-local `.aster/config.acl` with a non-DeepSeek `default_model` will honor CLI `--model` without cleaning that override (observed: workspace `default_model=glm-01/glm-5.3` overrode the footer until removed).
