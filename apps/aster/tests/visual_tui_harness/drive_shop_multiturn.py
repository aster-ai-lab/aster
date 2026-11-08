#!/usr/bin/env python3
"""Visual companion driver for the dual-defect shop fixture.

Uses agent-desktop for window focus, region screencapture for PNGs, Terminal
history + workspace files as oracles. Answers ask_user / permission cards
similar to tui_913_real_llm::answer_dialog.
"""
from __future__ import annotations

import argparse
import json
import subprocess
import time
from pathlib import Path

PASS_LINE = "DUAL-GATE-7K"
FAIL_LINE = "DUAL-HOLD"

RATE_BUG = """def sticker_price(count):
    return count * 3
"""
FEE_BUG = """def parcel_fee(count):
    return count + 11
"""
GATE_PY = """\
import sys
sys.dont_write_bytecode = True
import importlib
import fee
import rate

importlib.reload(rate)
importlib.reload(fee)
sticker_price = rate.sticker_price
parcel_fee = fee.parcel_fee
price_ok = sticker_price(4) == 32 and sticker_price(5) == 40
fee_ok = parcel_fee(4) == 20 and parcel_fee(3) == 15
status = "HOLD" if not (price_ok and fee_ok) else "GATE-7K"
line = "DUAL-" + status
print(line, flush=True)
open("runs.txt", "a", encoding="utf-8").write(line + "\\n")
raise SystemExit(0 if price_ok and fee_ok else 1)
"""

DIAG = "Read rate.py, fee.py, and gate.py. Name both wrong returns. Do not edit."
PARTIAL = (
    "Fix only rate.py. Run exactly `python3 gate.py`. "
    "If denied, request escalation. Quote output."
)
SECOND = (
    "Fix fee.py. Run exactly `python3 gate.py`. "
    "If denied, request escalation. Quote output."
)
RUN = "Run exactly `python3 gate.py`. If denied, request escalation. Quote its output."


def sh(args: list[str], check: bool = True, input_text: str | None = None, timeout: int = 120):
    return subprocess.run(
        args,
        check=check,
        text=True,
        capture_output=True,
        timeout=timeout,
        input=input_text,
    )


def osa(script: str) -> str:
    return sh(["osascript"], input_text=script).stdout


def terminal_history() -> str:
    return osa(
        """tell application "Terminal"
  set t to selected tab of front window
  return history of t as text
end tell"""
    )


def paste_submit(text: str) -> None:
    osa(
        """tell application "Terminal" to activate
delay 0.2
tell application "System Events"
  key code 53
  delay 0.08
  keystroke "a" using {command down}
  delay 0.08
  key code 51
  delay 0.08
end tell"""
    )
    sh(["pbcopy"], input_text=text)
    time.sleep(0.12)
    osa(
        """tell application "Terminal" to activate
delay 0.12
tell application "System Events"
  keystroke "v" using {command down}
  delay 0.28
  key code 36
end tell"""
    )


def keystroke_raw(script_body: str) -> None:
    osa(
        f"""tell application "Terminal" to activate
delay 0.15
tell application "System Events"
{script_body}
end tell"""
    )


def answer_dialogs(hist: str) -> bool:
    """Return True if a key was sent to dismiss/answer a blocking card."""
    if "Do you trust the contents of this directory" in hist:
        keystroke_raw('  keystroke "y"\n')
        return True
    # Permission card (formal answer_dialog types "1")
    if "Ctrl+o:yolo" in hist and ("Yes, proceed" in hist or "permission" in hist.lower()):
        keystroke_raw('  keystroke "1"\n')
        return True
    # ask_user multi-choice
    if "1/3:select" in hist or ("ask_user" in hist and "(●)" in hist):
        # Prefer the option that runs gate / accepts HOLD — usually row 2.
        # Tab cycles; then Return.
        if "Run it anyway" in hist or "accept the DUAL-HOLD" in hist:
            keystroke_raw(
                """  keystroke tab
  delay 0.2
  key code 36
"""
            )
            return True
        if "Also fix fee.py" in hist:
            # option 3 would fix fee early — avoid; take option 2 via "2"
            keystroke_raw(
                """  keystroke "2"
  delay 0.15
  key code 36
"""
            )
            return True
        keystroke_raw(
            """  keystroke tab
  delay 0.2
  key code 36
"""
        )
        return True
    return False


def settled(hist: str) -> bool:
    busy = any(
        token in hist
        for token in ("Responding", "Working", "Thinking", "1/3:select", "ask_user")
    )
    return (not busy) and ("Add a follow-up" in hist or "Type a message" in hist)


def wait_pred(pred, timeout: float, label: str, poll: float = 1.5) -> bool:
    deadline = time.time() + timeout
    while time.time() < deadline:
        hist = terminal_history()
        if answer_dialogs(hist):
            time.sleep(1.0)
            continue
        if pred(hist):
            print(f"OK {label}", flush=True)
            return True
        time.sleep(poll)
    print(f"FAIL {label}", flush=True)
    return False


def write_fixture(ws: Path) -> None:
    ws.mkdir(parents=True, exist_ok=True)
    (ws / "rate.py").write_text(RATE_BUG)
    (ws / "fee.py").write_text(FEE_BUG)
    (ws / "gate.py").write_text(GATE_PY)
    runs = ws / "runs.txt"
    if runs.exists():
        runs.unlink()


def capture(ev: Path, wid: str, name: str) -> None:
    out = json.loads(sh(["agent-desktop", "list-windows", "--app", "终端"]).stdout)
    bounds = None
    for w in out.get("data", []):
        if w.get("id") == wid:
            b = w["bounds"]
            bounds = f"{int(b['x'])},{int(b['y'])},{int(b['width'])},{int(b['height'])}"
            break
    if not bounds:
        raise RuntimeError(f"window {wid} not found")
    screens = ev / "screens"
    screens.mkdir(parents=True, exist_ok=True)
    sh(["screencapture", "-x", f"-R{bounds}", str(screens / f"{name}.png")])
    (ev / f"{name}-hist.txt").write_text(terminal_history())


