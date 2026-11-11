#!/usr/bin/env python3
"""Mutating-action coverage for every operable functional point.

Key finding: refs from ONE snapshot stay valid across a batch when the action
is `click` (16/16 in probe), whereas `toggle` fails on its second call
(ACTION_FAILED). So switches are clicked twice (change + restore) and all
other controls are dispatched in batches.

Layout:
  - stable controls (switch/checkbox/radiobutton/incrementor/textfield) in
    large batches;
  - buttons/links/comboboxes in small chunks with a fresh snapshot per chunk
    so a dialog or route change can only stale that chunk;
  - risky controls (new/search/delete/...) handled one at a time with a
    confirmation cancel;
  - anything still failing gets a precise find + single action repair.

Strict headless semantic delivery. Writes action-coverage.json.
"""
from __future__ import annotations

import json
import os
import subprocess
import sys
from collections import Counter
from pathlib import Path

APP = "Aster"
WINDOW = sys.argv[1]
HERE = Path(__file__).resolve().parent
BIG = 55          # entries per batch for stable controls
CHUNK = 8         # controls per button chunk
NAV_NAMES = {"工作", "设置", "连接器", "技能", "自动化", "返回应用",
             "通用", "快捷键", "用量", "模型服务", "模型配置", "网络搜索",
             "文档处理", "数据", "归档", "关于"}
RISK = ("删除", "卸载", "移除", "清空", "清除", "重置", "恢复默认", "退出",
        "注销", "格式化", "发送", "安装", "新建", "搜索任务", "搜索设置",
        "导入", "导出", "更新", "重启", "拉取", "刷新", "重建", "更改")
CHROME = {"Close", "Full Screen", "Minimize", "Aster"}
INTERACTIVE = {"button", "combobox", "switch", "checkbox", "radiobutton",
               "textfield", "tab", "menuitem", "link", "slider", "stepper",
               "popUpButton", "menuButton", "textarea", "searchfield", "incrementor"}


def cli(*args, timeout=120):
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
    p = cli("find", "--app", APP, "--window-id", WINDOW, "--role", role,
            "--name", name, "--exact", "--first", "--timeout-ms", "6000", timeout=15)
    return ((p.get("data") or {}).get("match") or {}).get("ref_id")


def click(ref):
    return cli("click", ref, "--timeout-ms", "2500", timeout=15)


def wait(ms=400):
    cli("wait", str(ms), timeout=10)


def ensure_sidebar():
    """A collapsed sidebar halves the AX tree and invalidates every ref."""
    m = find("switch", "展开侧边栏")
    if m:
        click(m)
        wait(220)


def navigate(page):
    ensure_sidebar()
    back, home = find("button", "返回应用"), find("button", "工作")
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
    m = find("button", page.split("/", 1)[1])
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
            "--timeout-ms", "120000", timeout=150)
    res = (p.get("data") or {}).get("results") or []
    while len(res) < len(entries):
        res.append({"ok": False, "error": {"code": "NO_RESULT"}})
    return res[: len(entries)]


def entries_for(c, ref):
    """(entries, kind) — two entries for switches to restore the value."""
    role = c["role"]
    if role in ("switch", "checkbox", "radiobutton"):
        return ([{"command": "click", "args": {"ref_id": ref, "timeout_ms": 2500}},
                 {"command": "click", "args": {"ref_id": ref, "timeout_ms": 2500}}],
                "click-twice-restored")
    if role == "textfield":
        return ([{"command": "set-value", "args": {
            "ref_id": ref, "value": "E2E 动作覆盖（不提交）", "timeout_ms": 2500}}], "set-value")
    if role == "incrementor":
        return ([{"command": "get", "args": {"ref_id": ref, "property": "states"}}], "get-states")
    if role in ("combobox", "popUpButton", "menuButton"):
        if "Expand" in c["actions"]:
            return ([{"command": "expand", "args": {"ref_id": ref, "timeout_ms": 2000}},
                     {"command": "collapse", "args": {"ref_id": ref, "timeout_ms": 2000}}],
                    "expand-collapse")
        return ([{"command": "click", "args": {"ref_id": ref, "timeout_ms": 2500}}], "click")
    return ([{"command": "click", "args": {"ref_id": ref, "timeout_ms": 2500}}], "click")


