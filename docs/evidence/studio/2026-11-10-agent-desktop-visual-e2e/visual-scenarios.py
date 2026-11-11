#!/usr/bin/env python3
"""Headless agent-desktop visual scenarios against the freshly built Aster window.

Every action goes through semantic AX delivery (find -> click -> verify). No
--headed, no focus-window, no coordinates, no mouse. Each scenario restores
the workbench so the next one starts from a known state.
"""
from __future__ import annotations

import json
import subprocess
import sys
from pathlib import Path

APP = "Aster"
WINDOW = sys.argv[1] if len(sys.argv) > 1 else "w-52438"
HERE = Path(__file__).resolve().parent
RESULTS: list[dict] = []


def cli(*args: str) -> dict:
    try:
        run = subprocess.run(
            ["agent-desktop", *args], capture_output=True, text=True, timeout=40
        )
        return json.loads(run.stdout)
    except Exception as error:  # observation failure is a scenario failure, not a crash
        return {"ok": False, "error": {"code": "HARNESS", "message": str(error)}}


def find(role: str, name: str | None = None, exact: bool = True) -> dict | None:
    args = ["find", "--app", APP, "--window-id", WINDOW, "--role", role]
    if name is not None:
        args += ["--name", name]
        if exact:
            args += ["--exact"]
    args += ["--first"]
    payload = cli(*args)
    return (payload.get("data") or {}).get("match") if payload.get("ok") else None


def click(name: str, role: str = "button") -> dict:
    match = find(role, name)
    if not match:
        return {"ok": False, "error": f"control not found: {name}"}
    return cli("click", match["ref_id"])


def snapshot() -> dict:
    # Conversation pages expose 100+ AX nodes; the default 3s deadline can
    # return an empty tree. Retry once with the same observation command.
    args = ["snapshot", "--app", APP, "--window-id", WINDOW, "-i", "--compact", "--timeout-ms", "15000"]
    payload = cli(*args)
    if not payload.get("ok") or not (payload.get("data") or {}).get("tree"):
        payload = cli(*args)
    return payload


def names(payload: dict) -> set[str]:
    out: set[str] = set()

    def walk(node):
        if not isinstance(node, dict):
            return
        if node.get("name"):
            out.add(str(node["name"]))
        for child in node.get("children") or []:
            walk(child)

    walk((payload.get("data") or {}).get("tree"))
    return out


def shot(filename: str) -> dict:
    return cli("screenshot", "--window-id", WINDOW, str(HERE / filename))


def restore_to_workbench() -> bool:
    """Return to the workbench between scenarios. Never changes focus."""
    for _ in range(7):
        visible = names(snapshot())
        if "新建任务" in visible and any("描述任务" in v for v in visible):
            return True
        acted = False
        for label in ("关闭", "返回应用"):
            match = find("button", label)
            if match:
                cli("click", match["ref_id"])
                acted = True
                break
        if not acted:
            # A collapsed sidebar hides the workbench controls; expanding it is
            # part of cleanup, not a product assertion.
            switch = find("switch")
            if switch:
                cli("click", switch["ref_id"])
                acted = True
        if not acted:
            work = find("button", "工作")
            if work:
                cli("click", work["ref_id"])
                acted = True
        if not acted:
            return False
    return False


def scenario(sid: str, title: str, body) -> None:
    entry = {"id": sid, "title": title, "steps": [], "status": "pass"}
    try:
        body(entry)
    except Exception as error:
        entry["status"] = "fail"
        entry["steps"].append({"error": f"{type(error).__name__}: {error}"})
    finally:
        entry["restored"] = restore_to_workbench()
        RESULTS.append(entry)
        print(f"{entry['status'].upper():4} {sid} {title} ({len(entry['steps'])} steps, restored={entry['restored']})")


def check(entry: dict, label: str, condition: bool, detail: str = "") -> None:
    entry["steps"].append({"check": label, "ok": bool(condition), "detail": detail})
    if not condition:
        raise AssertionError(f"{label}: {detail}")


def s01(entry):
    snap = snapshot()
    n = (snap.get("data") or {}).get("ref_count", 0)
    visible = names(snap)
    check(entry, "AX 树完整", snap.get("ok") and (snap.get("data") or {}).get("complete") is True)
    check(entry, "工作页主界面", {"工作", "设置", "新建任务", "搜索任务"} <= visible, f"ref_count={n}")
    check(entry, "composer 可访问", any("描述任务" in v for v in visible), f"ref_count={n}")
    check(entry, "无首启运行时模态", not any("正在准备组件" in v or "重试安装" in v for v in visible))
    entry["steps"].append({"observation": "ref_count", "value": n})
    entry["steps"].append({"screenshot": shot("work-home.png").get("ok")})