def find_wid(marker: str) -> str:
    # macOS often shows /private/tmp/... while Path.resolve() may still be /tmp/...
    markers = {marker, str(Path(marker))}
    try:
        markers.add(str(Path(marker).resolve()))
    except OSError:
        pass
    if marker.startswith("/tmp/"):
        markers.add("/private" + marker)
    resolved = str(Path(marker).resolve()) if Path(marker).exists() else marker
    if resolved.startswith("/private/tmp/"):
        markers.add(resolved.replace("/private/tmp/", "/tmp/", 1))
    for _ in range(60):
        out = json.loads(sh(["agent-desktop", "list-windows", "--app", "终端"]).stdout)
        for w in out.get("data", []):
            title = w.get("title") or ""
            if not w.get("visible") or "aster-acp" not in title:
                continue
            if any(m and m in title for m in markers):
                return w["id"]
            # Title may shorten the cwd; match a stable workspace leaf.
            leaf = Path(marker).name
            parent = Path(marker).parent.name
            if leaf and parent and f"{parent}/{leaf}" in title.replace("/private", ""):
                return w["id"]
        time.sleep(0.4)
    raise RuntimeError(f"no Terminal window matching {marker} (tried {sorted(markers)})")


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--bin-dir", type=Path, required=True)
    ap.add_argument("--work-dir", type=Path, required=True)
    ap.add_argument("--evidence", type=Path, required=True)
    ap.add_argument("--model", default="deepseek/deepseek-flash")
    ap.add_argument(
        "--config",
        type=Path,
        default=None,
        help="Host ASTER_CONFIG ACL (DeepSeek credentials). Defaults to repo .aster/config.acl when present.",
    )
    args = ap.parse_args()

    bin_dir = args.bin_dir.resolve()
    ws = (args.work_dir / "ws").resolve()
    ev = args.evidence.resolve()
    ev.mkdir(parents=True, exist_ok=True)
    write_fixture(ws)

    config = args.config
    if config is None:
        # drive_shop_multiturn.py → visual_tui_harness → tests → aster → apps → repo
        repo_acl = Path(__file__).resolve().parents[4] / ".aster" / "config.acl"
        config = repo_acl if repo_acl.is_file() else None
    else:
        config = config.resolve()
        if not config.is_file():
            raise SystemExit(f"--config not a file: {config}")

    marker = str(ws.resolve())
    # Keep the launch line simple: the first successful DeepSeek visual run used
    # this quoting. Write a companion script only as evidence of intent.
    launch = ev / "launch_aster.sh"
    config_line = f"export ASTER_CONFIG='{config}'" if config is not None else "# no ASTER_CONFIG"
    launch.write_text(
        "\n".join(
            [
                "#!/bin/bash",
                "set -euo pipefail",
                f"export PATH='{bin_dir}':\"$PATH\"",
                config_line,
                f"cd '{ws}'",
                "clear",
                f"exec aster --model '{args.model}' --no-plan --fullscreen",
                "",
            ]
        )
    )
    launch.chmod(0o755)
    # Prefer `open -a Terminal` on the launch script — AppleScript `do script`
    # with nested path quotes has been observed to drop ASTER_CONFIG/cwd on macOS.
    subprocess.run(["open", "-a", "Terminal", str(launch)], check=True)
    wid = find_wid(marker)
    (ev / "wid.txt").write_text(wid)
    sh(["agent-desktop", "focus-window", "--app", "终端", "--window-id", wid], check=False)
    time.sleep(2)

    def rate() -> str:
        return (ws / "rate.py").read_text()

    def fee() -> str:
        return (ws / "fee.py").read_text()

    def runs() -> str:
        p = ws / "runs.txt"
        return p.read_text() if p.exists() else ""

    paste_submit(DIAG)
    assert wait_pred(
        lambda h: settled(h) and "* 3" in rate() and "+ 11" in fee(),
        180,
        "diag",
    )
    capture(ev, wid, "03-multiturn-diag")

    paste_submit(PARTIAL)
    assert wait_pred(
        lambda h: settled(h)
        and "* 3" not in rate()
        and "+ 11" in fee()
        and FAIL_LINE in runs()
        and PASS_LINE not in runs(),
        360,
        "partial",
    )
    capture(ev, wid, "04-multiturn-partial")

    paste_submit(SECOND)
    assert wait_pred(
        lambda h: settled(h) and "+ 11" not in fee() and PASS_LINE in runs(),
        360,
        "repaired",
    )
    capture(ev, wid, "05-multiturn-repaired")

    before = runs().count(PASS_LINE)
    paste_submit(RUN)
    assert wait_pred(
        lambda h: settled(h) and runs().count(PASS_LINE) > before,
        240,
        "rerun",
    )
    capture(ev, wid, "06-multiturn-rerun")

    summary = {
        "visual_multiturn": True,
        "model": args.model,
        "runs": runs().splitlines(),
        "rate": rate(),
        "fee": fee(),
        "fail_before_pass": runs().find(FAIL_LINE) < runs().find(PASS_LINE),
        "pass_count": runs().count(PASS_LINE),
    }
    (ev / "visual-multiturn-summary.json").write_text(json.dumps(summary, indent=2))
    (ev / "visual-multiturn-runs.txt").write_text(runs())
    print(json.dumps(summary, indent=2))
    return 0 if summary["fail_before_pass"] and summary["pass_count"] >= 2 else 1


if __name__ == "__main__":
    raise SystemExit(main())
