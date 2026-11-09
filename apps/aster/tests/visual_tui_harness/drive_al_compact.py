#!/usr/bin/env python3
"""Visual AL-V1: dual-defect shop with mid-run /compact, then resume to GATE.

Proves the autonomous loop survives compaction in the product TUI (companion
to hermetic convergence). Oracle remains runs.txt HOLD→GATE + repaired files.
"""
from __future__ import annotations

import json
import subprocess
import sys
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from drive_shop_multiturn import (  # noqa: E402
    DIAG,
    FAIL_LINE,
    PASS_LINE,
    PARTIAL,
    RUN,
    SECOND,
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
    title_tag = f"ASTER-VIS-ALV1-{ws.name}-{int(time.time())}"
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

    paste_submit(DIAG)
    assert wait_pred(
        lambda h: settled(h) and "* 3" in rate() and "+ 11" in fee(),
        180,
        "diag",
    )
    capture(ev, wid, "02-diag")

    paste_submit(PARTIAL)
    assert wait_pred(
        lambda h: settled(h)
        and "* 3" not in rate()
        and "+ 11" in fee()
        and FAIL_LINE in runs(),
        360,
        "partial-hold",
    )
    capture(ev, wid, "03-partial")

    # Mid-loop compaction — autonomous loop must remain usable afterward.
    paste_submit("/compact")
    assert wait_pred(
        lambda h: settled(h)
        and (
            "compact" in h.lower()
            or "Compact" in h
            or "nothing to compact" in h.lower()
            or "Add a follow-up" in h
        ),
        180,
        "compact",
    )
    capture(ev, wid, "04-compact")
    hist_compact = terminal_history()
    (ev / "04-compact-hist.txt").write_text(hist_compact)

    paste_submit(SECOND)
    assert wait_pred(
        lambda h: settled(h) and "+ 11" not in fee() and PASS_LINE in runs(),
        420,
        "post-compact-repair",
    )
    capture(ev, wid, "05-repaired")

    before = runs().count(PASS_LINE)
    paste_submit(RUN)
    assert wait_pred(
        lambda h: settled(h) and runs().count(PASS_LINE) > before,
        240,
        "rerun",
    )
    capture(ev, wid, "06-rerun")

    summary = {
        "visual_al_v1_compact": True,
        "model": args.model,
        "config": str(config) if config else None,
        "runs": runs().splitlines(),
        "rate": rate(),
        "fee": fee(),
        "fail_before_pass": FAIL_LINE in runs()
        and PASS_LINE in runs()
        and runs().find(FAIL_LINE) < runs().find(PASS_LINE),
        "pass_count": runs().count(PASS_LINE),
        "compact_hist_mentions_compact": "compact" in hist_compact.lower(),
        "pass": (
            FAIL_LINE in runs()
            and PASS_LINE in runs()
            and runs().find(FAIL_LINE) < runs().find(PASS_LINE)
            and "* 3" not in rate()
            and "+ 11" not in fee()
        ),
    }
    (ev / "visual-al-v1-summary.json").write_text(json.dumps(summary, indent=2))
    (ev / "visual-al-v1-runs.txt").write_text(runs())
    print(json.dumps(summary, indent=2))
    return 0 if summary["pass"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
