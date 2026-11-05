#!/usr/bin/env python3
"""Native Aster scenarios driven by agent-desktop observe/act/verify loops."""

from __future__ import annotations

import argparse
import json
import subprocess
import time
import traceback
from pathlib import Path
from typing import Any, Callable

COMPOSER = "描述任务"


class AgentDesktop:
    def __init__(self, app: str, pid: int, window_id: str) -> None:
        self.app, self.pid, self.window_id = app, pid, window_id
        self.steps: list[dict[str, Any]] = []

    def call(self, *args: str, allow_error: bool = False) -> dict[str, Any]:
        result = subprocess.run(["agent-desktop", *args], capture_output=True, text=True, timeout=30, check=False)
        payload = json.loads(result.stdout)
        if result.returncode and not allow_error:
            error = payload.get("error", {})
            raise RuntimeError(f"{error.get('code', 'COMMAND_FAILED')}: {error.get('message', 'unknown error')}")
        return payload

    def bind_window(self) -> None:
        windows = self.call("list-windows", "--app", self.app).get("data") or []
        match = next((window for window in windows if window.get("id") == self.window_id and window.get("pid") == self.pid), None)
        if not match or not match.get("visible"):
            raise AssertionError("the requested isolated window is not visible")
        snapshot = self.call("snapshot", "--app", self.app, "--window-id", self.window_id, "-i", "--skeleton", "--compact")
        if not snapshot.get("data", {}).get("complete"):
            raise AssertionError("the isolated window accessibility tree is incomplete")

    def find(self, role: str, name: str | None = None, *, exact: bool = True) -> dict[str, Any] | None:
        args = ["find", "--app", self.app, "--window-id", self.window_id, "--role", role, "--first"]
        if name is not None:
            args += ["--name", name]
            if exact:
                args += ["--exact"]
        for attempt in range(3):
            payload = self.call(*args, allow_error=True)
            if payload.get("ok"):
                return payload["data"].get("match")
            error = payload.get("error", {})
            if error.get("code") != "TIMEOUT" or attempt == 2:
                raise RuntimeError(f"{error.get('code', 'COMMAND_FAILED')}: {error.get('message', 'find failed')}")
            time.sleep(0.2)
        return None

    def click(self, role: str, name: str) -> None:
        for attempt in range(3):
            match = self.find(role, name)
            if not match:
                raise AssertionError(f"missing {role}: {name}")
            payload = self.call("click", match["ref_id"], allow_error=True)
            if payload.get("ok"):
                self.steps.append({"action": "click", "target": name, "result": "delivered"})
                return
            error = payload.get("error", {})
            code = error.get("code")
            if code == "STALE_REF" and attempt < 2:
                continue
            if code in {"POLICY_DENIED", "ACTION_NOT_SUPPORTED"}:
                self.call("focus", match["ref_id"])
                self.press("return")
                self.steps.append({"action": "focus+return", "target": name, "result": "delivered"})
                return
            if code == "ACTION_FAILED" and error.get("disposition", {}).get("delivery") == "delivered_unverified":
                self.steps.append({"action": "click", "target": name, "result": "delivered_unverified"})
                return
            raise RuntimeError(f"{code}: {error.get('message', 'click failed')}")

    def press(self, key: str) -> None:
        for attempt in range(2):
            focused = self.call("focus-window", "--app", self.app, "--window-id", self.window_id, allow_error=True)
            if not focused.get("ok"):
                windows = self.call("list-windows", "--app", self.app).get("data") or []
                current = next((window for window in windows if window.get("id") == self.window_id and window.get("pid") == self.pid), None)
                if not current or not current.get("is_focused"):
                    if attempt == 0:
                        time.sleep(0.2)
                        continue
                    raise RuntimeError("the isolated window did not become active")
            payload = self.call("press", key, "--app", self.app, "--window-id", self.window_id, allow_error=True)
            if payload.get("ok"):
                self.steps.append({"action": "press", "target": key, "result": "delivered"})
                return
            error = payload.get("error", {})
            if attempt == 0 and error.get("code") in {"STALE_REF", "TIMEOUT"}:
                time.sleep(0.2)
                continue
            raise RuntimeError(f"{error.get('code', 'COMMAND_FAILED')}: {error.get('message', 'key delivery failed')}")


