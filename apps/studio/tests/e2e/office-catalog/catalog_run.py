"""Office catalog on the live API and the isolated ToWork window.

Every accessibility action uses delivery_mode background. The user's ToWork
processes are never stopped. Tokens and the database URL are not printed.

Usage:
  python3 catalog_run.py eight
"""

from __future__ import annotations

import json
import os
import select
import sys
import time
import uuid
import urllib.error
import urllib.request
from pathlib import Path
from typing import Any

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "towork"))
sys.path.insert(0, str(Path(__file__).resolve().parent))

os.environ.pop("A3S_GUI_ALLOW_MOUSE", None)

from catalog_cua import (  # noqa: E402
    RUNTIME_FILE,
    SESSION_FILE,
    abort_if_unsafe,
    frontmost_bundle,
    isolated_pids,
    pointer,
    user_pids,
    window_for,
)
from driver import Drive  # noqa: E402

SCRATCH = Path("/var/folders/3d/0pn3wn612t7cs79b5ml89x440000gn/T/grok-goal-a4dd21263277/implementer")
API = "http://127.0.0.1:3000/v1/api/org/members"
ENV_FILE = Path("/Users/roylin/code/a3s/apps/api/.env")


def _send_long(self: Drive, payload: dict[str, Any]) -> dict[str, Any]:
    assert self.proc.stdin and self.proc.stdout
    self.proc.stdin.write(json.dumps(payload) + "\n")
    self.proc.stdin.flush()
    ready, _, _ = select.select([self.proc.stdout], [], [], 90)
    if not ready:
        raise RuntimeError("cua-driver timed out")
    line = self.proc.stdout.readline()
    if not line:
        raise RuntimeError("cua-driver closed stdout")
    return json.loads(line)


Drive._send = _send_long  # type: ignore[method-assign]


def database_url() -> str:
    for line in ENV_FILE.read_text().splitlines():
        if line.startswith("DATABASE_URL="):
            return line.split("=", 1)[1].strip().strip('"').strip("'")
    raise RuntimeError("DATABASE_URL is missing")


def sql_one(statement: str, params: tuple[Any, ...]) -> Any:
    import psycopg2

    with psycopg2.connect(database_url()) as conn:
        with conn.cursor() as cur:
            cur.execute(statement, params)
            row = cur.fetchone()
            return None if row is None else row[0]


def load_session() -> dict[str, Any]:
    return json.loads(SESSION_FILE.read_text())


def message_of(payload: Any, raw: str) -> str:
    if isinstance(payload, dict):
        text = payload.get("message") or payload.get("error") or ""
        if text and text != "OK":
            return str(text)[:240]
        if any(key in payload for key in ("id", "version", "nodes", "ok")):
            return ""
    try:
        body = json.loads(raw) if raw else None
    except json.JSONDecodeError:
        return raw[:240]
    if isinstance(body, dict):
        text = body.get("message") or body.get("error") or ""
        nested = body.get("data")
        if not text and isinstance(nested, dict):
            text = nested.get("message") or nested.get("error") or ""
        if text and text != "OK":
            return str(text)[:240]
    return ""


def call(token: str, method: str, path: str, body: dict[str, Any] | None = None) -> tuple[int, Any, str]:
    data = None if body is None else json.dumps(body).encode()
    headers = {"content-type": "application/json", "authorization": f"Bearer {token}"}
    req = urllib.request.Request(API + path, data=data, headers=headers, method=method)
    try:
        with urllib.request.urlopen(req, timeout=30) as response:
            raw = response.read().decode()
            status = response.status
    except urllib.error.HTTPError as error:
        raw = error.read().decode()
        status = error.code
    try:
        payload = json.loads(raw) if raw else None
    except json.JSONDecodeError:
        payload = None
    if isinstance(payload, dict) and payload.get("success") is True and "data" in payload:
        payload = payload["data"]
    return status, payload, message_of(payload, raw)


def nodes_of(payload: Any) -> list[str] | None:
    body = payload
    if isinstance(payload, dict) and isinstance(payload.get("data"), dict):
        body = payload["data"]
    if not isinstance(body, dict) or "version" not in body or not isinstance(body.get("nodes"), list):
        return None
    names = []
    for node in body["nodes"]:
        if isinstance(node, dict) and isinstance(node.get("name"), str):
            names.append(node["name"])
    return names


def item_id(payload: Any) -> str:
    if isinstance(payload, dict) and payload.get("id"):
        return str(payload["id"])
    return ""


def item_status(payload: Any) -> str:
    if isinstance(payload, dict):
        return str(payload.get("status") or "")
    return ""