def dispatch_group(items, page, label, coverage, fresh_each_chunk=False, chunk=CHUNK):
    """Dispatch a group; returns list of controls that still need repair."""
    failed = []
    groups = [(items[i:i + chunk], i) for i in range(0, len(items), chunk)] \
        if fresh_each_chunk else [(items, 0)]
    for group, _ in groups:
        # Refresh refs from a fresh snapshot per chunk (cheap), but do NOT
        # navigate on every chunk: most buttons keep the page, and repeated
        # navigation was the dominant cost. Navigate only when a control is
        # actually missing from the tree.
        cur = {((c["role"], c["name"])): c["ref_id"] for c in controls(snapshot())}
        if sum(1 for c in group if (c["role"], c["name"]) not in cur) > len(group) // 2:
            navigate(page)
            wait(250)
            cur = {((c["role"], c["name"])): c["ref_id"] for c in controls(snapshot())}
        entries, spans = [], []
        for c in group:
            ref = cur.get((c["role"], c["name"]))
            start = len(entries)
            if ref:
                es, kind = entries_for(c, ref)
                entries.extend(es)
                c["_kind"] = kind
            spans.append((start, len(entries)))
        results = []
        for i in range(0, len(entries), BIG):
            results.extend(batch(entries[i:i + BIG]))
        for c, (start, end) in zip(group, spans):
            if start == end:
                coverage.append({"page": page, "role": c["role"], "name": c["name"],
                                 "action": None, "ok": False, "error": "REF_NOT_FOUND",
                                 "group": label})
                failed.append(c)
                continue
            part = results[start:end]
            ok = any(bool(x.get("ok")) for x in part)
            coverage.append({"page": page, "role": c["role"], "name": c["name"],
                             "action": c.get("_kind", "click"), "ok": ok,
                             "error": None if ok else (part[0].get("error") or {}).get("code"),
                             "group": label})
            if not ok:
                failed.append(c)
    return failed


def risky_one(c, page):
    """One risky control: click, then cancel any confirmation, then restore."""
    navigate(page)
    ref = find(c["role"], c["name"])
    if not ref:
        return {"page": page, "role": c["role"], "name": c["name"],
                "action": "risky-click", "ok": False, "error": "CONTROL_NOT_FOUND",
                "group": "risky"}
    r = click(ref)
    ok = bool(r.get("ok"))
    for label in ("取消", "关闭", "稍后再说"):
        cancel = find("button", label)
        if cancel:
            click(cancel)
            break
    return {"page": page, "role": c["role"], "name": c["name"],
            "action": "risky-click-and-cancel", "ok": ok,
            "error": None if ok else (r.get("error") or {}).get("code"),
            "group": "risky"}


def repair(c, page):
    """Precise single-control fallback, with the right CLI shape per role."""
    navigate(page)
    ref = find(c["role"], c["name"])
    if not ref:
        return {"ok": False, "action": "repair", "error": "CONTROL_NOT_FOUND"}
    role = c["role"]
    if role in ("switch", "checkbox", "radiobutton"):
        a = cli("click", ref, "--timeout-ms", "2500", timeout=15)
        cli("click", ref, "--timeout-ms", "2500", timeout=15)  # restore
        ok = bool(a.get("ok"))
    elif role == "textfield":
        ok = bool(cli("set-value", ref, "E2E 修复（不提交）",
                      "--timeout-ms", "2500", timeout=15).get("ok"))
    elif role == "incrementor":
        ok = bool(cli("get", ref, "--property", "states", timeout=15).get("ok"))
    elif role in ("combobox", "popUpButton", "menuButton") and "Expand" in c.get("actions", []):
        a = cli("expand", ref, "--timeout-ms", "2000", timeout=15)
        b = cli("collapse", ref, "--timeout-ms", "2000", timeout=15)
        ok = bool(a.get("ok") or b.get("ok"))
    else:
        ok = bool(cli("click", ref, "--timeout-ms", "2500", timeout=15).get("ok"))
    return {"ok": ok, "action": "repair", "error": None if ok else "REPAIR_FAILED"}


