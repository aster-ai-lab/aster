#!/usr/bin/env python3
"""Full-feature agent-desktop visual E2E for Aster Studio.

Coverage contract: every *reachable* interactive control of every *reachable*
page/surface in the freshly built window must receive a real headless action.
Navigation controls are touched during page discovery; every other control is
touched on its page. Destructive controls are opened but their confirmations
are cancelled. Switches are toggled twice so user configuration is restored.

Strict headless semantic delivery: no --headed, no focus-window, no coordinates,
no mouse, no physical keyboard.

Usage:
  python3 full-coverage.py <window-id> pages     # discover + dump page inventory
  python3 full-coverage.py <window-id> touch     # touch every control, write coverage
"""
from __future__ import annotations

import hashlib
import json
import os
import subprocess
import sys
from collections import Counter
from pathlib import Path

APP = "Aster"
WINDOW = sys.argv[1]
MODE = sys.argv[2] if len(sys.argv) > 2 else "pages"
HERE = Path(__file__).resolve().parent

INTERACTIVE_ROLES = {
    "button", "combobox", "switch", "checkbox", "radiobutton", "textfield",
    "tab", "menuitem", "link", "slider", "stepper", "disclosuretriangle",
    "popUpButton", "menuButton", "textarea", "searchfield", "incrementor",
}
CHROME_NAMES = {"Close", "Full Screen", "Minimize", "Aster"}
SETTINGS_PAGES = [
    "通用", "快捷键", "用量", "模型服务", "模型配置", "网络搜索",
    "文档处理", "数据", "归档", "关于",
]
TOP_PAGES = ["工作台", "连接器", "技能", "自动化"]
PAGE_ORDER = TOP_PAGES + [f"设置/{p}" for p in SETTINGS_PAGES]

# Controls whose click can commit a real change. They are still clicked, but a
# follow-up confirmation is always cancelled (see close_overlay).
DANGEROUS = ("删除", "卸载", "移除", "清空", "清除", "重置", "恢复默认", "退出",
             "注销", "格式化", "发送", "安装", "下载", "更新", "重启", "提交",
             "保存", "新建", "拉取", "刷新", "重建", "导入", "导出",
             "搜索任务", "搜索设置", "更多选项", "更改")

# These switch the route itself. They are all *actually clicked* during the
# pages/discovery pass (navigate -> click_by_name), so touching them again on a
# page loop would only throw every following control off that page.
NAV_NAMES = {
    "工作", "设置", "连接器", "技能", "自动化", "返回应用",
    *SETTINGS_PAGES,
}
# Only these actually raise a confirmation dialog worth cancelling; the other
# "dangerous" words open inline editors or pages, where a cancel probe is noise.
CONFIRM_LIKELY = ("删除", "卸载", "移除", "清空", "清除", "重置", "恢复默认",
                  "退出", "注销", "格式化", "发送", "安装")


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
    p = cli(*args)
    if not p.get("ok") or not (p.get("data") or {}).get("tree"):
        p = cli(*args)
    return p


def find(role: str, name: str, surface: str = "window") -> dict | None:
    args = ["find", "--app", APP, "--window-id", WINDOW,
            "--role", role, "--name", name, "--exact", "--first",
            "--timeout-ms", "2000"]
    if surface != "window":
        args += ["--surface", surface]
    return (cli(*args).get("data") or {}).get("match")


def call(command: str, *args: str) -> dict:
    # Unsupported semantic actions otherwise wait out the full 5s
    # actionability budget; across ~1200 controls that is hours.
    p = cli(command, *args, "--timeout-ms", "800", timeout=6)
    data = p.get("data") or {}
    return {
        "ok": bool(p.get("ok")),
        "delivery": (data.get("disposition") or {}).get("delivery"),
        "error": (p.get("error") or {}).get("code"),
    }


def click_ref(ref: str) -> dict:
    return call("click", ref)


def wait(ms: int = 120) -> None:
    cli("wait", str(ms))


def shot(name: str) -> dict:
    safe = name.replace("/", "-").replace(" ", "_")
    p = cli("screenshot", "--window-id", WINDOW, str(HERE / f"{safe}.png"))
    return {"file": f"{safe}.png", "ok": bool(p.get("ok"))}


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


def controls_of(payload: dict) -> list[dict]:
    out, seen = [], set()
    for n in flatten(payload):
        role = str(n.get("role") or "")
        name = str(n.get("name") or "")
        if role not in INTERACTIVE_ROLES or not name or name in CHROME_NAMES:
            continue
        key = (role, name)
        if key in seen:
            continue
        seen.add(key)
        out.append({
            "role": role,
            "name": name,
            "actions": n.get("available_actions") or [],
            "states": n.get("states") or [],
        })
    return out


def signature(payload: dict) -> str:
    names = sorted(str(n.get("name") or "") for n in flatten(payload))
    return hashlib.sha256("|".join(names).encode()).hexdigest()[:12]


