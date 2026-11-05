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
            code = error.get("code")
            if attempt == 2 or code not in {"TIMEOUT", "APP_UNRESPONSIVE"}:
                raise RuntimeError(f"{code or 'COMMAND_FAILED'}: {error.get('message', 'find failed')}")
            if code == "APP_UNRESPONSIVE":
                self.call("focus-window", "--app", self.app, "--window-id", self.window_id)
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
        for attempt in range(5):
            focused = self.call("focus-window", "--app", self.app, "--window-id", self.window_id, allow_error=True)
            if not focused.get("ok"):
                windows = self.call("list-windows", "--app", self.app).get("data") or []
                current = next((window for window in windows if window.get("id") == self.window_id and window.get("pid") == self.pid), None)
                if not current or not current.get("is_focused"):
                    if attempt < 4:
                        time.sleep(0.3)
                        continue
                    raise RuntimeError("the isolated window did not become active")
            payload = self.call("press", key, "--app", self.app, "--window-id", self.window_id, allow_error=True)
            if payload.get("ok"):
                self.steps.append({"action": "press", "target": key, "result": "delivered"})
                return
            error = payload.get("error", {})
            if attempt < 4 and error.get("code") in {"STALE_REF", "TIMEOUT", "APP_UNRESPONSIVE"}:
                time.sleep(0.3)
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


def wait_value(
    driver: AgentDesktop, role: str, name: str, expected: str, timeout: float = 10, *, exact: bool = True
) -> dict[str, Any]:
    deadline = time.monotonic() + timeout
    latest = None
    while time.monotonic() < deadline:
        latest = driver.find(role, name, exact=exact)
        if latest and str(latest.get("value") or "") == expected:
            return latest
        time.sleep(0.15)
    raise AssertionError(f"expected {role} {name!r} value {expected!r}; latest={latest}")


def replace_text(driver: AgentDesktop, role: str, name: str, text: str, *, exact: bool = True) -> None:
    def press(key: str) -> None:
        driver.call("press", key, "--app", driver.app, "--window-id", driver.window_id)

    for focus_attempt in range(3):
        field = driver.find(role, name, exact=exact) or wait_find(driver, role, None)
        try:
            driver.call("focus-window", "--app", driver.app, "--window-id", driver.window_id)
            driver.call("focus", field["ref_id"])
            press("cmd+a")
            press("delete")
            break
        except RuntimeError as error:
            retryable = "STALE_REF" in str(error) or "APP_UNRESPONSIVE" in str(error)
            if not retryable or focus_attempt == 2:
                raise
            time.sleep(0.3)
    for character in text:
        key = {" ": "space", "\t": "tab", "\n": "return", "\r": "return"}.get(character, character)
        for attempt in range(3):
            try:
                press(key)
                break
            except RuntimeError as error:
                retryable = "STALE_REF" in str(error) or "APP_UNRESPONSIVE" in str(error)
                if not retryable or attempt == 2:
                    raise
                field = driver.find(role, name, exact=exact) or wait_find(driver, role, None)
                driver.call("focus", field["ref_id"])
    deadline = time.monotonic() + 8
    while time.monotonic() < deadline:
        current = driver.find(role, name, exact=exact) or driver.find(role)
        if current and str(current.get("value") or "").rstrip("\n") == text.rstrip("\n"):
            return
        time.sleep(0.2)
    raise AssertionError(f"{role} {name!r} did not accept {text!r}; latest={current}")


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


def tree_text(driver: AgentDesktop, *, interactive: bool = True) -> str:
    args = ["snapshot", "--app", driver.app, "--window-id", driver.window_id, "--compact"]
    if interactive:
        args.insert(5, "-i")
    snapshot = driver.call(*args)
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


def wait_text(driver: AgentDesktop, expected: str, *, interactive: bool = True) -> None:
    deadline = time.monotonic() + 10
    while time.monotonic() < deadline:
        if expected in tree_text(driver, interactive=interactive):
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


