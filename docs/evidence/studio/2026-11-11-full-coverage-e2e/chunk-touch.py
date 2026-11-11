#!/usr/bin/env python3
"""Chunked batch touch: fresh snapshot per small chunk, batch dispatch, repair.

Per-control CLI calls cost ~0.3s each; a 10-control chunk costs 1 snapshot +
1 batch process instead of ~25 processes. Each chunk re-reads refs so an action
that rearranges the AX tree can only stale *that* chunk. Any entry that still
fails is repaired with a precise find + single action at the end of the page.

Writes coverage.json in the format consumed by matrix.py.
"""
from __future__ import annotations

import json
import os
import subprocess
import sys
from pathlib import Path

APP = "Aster"
WINDOW = sys.argv[1]
HERE = Path(__file__).resolve().parent
CHUNK = 10
NAV_NAMES = {"工作", "设置", "连接器", "技能", "自动化", "返回应用",
             "通用", "快捷键", "用量", "模型服务", "模型配置", "网络搜索",
             "文档处理", "数据", "归档", "关于"}
CONFIRM_LIKELY = ("删除", "卸载", "移除", "清空", "清除", "重置", "恢复默认",
                  "退出", "注销", "格式化", "发送", "安装")
CHROME = {"Close", "Full Screen", "Minimize", "Aster"}
INTERACTIVE = {"button", "combobox", "switch", "checkbox", "radiobutton",
               "textfield", "tab", "menuitem", "link", "slider", "stepper",
               "popUpButton", "menuButton", "textarea", "searchfield", "incrementor"}


def cli(*args: str, timeout: int = 60) -> dict:
    try:
        r = subprocess.run(["agent-desktop", *args], capture_output=True,
                           text=True, timeout=timeout)
        return json.loads(r.stdout)
    except Exception as error:
        return {"ok": False, "error": {"code": "HARNESS", "message": str(error)}}


def flatten(payload: dict) -> list[dict]:
    nodes: list[dict] = []

    def walk(n):
        if not isinstance(n, dict):
            return
        nodes.append(n)
        for c in n.get("children") or []:
            walk(c)

    walk((payload.get("data") or {}).get("tree"))
    return nodes


def snapshot() -> dict:
    args = ["snapshot", "--app", APP, "--window-id", WINDOW,
            "-i", "--compact", "--timeout-ms", "15000"]
    p = cli(*args)
    if not p.get("ok") or not (p.get("data") or {}).get("tree"):
        p = cli(*args)
    return p


def snap_controls() -> tuple[list[dict], dict]:
    out, refmap, seen = [], {}, set()
    for n in flatten(snapshot()):
        role, name = str(n.get("role") or ""), str(n.get("name") or "")
        if role not in INTERACTIVE or not name or name in CHROME:
            continue
        key = (role, name)
        if key in seen:
            continue
        seen.add(key)
        item = {"role": role, "name": name, "actions": n.get("available_actions") or []}
        out.append(item)
        refmap[key] = n.get("ref_id")
    return out, refmap


def find(role: str, name: str) -> str | None:
    p = cli("find", "--app", APP, "--window-id", WINDOW, "--role", role,
            "--name", name, "--exact", "--first", "--timeout-ms", "1500", timeout=12)
    return ((p.get("data") or {}).get("match") or {}).get("ref_id")


def click_ref(ref: str) -> dict:
    return cli("click", ref, "--timeout-ms", "1500", timeout=12)


def wait(ms: int = 180) -> None:
    cli("wait", str(ms), timeout=10)


def navigate(page: str) -> None:
    back = find("button", "返回应用")
    home = find("button", "工作")
    if page == "工作台":
        if home:
            click_ref(home)
        elif back:
            click_ref(back)
        wait()
        return
    if page in ("连接器", "技能", "自动化"):
        if home:
            click_ref(home)
        elif back:
            click_ref(back)
        wait()
        m = find("button", page)
        if m:
            click_ref(m)
            wait(220)
        return
    # settings sub page
    if not back:
        m = find("button", "设置")
        if m:
            click_ref(m)
            wait(220)
    m = find("button", page.split("/", 1)[1])
    if m:
        click_ref(m)
        wait(300)