def preview(token: str, org_id: str, key: str, form: dict[str, Any], amount: str = "") -> tuple[int, Any, str, list[str] | None]:
    status, payload, message = call(
        token,
        "POST",
        "/work-items/process-preview",
        {"org_id": org_id, "process_key": key, "amount": amount, "form": form},
    )
    return status, payload, message, nodes_of(payload)


def start(token: str, org_id: str, key: str, form: dict[str, Any], amount: str = "") -> tuple[int, Any, str]:
    return call(
        token,
        "POST",
        "/work-items/process",
        {
            "org_id": org_id,
            "idempotency_key": str(uuid.uuid4()),
            "process_key": key,
            "amount": amount,
            "form": form,
        },
    )


def decide(token: str, item: str, decision: str, comment: str) -> tuple[int, Any, str]:
    return call(token, "POST", f"/work-items/{item}/decision", {"decision": decision, "comment": comment})


def person_key(people: dict[str, Any], member_id: str) -> str:
    for key, person in people.items():
        if str(person.get("id")) == member_id:
            return key
    return ""


def approve_chain(people: dict[str, Any], payload: Any, item: str, lines: list[str]) -> bool:
    body = payload if isinstance(payload, dict) else {}
    nodes = body.get("nodes") if isinstance(body.get("nodes"), list) else []
    ok = True
    for node in nodes:
        if not isinstance(node, dict):
            continue
        assignees = [str(value) for value in node.get("assignees") or []]
        actor = ""
        for assignee in assignees:
            actor = person_key(people, assignee)
            if actor:
                break
        if not actor:
            lines.append(f"approve missing actor node={node.get('name')}")
            ok = False
            break
        status, decided, message = decide(people[actor]["token"], item, "approve", "同意")
        lines.append(
            f"approve actor={actor} node={node.get('name')} status={status} item_status={item_status(decided)} message={message}"
        )
        if status not in (200, 201):
            ok = False
            break
    return ok