def sidebar_roundtrip(driver: AgentDesktop) -> dict[str, Any]:
    work(driver)
    expanded = wait_find(driver, "switch", "收起侧边栏")
    driver.call("toggle", expanded["ref_id"])
    collapsed = wait_find(driver, "switch", "展开侧边栏")
    wait_any_composer(driver)
    if driver.find("button", "新建任务"):
        raise AssertionError("collapsed sidebar retained its navigation controls")
    driver.call("toggle", collapsed["ref_id"])
    wait_find(driver, "switch", "收起侧边栏")
    wait_find(driver, "button", "新建任务")
    return {"collapsed": True, "navigation_hidden": True, "composer_available": True, "restored": True}


def draft_roundtrip(driver: AgentDesktop) -> dict[str, Any]:
    work(driver)
    driver.click("button", "新建任务")
    original = str(wait_any_composer(driver).get("value") or "")
    replace_text(driver, "textfield", COMPOSER, "0101", exact=False)
    driver.click("button", "设置")
    if not driver.find("combobox", "搜索设置", exact=False):
        search = wait_find(driver, "button", "搜索设置", exact=False)
        driver.call("click", search["ref_id"])
    wait_find(driver, "combobox", "搜索设置", exact=False)
    driver.click("button", "工作")
    composer = wait_any_composer(driver)
    if str(composer.get("value") or "").rstrip("\n") != "0101":
        raise AssertionError(f"draft was not preserved: {composer}")
    replace_text(driver, "textfield", COMPOSER, original, exact=False)
    return {"draft": "0101", "preserved": True, "restored": True}


def global_search_roundtrip(driver: AgentDesktop) -> dict[str, Any]:
    work(driver)
    driver.click("button", "工作")
    driver.click("button", "新建任务")
    original = str(wait_any_composer(driver).get("value") or "")
    try:
        replace_text(driver, "textfield", COMPOSER, "0101", exact=False)
        driver.press("cmd+shift+f")
        field = wait_find(driver, "combobox", "搜索对话", exact=False)
        driver.call("set-value", field["ref_id"], "0101")
        wait_value(driver, "combobox", "搜索对话", "0101", timeout=10, exact=False)
        driver.press("escape")
        composer = wait_any_composer(driver)
        if str(composer.get("value") or "").rstrip("\n") != "0101" or "focused" not in (composer.get("states") or []):
            raise AssertionError(f"global search did not restore the composer: {composer}")
        return {"query": "0101", "closed": True, "composer_restored": True, "focus_restored": True}
    finally:
        if find_composer(driver):
            replace_text(driver, "textfield", COMPOSER, original, exact=False)


def global_search_opens_session(driver: AgentDesktop) -> dict[str, Any]:
    driver.click("button", "工作")
    driver.click("button", "新建任务")
    wait_any_composer(driver)
    driver.press("cmd+shift+f")
    field = wait_find(driver, "combobox", "搜索对话", exact=False)
    driver.call("set-value", field["ref_id"], "NAV06 Fixture")
    wait_find(driver, "statictext", "NAV06 Fixture Task", exact=False)
    wait_find(driver, "statictext", "NAV06 Fixture Agent", exact=False)
    field = wait_find(driver, "combobox", "搜索对话", exact=False)
    driver.call("focus-window", "--app", driver.app, "--window-id", driver.window_id)
    driver.call("focus", field["ref_id"])
    driver.call("press", "return", "--app", driver.app, "--window-id", driver.window_id)
    deadline = time.monotonic() + 10
    while driver.find("dialog") and time.monotonic() < deadline:
        time.sleep(0.15)
    if driver.find("dialog"):
        raise AssertionError("global search did not close after opening the session")
    wait_find(driver, "button", "打开任务工作区")
    return {"query": "NAV06 Fixture", "session_result": True, "opened": True}


def tab_roundtrip(driver: AgentDesktop) -> dict[str, Any]:
    work(driver)
    original = str(wait_any_composer(driver).get("value") or "")
    draft = "010102"
    replace_text(driver, "textfield", COMPOSER, draft, exact=False)
    driver.click("button", "设置")
    if not driver.find("combobox", "搜索设置", exact=False):
        search = wait_find(driver, "button", "搜索设置", exact=False)
        driver.call("click", search["ref_id"])
    wait_find(driver, "combobox", "搜索设置", exact=False)
    driver.press("cmd+w")
    composer = wait_any_composer(driver)
    if str(composer.get("value") or "").rstrip("\n") != draft:
        raise AssertionError(f"closing the settings tab lost the work draft: {composer}")
    time.sleep(0.5)
    wait_find(driver, "textfield", COMPOSER, exact=False)
    replace_text(driver, "textfield", COMPOSER, original, exact=False)
    return {"created": "settings", "switched": True, "closed": True, "draft_preserved": True}