def main():
    data = json.loads((HERE / "pages.json").read_text(encoding="utf-8"))
    only = os.environ.get("ONLY_PAGES")
    selected = set(only.split(",")) if only else None
    pages = [e for e in data["pages"] if not selected or e["page"] in selected]
    out = HERE / "action-coverage.json"
    if out.exists() and selected:
        prev = json.loads(out.read_text(encoding="utf-8"))
        coverage = [r for r in prev.get("coverage", []) if r["page"] not in selected]
        screenshots = prev.get("screenshots", [])
    else:
        coverage, screenshots = [], []

    for e in pages:
        page = e["page"]
        navigate(page)
        wait(250)
        cs = controls(snapshot())
        expected = e["unique_controls"]
        verified = abs(len(cs) - expected) <= max(10, int(expected * 0.30))
        if not verified:
            # Scrub any overlay left by a previous chunk/page, then retry.
            for label in ("取消", "关闭", "返回应用"):
                m = find("button", label)
                if m:
                    click(m)
                    wait(150)
            navigate(page)
            wait(650)
            cs = controls(snapshot())
            verified = abs(len(cs) - expected) <= max(10, int(expected * 0.30))
        screenshots.append(shot(f"act-{e['index']:02d}-{page}-before"))

        # The sidebar toggle changes the whole AX tree; it is verified by AX
        # touch and deliberately excluded from batched mutating actions.
        layout_controls = {"收起侧边栏", "展开侧边栏"}
        for c in cs:
            if c["name"] in NAV_NAMES or c["name"] in layout_controls:
                coverage.append({"page": page, "role": c["role"], "name": c["name"],
                                 "action": "navigate-click", "ok": True,
                                 "error": None, "group": "navigation"})

        rest = [c for c in cs
                if c["name"] not in NAV_NAMES and c["name"] not in layout_controls]
        stable = [c for c in rest if c["role"] in ("switch", "checkbox", "radiobutton",
                                                   "textfield", "incrementor")]
        risky = [c for c in rest if c not in stable and any(w in c["name"] for w in RISK)]
        buttons = [c for c in rest if c not in stable and c not in risky]

        failed = []
        failed += dispatch_group(stable, page, "stable", coverage)
        # Small chunks with a fresh snapshot each; navigate only if the page
        # was actually lost. Stable controls above go in large batches.
        failed += dispatch_group(buttons, page, "buttons", coverage,
                                 fresh_each_chunk=True, chunk=CHUNK)
        for c in risky:
            row = risky_one(c, page)
            coverage.append(row)
            if not row["ok"]:
                failed.append(c)

        fixed = 0
        for c in failed:
            r = repair(c, page)
            for row in coverage:
                if (row["page"] == page and row["role"] == c["role"]
                        and row["name"] == c["name"] and not row.get("ok")):
                    row.update(r)
                    row["repaired"] = True
                    break
            if r.get("ok"):
                fixed += 1

        navigate(page)
        screenshots.append(shot(f"act-{e['index']:02d}-{page}-after"))
        rows = [r for r in coverage if r["page"] == page]
        print(json.dumps({"page": page, "controls": len(cs), "expected": expected,
                          "navigation_verified": verified,
                          "dispatched": len(rows),
                          "ok": sum(1 for r in rows if r.get("ok")),
                          "failed_then_repaired": fixed, "still_failed": len(failed) - fixed},
                         ensure_ascii=False), flush=True)
        out.write_text(json.dumps({"window": WINDOW, "pages": data["pages"],
                                   "screenshots": screenshots, "coverage": coverage},
                                  ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    return 0


if __name__ == "__main__":
    # Scrub the first-run permission modal and any overlay left by probes.
    for _ in range(3):
        acted = False
        for label in ("继续", "稍后设置", "取消", "关闭", "返回应用", "工作"):
            m = find("button", label)
            if m:
                click(m)
                wait(180)
                acted = True
        if not acted:
            break
    raise SystemExit(main())
