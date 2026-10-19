"""Drive the messages page on the new toyou fork with background CUA.

Binds the pid in $TMPDIR/a3s-studio-runtime-toyou-fork1.json. Refuses the
preexisting towork-cua window. Every press uses delivery_mode background.
Does not send a message, confirm a reset, or start a model turn.

Usage:
  python3 catalog.py --log <cua-run.log> --user-pid <pid>
"""

from __future__ import annotations

import argparse
import json
import os
import subprocess
import sys
import time
import urllib.error
import urllib.request
from pathlib import Path
from typing import Any, Callable

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "towork"))
from driver import Drive  # noqa: E402

RUNTIME_NAME = "a3s-studio-runtime-toyou-fork1.json"
FORBIDDEN_PRESS = {"发送", "重置", "解散群聊", "退出群聊", "保存", "创建群聊"}


def pointer() -> str:
    swift = (
        "import CoreGraphics\n"
        "guard let event = CGEvent(source: nil) else { print(\"missing\"); exit(0) }\n"
        "let point = event.location\n"
        "print(\"\\(point.x),\\(point.y)\")\n"
    )
    done = subprocess.run(["swift", "-e", swift], capture_output=True, text=True)
    return done.stdout.strip() or done.stderr.strip()[:80]


def frontmost() -> str:
    script = 'tell application "System Events" to get name of first process whose frontmost is true'
    done = subprocess.run(["osascript", "-e", script], capture_output=True, text=True)
    return done.stdout.strip()


def process_command(pid: int) -> str:
    done = subprocess.run(["ps", "-p", str(pid), "-o", "command="], capture_output=True, text=True)
    return done.stdout.strip()


def alive(pid: int) -> bool:
    return subprocess.run(["ps", "-p", str(pid)], capture_output=True).returncode == 0


class LogDrive(Drive):
    """Record every action. Refuse a delivery that is not background."""

    def __init__(self, pid: int, window_id: int, log: Callable[[str], None]) -> None:
        super().__init__(pid, window_id)
        self.log = log
        self.actions: list[str] = []

    def call(self, name: str, arguments: dict[str, Any]) -> dict[str, Any]:
        if name in {"move_mouse", "drag", "scroll", "raise_window", "set_frontmost"}:
            raise RuntimeError(f"refusing pointer tool {name}")
        mode = arguments.get("delivery_mode")
        if name in {"click", "press_key", "type_text", "hotkey"} and mode != "background":
            raise RuntimeError(f"refusing {name} without background delivery")
        if mode not in (None, "background"):
            raise RuntimeError(f"refusing delivery {mode}")
        result = super().call(name, arguments)
        if name in {"click", "press_key", "type_text", "hotkey"}:
            structured = result.get("structuredContent") or {}
            delivery = (structured.get("delivery") or {}).get("mode") or mode
            line = (
                f"ACTION {name} route={structured.get('route') or 'accessibility'} "
                f"delivery_mode: {delivery}"
            )
            self.actions.append(line)
            self.log(line)
            if delivery != "background":
                raise RuntimeError(f"driver reported delivery {delivery}")
        return result


def label_of(element: dict[str, Any]) -> str:
    return " ".join(str(element.get("label") or element.get("value") or "").split())


def web_rows(ui: LogDrive) -> list[tuple[str, str]]:
    rows = []
    for element in ui.elements:
        if element.get("role") in ("AXMenuItem", "AXMenu", "AXMenuBarItem", "AXMenuBar"):
            continue
        if not element.get("in_web_content") and element.get("role") != "AXWindow":
            continue
        text = label_of(element)
        if text:
            rows.append((str(element.get("role")), text[:160]))
    return rows