class Background:
    def __init__(self, pid: int, window: int) -> None:
        abort_if_unsafe(pid)
        if pid in set(user_pids()):
            raise RuntimeError("target pid is the user's ToWork")
        self.pid = pid
        self.ui = Drive(pid, window)
        self.actions = 0
        self.non_background = 0
        self.lines: list[str] = []

    def close(self) -> None:
        self.ui.close()

    def snap(self, query: str = "") -> list[dict[str, Any]]:
        arguments: dict[str, Any] = {
            "pid": self.pid,
            "window_id": self.ui.window_id,
            "include_screenshot": False,
            "max_depth": 32,
            "max_elements": 2500,
        }
        if query:
            arguments["query"] = query
        result = self.ui.call("get_window_state", arguments)
        structured = result.get("structuredContent") or {}
        self.ui.elements = list(structured.get("elements") or [])
        return self.ui.elements

    def labels(self, role: str | None = None) -> list[str]:
        found = []
        for element in self.ui.elements:
            if role and element.get("role") != role:
                continue
            label = " ".join(str(element.get("label") or element.get("value") or "").split())
            if label:
                found.append(label)
        return found

    def press_element(self, element: dict[str, Any], note: str) -> bool:
        token = element.get("element_token")
        if not token:
            self.lines.append(f"press {note} missing_token")
            self.non_background += 1
            return False
        before = pointer()
        front_before = frontmost_bundle()
        result = self.ui.call(
            "click",
            {
                "pid": self.pid,
                "window_id": self.ui.window_id,
                "element_token": token,
                "action": "press",
                "delivery_mode": "background",
            },
        )
        after = pointer()
        front_after = frontmost_bundle()
        structured = result.get("structuredContent") or {}
        delivery = structured.get("delivery") if isinstance(structured.get("delivery"), dict) else {}
        mode = str(delivery.get("mode") or "")
        self.actions += 1
        if mode != "background":
            self.non_background += 1
        content = " ".join(
            str(item.get("text") or "") for item in result.get("content") or [] if isinstance(item, dict)
        )
        self.lines.append(
            f"press {note} delivery={mode or 'omitted'} route={structured.get('route')} "
            f"pointer={before[0]},{before[1]}->{after[0]},{after[1]} same={str(before == after).lower()} "
            f"front={front_before}->{front_after} same={str(front_before == front_after).lower()} "
            f"content={content[:120]}"
        )
        return mode == "background"

    def press_label(self, label: str, role: str, note: str) -> bool:
        matches = []
        for element in self.ui.elements:
            text = " ".join(str(element.get("label") or element.get("value") or "").split())
            if element.get("role") == role and text == label and element.get("element_token"):
                matches.append(element)
        if not matches:
            self.lines.append(f"press {note} missing {role} {label}")
            return False
        return self.press_element(matches[0], note)

    def type_text(self, element: dict[str, Any], text: str, note: str) -> bool:
        token = element.get("element_token")
        if not token:
            self.lines.append(f"type {note} missing_token")
            self.non_background += 1
            return False
        before = pointer()
        front_before = frontmost_bundle()
        result = self.ui.call(
            "type_text",
            {
                "pid": self.pid,
                "window_id": self.ui.window_id,
                "element_token": token,
                "text": text,
                "delivery_mode": "background",
                "delay_ms": 0,
            },
        )
        after = pointer()
        front_after = frontmost_bundle()
        structured = result.get("structuredContent") or {}
        delivery = structured.get("delivery") if isinstance(structured.get("delivery"), dict) else {}
        mode = str(delivery.get("mode") or "")
        self.actions += 1
        if mode != "background":
            self.non_background += 1
        self.lines.append(
            f"type {note} delivery={mode or 'omitted'} pointer={before[0]},{before[1]}->{after[0]},{after[1]} "
            f"same={str(before == after).lower()} front={front_before}->{front_after} "
            f"same={str(front_before == front_after).lower()}"
        )
        return mode == "background"

    def set_value(self, element: dict[str, Any], text: str, note: str) -> bool:
        token = element.get("element_token")
        if not token:
            self.lines.append(f"value {note} missing_token")
            self.non_background += 1
            return False
        before = pointer()
        front_before = frontmost_bundle()
        result = self.ui.call(
            "set_value",
            {
                "pid": self.pid,
                "window_id": self.ui.window_id,
                "element_token": token,
                "value": text,
            },
        )
        after = pointer()
        front_after = frontmost_bundle()
        structured = result.get("structuredContent") or {}
        delivery = structured.get("delivery") if isinstance(structured.get("delivery"), dict) else {}
        mode = str(delivery.get("mode") or "")
        self.actions += 1
        if mode != "background":
            self.non_background += 1
        self.lines.append(
            f"value {note} delivery={mode or 'omitted'} pointer={before[0]},{before[1]}->{after[0]},{after[1]} "
            f"same={str(before == after).lower()} front={front_before}->{front_after} "
            f"same={str(front_before == front_after).lower()}"
        )
        return mode == "background"

    def use_window(self, window: int) -> None:
        self.ui.window_id = window

    def windows_titled(self, title: str) -> list[int]:
        result = self.ui.call("list_windows", {"pid": self.pid})
        rows = (result.get("structuredContent") or {}).get("windows") or []
        return [int(row["window_id"]) for row in rows if row.get("title") == title]

    def find_titled(self, title: str) -> int | None:
        found = self.windows_titled(title)
        return found[-1] if found else None

    def bind_ready(self, title: str, before: set[int], ready: tuple[str, ...], timeout: float) -> int | None:
        """Bind a newly opened applet whose tree contains the form.

        Ghost windows keep the applet title and an empty tree. The last id in
        list order is often that ghost, so a fresh id is required while one exists.
        """
        deadline = time.time() + timeout
        while time.time() < deadline:
            current = self.windows_titled(title)
            fresh = [window for window in current if window not in before]
            # A second press focuses the applet that is already open. Use that
            # tree when the press did not allocate a new window id.
            pool = fresh if fresh else current
            for window in sorted(pool, reverse=True):
                self.use_window(window)
                try:
                    self.snap()
                except Exception as error:
                    self.lines.append(f"snap {title} {window} {error}")
                    continue
                labels = self.labels()
                if "允许" in labels:
                    self.press_label("允许", "AXButton", "permission-allow")
                    time.sleep(0.8)
                    break
                if any(name in labels for name in ready):
                    reused = "reused" if window in before else "new"
                    self.lines.append(
                        f"{title} window={window} elements={len(self.ui.elements)} {reused}"
                    )
                    return window
            time.sleep(0.4)
        self.lines.append(f"{title} form missing")
        return None


def frame_width(element: dict[str, Any]) -> float:
    frame = element.get("frame") or {}
    return float(frame.get("w") or frame.get("width") or 0)


def has_heading(elements: list[dict[str, Any]], name: str) -> bool:
    return any(
        element.get("role") in {"AXHeading", "AXStaticText"}
        and name in str(element.get("label") or "")
        for element in elements
    )


def show_workbench(ui: Background) -> bool:
    elements = ui.snap()
    if has_heading(elements, "人事专区"):
        return True
    benches = [
        element
        for element in elements
        if element.get("role") == "AXButton"
        and " ".join(str(element.get("label") or "").split()) == "工作台"
        and element.get("element_token")
    ]
    if not benches or not ui.press_element(max(benches, key=frame_width), "sidebar-workbench"):
        return False
    time.sleep(0.6)
    return has_heading(ui.snap(), "人事专区")


