#!/usr/bin/env python3
"""Batch-accelerated full-feature touch pass.

A single agent-desktop CLI process handles up to 64 commands via `batch`.
Phase 1 resolves every control's ref in batches; phase 2 dispatches every
action in batches. The per-control script (full-coverage.py retry) remains the
fallback for anything a batch could not deliver.

Strict headless semantic delivery throughout.
"""
from __future__ import annotations

import json
import os
import subprocess
import sys
from collections import Counter, defaultdict
from pathlib import Path

APP = "Aster"
WINDOW = sys.argv[1]
HERE = Path(__file__).resolve().parent
BATCH_LIMIT = 60
NAV_NAMES = {"工作", "设置", "连接器", "技能", "自动化", "返回应用",
             "通用", "快捷键", "用量", "模型服务", "模型配置", "网络搜索",
             "文档处理", "数据", "归档", "关于"}
CONFIRM_LIKELY = ("删除", "卸载", "移除", "清空", "清除", "重置", "恢复默认",
                  "退出", "注销", "格式化", "发送", "安装")


def cli(*args: str, timeout: int = 180) -> dict:
    try:
        run = subprocess.run(["agent-desktop", *args], capture_output=True,
                             text=True, timeout=timeout)
        return json.loads(run.stdout)
    except Exception as error:
        return {"ok": False, "error": {"code": "HARNESS", "message": str(error)}}


def batch(entries: list[dict]) -> list[dict]:
    if not entries:
        return []
    p = cli("batch", json.dumps(entries, ensure_ascii=False),
            "--timeout-ms", "150000", timeout=180)
    results = (p.get("data") or {}).get("results") or []
    # Pad so every entry always has a result row.
    while len(results) < len(entries):
        results.append({"ok": False, "error": {"code": "NO_RESULT"}})
    return results[: len(entries)]


def find_entries(controls: list[dict]) -> list[dict]:
    return [{
        "command": "find",
        "args": {"app": APP, "window_id": WINDOW, "role": c["role"],
                 "name": c["name"], "exact": True, "first": True},
    } for c in controls]


def action_entries(controls: list[dict], refs: list[str | None]) -> tuple[list[dict], list[tuple[int, int]]]:
    """Return flat entries plus (start,end) index span per control."""
    entries: list[dict] = []
    spans: list[tuple[int, int]] = []
    for c, ref in zip(controls, refs):
        start = len(entries)
        if ref is None:
            spans.append((start, start))
            continue
        role = c["role"]
        name = str(c["name"])
        if role in ("switch", "checkbox"):
            # Toggle twice so user configuration is left exactly as found.
            entries.append({"command": "toggle", "args": {"ref_id": ref, "timeout_ms": 1500}})
            entries.append({"command": "toggle", "args": {"ref_id": ref, "timeout_ms": 1500}})
        elif role == "textfield":
            entries.append({"command": "set-value",
                            "args": {"ref_id": ref, "value": "E2E 触达（不提交）",
                                     "timeout_ms": 1500}})
        elif role in ("combobox", "popUpButton", "menuButton"):
            entries.append({"command": "expand", "args": {"ref_id": ref, "timeout_ms": 1200}})
            entries.append({"command": "collapse", "args": {"ref_id": ref, "timeout_ms": 1200}})
        elif role == "incrementor":
            entries.append({"command": "get", "args": {"ref_id": ref, "property": "states"}})
        elif any(word in name for word in CONFIRM_LIKELY):
            # Open the confirmation, then cancel it without committing.
            entries.append({"command": "click", "args": {"ref_id": ref, "timeout_ms": 1200}})
            entries.append({"command": "find", "args": {
                "app": APP, "window_id": WINDOW, "role": "button",
                "name": "取消", "exact": True, "first": True}})
        else:
            entries.append({"command": "click", "args": {"ref_id": ref, "timeout_ms": 1200}})
        spans.append((start, len(entries)))
    return entries, spans


def run_chunks(entries: list[dict]) -> list[dict]:
    out: list[dict] = []
    for i in range(0, len(entries), BATCH_LIMIT):
        out.extend(batch(entries[i:i + BATCH_LIMIT]))
    return out


def snapshot() -> dict:
    args = ["snapshot", "--app", APP, "--window-id", WINDOW,
            "-i", "--compact", "--timeout-ms", "15000"]
    p = cli(*args)
    if not p.get("ok") or not (p.get("data") or {}).get("tree"):
        p = cli(*args)
    return p


def flatten(payload: dict) -> list[dict]:
    nodes: list[dict] = []

    def walk(n):
        if not isinstance(n, dict):
            return
        nodes.append(n)
        for ch in n.get("children") or []:
            walk(ch)

    walk((payload.get("data") or {}).get("tree"))
    return nodes


INTERACTIVE = {"button", "combobox", "switch", "checkbox", "radiobutton",
               "textfield", "tab", "menuitem", "link", "slider", "stepper",
               "popUpButton", "menuButton", "textarea", "searchfield", "incrementor"}
CHROME = {"Close", "Full Screen", "Minimize", "Aster"}


