#!/usr/bin/env python3
"""Stage 3 visual re-check against the freshly built static-preview window.

Headless semantic delivery only: snapshot / find / click / screenshot.
No focus-window, no coordinates, no mouse, no --headed.
"""
from __future__ import annotations

import json
import subprocess
import sys
from pathlib import Path

APP = "Aster"
WINDOW = sys.argv[1]
HERE = Path(__file__).resolve().parent
RESULTS: list[dict] = []

INTERNAL_MARKERS = (
    "completion gate",
    "effect_digest",
    "reasoning effort default",
    "/Users/",
)


def cli(*args: str) -> dict:
    try:
        run = subprocess.run(
            ["agent-desktop", *args], capture_output=True, text=True, timeout=45
        )
        return json.loads(run.stdout)
    except Exception as error:
        return {"ok": False, "error": {"code": "HARNESS", "message": str(error)}}


def find(role: str, name: str) -> dict | None:
    payload = cli(
        "find", "--app", APP, "--window-id", WINDOW,
        "--role", role, "--name", name, "--exact", "--first",
    )
    return (payload.get("data") or {}).get("match") if payload.get("ok") else None


def click(role: str, name: str) -> bool:
    match = find(role, name)
    if not match:
        return False
    return bool(cli("click", match["ref_id"]).get("ok"))


def snapshot() -> dict:
    args = [
        "snapshot", "--app", APP, "--window-id", WINDOW,
        "-i", "--compact", "--timeout-ms", "15000",
    ]
    payload = cli(*args)
    if not payload.get("ok") or not (payload.get("data") or {}).get("tree"):
        payload = cli(*args)
    return payload


def collect_names(payload: dict) -> list[str]:
    out: list[str] = []

    def walk(node):
        if not isinstance(node, dict):
            return
        if node.get("name"):
            out.append(str(node["name"]))
        for child in node.get("children") or []:
            walk(child)

    walk((payload.get("data") or {}).get("tree"))
    return out


def shot(filename: str) -> dict:
    return cli("screenshot", "--window-id", WINDOW, str(HERE / filename))


def record(scenario: str, **fields) -> None:
    entry = {"scenario": scenario, **fields}
    RESULTS.append(entry)
    print(json.dumps(entry, ensure_ascii=False), flush=True)


def dismiss_onboarding() -> dict:
    """A fresh install shows a macOS permission modal that hides the whole app.

    Take a real screenshot first (00-onboarding.png), then dismiss it so the
    following AX snapshots describe the actual application, not the modal.
    """
    shot("00-onboarding.png")
    for label in ("继续", "稍后设置", "关闭"):
        match = find("button", label)
        if match:
            return {"label": label, "click_ok": bool(cli("click", match["ref_id"]).get("ok"))}
    return {"label": None, "click_ok": False}


def main() -> int:
    dismissed = dismiss_onboarding()
    first = snapshot()
    names = collect_names(first)
    record(
        "workbench",
        onboarding_dismissed=dismissed,
        snapshot_ok=bool(first.get("ok")),
        ax_nodes=len(names),
        screenshot_ok=bool(shot("01-workbench.png").get("ok")),
        internal_markers=[m for m in INTERNAL_MARKERS if any(m in n for n in names)],
    )

    # The workbench sidebar calls the scheduled-task page "自动化".
    matched = (
        find("button", "自动化")
        or find("tab", "自动化")
        or find("staticText", "自动化")
    )
    clicked = bool(matched and cli("click", matched["ref_id"]).get("ok"))
    page_names = collect_names(snapshot())
    record(
        "automation",
        control_found=bool(matched),
        click_ok=clicked,
        ax_nodes=len(page_names),
        screenshot_ok=bool(shot("02-automation.png").get("ok")),
        internal_markers=[m for m in INTERNAL_MARKERS if any(m in n for n in page_names)],
    )

    # Best effort: trigger a test run and look for the new visible feedback.
    # No tasks on this page means no button; that is reported, not faked.
    run_match = find("button", "测试运行")
    run_clicked = bool(run_match and cli("click", run_match["ref_id"]).get("ok"))
    after = collect_names(snapshot())
    record(
        "automation-test-run-feedback",
        control_found=bool(run_match),
        click_ok=run_clicked,
        toast_visible=any("已记录一次测试运行" in n for n in after),
        ax_nodes=len(after),
        screenshot_ok=bool(shot("03-automation-feedback.png").get("ok")),
    )

    # A second page widens the AX sweep for leaked internal diagnostics.
    secondary = find("button", "技能") or find("button", "连接器")
    secondary_label = "技能" if find("button", "技能") else "连接器"
    secondary_clicked = bool(secondary and cli("click", secondary["ref_id"]).get("ok"))
    secondary_names = collect_names(snapshot())
    record(
        "secondary",
        label=secondary_label,
        control_found=bool(secondary),
        click_ok=secondary_clicked,
        ax_nodes=len(secondary_names),
        screenshot_ok=bool(shot("04-secondary.png").get("ok")),
        internal_markers=[m for m in INTERNAL_MARKERS if any(m in n for n in secondary_names)],
    )

    (HERE / "visual-check-results.json").write_text(
        json.dumps({"window": WINDOW, "results": RESULTS}, ensure_ascii=False, indent=2) + "\n",
        encoding="utf-8",
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
