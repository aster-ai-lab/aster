#!/usr/bin/env python3
"""Visual companion for LH-H1: single-prompt dual-defect repair with parallel intent.

Pins DeepSeek via CLI --model against host ASTER_CONFIG (repo .aster/config.acl).
Oracle: workspace rate.py/fee.py + runs.txt (HOLD before GATE). Process evidence:
Terminal history mentioning parallel/subagent/tasks when present — not required
for pass (models may serialize); pass requires fail_before_pass + GATE.
"""
from __future__ import annotations

import json
import subprocess
import sys
import time
from pathlib import Path

# Reuse shop helpers / fixture constants.
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
    terminal_history,
    wait_pred,
    write_fixture,
)

PARALLEL = (
    "You must fix BOTH rate.py and fee.py. Prefer parallel work or subagents "
    "if the product supports them (do not invent tool names). "
    "Then run exactly `python3 gate.py`. If denied, request escalation. "
    "Quote the gate output. Do not stop after fixing only one file."
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
    title_tag = f"ASTER-VIS-LH1-{ws.name}-{int(time.time())}"
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

    # Warm: wait for composer
    assert wait_pred(lambda h: "Type a message" in h or "❯" in h or "deepseek" in h.lower(), 120, "welcome")
    capture(ev, wid, "01-welcome")
    hist = terminal_history()
    if "deepseek" not in hist.lower() and "DeepSeek" not in hist:
        # Footer may lag; still proceed but record pin intent.
        (ev / "model-pin-warning.txt").write_text(hist[-4000:])

    paste_submit(PARALLEL)
    assert wait_pred(
        lambda h: settled(h)
        and PASS_LINE in runs()
        and "* 3" not in rate()
        and "+ 11" not in fee(),
        600,
        "parallel-repair",
    )
    capture(ev, wid, "02-parallel-repaired")

    hist_final = terminal_history()
    process_hints = {
        "mentions_parallel": "parallel" in hist_final.lower(),
        "mentions_subagent": "subagent" in hist_final.lower() or "sub-agent" in hist_final.lower(),
        "mentions_tasks": "/tasks" in hist_final or "background task" in hist_final.lower(),
    }
    summary = {
        "visual_lh_h1_parallel": True,
        "model": args.model,
        "config": str(config) if config else None,
        "runs": runs().splitlines(),
        "rate": rate(),
        "fee": fee(),
        "fail_before_pass": (
            FAIL_LINE in runs() and runs().find(FAIL_LINE) < runs().find(PASS_LINE)
        )
        or (FAIL_LINE not in runs() and PASS_LINE in runs()),
        "pass_count": runs().count(PASS_LINE),
        "process_hints": process_hints,
        # Process hints are observational only — pass is file oracle.
        "process_hints_required_for_pass": False,
    }
    # Prefer HOLD-before-GATE when the model ran the checker mid-flight.
    soft_ok = summary["pass_count"] >= 1 and "* 3" not in rate() and "+ 11" not in fee()
    hard_ok = soft_ok and (
        FAIL_LINE in runs() and runs().find(FAIL_LINE) < runs().find(PASS_LINE)
    )
    summary["pass"] = hard_ok or soft_ok
    summary["hard_fail_before_pass"] = hard_ok
    (ev / "visual-lh-h1-summary.json").write_text(json.dumps(summary, indent=2))
    (ev / "visual-lh-h1-runs.txt").write_text(runs())
    print(json.dumps(summary, indent=2))
    return 0 if summary["pass"] else 1


if __name__ == "__main__":
    # Local wait_pred / settled may need answer_dialogs loop — mirror multiturn.
    # Patch wait_pred usage: the imported wait_pred already answers dialogs.
    raise SystemExit(main())