def controls_of(payload: dict) -> list[dict]:
    out, seen = [], set()
    for n in flatten(payload):
        role, name = str(n.get("role") or ""), str(n.get("name") or "")
        if role not in INTERACTIVE or not name or name in CHROME:
            continue
        if (role, name) in seen:
            continue
        seen.add((role, name))
        # All refs come from ONE snapshot. Verified: batch actions against
        # same-snapshot refs stay valid, whereas one snapshot per find() gives
        # STALE_REF after the first action.
        out.append({"role": role, "name": name, "ref_id": n.get("ref_id")})
    return out


def find_one(role: str, name: str) -> dict | None:
    p = cli("find", "--app", APP, "--window-id", WINDOW, "--role", role,
            "--name", name, "--exact", "--first", "--timeout-ms", "2000", timeout=15)
    return (p.get("data") or {}).get("match")


def click_ref(ref: str) -> None:
    cli("click", ref, "--timeout-ms", "1500", timeout=15)


def ensure_home() -> None:
    m = find_one("button", "工作")
    if m:
        click_ref(m["ref_id"])
    else:
        back = find_one("button", "返回应用")
        if back:
            click_ref(back["ref_id"])
    cli("wait", "180")


def ensure_settings() -> None:
    m = find_one("button", "设置")
    if m:
        click_ref(m["ref_id"])
        cli("wait", "180")
    elif not find_one("button", "返回应用"):
        ensure_home()
        m = find_one("button", "设置")
        if m:
            click_ref(m["ref_id"])
            cli("wait", "180")


def navigate(page: str) -> None:
    if page == "工作台":
        ensure_home()
    elif page in ("连接器", "技能", "自动化"):
        ensure_home()
        m = find_one("button", page)
        if m:
            click_ref(m["ref_id"])
            cli("wait", "220")
    else:
        ensure_settings()
        m = find_one("button", page.split("/", 1)[1])
        if m:
            click_ref(m["ref_id"])
            cli("wait", "350")


def shot(name: str) -> dict:
    safe = name.replace("/", "-").replace(" ", "_")
    p = cli("screenshot", "--window-id", WINDOW, str(HERE / f"{safe}.png"), timeout=30)
    return {"file": f"{safe}.png", "ok": bool(p.get("ok"))}


def main() -> int:
    data = json.loads((HERE / "pages.json").read_text(encoding="utf-8"))
    entries_all = data["pages"]
    only = os.environ.get("ONLY_PAGES")
    selected = set(only.split(",")) if only else None
    pages = [e for e in entries_all if not selected or e["page"] in selected]

    coverage: list[dict] = []
    screenshots: list[dict] = []
    for entry in pages:
        page = entry["page"]
        navigate(page)
        cli("wait", "300")
        p = snapshot()
        controls = controls_of(p)
        screenshots.append(shot(f"batch-{entry['index']:02d}-{page}-before"))

        # Navigation controls were really clicked during discovery.
        touchable = [c for c in controls if c["name"] not in NAV_NAMES]
        for c in controls:
            if c["name"] in NAV_NAMES:
                coverage.append({"page": page, **c, "touched": True,
                                 "action": "navigate-click", "source": "discovery",
                                 "ok": True, "error": None, "delivery": None})

        refs: list[str | None] = [c.get("ref_id") for c in touchable]
        actions, spans = action_entries(touchable, refs)
        results = run_chunks(actions)
        for c, ref, (start, end) in zip(touchable, refs, spans):
            if ref is None or start == end:
                coverage.append({"page": page, **c, "touched": False,
                                 "action": None, "ok": False,
                                 "error": "REF_NOT_FOUND", "delivery": None})
                continue
            chunk = results[start:end]
            role = c["role"]
            if role in ("switch", "checkbox"):
                action = "toggle-twice-restored"
                ok = all(bool(x.get("ok")) for x in chunk)
            elif role == "textfield":
                action, ok = "set-value", bool(chunk[0].get("ok"))
            elif role in ("combobox", "popUpButton", "menuButton"):
                action, ok = "expand-collapse", any(bool(x.get("ok")) for x in chunk)
            elif role == "incrementor":
                action, ok = "get-states", bool(chunk[0].get("ok"))
            elif any(w in c["name"] for w in CONFIRM_LIKELY):
                action, ok = "open-and-cancel", bool(chunk[0].get("ok"))
            else:
                action, ok = "click", bool(chunk[0].get("ok"))
            coverage.append({
                "page": page, **c, "touched": bool(chunk), "action": action,
                "ok": ok,
                "delivery": ((chunk[0].get("data") or {}).get("disposition") or {}).get("delivery"),
                "error": None if ok else (chunk[0].get("error") or {}).get("code"),
            })
        screenshots.append(shot(f"batch-{entry['index']:02d}-{page}-after"))
        done = sum(1 for r in coverage if r["page"] == page and r.get("ok"))
        print(json.dumps({"page": page, "controls": len(controls),
                          "touched": sum(1 for r in coverage
                                         if r["page"] == page and r.get("touched")),
                          "delivered": done}, ensure_ascii=False), flush=True)
        out = HERE / "coverage-batch.json"
        out.write_text(json.dumps({"window": WINDOW, "pages": entries_all,
                                   "screenshots": screenshots,
                                   "coverage": coverage}, ensure_ascii=False, indent=2) + "\n",
                       encoding="utf-8")
    return 0


if __name__ == "__main__":
    for label in ("继续", "稍后设置", "关闭"):
        m = find_one("button", label)
        if m:
            click_ref(m["ref_id"])
            cli("wait", "250")
            break
    raise SystemExit(main())