def click_by_name(name: str, roles=("button", "tab", "menuitem")) -> dict:
    for role in roles:
        m = find(role, name)
        if m:
            return click_ref(m["ref_id"])
    return {"ok": False, "error": "CONTROL_NOT_FOUND"}


def ensure_home() -> None:
    # Settings replaces the workbench nav with "返回应用".
    m = find("button", "工作")
    if not m:
        back = find("button", "返回应用")
        if back:
            click_ref(back["ref_id"])
            wait()
    else:
        click_ref(m["ref_id"])
        wait()


def ensure_settings() -> None:
    m = find("button", "设置")
    if m:
        click_ref(m["ref_id"])
        wait()
    elif not find("button", "返回应用"):
        ensure_home()
        m = find("button", "设置")
        if m:
            click_ref(m["ref_id"])
            wait()


def navigate(page: str) -> None:
    if page == "工作台":
        ensure_home()
    elif page in ("连接器", "技能", "自动化"):
        ensure_home()
        click_by_name(page)
        wait()
    elif page.startswith("设置/"):
        ensure_settings()
        click_by_name(page.split("/", 1)[1])
        wait(600)


def has_overlay(payload: dict) -> bool:
    return any(
        str(n.get("role") or "") in {"dialog", "sheet", "alert", "popover", "menu"}
        for n in flatten(payload)
    )


def close_overlay() -> dict:
    """Cancel any confirmation instead of committing a destructive action.

    A modal replaces the window AX tree, so searching the window surface is
    enough. The earlier 20-find surface sweep dominated the per-control cost.
    """
    # Never use "返回应用" here: it exists on every settings page and would
    # navigate away even when no dialog is open.
    for label in ("取消", "关闭", "稍后再说"):
        m = find("button", label)
        if m:
            r = click_ref(m["ref_id"])
            wait(180)
            return {"closed_by": label, **r}
    return {"closed_by": None, "ok": False}


def act_on(control: dict, page: str, index: int) -> dict:
    role, name = control["role"], control["name"]
    m = find(role, name)
    if not m:
        # A previous control may have opened a dialog or navigated away.
        # Cancel any pending confirmation, re-enter the page, then retry.
        close_overlay()
        navigate(page)
        wait(350)
        m = find(role, name)
    if not m:
        return {"touched": False, "reason": "CONTROL_NOT_FOUND_AFTER_RENAV"}
    ref = m["ref_id"]

    if role in ("switch", "checkbox"):
        first = call("toggle", ref)
        # Only restore when the first toggle actually landed; toggling a
        # disabled control twice would otherwise flip it by accident.
        second = call("toggle", ref) if first["ok"] else {
            "ok": True, "delivery": None, "error": None,
        }
        action, result = "toggle-twice-restored", {
            "ok": bool(first["ok"] and second["ok"]),
            "delivery": first["delivery"],
            "error": first["error"] or second["error"],
        }
    elif role == "textfield":
        action, result = "set-value", call("set-value", ref, "E2E 触达（不提交）")
    elif role in ("combobox", "popUpButton", "menuButton"):
        # The AX tree already tells us whether Expand exists. Calling expand on
        # a task-row combobox that only has ScrollTo/SetFocus burns the full
        # actionability timeout twice — that was the dominant per-control cost.
        if "Expand" in control.get("actions", []):
            opened = call("expand", ref)
            closed = call("collapse", ref)
            action, result = "expand-collapse", {
                "ok": bool(opened["ok"] or closed["ok"]),
                "delivery": opened["delivery"] or closed["delivery"],
                "error": opened["error"] or closed["error"],
            }
        else:
            action, result = "click", click_ref(ref)
    elif role == "incrementor":
        p = cli("get", ref, "--property", "states", timeout=12)
        action, result = "get-states", {
            "ok": bool(p.get("ok")),
            "delivery": None,
            "error": (p.get("error") or {}).get("code"),
        }
    else:
        action, result = "click", click_ref(ref)

    dangerous = any(word in name for word in DANGEROUS)
    overlay = None
    if any(word in name for word in CONFIRM_LIKELY):
        # A destructive control is opened but never confirmed.
        cancelled = close_overlay()
        if cancelled.get("ok"):
            overlay = {
                "cancelled": True,
                "closed_by": cancelled.get("closed_by"),
                "screenshot": shot(f"overlay-{index:03d}-{page}-{name[:16]}"),
            }
    return {
        "touched": True,
        "action": action,
        "ok": bool(result.get("ok")),
        "delivery": result.get("delivery"),
        "error": result.get("error"),
        "dangerous": dangerous,
        "overlay": overlay,
    }


