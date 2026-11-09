#!/usr/bin/env python3
"""Visual SR: semantic/meaning search with workspace_retrieval enabled.

Copies host ASTER_CONFIG and appends workspace_retrieval { enabled / local_cpu }.
Needle lives under a nested path with meaning-rich comment (no path in prompt).
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

PASS_LINE = "SEMANTIC-GATE-9c2e"
FAIL_LINE = "SEMANTIC-HOLD"

NEEDLE = '''def compute_invoice_subtotal(lines):
    """Sum line amounts for the quarterly invoice reconciliation report."""
    # BUG: doubles every line
    return sum(x * 2 for x in lines)
'''

FIXED_OK = "sum(x for x in lines)"  # substring we expect after fix

DECOY = '''def compute_invoice_subtotal(lines):
    """Unrelated decoy — already correct; do not edit."""
    return sum(x for x in lines)
'''

GATE_PY = f'''\
import sys
sys.dont_write_bytecode = True
from pathlib import Path
import importlib.util
needle = Path("finance/quarterly/invoice_math.py")
decoy = Path("decoy/invoice_math.py")
spec = importlib.util.spec_from_file_location("invoice_math", needle)
mod = importlib.util.module_from_spec(spec)
spec.loader.exec_module(mod)
ok = mod.compute_invoice_subtotal([10, 20, 30]) == 60
decoy_text = decoy.read_text(encoding="utf-8")
decoy_ok = "do not edit" in decoy_text and "* 2" not in decoy_text
if not ok:
    line = "{FAIL_LINE}"
elif not decoy_ok:
    line = "SEMANTIC-DECOY-TOUCHED"
else:
    line = "{PASS_LINE}"
print(line, flush=True)
open("runs.txt", "a", encoding="utf-8").write(line + "\\n")
raise SystemExit(0 if line == "{PASS_LINE}" else 1)
'''

PROMPT = (
    "By meaning (prefer semantic or hybrid search if available), find the function "
    "that sums line amounts for the quarterly invoice reconciliation report. "
    "It currently doubles each line. Fix only that implementation so "
    "compute_invoice_subtotal([10,20,30])==60. Do not edit decoys. "
    "Then run exactly `python3 gate.py` and quote its output. "
    "Do not guess filesystem paths from this prompt."
)


def write_fixture(ws: Path) -> None:
    ws.mkdir(parents=True, exist_ok=True)
    (ws / "finance" / "quarterly").mkdir(parents=True, exist_ok=True)
    (ws / "decoy").mkdir(parents=True, exist_ok=True)
    (ws / "finance" / "quarterly" / "invoice_math.py").write_text(NEEDLE)
    (ws / "decoy" / "invoice_math.py").write_text(DECOY)
    (ws / "gate.py").write_text(GATE_PY)
    runs = ws / "runs.txt"
    if runs.exists():
        runs.unlink()


def write_retrieval_config(host: Path, dest: Path) -> Path:
    text = host.read_text(encoding="utf-8")
    if "workspace_retrieval" not in text:
        text += """

workspace_retrieval {
  enabled = true
  local_cpu {}
}
"""
    dest.write_text(text, encoding="utf-8")
    return dest


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

    host = args.config
    if host is None:
        host = Path(__file__).resolve().parents[4] / ".aster" / "config.acl"
    else:
        host = host.resolve()
    if not host.is_file():
        raise SystemExit(f"host config missing: {host}")
    config = write_retrieval_config(host, ev / "config.retrieval.acl")

    marker = str(ws.resolve())
    title_tag = f"ASTER-VIS-SR-{ws.name}-{int(time.time())}"
    launch = ev / "launch_aster.sh"
    launch.write_text(
        "\n".join(
            [
                "#!/bin/bash",
                "set -euo pipefail",
                f"export PATH='{bin_dir}':\"$PATH\"",
                f"export ASTER_CONFIG='{config}'",
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
    sh(["agent-desktop", "focus-window", "--app", "终端", "--window-id", wid], check=False)
    time.sleep(2)

    needle_path = ws / "finance" / "quarterly" / "invoice_math.py"
    decoy_path = ws / "decoy" / "invoice_math.py"
    decoy_before = decoy_path.read_text()

    def needle() -> str:
        return needle_path.read_text()

    def runs() -> str:
        p = ws / "runs.txt"
        return p.read_text() if p.exists() else ""

    assert wait_pred(
        lambda h: "Type a message" in h or "❯" in h or "deepseek" in h.lower(),
        180,
        "welcome",
    )
    capture(ev, wid, "01-welcome")

    paste_submit(PROMPT)
    assert wait_pred(
        lambda h: settled(h)
        and PASS_LINE in runs()
        and "x * 2" not in needle()
        and decoy_path.read_text() == decoy_before,
        720,
        "semantic-repair",
    )
    hist = terminal_history()
    try:
        capture(ev, wid, "02-semantic-repaired")
    except Exception as exc:  # noqa: BLE001
        (ev / "capture-02-error.txt").write_text(str(exc))

    search_evidence = any(
        tok in hist.lower()
        for tok in (
            "semantic",
            "hybrid",
            "search",
            "grep",
            "invoice",
            "quarterly",
            "reconciliation",
        )
    )
    summary = {
        "visual_sr_semantic": True,
        "model": args.model,
        "config": str(config),
        "workspace_retrieval_appended": True,
        "runs": runs().splitlines(),
        "needle_file": needle(),
        "decoy_unchanged": decoy_path.read_text() == decoy_before,
        "search_evidence_in_history": search_evidence,
        "pass": PASS_LINE in runs()
        and "x * 2" not in needle()
        and decoy_path.read_text() == decoy_before,
    }
    (ev / "visual-sr-semantic-summary.json").write_text(json.dumps(summary, indent=2))
    (ev / "visual-sr-semantic-runs.txt").write_text(runs())
    (ev / "02-semantic-repaired-hist.txt").write_text(hist)
    print(json.dumps(summary, indent=2))
    return 0 if summary["pass"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