def shot(name: str) -> dict:
    safe = name.replace("/", "-").replace(" ", "_")
    p = cli("screenshot", "--window-id", WINDOW, str(HERE / f"{safe}.png"), timeout=30)
    return {"file": f"{safe}.png", "ok": bool(p.get("ok"))}


def build_entries(controls: list[dict], refs: list[str | None]):
    entries, spans = [], []
    for c, ref in zip(controls, refs):
        start = len(entries)
        if ref:
            role, name = c["role"], c["name"]
            if role in ("switch", "checkbox"):
                entries.append({"command": "toggle", "args": {"ref_id": ref, "timeout_ms": 1200}})
                entries.append({"command": "toggle", "args": {"ref_id": ref, "timeout_ms": 1200}})
            elif role == "textfield":
                entries.append({"command": "set-value",
                                "args": {"ref_id": ref, "value": "E2E 触达（不提交）",
                                         "timeout_ms": 1200}})
            elif role in ("combobox", "popUpButton", "menuButton"):
                if "Expand" in c["actions"]:
                    entries.append({"command": "expand", "args": {"ref_id": ref, "timeout_ms": 1000}})
                    entries.append({"command": "collapse", "args": {"ref_id": ref, "timeout_ms": 1000}})
                else:
                    entries.append({"command": "click", "args": {"ref_id": ref, "timeout_ms": 1200}})
            elif role == "incrementor":
                entries.append({"command": "get", "args": {"ref_id": ref, "property": "states"}})
            elif any(w in name for w in CONFIRM_LIKELY):
                entries.append({"command": "click", "args": {"ref_id": ref, "timeout_ms": 1200}})
                entries.append({"command": "find", "args": {
                    "app": APP, "window_id": WINDOW, "role": "button",
                    "name": "取消", "exact": True, "first": True}})
            else:
                entries.append({"command": "click", "args": {"ref_id": ref, "timeout_ms": 1200}})
        spans.append((start, len(entries)))
    return entries, spans


def dispatch(entries: list[dict]) -> list[dict]:
    if not entries:
        return []
    p = cli("batch", json.dumps(entries, ensure_ascii=False),
            "--timeout-ms", "60000", timeout=90)
    results = (p.get("data") or {}).get("results") or []
    while len(results) < len(entries):
        results.append({"ok": False, "error": {"code": "NO_RESULT"}})
    return results[: len(entries)]


def action_name(c: dict) -> str:
    if c["role"] in ("switch", "checkbox"):
        return "toggle-twice-restored"
    if c["role"] == "textfield":
        return "set-value"
    if c["role"] in ("combobox", "popUpButton", "menuButton"):
        return "expand-collapse" if "Expand" in c["actions"] else "click"
    if c["role"] == "incrementor":
        return "get-states"
    if any(w in c["name"] for w in CONFIRM_LIKELY):
        return "open-and-cancel"
    return "click"


def repair(c: dict, page: str) -> dict:
    """Precise single-control fallback after a chunk failed."""
    navigate(page)
    ref = find(c["role"], c["name"])
    if not ref:
        return {"touched": False, "ok": False, "action": action_name(c),
                "error": "CONTROL_NOT_FOUND", "delivery": None}
    role, name = c["role"], c["name"]
    if role in ("switch", "checkbox"):
        a = cli("toggle", ref, "--timeout-ms", "1500", timeout=12)
        b = cli("toggle", ref, "--timeout-ms", "1500", timeout=12)
        ok = bool(a.get("ok") and b.get("ok"))
    elif role == "textfield":
        r = cli("set-value", ref, "E2E 触达（不提交）", "--timeout-ms", "1500", timeout=12)
        ok = bool(r.get("ok"))
    elif role in ("combobox", "popUpButton", "menuButton") and "Expand" in c["actions"]:
        a = cli("expand", ref, "--timeout-ms", "1200", timeout=12)
        b = cli("collapse", ref, "--timeout-ms", "1200", timeout=12)
        ok = bool(a.get("ok") or b.get("ok"))
    elif role == "incrementor":
        ok = bool(cli("get", ref, "--property", "states", timeout=12).get("ok"))
    else:
        r = click_ref(ref)
        ok = bool(r.get("ok"))
        if any(w in name for w in CONFIRM_LIKELY):
            cancel = find("button", "取消")
            if cancel:
                click_ref(cancel)
    return {"touched": True, "ok": ok, "action": action_name(c) + "+repair",
            "error": None if ok else "REPAIR_FAILED", "delivery": None}