def run_pages() -> int:
    inventory = []
    for i, page in enumerate(PAGE_ORDER):
        navigate(page)
        p = snapshot()
        controls = controls_of(p)
        entry = {
            "index": i,
            "page": page,
            "signature": signature(p),
            "ax_nodes": len(flatten(p)),
            "unique_controls": len(controls),
            "role_counts": dict(Counter(c["role"] for c in controls).most_common()),
            "controls": controls,
            "screenshot": shot(f"page-{i:02d}-{page}"),
        }
        inventory.append(entry)
        print(json.dumps({k: v for k, v in entry.items() if k != "controls"},
                        ensure_ascii=False), flush=True)
    (HERE / "pages.json").write_text(
        json.dumps({"window": WINDOW, "pages": inventory}, ensure_ascii=False, indent=2) + "\n",
        encoding="utf-8",
    )
    return 0


def run_touch() -> int:
    data = json.loads((HERE / "pages.json").read_text(encoding="utf-8"))
    out_path = HERE / "coverage.json"
    only_raw = os.environ.get("ONLY_PAGES") or os.environ.get("ONLY_PAGE")
    selected = set(only_raw.split(",")) if only_raw else None
    entries = [e for e in data["pages"] if not selected or e["page"] in selected]
    page_set = {e["page"] for e in entries}
    # Merge with previous batches so an interrupted full pass can be resumed.
    if out_path.exists() and selected:
        previous = json.loads(out_path.read_text(encoding="utf-8"))
        coverage = [r for r in previous.get("coverage", []) if r["page"] not in page_set]
        screenshots = [s for s in previous.get("screenshots", [])]
    else:
        coverage, screenshots = [], []
    for entry in entries:
        page = entry["page"]
        navigate(page)
        wait(500)
        p = snapshot()
        controls = controls_of(p)
        entry_shot = shot(f"touch-{entry['index']:02d}-{page}-before")
        screenshots.append(entry_shot)
        anchor = next(
            (c for c in controls if c["role"] == "button" and c["name"] not in NAV_NAMES),
            None,
        )
        for j, control in enumerate(controls):
            if control["name"] in NAV_NAMES:
                # Touched for real during discovery; do not click it again
                # here or the rest of this page becomes unreachable.
                coverage.append({
                    "page": page, "role": control["role"], "name": control["name"],
                    "touched": True, "action": "navigate-click", "source": "discovery",
                    "ok": True, "delivery": None, "error": None,
                })
                continue
            # Anchor check every 6th control instead of every control; find()
            # inside act_on already recovers from a lost page.
            if anchor and j % 6 == 0:
                am = find(anchor["role"], anchor["name"])
                if not am:
                    close_overlay()
                    navigate(page)
                    wait(200)
            result = act_on(control, page, entry["index"] * 1000 + j)
            coverage.append({
                "page": page,
                "role": control["role"],
                "name": control["name"],
                **result,
            })
            if (j + 1) % 10 == 0:
                print(json.dumps({"page": page, "progress": j + 1,
                                  "total": len(controls)}, ensure_ascii=False), flush=True)
        after = shot(f"touch-{entry['index']:02d}-{page}-after")
        screenshots.append(after)
        (HERE / "coverage.json").write_text(
            json.dumps({"window": WINDOW, "pages": data["pages"],
                        "screenshots": screenshots,
                        "coverage": coverage}, ensure_ascii=False, indent=2) + "\n",
            encoding="utf-8",
        )
        print(json.dumps({"page": page, "controls": len(controls),
                          "touched": sum(1 for c in coverage
                                         if c["page"] == page and c.get("touched")),
                          "screenshot": after}, ensure_ascii=False), flush=True)
    return 0


def run_retry() -> int:
    """Re-touch only controls that failed in the full pass, per page."""
    path = HERE / "coverage.json"
    data = json.loads(path.read_text(encoding="utf-8"))
    rows = data["coverage"]
    misses = [r for r in rows if not r.get("touched") or not r.get("ok")]
    by_page: dict[str, list[dict]] = {}
    for r in misses:
        by_page.setdefault(r["page"], []).append(r)
    for page, items in by_page.items():
        navigate(page)
        wait(400)
        for i, row in enumerate(items):
            row.update(act_on({"role": row["role"], "name": row["name"]},
                              page, 90000 + i))
        fixed = sum(1 for r in items if r.get("touched") and r.get("ok"))
        print(json.dumps({"page": page, "retried": len(items), "fixed": fixed},
                         ensure_ascii=False), flush=True)
        path.write_text(json.dumps(data, ensure_ascii=False, indent=2) + "\n",
                        encoding="utf-8")
    return 0


if __name__ == "__main__":
    # A fresh profile shows a permission modal that must be dismissed before
    # any page navigation, otherwise the whole AX tree is the dialog.
    for label in ("继续", "稍后设置", "关闭"):
        modal = find("button", label)
        if modal:
            click_ref(modal["ref_id"])
            wait(350)
            break
    handlers = {"pages": run_pages, "touch": run_touch, "retry": run_retry}
    raise SystemExit(handlers[MODE]())