def task_search(driver: AgentDesktop) -> dict[str, Any]:
    work(driver)
    driver.click("button", "搜索任务")
    field = wait_find(driver, "textfield", "搜索任务")
    driver.call("set-value", field["ref_id"], "0101")
    wait_value(driver, "textfield", "搜索任务", "0101")
    driver.press("escape")
    wait_any_composer(driver)
    return {"query": "0101", "closed": True}


def task_search_empty_and_clear(driver: AgentDesktop) -> dict[str, Any]:
    work(driver)
    driver.click("button", "搜索任务")
    field = wait_find(driver, "textfield", "搜索任务")
    driver.call("set-value", field["ref_id"], "e2e-no-such-task")
    wait_text(driver, "没有匹配的任务", interactive=False)
    driver.click("button", "清除")
    wait_value(driver, "textfield", "搜索任务", "")
    if "没有匹配的任务" in tree_text(driver, interactive=False):
        raise AssertionError("clearing the query retained the no-result state")
    driver.press("escape")
    wait_any_composer(driver)
    return {"empty_state": "没有匹配的任务", "cleared": True, "empty_state_cleared": True}


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
    replace_text(driver, "textfield", "搜索连接器或关键词", "Fetch")
    wait_find(driver, "article", "Fetch网页内容抓取")
    replace_text(driver, "textfield", "搜索连接器或关键词", original)
    work(driver)
    return {"query": "Fetch", "result": "Fetch网页内容抓取", "restored": True}


def connector_detail_cancel(driver: AgentDesktop) -> dict[str, Any]:
    work(driver)
    driver.click("button", "连接器")
    field = wait_find(driver, "textfield", "搜索连接器或关键词")
    original = str(field.get("value") or "")
    replace_text(driver, "textfield", "搜索连接器或关键词", "Fetch")
    driver.click("article", "Fetch网页内容抓取")
    dialog = wait_dialog(driver)
    if dialog.get("name") != "Fetch网页内容抓取" or not driver.find("button", "安装"):
        raise AssertionError(f"unexpected connector detail: {dialog}")
    close_dialog(driver)
    wait_find(driver, "article", "Fetch网页内容抓取")
    current = wait_find(driver, "textfield", "Fetch")
    replace_text(driver, "textfield", str(current.get("name")), original)
    work(driver)
    return {"detail": dialog.get("name"), "closed": True, "install_triggered": False, "search_restored": True}


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


def general_tray_switches(driver: AgentDesktop) -> dict[str, Any]:
    work(driver)
    driver.click("button", "设置")
    driver.click("button", "通用")
    results = [
        toggle_and_restore(driver, name)
        for name in ("启动时最小化到托盘", "显示托盘图标", "关闭时最小化到托盘", "运行任务时保持系统唤醒")
    ]
    work(driver)
    return {"switches": results, "app_close_triggered": False, "task_run_triggered": False}


def pin_item(driver: AgentDesktop) -> dict[str, Any] | None:
    return driver.find("menuitem", "取消置顶") or driver.find("menuitem", "置顶")


def open_task_menu(driver: AgentDesktop) -> None:
    if pin_item(driver):
        return
    combo = wait_find(driver, "combobox", "更多：你好")
    driver.call("focus", combo["ref_id"])
    driver.press("return")
    deadline = time.monotonic() + 10
    while time.monotonic() < deadline:
        if pin_item(driver):
            return
        time.sleep(0.15)
    raise AssertionError("task pin menuitem did not appear")


def set_switch(driver: AgentDesktop, expected: str) -> None:
    label = "置顶" if expected == "1" else "取消置顶"
    for action in ("click", "space", "return"):
        item = wait_find(driver, "menuitem", label)
        if action == "click":
            driver.call("click", item["ref_id"])
        else:
            driver.call("focus", item["ref_id"])
            driver.press(action)
        open_task_menu(driver)
        current = pin_item(driver)
        if current and current.get("name") == ("取消置顶" if expected == "1" else "置顶"):
            return
    raise AssertionError(f"pin menuitem did not become {expected}")


