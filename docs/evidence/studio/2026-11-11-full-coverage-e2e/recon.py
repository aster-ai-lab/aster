#!/usr/bin/env python3
"""Recon pass: dump the live interactive AX inventory of the fresh window.

The full-coverage runner uses this as its *authoritative* inventory; the older
2026-11-10 ledger is only a cross-check, never the source of truth.
"""
from __future__ import annotations

import json
import subprocess
import sys
from collections import Counter
from pathlib import Path

APP = "Aster"
WINDOW = sys.argv[1]
HERE = Path(__file__).resolve().parent

INTERACTIVE_ROLES = {
    "button", "combobox", "switch", "checkbox", "radiobutton", "textfield",
    "tab", "menuitem", "link", "slider", "stepper", "disclosuretriangle",
    "popUpButton", "menuButton", "textarea", "searchfield", "incrementor",
}


def cli(*args: str, timeout: int = 45) -> dict:
    try:
        run = subprocess.run(
            ["agent-desktop", *args], capture_output=True, text=True, timeout=timeout
        )
        return json.loads(run.stdout)
    except Exception as error:
        return {"ok": False, "error": {"code": "HARNESS", "message": str(error)}}


def snapshot(surface: str = "window") -> dict:
    args = ["snapshot", "--app", APP, "--window-id", WINDOW,
            "-i", "--compact", "--timeout-ms", "15000"]
    if surface != "window":
        args += ["--surface", surface]
    payload = cli(*args)
    if not payload.get("ok") or not (payload.get("data") or {}).get("tree"):
        payload = cli(*args)
    return payload


def find(role: str, name: str, surface: str = "window") -> dict | None:
    args = ["find", "--app", APP, "--window-id", WINDOW,
            "--role", role, "--name", name, "--exact", "--first"]
    if surface != "window":
        args += ["--surface", surface]
    return (cli(*args).get("data") or {}).get("match")


def click(ref: str) -> dict:
    p = cli("click", ref)
    return {"ok": bool(p.get("ok")), "error": (p.get("error") or {}).get("code")}


def shot(name: str) -> None:
    cli("screenshot", "--window-id", WINDOW, str(HERE / name))


def flatten(payload: dict) -> list[dict]:
    nodes: list[dict] = []

    def walk(node):
        if not isinstance(node, dict):
            return
        nodes.append(node)
        for child in node.get("children") or []:
            walk(child)

    walk((payload.get("data") or {}).get("tree"))
    return nodes


def summarize(label: str) -> dict:
    nodes = flatten(snapshot())
    controls = []
    for n in nodes:
        role = str(n.get("role") or "")
        if role in INTERACTIVE_ROLES and n.get("name"):
            controls.append({
                "role": role,
                "name": str(n["name"]),
                "actions": n.get("available_actions") or [],
                "states": n.get("states") or [],
            })
    names = [str(n["name"]) for n in nodes if n.get("name")]
    return {
        "label": label,
        "nodes": len(nodes),
        "named_nodes": len(names),
        "interactive_instances": len(controls),
        "unique_interactive": len({(c["role"], c["name"]) for c in controls}),
        "role_counts": dict(Counter(c["role"] for c in controls).most_common()),
        "controls": controls,
        "all_names": names,
    }


def main() -> int:
    # First-run permission modal on a fresh profile.
    for label in ("继续", "稍后设置", "关闭"):
        m = find("button", label)
        if m:
            click(m["ref_id"])
            break

    states = [summarize("工作台")]
    shot("recon-00-workbench.png")

    # Every top-level destination visible in the persistent sidebar.
    for label in ("连接器", "技能", "自动化", "项目"):
        m = find("button", label)
        if m:
            click(m["ref_id"])
            states.append(summarize(label))
            shot(f"recon-{label}.png")

    # Settings and its full secondary menu are the densest area.
    m = find("button", "设置")
    if m:
        click(m["ref_id"])
        settings = summarize("设置")
        states.append(settings)
        shot("recon-settings.png")

    (HERE / "recon.json").write_text(
        json.dumps({"window": WINDOW, "states": states}, ensure_ascii=False, indent=2) + "\n",
        encoding="utf-8",
    )
    for s in states:
        print(json.dumps({k: v for k, v in s.items()
                         if k not in ("controls", "all_names")}, ensure_ascii=False), flush=True)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