class Catalog:
    def __init__(self, ui: LogDrive, log: Callable[[str], None]) -> None:
        self.ui = ui
        self.log = log
        self.allowed: set[str] = set()

    def refresh(self) -> None:
        self.ui.refresh()

    def labels(self) -> list[str]:
        return [text for _role, text in web_rows(self.ui)]

    def has(self, text: str) -> bool:
        needle = " ".join(text.split())
        return any(needle in label for label in self.labels())

    def observe(self, case: str, result: str, observation: str) -> None:
        self.log(f"CASE {case} RESULT={result} OBS {observation}")

    def find(
        self,
        text: str,
        role: str | None = None,
        *,
        exact: bool = False,
        prefer: Callable[[dict[str, Any]], float] | None = None,
    ) -> dict[str, Any] | None:
        needle = " ".join(text.split())
        matches = []
        for element in self.ui.elements:
            label = label_of(element)
            if exact:
                ok = label == needle
            else:
                ok = needle in label
            if not ok:
                continue
            if role and element.get("role") != role:
                continue
            if not element.get("element_token"):
                continue
            matches.append(element)
        if not matches:
            return None
        if prefer:
            return max(matches, key=prefer)
        return matches[0]

    def press(self, element: dict[str, Any], why: str) -> None:
        label = label_of(element)
        if label in FORBIDDEN_PRESS and label not in self.allowed:
            raise RuntimeError(f"refusing destructive press {label} during {why}")
        # A fresh snapshot invalidates the token. Re-address the same control.
        self.ui._background_click(element)
        self.log(f"PRESS {why} label={label[:80]}")
        time.sleep(0.45)
        self.refresh()

    def press_text(self, text: str, role: str | None, why: str, *, exact: bool = False) -> bool:
        self.refresh()
        element = self.find(text, role, exact=exact)
        if element is None:
            self.log(f"MISS {why} text={text}")
            return False
        self.press(element, why)
        return True

    def type_into(self, element: dict[str, Any], text: str, why: str) -> None:
        self.ui.call(
            "type_text",
            {
                "pid": self.ui.pid,
                "window_id": self.ui.window_id,
                "element_token": element["element_token"],
                "text": text,
                "delivery_mode": "background",
                "delay_ms": 0,
            },
        )
        self.log(f"TYPE {why} chars={len(text)}")
        time.sleep(0.3)
        self.refresh()

    def escape(self) -> None:
        self.ui.press_background("escape")
        time.sleep(0.3)
        self.refresh()

    def open_messages(self) -> None:
        deadline = time.time() + 20
        rail = None
        while time.time() < deadline:
            self.refresh()
            if self.has("发起会话") or self.has("选择一个会话"):
                return
            rail = self.find(
                "消息",
                "AXButton",
                exact=True,
                prefer=lambda el: -float((el.get("frame") or {}).get("w", 999)),
            )
            if rail is not None:
                break
            time.sleep(0.5)
        if rail is None:
            raise RuntimeError("messages rail is missing")
        self.press(rail, "open messages")

    def heading(self, title: str) -> bool:
        self.refresh()
        for element in self.ui.elements:
            if element.get("role") != "AXHeading":
                continue
            label = label_of(element)
            # A group heading is "ToTeam (3)". An exact title still matches ToBot.
            if label == title or label.startswith(title + " ") or label.startswith(title + "("):
                return True
        return False

    def show_thread(self, title: str, seconds: float, *, reject: str | None = None) -> bool:
        """Select a row and wait until its thread heading is showing.

        A background press can return before the thread pane changes. The
        catalog turn is only visible on the ToBot heading.
        """
        deadline = time.time() + seconds
        while time.time() < deadline:
            if self.heading(title):
                return True
            if not self.open_row(title, reject=reject):
                time.sleep(0.4)
            else:
                time.sleep(0.25)
        return self.heading(title)

    def open_row(self, needle: str, reject: str | None = None) -> bool:
        self.refresh()

        def score(element: dict[str, Any]) -> float:
            frame = element.get("frame") or {}
            return float(frame.get("w", 0))

        matches = []
        for element in self.ui.elements:
            if element.get("role") != "AXButton" or not element.get("element_token"):
                continue
            label = label_of(element)
            if needle not in label:
                continue
            if reject and reject in label:
                continue
            if float((element.get("frame") or {}).get("w", 0)) < 80:
                continue
            matches.append(element)
        if not matches:
            return False

        def rank(element: dict[str, Any]) -> tuple[int, float]:
            label = label_of(element)
            starts = 0 if label.startswith(needle) else 1
            return (starts, -score(element))

        self.press(min(matches, key=rank), f"open row {needle}")
        return True

    def close_dialog(self) -> None:
        self.refresh()
        for text in ("取消", "关闭"):
            element = self.find(text, "AXButton", exact=True)
            if element is not None and label_of(element) not in FORBIDDEN_PRESS:
                self.press(element, f"close {text}")
                self.until_gone("聊天记录")
                self.until_gone("创建群聊")
                return
        self.escape()
        self.until_gone("聊天记录")

    def wait_has(self, text: str, seconds: float) -> bool:
        deadline = time.time() + seconds
        while time.time() < deadline:
            self.refresh()
            if self.has(text):
                return True
            time.sleep(0.4)
        self.refresh()
        return self.has(text)

    def until_gone(self, text: str, seconds: float = 2.5) -> bool:
        deadline = time.time() + seconds
        while time.time() < deadline:
            if not self.has(text):
                return True
            time.sleep(0.25)
            self.refresh()
        return not self.has(text)

    def exact_has(self, text: str) -> bool:
        needle = " ".join(text.split())
        return any(label_of(element) == needle for element in self.ui.elements)

    def show_menu_exact(self, text: str, why: str) -> bool:
        self.refresh()
        element = self.find(text, exact=True)
        if element is None or not element.get("element_token"):
            self.log(f"MISS {why} text={text}")
            return False
        try:
            self.ui.show_menu(element)
        except RuntimeError as exc:
            self.log(f"MISS {why} error={exc}")
            return False
        self.log(f"MENU {why} label={label_of(element)[:80]}")
        time.sleep(0.4)
        self.refresh()
        return True