def task_pin_and_archive_cancel(driver: AgentDesktop) -> dict[str, Any]:
    work(driver)
    open_task_menu(driver)
    original = "1" if pin_item(driver).get("name") == "取消置顶" else "0"
    set_switch(driver, "0" if original == "1" else "1")
    set_switch(driver, original)
    driver.click("menuitem", "归档")
    wait_dialog(driver)
    if driver.find("button", "取消"):
        driver.click("button", "取消")
    else:
        close_dialog(driver)
    open_task_menu(driver)
    return {"pin_restored": original, "archive_cancelled": True}


def archive_cancel(driver: AgentDesktop) -> dict[str, Any]:
    work(driver)
    open_task_menu(driver)
    driver.click("menuitem", "归档")
    dialog = wait_dialog(driver)
    if driver.find("button", "取消"):
        driver.click("button", "取消")
    else:
        close_dialog(driver)
    open_task_menu(driver)
    return {"dialog": dialog.get("name"), "cancelled": True}


def archive_and_restore(driver: AgentDesktop) -> dict[str, Any]:
    if driver.find("dialog"):
        driver.press("escape")
    work(driver)
    if driver.find("switch", "展开侧边栏"):
        driver.click("switch", "展开侧边栏")
    combo = wait_find(driver, "combobox", "更多：使用技能 onetake", exact=False)
    driver.call("focus", combo["ref_id"])
    driver.press("return")
    driver.click("menuitem", "归档")
    wait_dialog(driver)
    driver.click("button", "归档")
    wait_find(driver, "button", "设置")
    driver.click("button", "设置")
    driver.click("button", "归档")
    driver.click("button", "恢复")
    wait_dialog(driver)
    confirm = driver.call("find", "--app", driver.app, "--window-id", driver.window_id, "--role", "button", "--name", "恢复", "--last")
    driver.call("focus", confirm["data"]["match"]["ref_id"])
    driver.press("return")
    driver.click("button", "工作")
    driver.click("button", "搜索任务")
    field = wait_find(driver, "textfield", "搜索任务")
    driver.call("set-value", field["ref_id"], "onetake")
    wait_find(driver, "statictext", "使用技能 onetake", exact=False)
    driver.press("escape")
    return {"archived": True, "restored": True, "search_found": "使用技能 onetake："}


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


def project_required_fields(driver: AgentDesktop) -> dict[str, Any]:
    work(driver)
    driver.click("button", "新建项目")
    wait_dialog(driver)
    if "disabled" not in (wait_find(driver, "button", "创建").get("states") or []):
        raise AssertionError("empty project form enabled Create")
    replace_text(driver, "textfield", "项目名称", "20261105")
    if "disabled" in (wait_find(driver, "button", "创建").get("states") or []):
        raise AssertionError("entering a project name did not enable Create")
    close_dialog(driver)
    work(driver)
    return {"empty_create_disabled": True, "name_enables_create": True, "create_triggered": False}


def provider_dialog_cancel(driver: AgentDesktop) -> dict[str, Any]:
    work(driver)
    return dialog_cancel(driver, lambda: (driver.click("button", "设置"), driver.click("button", "模型服务"), driver.click("button", "添加服务商")), return_work=True)


def provider_request_config_cancel(driver: AgentDesktop) -> dict[str, Any]:
    work(driver)
    result = dialog_cancel(
        driver,
        lambda: (
            driver.click("button", "设置"),
            driver.click("button", "模型服务"),
            driver.click("button", "配置 API Host 与自定义请求头"),
        ),
        return_work=True,
    )
    result["save_triggered"] = False
    result["connection_test_triggered"] = False
    return result


def translation_response_switches(driver: AgentDesktop) -> dict[str, Any]:
    work(driver)
    driver.click("button", "设置")
    driver.click("button", "模型配置")
    driver.click("button", "翻译设置")
    wait_dialog(driver)
    results = [toggle_and_restore(driver, name) for name in ("模型温度", "Top-P")]
    close_dialog(driver)
    work(driver)
    return {"switches": results, "translation_triggered": False}