def open_team_workbench(ui: Background) -> bool:
    elements = ui.snap()
    if has_heading(elements, "人事专区"):
        ui.lines.append("workbench already open")
        return True
    team = [
        element
        for element in elements
        if element.get("role") == "AXButton"
        and " ".join(str(element.get("label") or "").split()) == "团队"
        and element.get("element_token")
    ]
    if team and ui.press_element(min(team, key=frame_width), "activity-team"):
        time.sleep(0.8)
        if show_workbench(ui):
            return True
    popup = next(
        (
            element
            for element in ui.ui.elements
            if element.get("role") == "AXPopUpButton" and element.get("element_token")
        ),
        None,
    )
    if popup is None:
        ui.lines.append("account popup missing")
        return False
    if not ui.press_element(popup, "account-menu"):
        return False
    time.sleep(0.4)
    ui.snap("团队")
    radio = next(
        (
            element
            for element in ui.ui.elements
            if element.get("role") == "AXRadioButton"
            and " ".join(str(element.get("label") or "").split()) == "团队"
            and element.get("element_token")
        ),
        None,
    )
    if radio is None or not ui.press_element(radio, "scope-team"):
        ui.lines.append("team radio missing")
        return False
    time.sleep(0.4)
    ui.snap("办公目录甲")
    row = next(
        (
            element
            for element in ui.ui.elements
            if element.get("role") == "AXMenuItem"
            and str(element.get("label") or "").startswith("办公目录甲")
            and element.get("element_token")
        ),
        None,
    )
    if row is None or not ui.press_element(row, "org-row"):
        ui.lines.append("org row missing")
        return False
    time.sleep(0.5)
    ui.snap()
    team = [
        element
        for element in ui.ui.elements
        if element.get("role") == "AXButton"
        and " ".join(str(element.get("label") or "").split()) == "团队"
        and element.get("element_token")
    ]
    if not team or not ui.press_element(min(team, key=frame_width), "activity-team-after-org"):
        ui.lines.append("activity team missing after org")
        return False
    time.sleep(0.8)
    return show_workbench(ui)


def zone_case(ui: Background, main_window: int) -> list[str]:
    ui.use_window(main_window)
    open_team_workbench(ui)
    elements = ui.snap()
    headings = []
    for name in ("人事专区", "财务专区", "研发专区", "行政专区"):
        if has_heading(elements, name):
            headings.append(name)
    buttons = {}
    for element in elements:
        if element.get("role") != "AXButton":
            continue
        label = " ".join(str(element.get("label") or "").split())
        if label:
            buttons.setdefault(label, 0)
            buttons[label] += 1
    correction = ui.snap("补卡")
    correction_buttons = [
        element
        for element in correction
        if element.get("role") == "AXButton" and "补卡" in str(element.get("label") or "")
    ]
    expected = ["人事专区", "财务专区", "研发专区", "行政专区"]
    tiles_ok = all(name in buttons for name in ("请假", "转正", "打卡", "报销", "出差", "AI 资源", "审批", "会议", "用印"))
    passed = headings == expected and tiles_ok and not correction_buttons
    return [
        "case WB-ZONE",
        f"control headings={','.join(headings)}",
        f"control tiles={','.join(f'{name}:{count}' for name, count in buttons.items())}",
        f"control correction_tile={len(correction_buttons)}",
        f"result={'pass' if passed else 'fail'}",
    ]


def open_leave(ui: Background, main_window: int) -> bool:
    ui.use_window(main_window)
    ui.snap("请假")
    matches = []
    for element in ui.ui.elements:
        if element.get("role") != "AXButton":
            continue
        if " ".join(str(element.get("label") or "").split()) != "请假":
            continue
        if element.get("element_token"):
            matches.append(element)
    if not matches:
        ui.lines.append("leave tile missing")
        return False
    chosen = max(matches, key=lambda element: float((element.get("frame") or {}).get("y") or 0))
    before = set(ui.windows_titled("请假"))
    if not ui.press_element(chosen, "zone-leave"):
        return False
    return ui.bind_ready("请假", before, ("请假类型", "事由"), 25) is not None


def element_label(element: dict[str, Any]) -> str:
    return " ".join(str(element.get("label") or element.get("value") or "").split())


def press_matching(ui: Background, role: str, note: str, predicate) -> bool:
    ui.snap()
    for element in ui.ui.elements:
        if element.get("role") != role or not element.get("element_token"):
            continue
        if predicate(element_label(element)):
            return ui.press_element(element, note)
    ui.lines.append(f"press {note} missing {role}")
    return False