def s02(entry):
    open_settings = click("设置")
    check(entry, "语义点击设置", open_settings.get("ok") is True)
    snap = snapshot()
    visible = names(snap)
    check(entry, "设置侧栏出现", {"模型配置", "用量", "快捷键"} <= visible, f"ref_count={(snap.get('data') or {}).get('ref_count')}")
    check(entry, "无 API 网关残留页", not any("API 网关" in v or "本地网关" in v for v in visible))
    entry["steps"].append({"screenshot": shot("settings-general.png").get("ok")})
    back = click("返回应用")
    check(entry, "返回应用", back.get("ok") is True)
    check(entry, "回到工作页", "搜索任务" in names(snapshot()))


def s03(entry):
    opened = click("搜索任务")
    check(entry, "打开搜索对话框", opened.get("ok") is True)
    snap = snapshot()
    visible = names(snap)
    # -i snapshots expose interactive nodes only; the empty-state caption is
    # static text, so it is verified visually in the screenshot instead.
    check(entry, "对话框可关闭且隔离主界面", "关闭" in visible and "新建任务" not in visible, f"refs={(snap.get('data') or {}).get('ref_count')}")
    entry["steps"].append({"screenshot": shot("search-dialog.png").get("ok")})
    closed = click("关闭")
    check(entry, "关闭对话框", closed.get("ok") is True)
    check(entry, "工作页恢复", "搜索任务" in names(snapshot()))


def s04(entry):
    opened = click("你有什么技能？")
    check(entry, "打开任务会话", opened.get("ok") is True)
    snap = snapshot()
    refs = (snap.get("data") or {}).get("ref_count", 0)
    visible = names(snap)
    check(entry, "会话页加载消息流", snap.get("ok") is True and (refs > 20 or len(visible) > 20), f"ref_count={refs}, names={len(visible)}")
    entry["steps"].append({"screenshot": shot("task-conversation.png").get("ok")})
    # Cleanup is owned by restore_to_workbench(); record the navigation click
    # as an observation rather than asserting on harness cleanup.
    entry["steps"].append({"observation": "return_click_ok", "value": click("工作").get("ok")})


def s05(entry):
    # The trigger exposes either the display name or the model id depending on
    # which workbench surface is mounted; match on the stable model token.
    match = find("combobox", "DeepSeek Flash") or find("combobox", "deepseek-flash")
    if not match:
        nodes: list[dict] = []

        def walk(node):
            if not isinstance(node, dict):
                return
            nodes.append(node)
            for child in node.get("children") or []:
                walk(child)

        walk((snapshot().get("data") or {}).get("tree"))
        for node in nodes:
            label = str(node.get("name") or "").lower()
            if node.get("role") == "combobox" and ("deepseek" in label or "flash" in label):
                match = {"ref_id": node["ref_id"]}
                break
    check(entry, "模型选择器触发存在", match is not None)
    opened = cli("click", match["ref_id"])
    check(entry, "打开模型选择器", opened.get("ok") is True)
    snap = snapshot()
    entry["steps"].append({"observation": "ref_count", "value": (snap.get("data") or {}).get("ref_count")})
    entry["steps"].append({"screenshot": shot("model-selector.png").get("ok")})
    check(entry, "选择器弹出后有可选模型", len(names(snap)) > 10)
    # Restore without a key event: the trigger stays addressable in the AX tree.
    trigger = find("combobox", "DeepSeek Flash")
    if trigger:
        cli("click", trigger["ref_id"])


def s06(entry):
    switch = find("switch", "收起侧边栏")
    check(entry, "侧栏开关存在", switch is not None)
    click_result = cli("click", switch["ref_id"])
    check(entry, "切换侧栏", click_result.get("ok") is True)
    collapsed = names(snapshot())
    check(entry, "收起后导航控件消失", "新建任务" not in collapsed)
    entry["steps"].append({"screenshot": shot("sidebar-collapsed.png").get("ok")})
    # The collapsed switch may expose a different accessible name; address the
    # first switch on the workbench rather than assuming the label is stable.
    restored = find("switch")
    if restored:
        cli("click", restored["ref_id"])
    check(entry, "恢复后导航控件回来", "新建任务" in names(snapshot()))


def s07(entry):
    snap = snapshot()
    visible = names(snap)
    forbidden = [v for v in visible if "Runtime plugin" in v or "pin identity" in v or "正在准备组件" in v or "重试安装" in v]
    check(entry, "无运行时组件错误态", not forbidden, f"matched={forbidden}")
    check(entry, "窗口仍可访问", "搜索任务" in visible or "描述任务" in " ".join(visible))


if __name__ == "__main__":
    restore_to_workbench()
    for fn in (s01, s02, s03, s04, s05, s06, s07):
        scenario(fn.__name__.upper(), fn.__doc__ or fn.__name__, fn)
    report = {
        "app": APP,
        "windowId": WINDOW,
        "delivery": "agent-desktop-headless-semantic",
        "scenarios": RESULTS,
        "counts": {
            "pass": sum(r["status"] == "pass" for r in RESULTS),
            "fail": sum(r["status"] == "fail" for r in RESULTS),
            "skipped": 0,
            "notExecuted": 0,
        },
    }
    (HERE / "scenarios.json").write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n", "utf8")
    print(json.dumps(report["counts"]))