def wait_find(driver: AgentDesktop, role: str, name: str, *, exact: bool = True, timeout: float = 10) -> dict[str, Any]:
    deadline = time.monotonic() + timeout
    latest = None
    while time.monotonic() < deadline:
        latest = driver.find(role, name, exact=exact)
        if latest:
            return latest
        time.sleep(0.15)
    raise AssertionError(f"missing {role} {name!r}; latest={latest}")


def wait_value(driver: AgentDesktop, role: str, name: str, expected: str) -> dict[str, Any]:
    deadline = time.monotonic() + 10
    latest = None
    while time.monotonic() < deadline:
        latest = driver.find(role, name)
        if latest and str(latest.get("value") or "") == expected:
            return latest
        time.sleep(0.15)
    raise AssertionError(f"expected {role} {name!r} value {expected!r}; latest={latest}")


def find_composer(driver: AgentDesktop) -> dict[str, Any] | None:
    return driver.find("textfield", COMPOSER, exact=False) or driver.find("textfield", "输入消息", exact=False)


def wait_any_composer(driver: AgentDesktop) -> dict[str, Any]:
    deadline = time.monotonic() + 10
    while time.monotonic() < deadline:
        composer = find_composer(driver)
        if composer:
            return composer
        time.sleep(0.15)
    raise AssertionError("no home or task composer is available")


def work(driver: AgentDesktop) -> None:
    if driver.find("combobox", "搜索设置", exact=False) or driver.find("button", "取消搜索"):
        if driver.find("button", "取消搜索"):
            driver.click("button", "取消搜索")
        driver.click("button", "工作")
        wait_any_composer(driver)
        return
    if find_composer(driver):
        return
    dialog = driver.find("dialog")
    if dialog and dialog.get("name") == "搜索任务":
        driver.press("escape")
    elif dialog and driver.find("button", "返回应用"):
        driver.click("button", "返回应用")
    if find_composer(driver):
        return
    driver.click("button", "新建任务")
    wait_any_composer(driver)


def wait_dialog(driver: AgentDesktop) -> dict[str, Any]:
    deadline = time.monotonic() + 8
    while time.monotonic() < deadline:
        dialog = driver.find("dialog")
        if dialog:
            return dialog
        time.sleep(0.15)
    raise AssertionError("dialog did not appear")


def close_dialog(driver: AgentDesktop) -> None:
    driver.press("escape")
    deadline = time.monotonic() + 8
    while time.monotonic() < deadline:
        if not driver.find("dialog"):
            return
        time.sleep(0.15)
    raise AssertionError("dialog did not close")


def tree_text(driver: AgentDesktop) -> str:
    snapshot = driver.call("snapshot", "--app", driver.app, "--window-id", driver.window_id, "-i", "--compact")
    parts: list[str] = []

    def visit(node: Any) -> None:
        if isinstance(node, dict):
            parts.extend(str(node.get(key) or "") for key in ("name", "description"))
            for child in node.get("children") or []:
                visit(child)
        elif isinstance(node, list):
            for child in node:
                visit(child)

    visit((snapshot.get("data") or {}).get("tree") or {})
    return "\n".join(parts)


def wait_text(driver: AgentDesktop, expected: str) -> None:
    deadline = time.monotonic() + 10
    while time.monotonic() < deadline:
        if expected in tree_text(driver):
            return
        time.sleep(0.15)
    raise AssertionError(f"missing page text {expected!r}")