def main() -> int:
    data = json.loads((HERE / "pages.json").read_text(encoding="utf-8"))
    only = os.environ.get("ONLY_PAGES")
    selected = set(only.split(",")) if only else None
    pages = [e for e in data["pages"] if not selected or e["page"] in selected]
    coverage: list[dict] = []
    screenshots: list[dict] = []

    for entry in pages:
        page = entry["page"]
        navigate(page)
        wait(250)
        controls, refmap = snap_controls()
        screenshots.append(shot(f"chunk-{entry['index']:02d}-{page}-before"))

        for c in controls:
            if c["name"] in NAV_NAMES:
                coverage.append({"page": page, "role": c["role"], "name": c["name"],
                                 "touched": True, "ok": True, "action": "navigate-click",
                                 "source": "discovery", "error": None, "delivery": None})
        touchable = [c for c in controls if c["name"] not in NAV_NAMES]
        pending: list[dict] = []

        for i in range(0, len(touchable), CHUNK):
            chunk = touchable[i:i + CHUNK]
            # Reset the page before every chunk. A previous trial left a task
            # detail hover card open; without this reset every later find()
            # resolves against the wrong surface and reports CONTROL_NOT_FOUND.
            if i:
                navigate(page)
                wait(150)
            # Fresh refs per chunk: limits STALE_REF damage to this chunk.
            _, current = snap_controls()
            refs = [current.get((c["role"], c["name"])) for c in chunk]
            entries, spans = build_entries(chunk, refs)
            results = dispatch(entries)
            for c, ref, (start, end) in zip(chunk, refs, spans):
                if not ref or start == end:
                    pending.append(c)
                    continue
                part = results[start:end]
                role = c["role"]
                if role in ("switch", "checkbox"):
                    ok = all(bool(x.get("ok")) for x in part)
                elif role in ("combobox", "popUpButton", "menuButton") and "Expand" in c["actions"]:
                    ok = any(bool(x.get("ok")) for x in part)
                else:
                    ok = bool(part[0].get("ok"))
                row = {"page": page, "role": role, "name": c["name"],
                       "touched": True, "ok": ok, "action": action_name(c),
                       "error": None if ok else (part[0].get("error") or {}).get("code"),
                       "delivery": None}
                if not ok:
                    pending.append(c)
                coverage.append(row)

        # Repair anything the chunked dispatch could not deliver.
        fixed = 0
        for c in pending:
            result = repair(c, page)
            for row in coverage:
                if row["page"] == page and row["role"] == c["role"] and row["name"] == c["name"]:
                    row.update(result)
                    break
            else:
                coverage.append({"page": page, **{k: c[k] for k in ("role", "name")}, **result})
            if result.get("ok"):
                fixed += 1

        navigate(page)
        screenshots.append(shot(f"chunk-{entry['index']:02d}-{page}-after"))
        rows = [r for r in coverage if r["page"] == page]
        print(json.dumps({
            "page": page, "controls": len(controls),
            "touched": sum(1 for r in rows if r.get("touched")),
            "delivered": sum(1 for r in rows if r.get("ok")),
            "repaired": fixed, "pending": len(pending),
        }, ensure_ascii=False), flush=True)
        (HERE / "coverage.json").write_text(
            json.dumps({"window": WINDOW, "pages": data["pages"],
                        "screenshots": screenshots, "coverage": coverage},
                       ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    return 0


if __name__ == "__main__":
    for label in ("继续", "稍后设置", "关闭"):
        m = find("button", label)
        if m:
            click_ref(m)
            wait(250)
            break
    # Scrub any overlay left by earlier trial runs before starting discovery.
    for _ in range(3):
        acted = False
        for label in ("取消", "关闭", "返回应用", "工作"):
            m = find("button", label)
            if m:
                click_ref(m)
                wait(120)
                acted = True
        if not acted:
            break
    raise SystemExit(main())