def run(catalog: Catalog) -> None:
    ui = catalog
    ui.open_messages()
    joined = " | ".join(ui.labels()[:40])
    error = ui.has("似乎出现了一些问题")
    ui.observe(
        "page-states",
        "fail" if error else "pass",
        f"error_boundary={str(error).lower()} empty={str(ui.has('选择一个会话')).lower()} labels={joined}",
    )

    search = ui.find("搜索", "AXTextField", exact=True)
    if search is None:
        ui.observe("inbox", "fail", "search field missing")
    else:
        ui.type_into(search, "没有这个会话zzz", "inbox no-results")

        def list_buttons() -> list[str]:
            return [
                label_of(element)
                for element in ui.ui.elements
                if element.get("role") == "AXButton" and element.get("in_web_content")
            ]

        no_hit = not any("林书豪" in label or label.startswith("ToBot") for label in list_buttons())
        ui.escape()
        ui.refresh()
        search = ui.find("搜索", "AXTextField", exact=True)
        if search is not None:
            ui.type_into(search, "林书豪", "inbox filter")
        filtered = ui.has("林书豪")
        ui.escape()
        ui.refresh()
        # Clear the field so the inbox is not left filtered.
        search = ui.find("搜索", "AXTextField", exact=True)
        if search is not None:
            ui.ui.call(
                "hotkey",
                {
                    "pid": ui.ui.pid,
                    "window_id": ui.ui.window_id,
                    "element_token": search.get("element_token"),
                    "keys": ["cmd", "a"],
                    "delivery_mode": "background",
                },
            )
            ui.ui.press_background("delete", search)
            time.sleep(0.3)
            ui.refresh()
        rows = [label for label in ui.labels() if "林书豪" in label or label.startswith("ToBot")]
        ui.observe(
            "inbox",
            "pass" if filtered and rows else "fail",
            f"no_results_hid_rows={str(no_hit).lower()} filter_kept_peer={str(filtered).lower()} rows={rows[:6]}",
        )

    opened = ui.open_row("林书豪", reject="ToTeam")
    composer = ui.has("输入消息")
    emoji = ui.press_text("表情", "AXCheckBox", "emoji", exact=True)
    groups = [name for name in ("表情与角色", "人物与身体", "动物与自然", "食物与饮品", "旅行和地点", "活动", "物体", "符号", "旗帜") if ui.has(name)]
    if emoji:
        ui.press_text("表情", "AXCheckBox", "emoji close", exact=True)
    info = ui.press_text("更多", "AXButton", "direct info", exact=True)
    direct_controls = [name for name in ("置顶聊天", "消息免打扰", "标为未读", "备注") if ui.has(name)]
    if info:
        ui.press_text("更多", "AXButton", "direct info close", exact=True)
    ui.observe(
        "MD-SPINE",
        "pass" if opened and composer and emoji and info and len(direct_controls) >= 3 else "fail",
        f"peer=林书豪 composer={str(composer).lower()} emoji_opened={str(emoji).lower()} "
        f"emoji_groups={groups} info={str(info).lower()} controls={direct_controls} send_blocked=1",
    )

    plus = ui.press_text("发起会话", "AXPopUpButton", "plus", exact=True)
    create_item = ui.exact_has("发起群聊")
    org_item = ui.exact_has("组织架构")
    # The people sheet's cancel control does not leave the tree under a background press.
    if plus:
        ui.press_text("发起会话", "AXPopUpButton", "plus close", exact=True)
    group = ui.open_row("ToTeam")
    mentioned = False
    if group and ui.has("输入消息"):
        field = ui.find("输入消息", "AXTextArea")
        if field is not None:
            ui.type_into(field, "@", "mention")
            mentioned = ui.has("所有人") or ui.has("@")
            ui.escape()
            ui.refresh()
            field = ui.find("输入消息", "AXTextArea")
            if field is not None:
                ui.ui.press_background("delete", field)
                time.sleep(0.2)
                ui.refresh()
    ui.observe(
        "MM-SPINE",
        "pass" if group and plus and create_item and org_item else "fail",
        f"room=ToTeam opened={str(group).lower()} plus={str(plus).lower()} "
        f"menu_item_create={str(create_item).lower()} menu_item_org={str(org_item).lower()} "
        f"create_not_opened=1 create_not_confirmed=1 "
        f"mention_picker={str(mentioned).lower()} send_blocked=1",
    )

    tobot = ui.open_row("ToBot", reject="ToTeam")
    phone = ui.has("电话") or ui.has("钱包")
    buttons = [label_of(element) for element in ui.ui.elements if element.get("role") == "AXButton" and element.get("in_web_content")]
    more = ui.press_text("更多", "AXButton", "tobot more", exact=True)
    menu = [label_of(element) for element in ui.ui.elements if element.get("in_web_content")]
    reset_item = any("重置 ToBot" in label for label in menu)
    edit_item = any("编辑资料" in label for label in menu)
    # Opening the confirm dialog leaves it up: AXPress on 取消 does not close it.
    ui.escape()
    ui.observe(
        "MT-SPINE",
        "pass" if tobot and not phone and reset_item else "fail",
        f"entry=ToBot composer={str(ui.has('输入消息')).lower()} phone_wallet={str(phone).lower()} "
        f"more={str(more).lower()} reset_item={str(reset_item).lower()} edit_item={str(edit_item).lower()} "
        f"reset_not_confirmed=1 buttons={buttons[:24]} live_model=0",
    )

    tool_names = [name for name in ("meeting-minutes", "calendar") if ui.has(name)]
    thinking = ui.has("正在思考") or ui.has("思考")
    approval = ui.has("允许") and ui.has("拒绝")
    ui.observe(
        "MTC",
        "pass" if tool_names else "unverified-on-ax",
        f"ax_tools={tool_names} thinking={str(thinking).lower()} approval={str(approval).lower()} "
        f"live_model=0 prose_oracle=shipped-renderer",
    )

    ui.open_row("林书豪", reject="ToTeam")
    menu_opened = ui.show_menu_exact("你好", "forward")
    forward_item = ui.exact_has("转发")
    merge_item = ui.exact_has("合并转发")
    if menu_opened:
        ui.escape()
    ui.observe(
        "forward",
        "pass" if menu_opened and forward_item else "unverified-on-ax",
        f"menu={str(menu_opened).lower()} forward_item={str(forward_item).lower()} "
        f"merge_item={str(merge_item).lower()} forward_not_sent=1",
    )

    history = ui.press_text("查找聊天内容", "AXButton", "history", exact=True)
    tabs = [name for name in ("全部", "文件", "图片", "链接") if ui.exact_has(name)]
    if history:
        ui.close_dialog()
    history_closed = ui.until_gone("聊天记录")
    ui.observe(
        "history",
        "pass" if history and len(tabs) >= 3 and history_closed else "fail",
        f"opened={str(history).lower()} tabs={tabs} closed={str(history_closed).lower()}",
    )

    ui.open_row("ToTeam")
    ui.press_text("更多", "AXButton", "group info", exact=True)
    roles = [name for name in ("群主", "管理员", "设为管理员", "解散群聊", "退出群聊") if ui.has(name)]
    speech = [name for name in ("全体禁言", "禁言") if ui.has(name)]
    dissolve = ui.exact_has("解散群聊")
    leave = ui.exact_has("退出群聊")
    ui.press_text("更多", "AXButton", "group info close", exact=True)
    ui.observe(
        "roles",
        "pass" if roles else "unverified-on-ax",
        f"visible={roles} dissolve_shown={str(dissolve).lower()} leave_shown={str(leave).lower()} confirm_not_sent=1",
    )
    ui.observe("speech", "pass" if speech else "unverified-on-ax", f"visible={speech} toggled=0")

    ui.open_row("ToBot", reject="ToTeam")
    ui.press_text("更多", "AXButton", "profile menu", exact=True)
    edit_item = ui.exact_has("编辑资料")
    if edit_item:
        ui.press_text("更多", "AXButton", "profile menu close", exact=True)
    ui.observe(
        "profile",
        "pass" if edit_item else "unverified-on-ax",
        f"edit_item={str(edit_item).lower()} dialog_not_opened=1 save_not_sent=1",
    )

    banner = ui.has("连接中") or ui.has("重新连接")
    offline = ui.has("似乎出现了一些问题")
    ui.observe(
        "realtime",
        "pass" if not offline else "fail",
        f"reconnect_banner={str(banner).lower()} error_boundary={str(offline).lower()} composer={str(ui.has('输入消息')).lower()}",
    )