def fill_personal_leave(ui: Background) -> list[str]:
    lines = ["case LV-PERSONAL gui"]
    ui.snap()
    native = [
        element.get("role")
        for element in ui.ui.elements
        if element.get("role") in {"AXDateField", "AXTimeField"}
        or (
            element.get("role") == "AXPopUpButton"
            and "请假" in element_label(element)
        )
    ]
    roles = sorted({str(element.get("role")) for element in ui.ui.elements if element.get("role") not in {"AXMenuItem", "AXMenuBarItem", "AXMenu"}})
    lines.append(f"control roles={','.join(roles)}")
    lines.append(f"control native_date_or_select={native}")
    submitters = [
        element
        for element in ui.ui.elements
        if element.get("role") == "AXButton" and str(element.get("label") or "") == "提交"
    ]
    if submitters:
        enabled = submitters[0].get("enabled")
        lines.append(f"control submit_before enabled={enabled}")
    ui.snap()
    opened = False
    for element in ui.ui.elements:
        label = " ".join(str(element.get("label") or "").split())
        if element.get("role") in {"AXComboBox", "AXPopUpButton", "AXButton"} and "请假类型" in label:
            opened = ui.press_element(element, "leave-type")
            break
    if not opened:
        lines.append("control leave_type_control=missing")
        lines.append("result=fail")
        return lines
    time.sleep(0.3)
    ui.snap()
    menu = [label for label in ("事假", "婚假", "病假", "年假", "丧假", "产假", "陪产假", "育儿假") if label in ui.labels()]
    lines.append(f"control leave_menu={','.join(menu)}")
    lines.append(f"control tiaoxiu={'调休' in ui.labels()}")
    picked = False
    for element in ui.ui.elements:
        label = " ".join(str(element.get("label") or "").split())
        if label == "事假" and element.get("element_token") and element.get("role") in {"AXStaticText", "AXButton", "AXMenuItem"}:
            picked = ui.press_element(element, "leave-personal")
            break
    lines.append(f"control leave_type_selected={str(picked).lower()}")
    date_pressed = press_matching(
        ui,
        "AXButton",
        "leave-start",
        lambda label: label == "开始日期" or label.startswith("开始日期"),
    )
    lines.append(f"control start_opened={str(date_pressed).lower()}")
    def pick_day(note: str, needle: str) -> bool:
        # The day name is on the inner text. A press on that text selects the day.
        if press_matching(ui, "AXStaticText", note, lambda label: needle in label):
            return True
        return press_matching(ui, "AXButton", note, lambda label: needle in label)

    def pick_span() -> None:
        pick_day("day-19", "2026年10月19日")
        time.sleep(0.5)
        if not pick_day("day-22", "2026年10月22日"):
            press_matching(ui, "AXButton", "leave-end", lambda label: label.startswith("结束日期"))
            time.sleep(0.4)
            pick_day("day-22", "2026年10月22日")

    if date_pressed:
        time.sleep(0.5)
        pick_span()
        time.sleep(0.6)
        ui.snap()
        if "2026-10-19" not in " ".join(ui.labels()):
            press_matching(ui, "AXButton", "leave-start-retry", lambda label: label.startswith("开始日期"))
            time.sleep(0.5)
            pick_span()
    time.sleep(0.6)
    ui.snap()
    blob_dates = " ".join(ui.labels())
    dates_ok = "2026-10-19" in blob_dates and "2026-10-22" in blob_dates
    lines.append(f"control dates={str(dates_ok).lower()}")
    reason = None
    for element in ui.ui.elements:
        label = " ".join(str(element.get("label") or element.get("value") or "").split())
        if element.get("role") in {"AXTextArea", "AXTextField"} and "事由" in label and element.get("element_token"):
            reason = element
            break
    if reason is not None:
        # Key events are refused while this process has another eligible window.
        # Minimizing that window lets the background keystrokes reach this form.
        current = ui.ui.window_id
        for window in ui.windows_titled("ToWork"):
            if window == current:
                continue
            ui.use_window(window)
            ui.snap()
            chrome = [
                element
                for element in ui.ui.elements
                if element.get("role") == "AXButton"
                and not element.get("in_web_content")
                and element.get("element_token")
            ]
            chrome.sort(key=lambda element: float((element.get("frame") or {}).get("x") or 0))
            if len(chrome) >= 2:
                ui.press_element(chrome[1], "minimize-sibling")
        ui.use_window(current)
        ui.snap()
        reason = next(
            (
                element
                for element in ui.ui.elements
                if element.get("role") in {"AXTextArea", "AXTextField"}
                and "事由" in element_label(element)
                and element.get("element_token")
            ),
            None,
        )
    if reason is not None:
        typed = ui.type_text(reason, "事假四天", "reason")
        ui.snap()
        stored = next(
            (
                str(element.get("value") or "")
                for element in ui.ui.elements
                if element.get("role") in {"AXTextArea", "AXTextField"} and "事由" in element_label(element)
            ),
            "",
        )
        lines.append(f"control reason={stored if typed and stored == '事假四天' else 'failed'}")
    else:
        lines.append("control reason=missing")
    route = False
    deadline = time.time() + 8
    while time.time() < deadline:
        time.sleep(0.6)
        ui.snap()
        route = "直属主管" in ui.labels()
        if route:
            break
    lines.append(f"control route_manager={str(route).lower()}")
    hr = "HR" in ui.labels()
    lines.append(f"control route_hr={str(hr).lower()}")
    ui.snap()
    submitted = False
    for element in ui.ui.elements:
        if element.get("role") == "AXButton" and str(element.get("label") or "") == "提交" and element.get("element_token"):
            enabled = element.get("enabled")
            lines.append(f"control submit_ready enabled={enabled}")
            if enabled is False:
                break
            submitted = ui.press_element(element, "leave-submit")
            break
    confirmed = False
    if submitted:
        time.sleep(0.8)
        ui.snap()
        blob = " ".join(ui.labels())
        confirmed = "已提交" in blob
        lines.append(f"control submitted_text={str(confirmed).lower()}")
        lines.append(f"control submitted_blob={blob[:180]}")
        ui.press_label("关闭", "AXButton", "leave-close")
    lines.append(f"result={'pass' if route and not hr and submitted and confirmed and dates_ok else 'fail'}")
    return lines