def boot(driver: AgentDesktop) -> dict[str, Any]:
    driver.bind_window()
    deferred = False
    dialog = driver.find("dialog")
    if dialog and dialog.get("name") == "使用 Aster 前需要三项权限":
        driver.click("button", "稍后设置")
        deferred = True
        dialog = driver.find("dialog")
    if dialog and dialog.get("name") == "组件已就绪":
        driver.press("escape")
        dialog = driver.find("dialog")
    if dialog:
        raise AssertionError(f"unexpected startup dialog: {dialog.get('name')}")
    composer = wait_any_composer(driver)
    return {"composer_ready": bool(composer.get("ref_id")), "permissions_deferred": deferred}


def navigation(driver: AgentDesktop) -> dict[str, Any]:
    destinations = [("连接器", "搜索连接器或关键词"), ("技能", "搜索技能"), ("自动化", "搜索定时任务")]
    visited = []
    for label, expected in destinations:
        driver.click("button", label)
        wait_find(driver, "textfield", expected, exact=False)
        visited.append(label)
        work(driver)
    return {"visited": visited, "returned_to_work": True}


def task_search(driver: AgentDesktop) -> dict[str, Any]:
    work(driver)
    driver.click("button", "搜索任务")
    field = wait_find(driver, "textfield", "搜索任务")
    driver.call("set-value", field["ref_id"], "0101")
    wait_value(driver, "textfield", "搜索任务", "0101")
    driver.press("escape")
    wait_any_composer(driver)
    return {"query": "0101", "closed": True}


def settings_roundtrip(driver: AgentDesktop) -> dict[str, Any]:
    work(driver)
    field = driver.find("combobox", "搜索设置", exact=False)
    if not field and not driver.find("button", "搜索设置", exact=False):
        driver.click("button", "设置")
    if not field:
        search = wait_find(driver, "button", "搜索设置", exact=False)
        driver.call("click", search["ref_id"])
        field = wait_find(driver, "combobox", "搜索设置", exact=False)
    driver.call("set-value", field["ref_id"], "归档")
    wait_find(driver, "button", "归档")
    driver.click("button", "取消搜索")
    driver.click("button", "工作")
    wait_any_composer(driver)
    return {"query": "归档", "returned_to_work": True}


def connector_search(driver: AgentDesktop) -> dict[str, Any]:
    work(driver)
    driver.click("button", "连接器")
    field = wait_find(driver, "textfield", "搜索连接器或关键词")
    original = str(field.get("value") or "")
    payload = driver.call("set-value", field["ref_id"], "Fetch", allow_error=True)
    if not payload.get("ok") and payload.get("error", {}).get("disposition", {}).get("delivery") == "not_delivered":
        raise RuntimeError("connector search input was not delivered")
    wait_find(driver, "article", "Fetch网页内容抓取")
    changed = wait_value(driver, "textfield", "搜索连接器或关键词", "Fetch")
    payload = driver.call("set-value", changed["ref_id"], original, allow_error=True)
    if not payload.get("ok") and payload.get("error", {}).get("disposition", {}).get("delivery") == "not_delivered":
        raise RuntimeError("connector search restoration was not delivered")
    wait_value(driver, "textfield", "搜索连接器或关键词", original)
    work(driver)
    return {"query": "Fetch", "result": "Fetch网页内容抓取", "restored": True}


def toggle_and_restore(driver: AgentDesktop, name: str) -> dict[str, Any]:
    switch = wait_find(driver, "switch", name)
    original = str(switch.get("value") or "0")
    driver.call("toggle", switch["ref_id"])
    wait_value(driver, "switch", name, "0" if original == "1" else "1")
    current = wait_find(driver, "switch", name)
    driver.call("toggle", current["ref_id"])
    wait_value(driver, "switch", name, original)
    return {"control": name, "original": original, "changed_and_restored": True}