def route_and_budget_create_cancel(driver: AgentDesktop) -> dict[str, Any]:
    work(driver)
    driver.click("button", "设置")
    driver.click("button", "模型配置")
    driver.click("radiobutton", "路由计划")
    wait_text(driver, "还没有路由计划", interactive=False)
    driver.click("button", "新建路由")
    route_dialog = wait_dialog(driver)
    close_dialog(driver)
    driver.click("radiobutton", "本地预算")
    wait_text(driver, "还没有预算", interactive=False)
    driver.click("button", "添加预算")
    budget_dialog = wait_dialog(driver)
    close_dialog(driver)
    work(driver)
    return {"route_dialog": route_dialog.get("name"), "budget_dialog": budget_dialog.get("name"), "save_triggered": False}


def translation_languages_load(driver: AgentDesktop) -> dict[str, Any]:
    work(driver)
    driver.click("button", "设置")
    driver.click("button", "模型配置")
    driver.click("button", "翻译设置")
    wait_dialog(driver)
    deadline = time.monotonic() + 5
    while time.monotonic() < deadline:
        if "翻译模块初始化失败，部分功能可能不可用" in tree_text(driver, interactive=False):
            raise AssertionError("translation languages failed to load")
        time.sleep(0.2)
    close = wait_find(driver, "button", "关闭")
    commands = [
        {"command": "focus-window", "args": {"app": driver.app, "window_id": driver.window_id}},
        {"command": "focus", "args": {"ref_id": close["ref_id"]}},
        {"command": "press", "args": {"combo": "return", "app": driver.app, "window_id": driver.window_id}},
    ]
    payload = json.loads(subprocess.run(["agent-desktop", "batch", json.dumps(commands), "--stop-on-error"], capture_output=True, text=True, check=True).stdout)
    if not payload.get("ok"):
        raise AssertionError(f"translation close failed: {payload}")
    if driver.find("dialog"):
        raise AssertionError("translation dialog did not close")
    work(driver)
    return {"initialization_error_present": False}


def model_picker(driver: AgentDesktop) -> dict[str, Any]:
    work(driver)
    if driver.find("textfield", "搜索模型..."):
        driver.press("escape")
    original = wait_find(driver, "combobox", "GPT", exact=False)
    original_name = str(original["name"])
    target = "GPT-6.1 Terra" if original_name != "GPT-6.1 Terra" else "GPT-6.1 Sol"

    def select(label: str) -> None:
        current = wait_find(driver, "combobox", "GPT", exact=False)
        driver.call("click", current["ref_id"])
        field = wait_find(driver, "textfield", "搜索模型...")
        driver.call("set-value", field["ref_id"], label)
        item = wait_find(driver, "statictext", label)
        driver.call("click", item["ref_id"])
        wait_find(driver, "combobox", label)

    select(target)
    select(original_name)
    return {"original": original_name, "changed_to": target, "restored": True}


def reasoning_effort(driver: AgentDesktop) -> dict[str, Any]:
    for _ in range(2):
        if not driver.find("dialog"):
            break
        driver.press("escape")
    work(driver)
    driver.click("button", "设置")
    driver.click("button", "模型配置")
    driver.click("button", "翻译设置")
    combo = wait_find(driver, "combobox", "响应设置")

    def slider_value(tree: Any) -> int | None:
        if isinstance(tree, dict):
            if tree.get("role") == "slider" and tree.get("name") == "强度":
                return int(float(str(tree.get("value") or "0")))
            for child in tree.get("children") or []:
                found = slider_value(child)
                if found is not None:
                    return found
        elif isinstance(tree, list):
            for child in tree:
                found = slider_value(child)
                if found is not None:
                    return found
        return None

    commands = [
        {"command": "focus-window", "args": {"app": driver.app, "window_id": driver.window_id}},
        {"command": "focus", "args": {"ref_id": combo["ref_id"]}},
        {"command": "press", "args": {"combo": "return", "app": driver.app, "window_id": driver.window_id}},
        {"command": "snapshot", "args": {"app": driver.app, "window_id": driver.window_id, "interactive_only": True, "compact": True}},
        {"command": "press", "args": {"combo": "right", "app": driver.app, "window_id": driver.window_id}},
        {"command": "snapshot", "args": {"app": driver.app, "window_id": driver.window_id, "interactive_only": True, "compact": True}},
        {"command": "press", "args": {"combo": "left", "app": driver.app, "window_id": driver.window_id}},
        {"command": "snapshot", "args": {"app": driver.app, "window_id": driver.window_id, "interactive_only": True, "compact": True}},
    ]
    payload = json.loads(subprocess.run(["agent-desktop", "batch", json.dumps(commands), "--stop-on-error"], capture_output=True, text=True, check=True).stdout)
    values = [slider_value((result.get("data") or {}).get("tree")) for result in payload["data"]["results"] if result.get("command") == "snapshot"]
    if len(values) != 3 or values[0] is None or values[1] in (None, values[0]) or values[2] != values[0]:
        raise AssertionError(f"reasoning effort was not changed and restored: {values}")
    driver.press("escape")
    driver.press("escape")
    work(driver)
    return {"original_index": values[0], "changed_index": values[1], "restored": True}


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


