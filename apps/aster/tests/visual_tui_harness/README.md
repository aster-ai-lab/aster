# Visual TUI harness (agent-desktop companion)

Authoritative Layer C oracles remain the ignored PTY tests in
`tui_913_real_llm.rs`. This harness is a **visual companion** for macOS
Terminal + `agent-desktop` window ops.

## Trusted signals

| Signal | Use |
| --- | --- |
| `agent-desktop list-windows` / `focus-window` | Find and focus the Aster Terminal window |
| `screencapture -R` from window bounds | Window-scoped PNG evidence |
| Terminal `history of selected tab` | Readable TTY dump (AX alone is insufficient) |
| Workspace files (`rate.py`, `fee.py`, `runs.txt`) | Dual-defect coding oracles |

Do **not** treat AX textfields, `agent-desktop screenshot`, `press --headed`, or
`clipboard-set` as required (they fail closed on some hosts via `ps` / helper
path policy). Key delivery: `pbcopy` + System Events paste/return.

## Model pin

```bash
export ASTER_CONFIG=/abs/path/to/repo/.aster/config.acl   # DeepSeek provider + may default to glm
aster --model deepseek/deepseek-flash --no-plan --fullscreen
```

`drive_shop_multiturn.py` exports `ASTER_CONFIG` from `--config` or the repo
`.aster/config.acl` when present. CLI `--model` is forwarded to `aster-acp`
as `--model` (sets `ASTER_DEFAULT_MODEL`), which outranks host/workspace
`default_model` (A1). The repo ACL currently defaults to `glm-01/glm-5.3`
while still declaring `deepseek/deepseek-flash` — pin DeepSeek explicitly.

## Ask-user / permission cards

`drive_shop_multiturn.py` answers:

- Directory trust: type `y`
- Permission card (`Ctrl+o:yolo` footer): type `1`
- `ask_user` multi-choice (`1/3:select`): prefer the option that **runs**
  `gate.py` / accepts `DUAL-HOLD` (Tab to that row, then Return; fallback
  keystroke `2`)

## Run

Stage fresh `aster` + `aster-acp` beside each other, then:

```bash
python3 apps/aster/tests/visual_tui_harness/drive_shop_multiturn.py \
  --bin-dir /path/to/bins \
  --work-dir /tmp/aster-visual-shop \
  --model deepseek/deepseek-flash \
  --config /abs/path/to/repo/.aster/config.acl \
  --evidence /tmp/aster-visual-shop/evidence
```

The driver writes `launch_aster.sh` into the evidence dir and opens it with
`open -a Terminal` so `ASTER_CONFIG` / cwd quoting stays intact.