def general_switch(driver: AgentDesktop) -> dict[str, Any]:
    work(driver)
    driver.click("button", "设置")
    driver.click("button", "通用")
    result = toggle_and_restore(driver, "开机自动启动")
    work(driver)
    return result


def context_switch(driver: AgentDesktop) -> dict[str, Any]:
    work(driver)
    driver.click("button", "设置")
    driver.click("button", "通用")
    result = toggle_and_restore(driver, "启用上下文管理")
    work(driver)
    return result


def task_pin_and_archive_cancel(driver: AgentDesktop) -> dict[str, Any]:
    work(driver)
    switch = wait_find(driver, "switch", "置顶")
    original = str(switch.get("value") or "0")
    driver.click("switch", "置顶")
    changed = wait_value(driver, "switch", "置顶", "0" if original == "1" else "1")
    driver.click("switch", "置顶")
    wait_value(driver, "switch", "置顶", original)
    driver.click("button", "归档")
    wait_dialog(driver)
    if driver.find("button", "取消"):
        driver.click("button", "取消")
    else:
        close_dialog(driver)
    wait_find(driver, "switch", "置顶")
    return {"pin_restored": original, "archive_cancelled": True}


def archive_cancel(driver: AgentDesktop) -> dict[str, Any]:
    work(driver)
    driver.click("button", "归档")
    dialog = wait_dialog(driver)
    if driver.find("button", "取消"):
        driver.click("button", "取消")
    else:
        close_dialog(driver)
    wait_find(driver, "switch", "置顶")
    return {"dialog": dialog.get("name"), "cancelled": True}


def dialog_cancel(driver: AgentDesktop, open: Callable[[], None], *, return_work: bool = False) -> dict[str, Any]:
    open()
    dialog = wait_dialog(driver)
    close_dialog(driver)
    if return_work:
        work(driver)
    return {"dialog": dialog.get("name"), "cancelled": True}


def project_dialog_cancel(driver: AgentDesktop) -> dict[str, Any]:
    work(driver)
    return dialog_cancel(driver, lambda: driver.click("button", "新建项目"))


def provider_dialog_cancel(driver: AgentDesktop) -> dict[str, Any]:
    work(driver)
    return dialog_cancel(driver, lambda: (driver.click("button", "设置"), driver.click("button", "模型服务"), driver.click("button", "添加服务商")), return_work=True)


def permission_mode(driver: AgentDesktop) -> dict[str, Any]:
    work(driver)
    combo = wait_find(driver, "combobox", "授权模式", exact=False)
    original = str(combo.get("name") or "").split(":", 1)[-1].strip()
    target = "修改前确认" if original == "只读权限" else "只读权限"

    def select(label: str) -> None:
        current = wait_find(driver, "combobox", "授权模式", exact=False)
        driver.call("focus", current["ref_id"])
        driver.press("return")
        item = wait_find(driver, "menuitem", label, exact=False)
        driver.call("click", item["ref_id"])
        wait_find(driver, "combobox", label, exact=False)

    select(target)
    select(original)
    return {"original": original, "changed_to": target, "restored": True}


def connector_create_cancel(driver: AgentDesktop) -> dict[str, Any]:
    work(driver)
    return dialog_cancel(driver, lambda: (driver.click("button", "连接器"), driver.click("button", "创建")), return_work=True)


def skill_upload_cancel(driver: AgentDesktop) -> dict[str, Any]:
    work(driver)
    return dialog_cancel(driver, lambda: (driver.click("button", "技能"), driver.click("combobox", "创建"), driver.click("menuitem", "上传技能")), return_work=True)


def schedule_create_cancel(driver: AgentDesktop) -> dict[str, Any]:
    work(driver)
    return dialog_cancel(driver, lambda: (driver.click("button", "自动化"), driver.click("combobox", "新建")), return_work=True)


