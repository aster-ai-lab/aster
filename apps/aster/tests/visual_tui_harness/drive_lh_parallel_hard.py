#!/usr/bin/env python3
"""Visual LH-H1 hard variant: require HOLD in runs.txt before GATE.

Same dual-defect shop fixture as drive_shop_multiturn, but one prompt that
orders: edit first file → run gate (HOLD) → edit second → run gate (GATE).
"""
from __future__ import annotations

import json
import subprocess
import sys
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from drive_shop_multiturn import (  # noqa: E402
    FAIL_LINE,
    PASS_LINE,
    capture,
    find_wid,
    list_aster_terminal_windows,
    paste_submit,
    settled,
    sh,
    wait_pred,
    write_fixture,
)

HARD = (
    "Fix BOTH rate.py and fee.py, but you MUST follow this order exactly:\n"
    "1) Edit only rate.py, then run exactly `python3 gate.py` "
    "(expect DUAL-HOLD in the output / runs.txt).\n"
    "2) Only after that HOLD line exists, edit fee.py, then run "
    "`python3 gate.py` again until DUAL-GATE-7K.\n"
    "If denied, request escalation. Quote gate outputs. "
    "Do not fix both files before the first gate run."
)


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
    title_tag = f"ASTER-VIS-LH1H-{ws.name}-{int(time.time())}"
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

    def rate() -> str:
        return (ws / "rate.py").read_text()

    def fee() -> str:
        return (ws / "fee.py").read_text()

    def runs() -> str:
        p = ws / "runs.txt"
        return p.read_text() if p.exists() else ""

    assert wait_pred(
        lambda h: "Type a message" in h or "❯" in h or "deepseek" in h.lower(),
        120,
        "welcome",
    )
    capture(ev, wid, "01-welcome")

    paste_submit(HARD)
    assert wait_pred(
        lambda h: settled(h)
        and PASS_LINE in runs()
        and FAIL_LINE in runs()
        and runs().find(FAIL_LINE) < runs().find(PASS_LINE)
        and "* 3" not in rate()
        and "+ 11" not in fee(),
        720,
        "hard-parallel",
    )
    capture(ev, wid, "02-hard-repaired")

    summary = {
        "visual_lh_h1_hard": True,
        "model": args.model,
        "config": str(config) if config else None,
        "runs": runs().splitlines(),
        "rate": rate(),
        "fee": fee(),
        "fail_before_pass": runs().find(FAIL_LINE) < runs().find(PASS_LINE),
        "pass_count": runs().count(PASS_LINE),
        "pass": (
            PASS_LINE in runs()
            and FAIL_LINE in runs()
            and runs().find(FAIL_LINE) < runs().find(PASS_LINE)
            and "* 3" not in rate()
            and "+ 11" not in fee()
        ),
    }
    (ev / "visual-lh-h1-hard-summary.json").write_text(json.dumps(summary, indent=2))
    (ev / "visual-lh-h1-hard-runs.txt").write_text(runs())
    print(json.dumps(summary, indent=2))
    return 0 if summary["pass"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
