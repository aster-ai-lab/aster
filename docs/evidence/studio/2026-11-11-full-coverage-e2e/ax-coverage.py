#!/usr/bin/env python3
"""Exhaustive AX touch of every functional point on every reachable page.

`get --property states` is a real accessibility interaction that does not
mutate the UI, so refs from ONE snapshot stay valid across a whole batch
(verified 6/6). This gives a complete, reproducible account of which controls
are reachable and operable. Executed *actions* (click/toggle/...) are tracked
separately and merged from the action passes — never conflated.
"""
from __future__ import annotations

import json
import subprocess
import sys
from pathlib import Path

APP = "Aster"
WINDOW = sys.argv[1]
HERE = Path(__file__).resolve().parent
LIMIT = 60
CHROME = {"Close", "Full Screen", "Minimize", "Aster"}
INTERACTIVE = {"button", "combobox", "switch", "checkbox", "radiobutton",
               "textfield", "tab", "menuitem", "link", "slider", "stepper",
               "popUpButton", "menuButton", "textarea", "searchfield", "incrementor"}


def cli(*args, timeout=90):
    try:
        r = subprocess.run(["agent-desktop", *args], capture_output=True,
                           text=True, timeout=timeout)
        return json.loads(r.stdout)
    except Exception as e:
        return {"ok": False, "error": {"code": "HARNESS", "message": str(e)}}


def flat(p):
    out = []

    def walk(n):
        if not isinstance(n, dict):
            return
        out.append(n)
        for c in n.get("children") or []:
            walk(c)
    walk((p.get("data") or {}).get("tree"))
    return out


def snapshot():
    a = ["snapshot", "--app", APP, "--window-id", WINDOW,
         "-i", "--compact", "--timeout-ms", "15000"]
    p = cli(*a)
    if not p.get("ok") or not (p.get("data") or {}).get("tree"):
        p = cli(*a)
    return p


def controls(p):
    out, seen = [], set()
    for n in flat(p):
        role, name = str(n.get("role") or ""), str(n.get("name") or "")
        if role not in INTERACTIVE or not name or name in CHROME:
            continue
        k = (role, name)
        if k in seen:
            continue
        seen.add(k)
        out.append({"role": role, "name": name, "ref_id": n.get("ref_id"),
                    "actions": n.get("available_actions") or [],
                    "states": n.get("states") or []})
    return out


def find(role, name):
    # Usage page has 1165 AX nodes; a 1.5s traversal deadline silently misses
    # the settings menu and leaves the runner on the previous page.
    p = cli("find", "--app", APP, "--window-id", WINDOW, "--role", role,
            "--name", name, "--exact", "--first", "--timeout-ms", "6000", timeout=15)
    return ((p.get("data") or {}).get("match") or {}).get("ref_id")


def click(ref):
    cli("click", ref, "--timeout-ms", "1500", timeout=12)


def wait(ms=400):
    cli("wait", str(ms), timeout=10)


def navigate(page):
    """Always route through the workbench, then into the target page.

    Going from one settings subpage straight to another failed on the 1165-node
    usage page; a clean workbench -> settings -> subpage path is deterministic.
    """
    back = find("button", "返回应用")
    home = find("button", "工作")
    if back:
        click(back)
        wait()
    elif home:
        click(home)
        wait()
    if page == "工作台":
        return
    if page in ("连接器", "技能", "自动化"):
        m = find("button", page)
        if m:
            click(m)
            wait(500)
        return
    m = find("button", "设置")
    if m:
        click(m)
        wait(450)
    sub = page.split("/", 1)[1]
    m = find("button", sub)
    if m:
        click(m)
        wait(600)


def shot(name):
    safe = name.replace("/", "-").replace(" ", "_")
    p = cli("screenshot", "--window-id", WINDOW, str(HERE / f"{safe}.png"), timeout=30)
    return {"file": f"{safe}.png", "ok": bool(p.get("ok"))}


def batch(entries):
    if not entries:
        return []
    p = cli("batch", json.dumps(entries, ensure_ascii=False),
            "--timeout-ms", "90000", timeout=120)
    res = (p.get("data") or {}).get("results") or []
    while len(res) < len(entries):
        res.append({"ok": False, "error": {"code": "NO_RESULT"}})
    return res[: len(entries)]


def main():
    data = json.loads((HERE / "pages.json").read_text(encoding="utf-8"))
    coverage, screenshots = [], []
    for e in data["pages"]:
        page = e["page"]
        navigate(page)
        wait(250)
        p = snapshot()
        cs = controls(p)
        expected = e["unique_controls"]
        verified = abs(len(cs) - expected) <= max(12, int(expected * 0.35))
        if not verified:
            # Retry once from a clean workbench; a mismatched control count is
            # the signal that navigation silently stayed on the previous page.
            navigate(page)
            wait(700)
            p = snapshot()
            cs = controls(p)
            verified = abs(len(cs) - expected) <= max(12, int(expected * 0.35))
        screenshots.append(shot(f"ax-{e['index']:02d}-{page}"))
        entries = [{"command": "get", "args": {"ref_id": c["ref_id"], "property": "states"}}
                   for c in cs if c["ref_id"]]
        results = []
        for i in range(0, len(entries), LIMIT):
            results.extend(batch(entries[i:i + LIMIT]))
        ri = 0
        for c in cs:
            r = results[ri] if c["ref_id"] and ri < len(results) else None
            if c["ref_id"]:
                ri += 1
            coverage.append({
                "page": page, "role": c["role"], "name": c["name"],
                "ax_reachable": bool(c["ref_id"]),
                "ax_interaction_ok": bool(r and r.get("ok")),
                "states": c["states"],
                "available_actions": c["actions"],
                "error": None if r and r.get("ok") else (r or {}).get("error", {}).get("code")
                if r else "NO_REF",
                "action_executed": False,
                "navigation_verified": verified,
            })
        ok = sum(1 for x in coverage if x["page"] == page and x["ax_interaction_ok"])
        print(json.dumps({"page": page, "controls": len(cs),
                          "expected": expected, "ax_touched": ok,
                          "navigation_verified": verified},
                         ensure_ascii=False), flush=True)
        (HERE / "ax-coverage.json").write_text(
            json.dumps({"window": WINDOW, "pages": data["pages"],
                        "screenshots": screenshots, "coverage": coverage},
                       ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    return 0


if __name__ == "__main__":
    for label in ("继续", "稍后设置", "关闭"):
        m = find("button", label)
        if m:
            click(m)
            wait(200)
            break
    raise SystemExit(main())