def composer_local_switches(driver: AgentDesktop) -> dict[str, Any]:
    work(driver)

    def read_switches() -> dict[str, dict[str, Any]]:
        for _ in range(4):
            combo = wait_find(driver, "combobox", "更多选项")
            driver.call("click", combo["ref_id"])
            time.sleep(0.2)
            switches = {name: driver.find("switch", name) for name in ("自动审查", "产物追溯")}
            if all(switches.values()):
                return switches
        raise AssertionError("Composer local-options menu did not open")

    results = []
    for name in ("自动审查", "产物追溯"):
        switches = read_switches()
        original = str(switches[name].get("value") or "0")
        driver.call("toggle", switches[name]["ref_id"])
        switches = read_switches()
        changed = str(switches[name].get("value") or "0")
        if changed == original:
            raise AssertionError(f"{name} did not change")
        driver.call("toggle", switches[name]["ref_id"])
        switches = read_switches()
        restored = str(switches[name].get("value") or "0")
        if restored != original:
            raise AssertionError(f"{name} did not restore")
        results.append({"control": name, "original": original, "changed": changed, "restored": restored})
    driver.press("escape")
    return {"switches": results, "upload_triggered": False}


def upload_attachment_cancel(driver: AgentDesktop) -> dict[str, Any]:
    work(driver)
    original = str(wait_any_composer(driver).get("value") or "")
    combo = wait_find(driver, "combobox", "更多选项")
    driver.call("click", combo["ref_id"])
    upload = wait_find(driver, "combobox", "上传附件")
    driver.call("click", upload["ref_id"])
    deadline = time.monotonic() + 8
    panel_id = None
    while time.monotonic() < deadline:
        windows = driver.call("list-windows", "--app", driver.app).get("data") or []
        panel = next((window for window in windows if window.get("title") == "Open" and window.get("pid") == driver.pid), None)
        if panel:
            panel_id = panel["id"]
            break
        time.sleep(0.15)
    if not panel_id:
        raise AssertionError("native attachment panel did not open")
    panel_driver = AgentDesktop(driver.app, driver.pid, panel_id)
    cancel = wait_find(panel_driver, "button", "Cancel")
    panel_driver.call("click", cancel["ref_id"])
    deadline = time.monotonic() + 8
    while time.monotonic() < deadline:
        windows = driver.call("list-windows", "--app", driver.app).get("data") or []
        if not any(window.get("id") == panel_id for window in windows):
            break
        time.sleep(0.15)
    else:
        raise AssertionError("native attachment panel did not close")
    if str(wait_any_composer(driver).get("value") or "") != original:
        raise AssertionError("cancelling attachment selection changed the Composer")
    return {"panel": "Open", "cancelled": True, "attachment_added": False}


def slash_skill(driver: AgentDesktop) -> dict[str, Any]:
    work(driver)
    original = str(wait_any_composer(driver).get("value") or "")
    replace_text(driver, "textfield", COMPOSER, "/o", exact=False)
    wait_find(driver, "button", "onetake")
    if driver.find("button", "定时任务"):
        raise AssertionError("slash panel did not filter unrelated entries")
    driver.click("button", "onetake")
    deadline = time.monotonic() + 8
    composer = None
    while time.monotonic() < deadline:
        composer = driver.find("textfield", COMPOSER, exact=False)
        if composer and "使用技能 onetake" in str(composer.get("value") or ""):
            break
        time.sleep(0.15)
    else:
        raise AssertionError(f"skill token was not inserted: {composer}")
    replace_text(driver, "textfield", COMPOSER, "/", exact=False)
    driver.press("escape")
    if driver.find("button", "onetake"):
        raise AssertionError("slash panel did not close on Escape")
    replace_text(driver, "textfield", COMPOSER, original, exact=False)
    return {"filter": "/o", "selected": "onetake", "cancelled": True, "restored": True}


