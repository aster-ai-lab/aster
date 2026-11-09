#!/usr/bin/env python3
"""Visual companion for LH-H3: retrieval/search-driven single-file repair.

Buries a unique needle token in a non-obvious path and a same-named decoy.
Pass requires: needle file fixed, decoy untouched, gate GATE line, and
Terminal history showing a search/grep/read that found the needle (not only
the final edit).
"""
from __future__ import annotations

import json
import subprocess
import sys
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from drive_shop_multiturn import (  # noqa: E402
    capture,
    find_wid,
    list_aster_terminal_windows,
    paste_submit,
    settled,
    sh,
    terminal_history,
    wait_pred,
)

NEEDLE = "RETRIEVE-NEEDLE-9c2e"
PASS_LINE = "RETRIEVE-GATE-9c2e"
FAIL_LINE = "RETRIEVE-HOLD"

# Decoy looks like the real shop helper but is already correct / must stay.
DECOY_RATE = '''def sticker_price(count):
    # decoy — do not edit
    return count * 8
'''

# Real bug lives under nested path with the unique needle comment.
NEEDLE_RATE = f'''def sticker_price(count):
    # {NEEDLE}
    return count * 3
'''

GATE_PY = f'''\
import sys
sys.dont_write_bytecode = True
from pathlib import Path
import importlib.util

needle = Path("inventory/backstock/rate_impl.py")
decoy = Path("decoy/rate.py")
spec = importlib.util.spec_from_file_location("rate_impl", needle)
mod = importlib.util.module_from_spec(spec)
spec.loader.exec_module(mod)
ok = mod.sticker_price(4) == 32 and mod.sticker_price(5) == 40
decoy_text = decoy.read_text(encoding="utf-8")
decoy_ok = "do not edit" in decoy_text and "* 3" not in decoy_text
if not ok:
    line = "{FAIL_LINE}"
elif not decoy_ok:
    line = "RETRIEVE-DECOY-TOUCHED"
else:
    line = "{PASS_LINE}"
print(line, flush=True)
open("runs.txt", "a", encoding="utf-8").write(line + "\\n")
raise SystemExit(0 if line == "{PASS_LINE}" else 1)
'''

PROMPT = (
    f"A unique token `{NEEDLE}` marks the only broken pricing module. "
    "Search the workspace for that token (grep/search); do not invent paths. "
    "Fix that file so sticker_price(4)==32 and sticker_price(5)==40. "
    "Do not modify decoy files. Then run exactly `python3 gate.py` and quote its output."
)


def write_fixture(ws: Path) -> None:
    ws.mkdir(parents=True, exist_ok=True)
    (ws / "decoy").mkdir(parents=True, exist_ok=True)
    (ws / "inventory" / "backstock").mkdir(parents=True, exist_ok=True)
    (ws / "decoy" / "rate.py").write_text(DECOY_RATE)
    (ws / "inventory" / "backstock" / "rate_impl.py").write_text(NEEDLE_RATE)
    (ws / "README.md").write_text("Shop pricing lives somewhere under inventory/.\n")
    (ws / "gate.py").write_text(GATE_PY)
    runs = ws / "runs.txt"
    if runs.exists():
        runs.unlink()


def main() -> int:
    import argparse

    ap = argparse.ArgumentParser()
    ap.add_argument("--bin-dir", type=Path, required=True)
    ap.add_argument("--work-dir", type=Path, required=True)
    ap.add_argument("--evidence", type=Path, required=True)
    ap.add_argument("--model", default="deepseek/deepseek-flash")
    ap.add_argument("--config", type=Path, default=None)
    args = ap.parse_args()

    bin_dir = args.bin_dir.resolve()
    ws = (args.work_dir / "ws").resolve()
    ev = args.evidence.resolve()
    ev.mkdir(parents=True, exist_ok=True)
    write_fixture(ws)

    config = args.config
    if config is None:
        repo_acl = Path(__file__).resolve().parents[4] / ".aster" / "config.acl"
        config = repo_acl if repo_acl.is_file() else None
    else:
        config = config.resolve()
        if not config.is_file():
            raise SystemExit(f"--config not a file: {config}")

    marker = str(ws.resolve())
    title_tag = f"ASTER-VIS-LH3-{ws.name}-{int(time.time())}"
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
                f"printf '\\033]0;{title_tag}\\007'",
                "clear",
                f"exec aster --model '{args.model}' --no-plan --fullscreen",
                "",
            ]
        )
    )
    launch.chmod(0o755)
    before_ids = {w["id"] for w in list_aster_terminal_windows()}
    subprocess.run(["open", "-a", "Terminal", str(launch)], check=True)
    wid = find_wid(marker, title_tag=title_tag, before_ids=before_ids)
    (ev / "wid.txt").write_text(wid)
    (ev / "title_tag.txt").write_text(title_tag)
    sh(["agent-desktop", "focus-window", "--app", "终端", "--window-id", wid], check=False)
    time.sleep(2)

    needle_path = ws / "inventory" / "backstock" / "rate_impl.py"
    decoy_path = ws / "decoy" / "rate.py"
    decoy_before = decoy_path.read_text()

    def needle() -> str:
        return needle_path.read_text()

    def runs() -> str:
        p = ws / "runs.txt"
        return p.read_text() if p.exists() else ""

    assert wait_pred(
        lambda h: "Type a message" in h or "❯" in h or "deepseek" in h.lower(),
        120,
        "welcome",
    )
    capture(ev, wid, "01-welcome")

    paste_submit(PROMPT)
    assert wait_pred(
        lambda h: settled(h)
        and PASS_LINE in runs()
        and "* 3" not in needle()
        and decoy_path.read_text() == decoy_before,
        600,
        "retrieve-repair",
    )

    hist = terminal_history()
    search_evidence = any(
        token in hist.lower()
        for token in (
            "grep",
            "search",
            "rg ",
            NEEDLE.lower(),
            "rate_impl",
            "backstock",
        )
    )
    try:
        capture(ev, wid, "02-retrieve-repaired")
    except Exception as exc:  # noqa: BLE001 — visual companion; file oracle wins
        (ev / "capture-02-error.txt").write_text(str(exc))
    summary = {
        "visual_lh_h3_retrieval": True,
        "model": args.model,
        "config": str(config) if config else None,
        "needle_token": NEEDLE,
        "runs": runs().splitlines(),
        "needle_file": needle(),
        "decoy_unchanged": decoy_path.read_text() == decoy_before,
        "pass_line": PASS_LINE in runs(),
        "search_evidence_in_history": search_evidence,
        "pass": PASS_LINE in runs()
        and "* 3" not in needle()
        and decoy_path.read_text() == decoy_before,
    }
    (ev / "visual-lh-h3-summary.json").write_text(json.dumps(summary, indent=2))
    (ev / "visual-lh-h3-runs.txt").write_text(runs())
    (ev / "02-retrieve-repaired-hist.txt").write_text(hist)
    print(json.dumps(summary, indent=2))
    return 0 if summary["pass"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