def run_api(session: dict[str, Any]) -> list[str]:
    org_id = str(session["org_id"])
    people = session["people"]
    applicant = people["applicant"]["token"]
    lines: list[str] = []

    status, _payload, message, names = preview(
        applicant,
        org_id,
        "leave",
        {"leave_type": "事假", "start_date": "2026-10-19", "end_date": "2026-10-22", "reason": "事假四天目录"},
    )
    started, started_body, started_message = start(
        applicant,
        org_id,
        "leave",
        {"leave_type": "事假", "start_date": "2026-10-19", "end_date": "2026-10-22", "reason": "事假四天目录"},
    )
    started_id = item_id(started_body)
    persisted = sql_one("select status from org_work_items where id = %s::uuid", (started_id,)) if started_id else None
    personal_ok = names == ["直属主管"] and started in (200, 201) and started_id and persisted == "pending"
    lines += [
        "case LV-PERSONAL",
        f"http preview status={status} nodes={','.join(names or [])} message={message}",
        f"http start status={started} id={started_id} item_status={item_status(started_body)} message={started_message}",
        f"persisted status={persisted}",
        f"result={'pass' if personal_ok else 'fail'}",
    ]

    status, payload, message, names = preview(
        applicant,
        org_id,
        "leave",
        {"leave_type": "年假", "start_date": "2026-10-19", "end_date": "2026-10-21", "reason": "年假未配置目录"},
    )
    before = sql_one(
        "select count(*) from org_work_items where org_id = %s::uuid and payload::text like %s",
        (org_id, "%年假未配置目录%"),
    )
    started, started_body, started_message = start(
        applicant,
        org_id,
        "leave",
        {"leave_type": "年假", "start_date": "2026-10-19", "end_date": "2026-10-21", "reason": "年假未配置目录"},
    )
    after = sql_one(
        "select count(*) from org_work_items where org_id = %s::uuid and payload::text like %s",
        (org_id, "%年假未配置目录%"),
    )
    balance_ok = "尚未配置余额" in started_message and after == before and not item_id(started_body)
    lines += [
        "case LV-NO-BALANCE",
        f"http preview status={status} nodes={','.join(names or [])} message={message}",
        f"http start status={started} id={item_id(started_body)} message={started_message}",
        f"persisted rows_before={before} rows_after={after}",
        f"result={'pass' if balance_ok else 'fail'}",
    ]

    low_status, _low, low_message, low_names = preview(
        applicant,
        org_id,
        "expense",
        {"category": "其他", "spent_on": "2026-10-16", "detail": "五千目录"},
        "5000",
    )
    high_status, _high, high_message, high_names = preview(
        applicant,
        org_id,
        "expense",
        {"category": "其他", "spent_on": "2026-10-16", "detail": "五千零一目录"},
        "5001",
    )
    lead_ok = low_names == ["直属主管", "财务"] and high_names == ["直属主管", "财务", "财务负责人"]
    lines += [
        "case EX-LEAD",
        f"http preview5000 status={low_status} nodes={','.join(low_names or [])} message={low_message}",
        f"http preview5001 status={high_status} nodes={','.join(high_names or [])} message={high_message}",
        "http process=not_started",
        f"result={'pass' if lead_ok else 'fail'}",
    ]

    pay_status, pay_body, pay_message, pay_names = preview(
        applicant,
        org_id,
        "expense",
        {"category": "其他", "spent_on": "2026-10-16", "detail": "报销付款目录"},
        "5000",
    )
    started, started_body, started_message = start(
        applicant,
        org_id,
        "expense",
        {"category": "其他", "spent_on": "2026-10-16", "detail": "报销付款目录"},
        "5000",
    )
    pay_id = item_id(started_body)
    approved = approve_chain(people, pay_body, pay_id, lines) if pay_id else False
    pay, pay_result, pay_text = call(people["finance"]["token"], "POST", f"/work-items/{pay_id}/payment") if pay_id else (0, None, "no id")
    manager_pay, _manager_body, manager_text = (
        call(people["manager"]["token"], "POST", f"/work-items/{pay_id}/payment") if pay_id else (0, None, "no id")
    )
    missing = str(uuid.uuid4())
    missing_status, _missing_body, missing_text = call(people["manager"]["token"], "POST", f"/work-items/{missing}/payment")
    persisted = sql_one("select status from org_work_items where id = %s::uuid", (pay_id,)) if pay_id else None
    pay_fields = json.dumps(pay_result, ensure_ascii=False) if isinstance(pay_result, dict) else ""
    manager_hides = "work item not found" in manager_text and "报销付款目录" not in manager_text and "5000" not in manager_text
    missing_hides = "work item not found" in missing_text
    pay_ok = (
        pay_names == ["直属主管", "财务"]
        and approved
        and pay in (200, 201)
        and item_status(pay_result) == "paid"
        and persisted == "paid"
        and manager_hides
        and missing_hides
        and manager_text == missing_text
    )
    lines += [
        "case EX-PAY",
        f"http preview status={pay_status} nodes={','.join(pay_names or [])} message={pay_message}",
        f"http start status={started} id={pay_id} message={started_message}",
        f"http payment status={pay} item_status={item_status(pay_result)} message={pay_text}",
        f"http manager status={manager_pay} message={manager_text}",
        f"http missing status={missing_status} message={missing_text}",
        f"persisted status={persisted}",
        f"result={'pass' if pay_ok else 'fail'}",
    ]
    del pay_fields

    overlap_form = {"leave_type": "事假", "start_date": "2026-10-01", "end_date": "2026-10-02", "reason": "重叠请假目录"}
    _leave_preview_status, leave_preview, _leave_preview_message, _leave_names = preview(applicant, org_id, "leave", overlap_form)
    leave_status, leave_body, leave_message = start(applicant, org_id, "leave", overlap_form)
    leave_id = item_id(leave_body)
    leave_approved = approve_chain(people, leave_preview, leave_id, lines) if leave_id else False
    travel_form = {"destination": "上海", "start_date": "2026-10-01", "end_date": "2026-10-03", "reason": "重叠出差目录"}
    before = sql_one(
        "select count(*) from org_work_items where org_id = %s::uuid and payload::text like %s",
        (org_id, "%重叠出差目录%"),
    )
    travel_status, travel_body, travel_message = start(applicant, org_id, "travel", travel_form)
    after = sql_one(
        "select count(*) from org_work_items where org_id = %s::uuid and payload::text like %s",
        (org_id, "%重叠出差目录%"),
    )
    overlap_ok = (
        leave_approved
        and "出差与请假冲突" in travel_message
        and "2026-10-01" in travel_message
        and "2026-10-02" in travel_message
        and after == before
        and not item_id(travel_body)
    )
    lines += [
        "case TR-OVERLAP",
        f"http leave status={leave_status} id={leave_id} message={leave_message}",
        f"http travel status={travel_status} id={item_id(travel_body)} message={travel_message}",
        f"persisted rows_before={before} rows_after={after}",
        f"result={'pass' if overlap_ok else 'fail'}",
    ]

    clock_status, _clock_body, clock_message, clock_names = preview(applicant, org_id, "clock", {"punch_type": "上班"})
    punch_status, punch_body, punch_message = start(applicant, org_id, "clock", {"punch_type": "上班"})
    punch_id = item_id(punch_body)
    punch_row = sql_one("select count(*) from org_punches where id = %s::uuid", (punch_id,)) if punch_id else 0
    work_row = sql_one("select count(*) from org_work_items where id = %s::uuid", (punch_id,)) if punch_id else 0
    clock_ok = clock_names == [] and clock_status == 200 and punch_id and punch_row == 1 and work_row == 0
    lines += [
        "case CK-IN",
        f"http preview status={clock_status} nodes={','.join(clock_names or []) or 'empty'} message={clock_message}",
        f"http start status={punch_status} id={punch_id} message={punch_message}",
        f"persisted punch_rows={punch_row} work_item_rows={work_row}",
        f"result={'pass' if clock_ok else 'fail'}",
    ]

    hide_reason = "IBHIDE目录事由"
    hide_amount = "4321.50"
    hide_status, hide_body, hide_message, _hide_names = preview(
        applicant,
        org_id,
        "expense",
        {"category": "其他", "spent_on": "2026-10-16", "detail": hide_reason},
        hide_amount,
    )
    started, started_body, started_message = start(
        applicant,
        org_id,
        "expense",
        {"category": "其他", "spent_on": "2026-10-16", "detail": hide_reason},
        hide_amount,
    )
    hide_id = item_id(started_body)
    by_status, by_body, by_message = (
        decide(people["bystander"]["token"], hide_id, "approve", "旁观") if hide_id else (0, None, "no id")
    )
    listed_status, listed, listed_message = call(
        people["bystander"]["token"],
        "GET",
        f"/work-items?org={org_id}&box=todo",
    )
    listed_text = json.dumps(listed, ensure_ascii=False) if listed is not None else ""
    by_text = json.dumps(by_body, ensure_ascii=False) if by_body is not None else by_message
    omitted = hide_id not in listed_text
    hidden = hide_reason not in by_text and hide_amount not in by_text and hide_reason not in listed_text and hide_amount not in listed_text
    hide_ok = hide_id and "work item not found" in by_message and omitted and hidden
    lines += [
        "case IB-HIDE",
        f"http start status={started} id={hide_id} message={started_message} preview={hide_status} preview_message={hide_message}",
        f"http bystander status={by_status} message={by_message}",
        f"http list status={listed_status} omitted={str(omitted).lower()} hidden={str(hidden).lower()} message={listed_message}",
        f"result={'pass' if hide_ok else 'fail'}",
    ]
    return lines