def connector_create_cancel(driver: AgentDesktop) -> dict[str, Any]:
    work(driver)
    return dialog_cancel(driver, lambda: (driver.click("button", "连接器"), driver.click("button", "创建")), return_work=True)


def open_combo(driver: AgentDesktop, name: str, item: str | None = None) -> None:
    combo = wait_find(driver, "combobox", name)
    driver.call("focus", combo["ref_id"])
    driver.press("return")
    if item:
        wait_find(driver, "menuitem", item)
        driver.press("return")


def skill_upload_cancel(driver: AgentDesktop) -> dict[str, Any]:
    work(driver)
    return dialog_cancel(driver, lambda: (driver.click("button", "技能"), open_combo(driver, "创建", "上传技能")), return_work=True)


def schedule_create_cancel(driver: AgentDesktop) -> dict[str, Any]:
    work(driver)
    return dialog_cancel(driver, lambda: (driver.click("button", "自动化"), open_combo(driver, "新建", "手动创建")), return_work=True)


def document_ocr_status(driver: AgentDesktop) -> dict[str, Any]:
    work(driver)
    driver.click("button", "设置")
    driver.click("button", "文档处理")
    driver.click("button", "解析设置")
    driver.click("button", "PP-OCRv6")
    wait_text(driver, "默认已启用", interactive=False)
    wait_text(driver, "未安装", interactive=False)
    wait_text(driver, "无需密钥，也不上传文档", interactive=False)
    work(driver)
    return {"parser": "PP-OCRv6", "plugin_status": "未安装", "download_triggered": False}


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
    for label, expected in [("API 网关", "客户端"), ("模型配置", "默认模型"), ("用量", "每日用量地形图"), ("快捷键", "按下快捷键"), ("归档", "搜索已归档任务")]:
        driver.click("button", label)
        wait_text(driver, expected)
        visited.append(label)
    work(driver)
    return {"visited": visited}


def usage_filters(driver: AgentDesktop) -> dict[str, Any]:
    work(driver)
    driver.click("button", "设置")
    driver.click("button", "用量")
    driver.click("radiobutton", "30 天")
    wait_value(driver, "radiobutton", "30 天", "1")
    driver.click("radiobutton", "7 天")
    wait_value(driver, "radiobutton", "7 天", "1")
    driver.click("radiobutton", "请求数")
    wait_value(driver, "radiobutton", "请求数", "1")
    driver.click("radiobutton", "Token")
    wait_value(driver, "radiobutton", "Token", "1")
    work(driver)
    return {"period_restored": "7 天", "metric_restored": "Token", "export_triggered": False}


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


def shortcut_rebind(driver: AgentDesktop) -> dict[str, Any]:
    work(driver)
    wait_find(driver, "button", "设置")
    driver.click("button", "设置")
    driver.click("button", "快捷键")

    def record(combo: str) -> None:
        button = wait_find(driver, "button", "搜索消息: 按下快捷键")
        commands = [
            {"command": "focus-window", "args": {"app": driver.app, "window_id": driver.window_id}},
            {"command": "click", "args": {"ref_id": button["ref_id"]}},
            {"command": "press", "args": {"combo": combo, "app": driver.app, "window_id": driver.window_id}},
            {"command": "snapshot", "args": {"app": driver.app, "window_id": driver.window_id, "interactive_only": True, "compact": True}},
        ]
        payload = json.loads(subprocess.run(["agent-desktop", "batch", json.dumps(commands), "--stop-on-error"], capture_output=True, text=True, check=True).stdout)
        if not payload.get("ok"):
            raise AssertionError(f"shortcut recording failed for {combo}: {payload}")

    record("cmd+shift+j")
    driver.click("button", "工作")
    driver.press("cmd+shift+j")
    wait_find(driver, "combobox", "搜索对话", exact=False)
    driver.press("escape")
    wait_find(driver, "button", "设置")
    driver.click("button", "设置")
    driver.click("button", "快捷键")
    record("cmd+shift+f")
    driver.click("button", "工作")
    driver.press("cmd+shift+f")
    wait_find(driver, "combobox", "搜索对话", exact=False)
    driver.press("escape")
    return {"changed_to": "cmd+shift+j", "restored": "cmd+shift+f", "both_verified": True}