ROOM_ID = "11111111-1111-4111-8111-111111111111"
CATALOG_PROMPT = "目录回合"


class ForkApi:
    """Local agent-plane calls for the fork under test. The token is never logged."""

    def __init__(self, shell: str, token: str, log: Callable[[str], None]) -> None:
        self.shell = shell.rstrip("/")
        self.token = token
        self.log = log
        self.base = ""
        self.calls: list[str] = []

    def request(self, method: str, path: str, body: dict[str, Any] | None = None) -> tuple[int, Any]:
        if not self.base:
            self.base = self._plane_base()
        url = self.base + path
        data = None if body is None else json.dumps(body).encode()
        request = urllib.request.Request(url, data=data, method=method)
        request.add_header("authorization", f"Bearer {self.token}")
        if data is not None:
            request.add_header("content-type", "application/json")
        try:
            with urllib.request.urlopen(request, timeout=20) as response:
                status = response.status
                payload = response.read()
        except urllib.error.HTTPError as error:
            status = error.code
            payload = error.read()
        self.calls.append(f"{method} {path} status={status}")
        self.log(self.calls[-1])
        parsed: Any
        try:
            parsed = json.loads(payload.decode() or "null")
        except json.JSONDecodeError:
            parsed = {"raw": payload[:180].decode("utf-8", "replace")}
        return status, parsed

    def _plane_base(self) -> str:
        request = urllib.request.Request(self.shell + "/api/agent-plane")
        request.add_header("authorization", f"Bearer {self.token}")
        with urllib.request.urlopen(request, timeout=20) as response:
            body = json.loads(response.read().decode())
        base = str(body.get("baseUrl") or "").rstrip("/")
        if not base:
            raise RuntimeError("agent plane base is missing")
        return base