def settings_pages(driver: AgentDesktop) -> dict[str, Any]:
    work(driver)
    driver.click("button", "设置")
    visited = []
    for label, expected in [("网络搜索", "原生 API"), ("文档处理", "PP-OCRv6"), ("数据", "数据目录"), ("关于", "检查更新")]:
        driver.click("button", label)
        wait_text(driver, expected)
        visited.append(label)
    work(driver)
    return {"visited": visited}


def extended_settings_pages(driver: AgentDesktop) -> dict[str, Any]:
    work(driver)
    driver.click("button", "设置")
    visited = []
    for label, expected in [("API 网关", "客户端"), ("模型配置", "默认模型"), ("用量", "概览"), ("快捷键", "全部启用"), ("归档", "恢复")]:
        driver.click("button", label)
        wait_text(driver, expected)
        visited.append(label)
    work(driver)
    return {"visited": visited}


def web_search_mode(driver: AgentDesktop) -> dict[str, Any]:
    work(driver)
    driver.click("button", "设置")
    driver.click("button", "网络搜索")
    driver.click("radiobutton", "HTTP / RSS")
    wait_value(driver, "radiobutton", "HTTP / RSS", "1")
    driver.click("radiobutton", "原生 API")
    wait_value(driver, "radiobutton", "原生 API", "1")
    work(driver)
    return {"changed_to": "HTTP / RSS", "restored": "原生 API"}


def gateway_client_cancel(driver: AgentDesktop) -> dict[str, Any]:
    work(driver)
    return dialog_cancel(driver, lambda: (driver.click("button", "设置"), driver.click("button", "API 网关"), driver.click("button", "添加客户端")), return_work=True)


CASES: list[tuple[str, Callable[[AgentDesktop], dict[str, Any]]]] = [
    ("BOOT-01", boot),
    ("NAV-02", navigation),
    ("HOME-02", task_search),
    ("SET-01", settings_roundtrip),
    ("CAP-01A", connector_search),
    ("SET-02A", general_switch),
    ("SET-02B", context_switch),
    ("HOME-02B", task_pin_and_archive_cancel),
    ("HOME-02C", archive_cancel),
    ("HOME-01A", project_dialog_cancel),
    ("SET-03A", provider_dialog_cancel),
    ("COMP-03A", permission_mode),
    ("CAP-01B", connector_create_cancel),
    ("CAP-02A", skill_upload_cancel),
    ("CAP-03A", schedule_create_cancel),
    ("SET-NAV-01", settings_pages),
    ("SET-NAV-02", extended_settings_pages),
    ("SET-04A", web_search_mode),
    ("SET-16A", gateway_client_cancel),
]


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--app", required=True)
    parser.add_argument("--pid", required=True, type=int)
    parser.add_argument("--window-id", required=True)
    parser.add_argument("--output", required=True, type=Path)
    parser.add_argument("--only", help="comma-separated case IDs")
    args = parser.parse_args()
    only = set(args.only.split(",")) if args.only else None
    driver = AgentDesktop(args.app, args.pid, args.window_id)
    results = []
    for case_id, scenario in CASES:
        if only and case_id not in only:
            continue
        record: dict[str, Any] = {"id": case_id, "status": "failed"}
        try:
            record["details"] = scenario(driver)
            record["status"] = "passed"
        except Exception as error:
            record["error"] = str(error)
            record["traceback"] = traceback.format_exc()
            try:
                work(driver)
                record["recovery"] = "work"
            except Exception as recovery:
                record["recovery"] = str(recovery)
        results.append(record)
    args.output.mkdir(parents=True, exist_ok=True)
    evidence = {"app": args.app, "pid": args.pid, "window_id": args.window_id, "delivery": "agent-desktop-headless-semantic", "results": results, "steps": driver.steps}
    (args.output / "core-navigation.json").write_text(json.dumps(evidence, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    print(json.dumps({"results": results}, ensure_ascii=False, indent=2))
    return 0 if results and all(record["status"] == "passed" for record in results) else 1


if __name__ == "__main__":
    raise SystemExit(main())