def about_page(driver: AgentDesktop) -> dict[str, Any]:
    work(driver)
    driver.click("button", "关于")
    wait_text(driver, "当前已是最新版本", interactive=False)
    text = tree_text(driver, interactive=False)
    if "v0.1.11" not in text or "构建" not in text:
        raise AssertionError("version and build information are missing")
    wait_find(driver, "button", "检查更新")
    work(driver)
    return {"version": "v0.1.11", "build_present": True, "update_not_triggered": True}


def gateway_readonly_status(driver: AgentDesktop) -> dict[str, Any]:
    work(driver)
    driver.click("button", "设置")
    driver.click("button", "API 网关")
    text = tree_text(driver, interactive=False)
    for expected in ("已停止", "额外客户端使用独立密钥和权限。"):
        if expected not in text:
            raise AssertionError(f"missing gateway status: {expected}")
    wait_value(driver, "textfield", "URL", "http://127.0.0.1:23333")
    driver.click("button", "查看用量")
    wait_find(driver, "radiobutton", "30 天")
    driver.click("button", "API 网关")
    wait_text(driver, "已停止", interactive=False)
    work(driver)
    return {"status": "已停止", "url": "http://127.0.0.1:23333", "usage_navigation": True, "start_triggered": False}


def gateway_client_cancel(driver: AgentDesktop) -> dict[str, Any]:
    work(driver)
    return dialog_cancel(driver, lambda: (driver.click("button", "设置"), driver.click("button", "API 网关"), driver.click("button", "添加客户端")), return_work=True)


CASES: list[tuple[str, Callable[[AgentDesktop], dict[str, Any]]]] = [
    ("BOOT-01", boot),
    ("NAV-01", draft_roundtrip),
    ("NAV-02", navigation),
    ("NAV-03", global_search_roundtrip),
    ("NAV-04", tab_roundtrip),
    ("NAV-05", sidebar_roundtrip),
    ("NAV-06", global_search_opens_session),
    ("HOME-02", task_search),
    ("HOME-02D", task_search_empty_and_clear),
    ("SET-01", settings_roundtrip),
    ("CAP-01A", connector_search),
    ("SET-02A", general_switch),
    ("SET-02B", context_switch),
    ("SET-02C", general_tray_switches),
    ("HOME-02B", task_pin_and_archive_cancel),
    ("HOME-02C", archive_cancel),
    ("HOME-01A", project_dialog_cancel),
    ("HOME-01B", project_required_fields),
    ("SET-03A", provider_dialog_cancel),
    ("SET-03B", provider_request_config_cancel),
    ("SET-03C", translation_response_switches),
    ("SET-03D", route_and_budget_create_cancel),
    ("SET-03E", translation_languages_load),
    ("COMP-01", model_picker),
    ("COMP-02", reasoning_effort),
    ("COMP-03A", permission_mode),
    ("COMP-03B", composer_local_switches),
    ("COMP-04", upload_attachment_cancel),
    ("COMP-06", slash_skill),
    ("CAP-01B", connector_create_cancel),
    ("CAP-01C", connector_detail_cancel),
    ("CAP-02A", skill_upload_cancel),
    ("CAP-03A", schedule_create_cancel),
    ("SET-NAV-01", settings_pages),
    ("SET-NAV-02", extended_settings_pages),
    ("SET-04A", web_search_mode),
    ("SET-05A", document_ocr_status),
    ("SET-07A", shortcut_rebind),
    ("SET-12A", usage_filters),
    ("SET-14A", archive_and_restore),
    ("SET-15A", about_page),
    ("SET-16A", gateway_client_cancel),
    ("SET-16B", gateway_readonly_status),
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