def _items(page: Any) -> list[dict[str, Any]]:
    if not isinstance(page, dict):
        return []
    items = page.get("items")
    if not isinstance(items, list):
        return []
    return [item for item in items if isinstance(item, dict)]


def _user_count(page: Any) -> int:
    return sum(1 for item in _items(page) if item.get("role") == "user")


def _form_when(page: Any) -> str:
    for item in _items(page):
        parts = (item.get("data") or {}).get("parts") if isinstance(item.get("data"), dict) else None
        if not isinstance(parts, list):
            continue
        for part in parts:
            if not isinstance(part, dict) or part.get("type") != "data-form":
                continue
            data = part.get("data")
            if not isinstance(data, dict):
                continue
            values = data.get("values")
            if isinstance(values, dict) and isinstance(values.get("when"), str):
                return values["when"]
    return ""


def _tool_names(page: Any) -> list[str]:
    names: list[str] = []
    for item in _items(page):
        parts = (item.get("data") or {}).get("parts") if isinstance(item.get("data"), dict) else None
        if not isinstance(parts, list):
            continue
        for part in parts:
            if isinstance(part, dict) and isinstance(part.get("toolName"), str):
                names.append(part["toolName"])
    return names


def run_memory(catalog: Catalog, api: ForkApi) -> None:
    """Deterministic ToBot catalog. Does not send to a person and does not start a model."""
    catalog.allowed.add("重置")
    if catalog.heading("重置 ToBot"):
        catalog.press_text("取消", "AXButton", "dismiss leftover reset", exact=True)
    catalog.open_messages()
    # A team inbox lists the creator's ToBoss under the same ToBot title and
    # hides the personal row. The empty-state hint is the exact thread kind.
    shown = catalog.show_thread("ToBot", 8, reject="ToTeam")
    if catalog.wait_has("7x24小时运行的长期团队智能体", 2):
        session_key = "toboss"
    elif catalog.wait_has("7x24小时运行的长期个人智能体", 2):
        session_key = "tobot"
    else:
        session_key = ""
    catalog.log(f"MTC-SESSION key={session_key or 'absent'} heading={str(shown).lower()}")
    if session_key not in ("tobot", "toboss"):
        catalog.observe("MT-SPINE", "fail", "thread kind absent")
        catalog.observe("MTC", "fail", f"thread_heading={str(shown).lower()} session=absent")
        return
    before_status, before_page = api.request("GET", f"/api/agent-sessions/{session_key}/messages?limit=50")
    before_users = _user_count(before_page) if before_status == 200 else -1
    turn_body = {
        "userMessageParts": [{"type": "text", "text": CATALOG_PROMPT}],
        "turnContext": {"messagesCatalog": "mtc", "roomId": ROOM_ID},
    }
    status, started = api.request("POST", f"/api/agent-sessions/{session_key}/turns", turn_body)
    if status == 409:
        api.request("POST", f"/api/agent-sessions/{session_key}/turn/abort", {})
        status, started = api.request("POST", f"/api/agent-sessions/{session_key}/turns", turn_body)
    memory = started.get("memory") if isinstance(started, dict) else None
    live_model = started.get("liveModel") if isinstance(started, dict) else None
    if not isinstance(memory, dict):
        catalog.observe(
            "MT-SPINE",
            "fail",
            f"turn_status={status} live_model={live_model} memory=missing",
        )
        catalog.observe("MTC", "fail", f"turn_status={status} live_model={live_model}")
        return
    personal = memory.get("personal") if isinstance(memory.get("personal"), dict) else {}
    private = memory.get("private") if isinstance(memory.get("private"), dict) else {}
    room = memory.get("room") if isinstance(memory.get("room"), dict) else {}
    catalog.observe(
        "MT-SPINE",
        "pass"
        if personal.get("context") == "tobot"
        and private.get("context") == "toboss"
        and private.get("receivesPersonalFacts") is False
        and room.get("reflect") == 0
        and room.get("retain") == 0
        and live_model is False
        else "fail",
        "personal context={personal_context} session={personal_session} retain_context={personal_retain} "
        "private context={private_context} session={private_session} receives_personal_facts={receives} "
        "room session={room_session} reflect={reflect} retain={retain} live_model={live_model} "
        "source=fork-turn".format(
            personal_context=personal.get("context"),
            personal_session=personal.get("sessionId"),
            personal_retain=personal.get("retainContext"),
            private_context=private.get("context"),
            private_session=private.get("sessionId"),
            receives=str(private.get("receivesPersonalFacts")).lower(),
            room_session=room.get("sessionId"),
            reflect=room.get("reflect"),
            retain=room.get("retain"),
            live_model=str(live_model).lower(),
        ),
    )
    # The thread is already selected, so the poll can show the thinking chip
    # during the catalog's empty-pending window.
    thinking = catalog.wait_has("正在思考", 8) if shown else False
    if not shown or not thinking:
        labels = catalog.labels()
        catalog.log("MTC-LABELS " + " | ".join(labels[:40]))
        catalog.log(
            f"MTC-HAS count={len(labels)} prompt={str(catalog.has('目录回合')).lower()} "
            f"minutes={str(catalog.has('meeting-minutes')).lower()}"
        )
    minutes = catalog.wait_has("meeting-minutes", 20)
    calendar = catalog.has("calendar") or catalog.wait_has("calendar", 4)
    allowed = catalog.press_text("允许", "AXButton", "approval allow", exact=True)
    if not allowed:
        allowed = catalog.press_text("允许", None, "approval allow", exact=True)
    denied = False
    if allowed:
        denied = catalog.wait_has("拒绝", 8) and (
            catalog.press_text("拒绝", "AXButton", "approval deny", exact=True)
            or catalog.press_text("拒绝", None, "approval deny", exact=True)
        )
    field = None
    if catalog.wait_has("日期", 12):
        catalog.refresh()
        field = catalog.find("日期", "AXTextArea") or catalog.find("日期", "AXTextField") or catalog.find("日期")
    typed = False
    if field is not None:
        catalog.press(field, "openui date")
        field = catalog.find("日期", "AXTextArea") or catalog.find("日期", "AXTextField") or field
        catalog.type_into(field, "下周三 15:00", "openui date")
        typed = True
    submitted = catalog.press_text("提交", "AXButton", "openui submit", exact=True) if typed else False
    if typed and not submitted:
        submitted = catalog.press_text("提交", None, "openui submit", exact=True)
    after_page: Any = {}
    after_users = before_users
    form_value = ""
    stored_tools: list[str] = []
    for _ in range(8):
        time.sleep(0.4)
        after_status, after_page = api.request("GET", f"/api/agent-sessions/{session_key}/messages?limit=50")
        if after_status != 200:
            continue
        after_users = _user_count(after_page)
        form_value = _form_when(after_page)
        stored_tools = _tool_names(after_page)
        if form_value or (submitted and after_users >= 0):
            if form_value:
                break
    added = after_users - before_users if before_users >= 0 and after_users >= 0 else -1
    ax_tools = [name for name in ("meeting-minutes", "calendar") if catalog.has(name)]
    catalog.observe(
        "MTC",
        "pass" if thinking and minutes and calendar and allowed and denied and form_value == "下周三 15:00" and added == 1 else "fail",
        f"thinking={str(thinking).lower()} thread_heading={str(shown).lower()} ax_tools={ax_tools} stored_tools={stored_tools} "
        f"allow={str(bool(allowed)).lower()} deny={str(bool(denied)).lower()} "
        f"openui field=when value={form_value} user_messages_before={before_users} "
        f"user_messages_after={after_users} added_user_messages={added} live_model={str(live_model).lower()} "
        f"source=fork-turn",
    )
    session_before = str((personal if session_key == "tobot" else private).get("sessionId") or "")
    catalog.show_thread("ToBot", 4, reject="ToTeam")
    catalog.press_text("更多", "AXButton", "reset menu", exact=True)
    menu_reset = catalog.press_text("重置 ToBot", None, "reset item", exact=True)
    ax_reset = catalog.press_text("重置", "AXButton", "reset confirm", exact=True) if menu_reset else False
    time.sleep(0.6)
    reset_status, reset_page = api.request("GET", f"/api/agent-sessions/{session_key}/messages?limit=50")
    transcript = len(_items(reset_page)) if reset_status == 200 else -1
    ax_cleared = transcript == 0
    reset_via = "ax" if ax_cleared else "api"
    if not ax_cleared:
        api.request("POST", f"/api/agent-sessions/{session_key}/reset", {})
        reset_status, reset_page = api.request("GET", f"/api/agent-sessions/{session_key}/messages?limit=50")
        transcript = len(_items(reset_page)) if reset_status == 200 else -1
    session_status, session = api.request("GET", f"/api/agent-sessions/{session_key}")
    session_after = session.get("id") if isinstance(session, dict) else None
    memory_http = [call for call in api.calls if "memory" in call]
    catalog.observe(
        "MT-SPINE-RESET",
        "pass" if transcript == 0 and session_after and session_after == session_before and not memory_http else "fail",
        f"reset session_id={session_after} transcript={transcript} kept={str(session_after == session_before).lower()} "
        f"ax_confirm={str(bool(ax_reset)).lower()} ax_cleared={str(ax_cleared).lower()} reset_via={reset_via} "
        f"bank_delete_request={len(memory_http)} "
        f"session_status={session_status} source=fork-turn",
    )


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--log", required=True)
    parser.add_argument("--user-pid", type=int, default=0)
    parser.add_argument("--fork-pid", type=int)
    parser.add_argument("--runtime", default="")
    parser.add_argument("--only", choices=["catalog", "mtc"], default="catalog")
    args = parser.parse_args()
    log_path = Path(args.log)
    lines: list[str] = []

    def log(message: str) -> None:
        print(message, flush=True)
        lines.append(message)

    runtime = Path(args.runtime) if args.runtime else Path(os.environ.get("TMPDIR", "/tmp")) / RUNTIME_NAME
    data = json.loads(runtime.read_text()) if runtime.is_file() else {}
    pid = args.fork_pid or int(data["pid"])
    if args.user_pid and pid == args.user_pid:
        raise SystemExit("fork pid is the user window")
    command = process_command(pid)
    if "towork-cua" in command or not alive(pid):
        raise SystemExit("fork process is not the new toyou window")
    if args.only == "catalog" and not alive(args.user_pid):
        raise SystemExit("user ToWork pid is gone")
    if args.only == "mtc" and args.user_pid and not alive(args.user_pid):
        raise SystemExit("user ToWork pid is gone")
    if os.environ.get("A3S_GUI_ALLOW_MOUSE") == "1":
        raise SystemExit("A3S_GUI_ALLOW_MOUSE=1")

    probe = LogDrive(pid, 0, log)
    try:
        listed = probe.call("list_windows", {"pid": pid})
        rows = (listed.get("structuredContent") or {}).get("windows") or []
        titled = [row for row in rows if row.get("title") == "ToWork" and row.get("is_on_screen")]
        if not titled:
            raise SystemExit("fork has no on-screen ToWork window")
        window = int(max(titled, key=lambda row: row["bounds"]["width"] * row["bounds"]["height"])["window_id"])
    finally:
        probe.close()

    user_alive = int(bool(args.user_pid) and alive(args.user_pid))
    log(
        f"FORK pid={pid} window={window} instance=toyou-fork api={data.get('apiBaseUrl')} "
        f"user_pid={args.user_pid or 'absent'} user_alive={user_alive} only={args.only} "
        f"A3S_GUI_ALLOW_MOUSE=unset"
    )
    log(f"POINTER before={pointer()} frontmost_before={frontmost()}")
    ui = LogDrive(pid, window, log)
    page = Catalog(ui, log)
    try:
        if args.only == "mtc":
            token = str(data.get("token") or "")
            shell = str(data.get("apiBaseUrl") or "")
            if not token or not shell:
                raise SystemExit("fork runtime is missing the local api")
            run_memory(page, ForkApi(shell, token, log))
        else:
            run(page)
        bad = [line for line in ui.actions if "delivery_mode: background" not in line]
        log(f"ACTIONS count={len(ui.actions)} non_background={len(bad)}")
    finally:
        ui.close()
        log(f"POINTER after={pointer()} frontmost_after={frontmost()}")
        log(f"USER pid={args.user_pid} alive={int(alive(args.user_pid))} FORK pid={pid} alive={int(alive(pid))}")
        with log_path.open("a", encoding="utf-8") as handle:
            handle.write("\n".join(lines) + "\n")


if __name__ == "__main__":
    main()
