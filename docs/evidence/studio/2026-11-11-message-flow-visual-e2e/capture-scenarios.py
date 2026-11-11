#!/usr/bin/env python3
"""Capture render variants of the task-conversation message flow in the real Aster window.

All delivery is headless semantic AX (find -> click -> verify). No --headed,
no focus-window, no coordinates, no mouse, no key events.
"""
from __future__ import annotations

import json
import subprocess
import sys
from pathlib import Path

APP = "Aster"
WINDOW = sys.argv[1] if len(sys.argv) > 1 else "w-56826"
HERE = Path(__file__).resolve().parent
SCREENS = HERE / "screens"
RESULTS: list[dict] = []


def cli(*args: str) -> dict:
    try:
        run = subprocess.run(["agent-desktop", *args], capture_output=True, text=True, timeout=45)
        return json.loads(run.stdout)
    except Exception as error:
        return {"ok": False, "error": {"code": "HARNESS", "message": str(error)}}


def find(role: str, name: str | None = None, exact: bool = True, contains: bool = False) -> dict | None:
    args = ["find", "--app", APP, "--window-id", WINDOW, "--role", role]
    if name is not None:
        args += ["--name", name]
        if exact and not contains:
            args += ["--exact"]
    args += ["--first"]
    payload = cli(*args)
    return (payload.get("data") or {}).get("match") if payload.get("ok") else None


def find_like(role: str, token: str) -> dict | None:
    nodes: list[dict] = []

    def walk(node):
        if not isinstance(node, dict):
            return
        nodes.append(node)
        for child in node.get("children") or []:
            walk(child)

    walk((cli("snapshot", "--app", APP, "--window-id", WINDOW, "-i", "--compact", "--timeout-ms", "15000").get("data") or {}).get("tree"))
    for node in nodes:
        label = str(node.get("name") or "")
        if node.get("role") == role and token.lower() in label.lower() and node.get("ref_id"):
            return node
    return None


def click_ref(ref: str) -> bool:
    return cli("click", ref).get("ok") is True


def summary() -> dict:
    payload = cli("snapshot", "--app", APP, "--window-id", WINDOW, "-i", "--compact", "--timeout-ms", "15000")
    nodes: list[dict] = []

    def walk(node):
        if not isinstance(node, dict):
            return
        nodes.append(node)
        for child in node.get("children") or []:
            walk(child)

    walk((payload.get("data") or {}).get("tree"))
    names = [str(n.get("name")) for n in nodes if n.get("name")]
    roles: dict[str, int] = {}
    for node in nodes:
        role = str(node.get("role") or "")
        roles[role] = roles.get(role, 0) + 1
    return {
        "ok": payload.get("ok"),
        "ref_count": (payload.get("data") or {}).get("ref_count"),
        "named_nodes": len(names),
        "roles": roles,
        "sample": names[:28],
    }


def shot(name: str) -> bool:
    return cli("screenshot", "--window-id", WINDOW, str(SCREENS / f"{name}.png")).get("ok") is True


def restore() -> bool:
    for _ in range(4):
        state = summary()
        if "新建任务" in state["sample"] and "描述任务" in " ".join(state["sample"]):
            return True
        acted = False
        for label in ("关闭", "返回应用", "工作"):
            match = find("button", label)
            if match:
                click_ref(match["ref_id"])
                acted = True
                break
        if not acted:
            switch = find("switch")
            if switch:
                click_ref(switch["ref_id"])
                acted = True
        if not acted:
            return False
    return False


def case(cid: str, title: str, covers: list[str], action=None) -> None:
    entry = {"id": cid, "title": title, "covers": covers, "status": "pass"}
    try:
        if action:
            entry["action_ok"] = action()
        entry["before"] = summary()
        entry["screenshot"] = shot(cid)
        if not entry["screenshot"]:
            entry["status"] = "fail"
            entry["error"] = "screenshot failed"
    except Exception as error:
        entry["status"] = "fail"
        entry["error"] = f"{type(error).__name__}: {error}"
    finally:
        entry["restored"] = restore()
        RESULTS.append(entry)
        print(f"{entry['status'].upper():4} {cid:28} refs={entry.get('before',{}).get('ref_count')} shot={entry.get('screenshot')} restored={entry['restored']}")


def click_named(token: str, role: str = "button") -> bool:
    match = find_like(role, token)
    return click_ref(match["ref_id"]) if match else False


if __name__ == "__main__":
    case("01-transcript-overview", "任务会话全景（成果叙事 + 执行轨迹同屏）",
         ["R06", "R19", "R21", "R23", "B02", "B10", "B32", "L10", "L16", "S14", "F01-F20"])

    case("02-reasoning-expanded", "思考过程展开", ["B03", "B04", "C3-2"],
         lambda: click_named("思考过程"))

    case("03-tool-group-failed", "工具聚合组：17 次调用 1 失败", ["R23", "C3-4", "C3-8", "C3-9"],
         lambda: click_named("17 次工具调用"))

    case("04-tool-group-multi-fail", "工具聚合组：14 次调用 3 失败", ["R23", "C3-4", "C3-8"],
         lambda: click_named("14 次工具调用"))

    case("05-tool-group-clean", "工具聚合组：12 次调用无失败", ["R21", "C3-7"],
         lambda: click_named("12 次工具调用 1.9s"))

    case("06-explore-steps", "探索工作区 7 个步骤", ["R23", "R25"],
         lambda: click_named("探索工作区"))

    case("07-more-actions", "消息行「更多操作」菜单", ["A9", "A11", "A12", "A13"],
         lambda: click_named("更多操作", "combobox"))

    case("08-model-picker", "模型选择器弹层", ["C06", "F04"],
         lambda: click_named("deepseek-flash", "combobox"))

    case("09-permission-mode", "授权模式选择", ["C12", "E18"],
         lambda: click_named("授权模式", "combobox"))

    case("10-composer-more-options", "Composer 更多选项", ["C05", "F11"],
         lambda: click_named("更多选项", "combobox"))

    case("11-search-dialog", "搜索任务对话框", ["L15", "F16"],
         lambda: click_named("搜索任务"))

    case("12-turn-navigation", "轮次跳转锚点", ["L10", "R31"],
         lambda: click_named("跳转到第 1 轮对话"))

    case("13-workspace-pane", "打开任务工作区右侧面板", ["S16", "P1"],
         lambda: click_named("打开任务工作区"))

    case("14-workbench-shortcuts", "工作区快捷菜单", ["S16"],
         lambda: click_named("展开工作区快捷菜单"))

    case("15-settings", "设置页与左侧导航", ["P16", "R01"],
         lambda: click_named("设置"))

    case("16-new-task", "新建任务空态", ["L1", "R17"],
         lambda: click_named("新建任务"))

    report = {
        "app": APP,
        "windowId": WINDOW,
        "delivery": "agent-desktop-headless-semantic",
        "counts": {
            "total": len(RESULTS),
            "pass": sum(r["status"] == "pass" for r in RESULTS),
            "fail": sum(r["status"] == "fail" for r in RESULTS),
            "screenshots": sum(bool(r.get("screenshot")) for r in RESULTS),
        },
        "cases": RESULTS,
    }
    (HERE / "captures.json").write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n", "utf8")
    print(json.dumps(report["counts"]))