def main() -> None:
    command = sys.argv[1] if len(sys.argv) > 1 else "eight"
    if command != "eight":
        raise SystemExit(f"unknown command {command}")
    abort_if_unsafe()
    session = load_session()
    api_lines = run_api(session)
    (SCRATCH / "api-eight.log").write_text("\n".join(api_lines) + "\n")
    users = user_pids()
    pids = isolated_pids()
    runtime = None
    if RUNTIME_FILE.is_file():
        stored = json.loads(RUNTIME_FILE.read_text())
        if str(stored.get("pid") or "").isdigit():
            runtime = int(stored["pid"])
    if runtime in pids:
        pid = runtime
    elif pids:
        pid = pids[0]
    else:
        raise SystemExit("isolated window is gone")
    probe = Drive(0, 0)
    try:
        window = window_for(probe, pid)
    finally:
        probe.close()
    if window is None:
        raise SystemExit("isolated window has no titled frame")
    ui = Background(pid, window)
    zone = ["case WB-ZONE", "result=fail", "control=not_run"]
    gui_personal = ["case LV-PERSONAL gui", "result=fail", "control=not_run"]
    try:
        zone = zone_case(ui, window)
        if open_leave(ui, window):
            gui_personal = fill_personal_leave(ui)
        else:
            gui_personal = ["case LV-PERSONAL gui", "result=fail", "control form=missing"]
    except Exception as error:
        gui_personal = gui_personal + [f"gui_error={error}"]
    finally:
        actions = ui.actions
        missed = ui.non_background
        action_lines = list(ui.lines)
        ui.close()
    ordered = [
        *zone,
        *gui_personal,
        *api_lines,
        f"background_actions={actions}",
        f"non_background={missed}",
        f"user_pids={','.join(str(item) for item in users)}",
        f"isolated_pid={pid}",
    ]
    text = "\n".join(
        [
            "order=WB-ZONE,LV-PERSONAL,LV-NO-BALANCE,EX-LEAD,EX-PAY,TR-OVERLAP,CK-IN,IB-HIDE",
            *ordered,
            "gui_actions:",
            *action_lines,
        ]
    )
    destination = SCRATCH / "default-eight.log"
    destination.write_text(text + "\n")
    print(text)


if __name__ == "__main__":
    main()
