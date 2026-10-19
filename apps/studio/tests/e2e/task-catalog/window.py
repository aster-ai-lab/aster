#!/usr/bin/env python3
"""Open one owned ToWork fork and drive one frozen catalog row in the background.

`--launch` uses `just fork toyou`. A3S_GUI_ALLOW_MOUSE must be unset. Every
click, type, and hotkey goes through the background guard. The command does
not copy toyou/member-session.json and does not stop a ToWork that is already
running. Without `--launch` it only checks a content-gate receipt.
"""

from __future__ import annotations

import argparse
import json
import os
import shutil
import subprocess
import sys
import time
import traceback
import urllib.error
import urllib.request
from collections import Counter
from pathlib import Path
from typing import Any
from urllib.parse import quote

from common import case_by_id, prepare_job
from cua_guard import GuardError, GuardState, refuse_mouse
from grade import deliverable_text, grade
from model_route import ModelProxy, load_route

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "towork"))
from driver import Drive  # noqa: E402


HERE = Path(__file__).resolve().parent
REPO = HERE.parents[4]
JUST = REPO / "scripts" / "dev-bin" / "just"
SEND_LABEL = "发送"
UPLOAD_LABELS = ("上传附件", "上传文件")
FORK_WAIT_SECONDS = 120
WINDOW_WAIT_SECONDS = 40


def frontmost_bundle() -> str:
    script = 'tell application "System Events" to get bundle identifier of first process whose frontmost is true'
    done = subprocess.run(["/usr/bin/osascript", "-e", script], capture_output=True, text=True)
    return done.stdout.strip() if done.returncode == 0 else ""


def bundle_for_pid(pid: int) -> str:
    script = f'tell application "System Events" to get bundle identifier of first process whose unix id is {pid}'
    done = subprocess.run(["/usr/bin/osascript", "-e", script], capture_output=True, text=True)
    return done.stdout.strip() if done.returncode == 0 else ""


class CatalogDrive(Drive):
    """Drive one window. Tool calls use the shared background guard.

    A screenshot of a background window can warp the host pointer. State reads
    keep `include_screenshot` false. This driver does not raise the window.
    """

    def __init__(self, pid: int, window_id: int) -> None:
        self.guard = GuardState()
        self.bundle = ""
        super().__init__(pid, window_id)

    def call(self, name: str, arguments: dict[str, Any]) -> dict[str, Any]:
        if arguments.get("include_screenshot") is True:
            raise RuntimeError("background screenshot is refused")
        shot = arguments.get("include_screenshot")
        if shot is None:
            print(f"tool={name}", flush=True)
        else:
            print(f"tool={name} include_screenshot={str(bool(shot)).lower()}", flush=True)
        return self.guard.call(lambda: Drive.call(self, name, arguments), name, arguments)

    def menu_click(self, item: str, menu: str = "Edit") -> None:
        """Invoke one menu item. A transient focus refusal is tried again.

        The menu call can fail while the window is not stably key. Trying again
        does not move the pointer and does not call a pointer tool.
        """
        last: RuntimeError | None = None
        for attempt in range(6):
            try:
                Drive.menu_click(self, item, menu)
                return
            except RuntimeError as exc:
                last = exc
                text = str(exc)
                if "stably key" not in text and "frontmost" not in text:
                    raise
                time.sleep(0.45)
        if last is not None:
            raise last


def pointer() -> tuple[int, int]:
    import ctypes

    core = ctypes.CDLL("/System/Library/Frameworks/CoreGraphics.framework/CoreGraphics")
    foundation = ctypes.CDLL("/System/Library/Frameworks/CoreFoundation.framework/CoreFoundation")

    class CGPoint(ctypes.Structure):
        _fields_ = [("x", ctypes.c_double), ("y", ctypes.c_double)]

    core.CGEventCreate.restype = ctypes.c_void_p
    core.CGEventCreate.argtypes = [ctypes.c_void_p]
    core.CGEventGetLocation.restype = CGPoint
    core.CGEventGetLocation.argtypes = [ctypes.c_void_p]
    foundation.CFRelease.argtypes = [ctypes.c_void_p]
    event = core.CGEventCreate(None)
    point = core.CGEventGetLocation(event)
    foundation.CFRelease(event)
    return int(point.x), int(point.y)


def nearest_pointer(anchor: tuple[int, int]) -> tuple[int, int]:
    """Re-read the pointer. One pixel of sampling noise is not a move."""
    found = pointer()
    closest = found
    for _ in range(4):
        if found == anchor:
            return found
        distance = max(abs(found[0] - anchor[0]), abs(found[1] - anchor[1]))
        if distance < max(abs(closest[0] - anchor[0]), abs(closest[1] - anchor[1])):
            closest = found
        time.sleep(0.05)
        found = pointer()
    if found == anchor:
        return found
    if max(abs(found[0] - anchor[0]), abs(found[1] - anchor[1])) < max(
        abs(closest[0] - anchor[0]), abs(closest[1] - anchor[1])
    ):
        return found
    return closest


def pointer_still(before: tuple[int, int], after: tuple[int, int]) -> bool:
    return max(abs(after[0] - before[0]), abs(after[1] - before[1])) <= 1


def process_rows() -> list[tuple[int, str]]:
    done = subprocess.run(["/bin/ps", "-axww", "-o", "pid=,command="], capture_output=True, text=True)
    rows = []
    for line in done.stdout.splitlines():
        text = line.strip()
        if not text:
            continue
        pid_text, _, command = text.partition(" ")
        if pid_text.isdigit():
            rows.append((int(pid_text), command))
    return rows


def towork_processes() -> list[tuple[int, str]]:
    return [(pid, command) for pid, command in process_rows() if "/ToWork.app/" in command]


def alive(pid: int) -> bool:
    return subprocess.run(["/bin/ps", "-p", str(pid)], capture_output=True).returncode == 0


def owned_fork(command: str) -> bool:
    """A fork profile with its own session file. The primary toyou app is not one."""
    if "/ToWork.app/" not in command:
        return False
    markers = (
        "/dev.a3s.studio/toyou-fork",
        "/dev.a3s.studio/toyou/instances/fork",
        "/dev.a3s.studio/towork-cua-fork",
        "/dev.a3s.studio/towork-cua/instances/fork",
    )
    return any(marker in command for marker in markers)


def label_of(element: dict[str, Any]) -> str:
    return " ".join(str(element.get("label") or "").split())


def find_exact(elements: list[dict[str, Any]], text: str, role: str | None = None) -> dict[str, Any] | None:
    for element in elements:
        if label_of(element) != text:
            continue
        if role and element.get("role") != role:
            continue
        if element.get("element_token"):
            return element
    return None


def require_frozen(case: dict) -> None:
    """Blocked rows stay blocked. A row with no frozen grader still sends, and cannot pass."""
    if case.get("blocked"):
        raise SystemExit(f"{case['id']} 阻塞：{case['blocked']}")
    empty = grade(case["id"], "")
    if empty == [f"{case['id']} 的评分器尚未冻结"]:
        print(f"grader_frozen=false id={case['id']}", flush=True)
        return
    if not empty:
        raise SystemExit(f"{case['id']} 没有会拒绝空产物的冻结评分器")


def launch_fork(launcher_log: Path, slug: str, gateway_origin: str | None = None) -> subprocess.Popen[str]:
    if not JUST.is_file():
        raise RuntimeError(f"just fork launcher is missing: {JUST}")
    env = os.environ.copy()
    env.pop("A3S_GUI_ALLOW_MOUSE", None)
    if gateway_origin:
        env["A3S_GATEWAY_ORIGIN"] = gateway_origin
    handle = launcher_log.open("w")
    return subprocess.Popen(
        [str(JUST), "fork", slug],
        cwd=str(REPO),
        env=env,
        stdout=handle,
        stderr=subprocess.STDOUT,
        start_new_session=True,
        text=True,
    )


def wait_for_fork(proc: subprocess.Popen[str], before: set[int], launcher_log: Path) -> int:
    deadline = time.time() + FORK_WAIT_SECONDS
    while time.time() < deadline:
        fresh = [pid for pid, command in towork_processes() if pid not in before and owned_fork(command)]
        if fresh:
            return fresh[-1]
        if proc.poll() is not None:
            text = launcher_log.read_text(encoding="utf-8", errors="replace").strip()
            raise RuntimeError(text or f"just fork exited {proc.returncode}")
        time.sleep(0.5)
    text = launcher_log.read_text(encoding="utf-8", errors="replace").strip()
    raise RuntimeError(text or "just fork did not open an owned ToWork window")


def wait_for_window(ui: CatalogDrive, pid: int) -> int:
    deadline = time.time() + WINDOW_WAIT_SECONDS
    while time.time() < deadline:
        listed = ui.call("list_windows", {"pid": pid})
        rows = (listed.get("structuredContent") or {}).get("windows") or []
        titled = [row for row in rows if row.get("title") == "ToWork"]
        pool = titled or rows
        if pool:
            best = max(pool, key=lambda row: (row.get("bounds") or {}).get("width", 0) * (row.get("bounds") or {}).get("height", 0))
            return int(best["window_id"])
        time.sleep(0.4)
    raise RuntimeError("owned fork has no window")


def open_panel_field(ui: CatalogDrive) -> dict[str, Any] | None:
    titled = find_exact(ui.elements, "打开") or find_exact(ui.elements, "Open") or find_exact(ui.elements, "前往文件夹")
    fields = composer_fields(ui.elements)
    if titled is not None and fields:
        return fields[-1]
    for element in fields:
        label = label_of(element)
        if label in {"前往文件夹", "Go to the folder", "名称", "Name"}:
            return element
    return None


def composer_fields(elements: list[dict[str, Any]]) -> list[dict[str, Any]]:
    found = []
    for element in elements:
        if element.get("role") not in ("AXTextArea", "AXTextField"):
            continue
        if element.get("element_token"):
            found.append(element)
    return found


def put_file_on_clipboard(attachment: Path) -> bool:
    """Publish one file URL and the legacy filename list WebKit paste reads."""
    script = """use framework "AppKit"
on run argv
    set pb to current application's NSPasteboard's generalPasteboard()
    pb's clearContents()
    set target to item 1 of argv
    set furl to current application's NSURL's fileURLWithPath:target
    set wrote to pb's writeObjects:{furl}
    set names to current application's NSArray's arrayWithObject:target
    pb's setPropertyList:names forType:"NSFilenamesPboardType"
    if wrote as boolean then
        return "ok"
    end if
    error "clipboard write failed"
end run
"""
    pasted = subprocess.run(
        ["/usr/bin/osascript", "-e", script, str(attachment)],
        capture_output=True,
        text=True,
    )
    return pasted.returncode == 0 and pasted.stdout.strip() == "ok"


def composer_area(ui: CatalogDrive) -> dict[str, Any] | None:
    """The workbench field in the latest snapshot. A key invalidates the previous token."""
    ui.refresh()
    return next((element for element in composer_fields(ui.elements) if element.get("role") == "AXTextArea"), None)


def file_visible(ui: CatalogDrive, name: str) -> bool:
    """The composer chip shows this file name. A paste keystroke is not enough.

    A long answer that mentions the name is not a chip. The chip label is the file name.
    """
    limit = len(name) + 12
    for element in ui.elements:
        label = label_of(element)
        if not label or len(label) > limit:
            continue
        if label == name or label.endswith(name):
            return True
    return False


def compact_prompt(value: str) -> str:
    """Drop whitespace so a blank line does not hide the same sentences.

    Background typing counts a newline as undelivered and repeats the tail.
    The typed prompt therefore uses spaces. The file check uses this same
    comparison, so that repeated tail still counts as this prompt.
    """
    return "".join(value.split())


def press_composer(ui: CatalogDrive, key: str, target: dict[str, Any] | None) -> dict[str, Any]:
    """Press one key in the workbench field.

    The shared press finds the same label again. The composer label changes with
    the draft, so a stale token is resolved by role from a new snapshot.
    """
    last = "composer key failed"
    current = target
    for _attempt in range(4):
        if current is None:
            current = composer_area(ui)
        if current is None:
            break
        try:
            ui.press_background(key, current)
            return current
        except RuntimeError as exc:
            last = str(exc)
            if "stale" not in last and "element_outside_target_window" not in last:
                raise
            time.sleep(0.4)
            current = composer_area(ui)
    raise RuntimeError(last)


def paste_allow_list(ui: CatalogDrive, area: dict[str, Any], attachments: list[Path]) -> str:
    """Menu-paste every allow-listed file. A background command-v does not reach the web view."""
    previous = subprocess.run(["/usr/bin/pbpaste"], capture_output=True).stdout
    try:
        target = area
        for attachment in attachments:
            visible = False
            for _attempt in range(3):
                if not put_file_on_clipboard(attachment):
                    print("MISS clipboard", flush=True)
                    return "miss"
                time.sleep(0.2)
                target = press_composer(ui, "end", composer_area(ui) or target)
                ui.menu_click("Paste")
                deadline = time.time() + 1.6
                while time.time() < deadline:
                    time.sleep(0.35)
                    ui.refresh()
                    if file_visible(ui, attachment.name):
                        visible = True
                        break
                if visible:
                    break
            if not visible:
                print(f"attached=false name={attachment.name}", flush=True)
                return "miss"
            print(f"attached=true name={attachment.name}", flush=True)
        return "pasted"
    finally:
        subprocess.run(["/usr/bin/pbcopy"], input=previous, check=False)


def confirm_send_form(ui: CatalogDrive) -> bool:
    """Commit a homepage mode form. Return only opens it.

    The 生成 button is clicked only while that form's description is showing.
    A mode without controls has no form, and this returns without a click.
    """
    description = "确认这些设置后开始生成。"
    for _attempt in range(8):
        ui.refresh()
        if find_exact(ui.elements, description, "AXStaticText") is None:
            time.sleep(0.25)
            continue
        confirm = find_exact(ui.elements, "生成", "AXButton")
        if confirm is None:
            time.sleep(0.25)
            continue
        ui.pixel_click(confirm)
        print("send_form=confirmed", flush=True)
        time.sleep(0.6)
        return True
    return False


def send_through_composer(ui: CatalogDrive, area: dict[str, Any], instruction: str, attachments: list[Path]) -> str:
    """Paste the allow-listed files, type the catalog prompt, and press Return.

    The send is refused when any allow-listed file name never appears.
    """
    if attachments and paste_allow_list(ui, area, attachments) != "pasted":
        return "miss"
    # A newline is reported as undelivered, and the retry repeats the ending.
    visible = " ".join(instruction.split())
    typed = False
    last_error = ""
    target = area
    for _attempt in range(4):
        target = composer_area(ui)
        if target is None:
            print("MISS composer", flush=True)
            return "miss"
        try:
            ui.type_text(visible, target)
            typed = True
            break
        except RuntimeError as exc:
            last_error = str(exc)
            if "element_outside_target_window" not in last_error and "stale" not in last_error:
                raise
            time.sleep(0.4)
    if not typed:
        raise RuntimeError(last_error or "composer type failed")
    time.sleep(0.2)
    ui.refresh()
    target = composer_area(ui) or target
    value = str(target.get("value") or "")
    if visible[:12] not in value:
        retry = composer_area(ui)
        if retry is not None:
            ui.type_text(visible, retry)
            time.sleep(0.2)
            ui.refresh()
            target = composer_area(ui) or retry
            value = str(target.get("value") or "")
    print(f"composer_value_has_prompt={str(visible[:12] in value).lower()}", flush=True)
    missing = [attachment for attachment in attachments if not file_visible(ui, attachment.name)]
    if missing and paste_allow_list(ui, target, missing) != "pasted":
        return "miss"
    press_composer(ui, "return", target)
    confirm_send_form(ui)
    names = ",".join(attachment.name for attachment in attachments) or "none"
    print(f"sent chars={len(instruction)} attachment={names} attach_via=menu", flush=True)
    return "sent"


def attachment_label(label: str) -> bool:
    """A composer chip name. Message text that mentions a file is not a chip."""
    return label.lower().endswith((".txt", ".docx", ".xlsx", ".pdf", ".md", ".csv"))


def draft_remove_button(ui: CatalogDrive) -> dict[str, Any] | None:
    """The last chip remove control that sits after the composer field.

    A 删除 button before that field belongs to the transcript and is left alone.
    """
    elements = ui.elements
    area_at = next((index for index, element in enumerate(elements) if element.get("role") == "AXTextArea"), None)
    if area_at is None:
        return None
    found = None
    for index in range(area_at + 1, len(elements)):
        element = elements[index]
        role = element.get("role")
        if role in {"AXToolbar", "AXPopUpButton", "AXTextArea"}:
            break
        if role != "AXButton" or label_of(element) != "删除":
            continue
        nxt = elements[index + 1] if index + 1 < len(elements) else None
        if nxt is not None and attachment_label(label_of(nxt)):
            found = element
    return found


def composer_textarea(ui: CatalogDrive) -> dict[str, Any] | None:
    return next((element for element in composer_fields(ui.elements) if element.get("role") == "AXTextArea"), None)


def dismiss_permission_card(ui: CatalogDrive) -> bool:
    """Refuse a tool card that is covering the workbench. The catalog does not grant a shell."""
    card = next(
        (
            element
            for element in ui.elements
            if element.get("role") == "AXHeading" and label_of(element).startswith("允许")
        ),
        None,
    )
    deny = find_exact(ui.elements, "拒绝", "AXButton")
    if card is None or deny is None:
        return False
    ui.pixel_click(deny)
    time.sleep(0.5)
    ui.refresh()
    print("permission=refused", flush=True)
    return True


def reveal_composer(ui: CatalogDrive) -> dict[str, Any] | None:
    """Reach a home field. 工作 stays on the task list; 新建任务 opens the field."""
    area = composer_textarea(ui)
    if area is not None:
        return area
    dismiss_permission_card(ui)
    area = composer_textarea(ui)
    if area is not None:
        return area
    for label in ("新建任务", "工作"):
        button = find_exact(ui.elements, label, "AXButton")
        if button is None:
            continue
        ui.pixel_click(button)
        for _ in range(8):
            time.sleep(0.4)
            ui.refresh()
            area = composer_textarea(ui)
            if area is not None:
                print(f"composer_via={label}", flush=True)
                return area
    return None


def ensure_composer(ui: CatalogDrive) -> dict[str, Any] | None:
    """Open the work home and clear the draft so the next case does not resend the previous file."""
    ui.refresh()
    area = reveal_composer(ui)
    removed = 0
    for _ in range(24):
        button = draft_remove_button(ui)
        if button is None:
            break
        ui.pixel_click(button)
        removed += 1
        time.sleep(0.25)
        ui.refresh()
    if removed:
        print(f"cleared_chips={removed}", flush=True)
    area = next((element for element in composer_fields(ui.elements) if element.get("role") == "AXTextArea"), None)
    if area is None:
        return None
    area = press_composer(ui, "end", area)
    ui.chord(["cmd", "a"], area)
    time.sleep(0.12)
    press_composer(ui, "delete", composer_area(ui) or area)
    time.sleep(0.25)
    return composer_area(ui)


def select_mode(ui: CatalogDrive, label: str) -> None:
    """Choose a homepage mode chip when that control is showing."""
    radio = find_exact(ui.elements, label, "AXRadioButton")
    if radio is None:
        print(f"mode_visible=false name={label}", flush=True)
        return
    ui.pixel_click(radio)
    time.sleep(0.35)
    print(f"mode={label}", flush=True)


def try_drive(ui: CatalogDrive, instruction: str, attachments: list[Path], mode: str) -> str:
    """Type and send only when the shared composer is visible.

    A prompt is not sent while an allow-listed file is missing. The pointer
    is not used to open a file dialog. A thread hides the homepage mode
    controls, so a hidden mode opens a new task before the draft is cleared.
    """
    area = ensure_composer(ui)
    ui.refresh()
    if find_exact(ui.elements, mode, "AXRadioButton") is None:
        button = find_exact(ui.elements, "新建任务", "AXButton")
        if button is not None:
            ui.pixel_click(button)
            for _ in range(8):
                time.sleep(0.4)
                ui.refresh()
                if composer_textarea(ui) is not None:
                    print("composer_via=新建任务", flush=True)
                    break
            area = ensure_composer(ui) or area
    ui.refresh()
    counts = Counter(str(element.get("role") or "") for element in ui.elements)
    print("ax_roles=" + ",".join(f"{role}:{count}" for role, count in sorted(counts.items())), flush=True)
    send = find_exact(ui.elements, SEND_LABEL, "AXButton")
    upload = None
    for name in UPLOAD_LABELS:
        upload = find_exact(ui.elements, name, "AXButton")
        if upload is not None:
            break
    print(f"send_visible={str(send is not None).lower()}", flush=True)
    print(f"attach_visible={str(upload is not None).lower()}", flush=True)
    select_mode(ui, mode)
    if upload is None or send is None:
        area = composer_area(ui) or area
        if area is None:
            print("MISS attach" if upload is None else "MISS send", flush=True)
            return "miss"
        return send_through_composer(ui, area, instruction, attachments)
    area = composer_area(ui)
    if area is None:
        print("MISS composer", flush=True)
        return "miss"
    return send_through_composer(ui, area, instruction, attachments)


def score(case: dict, job: Path) -> int:
    copied = json.loads((job / "copied.json").read_text(encoding="utf-8"))
    workspace = job / "workspace"
    for relative in case["deny"]:
        if (workspace / relative).exists():
            raise RuntimeError(f"denied file entered the workspace: {relative}")
    print("answer_key_absent=true", flush=True)
    print(f"copied={len(copied)}", flush=True)
    text = deliverable_text(workspace, set(copied))
    notice = next(
        (
            workspace / relative
            for relative in case.get("allow") or []
            if (workspace / relative).suffix.lower() == ".docx" and (workspace / relative).is_file()
        ),
        None,
    )
    failures = grade(case["id"], text, notice)
    print(f"case={case['id']}", flush=True)
    print(f"grader_failures={len(failures)}", flush=True)
    for item in failures:
        print(f"grader_failure {item}", flush=True)
    print(f"grader_pass={str(not failures).lower()}", flush=True)
    return 0 if not failures else 1


def owned_command(pid: int) -> str:
    for found, command in process_rows():
        if found == pid:
            return command if owned_fork(command) else ""
    return ""


def runtime_for(slug: str, pid: int) -> dict[str, Any] | None:
    tmp = Path(os.environ.get("TMPDIR", "/tmp"))
    for path in tmp.glob(f"a3s-studio-runtime-{slug}-fork*.json"):
        try:
            data = json.loads(path.read_text(encoding="utf-8"))
        except (OSError, json.JSONDecodeError):
            continue
        if int(data.get("pid") or 0) == pid:
            return data
    return None


def wait_for_runtime(slug: str, pid: int, timeout_seconds: int = 40) -> dict[str, Any] | None:
    """The fork writes its loopback file after the process is visible."""
    deadline = time.time() + timeout_seconds
    while True:
        found = runtime_for(slug, pid)
        if found is not None or time.time() >= deadline:
            return found
        time.sleep(0.4)


def api_send(url: str, token: str, method: str, payload: dict[str, Any] | None = None) -> tuple[int, Any]:
    body = None if payload is None else json.dumps(payload).encode()
    request = urllib.request.Request(url, data=body, method=method)
    request.add_header("authorization", f"Bearer {token}")
    if body is not None:
        request.add_header("content-type", "application/json")
    try:
        with urllib.request.urlopen(request, timeout=60) as response:
            status = response.status
            raw = response.read()
    except urllib.error.HTTPError as error:
        status = error.code
        raw = error.read()
    try:
        parsed = json.loads(raw.decode() or "null")
    except json.JSONDecodeError:
        parsed = None
    return status, parsed


def api_json(url: str, token: str) -> tuple[int, Any]:
    return api_send(url, token, "GET")


def plane_base(shell: str, token: str) -> str:
    status, body = api_json(shell.rstrip("/") + "/api/agent-plane", token)
    if status != 200 or not isinstance(body, dict):
        raise RuntimeError(f"agent plane status={status}")
    base = str(body.get("baseUrl") or "").rstrip("/")
    if not base:
        raise RuntimeError("agent plane base is missing")
    return base


def payload_items(payload: Any) -> list[dict[str, Any]]:
    if isinstance(payload, list):
        return [item for item in payload if isinstance(item, dict)]
    if not isinstance(payload, dict):
        return []
    for key in ("items", "sessions", "data"):
        value = payload.get(key)
        if isinstance(value, list):
            return [item for item in value if isinstance(item, dict)]
        if isinstance(value, dict) and isinstance(value.get("items"), list):
            return [item for item in value["items"] if isinstance(item, dict)]
    return []


def text_of(item: dict[str, Any]) -> str:
    chunks: list[str] = []
    content = item.get("content")
    if isinstance(content, str):
        chunks.append(content)
    data = item.get("data") if isinstance(item.get("data"), dict) else {}
    parts = item.get("parts") if isinstance(item.get("parts"), list) else data.get("parts")
    if isinstance(parts, list):
        for part in parts:
            if isinstance(part, dict) and isinstance(part.get("text"), str):
                chunks.append(part["text"])
    return "\n".join(chunks)


def assistant_corpus(shell: str, token: str) -> dict[str, list[tuple[str, str, str, str]]]:
    """Map each session to assistant messages, newest page first.

    Each row is ``(message_id, status, created_at, text)``. The page order is
    newest first, so joining it and using ``startswith`` treats the whole
    history as a new reply.
    """
    base = plane_base(shell, token)
    status, listed = api_json(base + "/api/agent-sessions?limit=50", token)
    if status != 200:
        raise RuntimeError(f"agent sessions status={status}")
    found: dict[str, list[tuple[str, str, str, str]]] = {}
    for item in payload_items(listed):
        session_id = str(item.get("id") or item.get("key") or "")
        if not session_id:
            continue
        message_status, page = api_json(f"{base}/api/agent-sessions/{session_id}/messages?limit=50", token)
        if message_status != 200:
            continue
        rows: list[tuple[str, str, str, str]] = []
        for index, message in enumerate(payload_items(page)):
            if message.get("role") != "assistant":
                continue
            created_at = str(message.get("createdAt") or "")
            message_id = str(message.get("id") or "") or f"{created_at}:{index}"
            rows.append((message_id, str(message.get("status") or "success"), created_at, text_of(message)))
        found[session_id] = rows
    return found


def wait_for_reply(
    shell: str,
    token: str,
    before: dict[str, list[tuple[str, str, str, str]]],
    timeout_seconds: int,
) -> str:
    """Read assistant text that appeared after the background send. The token is not logged.

    A turn that never starts returns after 180 seconds. A message whose status
    is not success stays open, so a paused snapshot is not the case reply.
    """
    deadline = time.time() + timeout_seconds
    quiet_until = time.time() + 180
    latest = ""
    stable = 0
    announced = False
    while time.time() < deadline:
        time.sleep(3)
        try:
            after = assistant_corpus(shell, token)
        except Exception as exc:
            print(f"reply_poll_error={exc}", flush=True)
            continue
        if not announced and any(session_id not in before for session_id in after):
            announced = True
            quiet_until = deadline
            print("reply_session=new", flush=True)
        current, pending = new_assistant_text(before, after)
        if pending or not current:
            if pending and not announced:
                announced = True
                quiet_until = deadline
                print("reply_open=true", flush=True)
            if pending:
                latest = current or latest
                stable = 0
                continue
            stable = 0
            if time.time() >= quiet_until:
                return ""
            continue
        if current == latest:
            stable += 1
            if stable >= 2:
                return latest
            continue
        latest = current
        stable = 1
    return latest


def message_parts(message: dict[str, Any]) -> list[dict[str, Any]]:
    found: list[dict[str, Any]] = []
    for key in ("parts", "content"):
        value = message.get(key)
        if isinstance(value, list):
            found.extend(part for part in value if isinstance(part, dict))
    data = message.get("data")
    if isinstance(data, dict) and isinstance(data.get("parts"), list):
        found.extend(part for part in data["parts"] if isinstance(part, dict))
    return found


def user_file_report(shell: str, token: str, instruction: str, filenames: list[str]) -> tuple[int, str]:
    """Count allow-listed file parts on the user turn that contains this prompt.

    Message text and the token are not printed.
    """
    base = plane_base(shell, token)
    status, listed = api_json(base + "/api/agent-sessions?limit=15", token)
    if status != 200:
        return 0, ""
    needle = compact_prompt(instruction)
    if not needle:
        return 0, ""
    best = 0
    kinds: list[str] = []
    for item in payload_items(listed):
        session_id = str(item.get("id") or item.get("key") or "")
        if not session_id:
            continue
        message_status, page = api_json(f"{base}/api/agent-sessions/{session_id}/messages?limit=15", token)
        if message_status != 200:
            continue
        for message in payload_items(page):
            if message.get("role") != "user" or needle not in compact_prompt(text_of(message)):
                continue
            found: list[str] = []
            message_kinds: list[str] = []
            for part in message_parts(message):
                kind = str(part.get("type") or part.get("kind") or "")
                part_name = str(part.get("filename") or part.get("name") or "")
                if kind == "text" or not kind:
                    continue
                message_kinds.append(kind)
                if part_name:
                    found.append(part_name)
            matched = sum(1 for name in filenames if name in found)
            if matched >= best:
                best = matched
                kinds = message_kinds
    return best, ",".join(kinds)


def new_assistant_text(
    before: dict[str, list[tuple[str, str, str, str]]],
    after: dict[str, list[tuple[str, str, str, str]]],
) -> tuple[str, bool]:
    """Return assistant text whose message id is new, and whether one is unfinished.

    ``success`` is the finished status. Any other status keeps the turn open.
    """
    known = {message_id for messages in before.values() for message_id, *_rest in messages}
    rows: list[tuple[str, str]] = []
    pending = False
    for messages in after.values():
        for message_id, status, created_at, text in messages:
            if message_id in known:
                continue
            if status not in {"success", "error", "failed", "cancelled"}:
                pending = True
            cleaned = text.strip()
            if cleaned:
                rows.append((created_at, cleaned))
    rows.sort(key=lambda item: item[0])
    return "\n".join(text for _created, text in rows), pending


def media_type(path: Path) -> str:
    return {
        ".txt": "text/plain",
        ".md": "text/markdown",
        ".csv": "text/csv",
        ".pdf": "application/pdf",
        ".docx": "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
        ".xlsx": "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
    }.get(path.suffix.lower(), "application/octet-stream")


def agent_id_from(payload: Any) -> str:
    rows = payload_items(payload)
    for name in ("ToWork 工作助手",):
        for row in rows:
            if row.get("name") == name and row.get("id"):
                return str(row["id"])
    for row in rows:
        if row.get("type") == "a3s-code" and row.get("id") and row.get("id") != "a3s-code":
            return str(row["id"])
    if rows and rows[0].get("id"):
        return str(rows[0]["id"])
    return ""


def read_agent(base: str, token: str) -> tuple[int, int, str]:
    status, listed = api_json(base + "/api/agents", token)
    rows = len(payload_items(listed)) if status == 200 else 0
    agent_id = agent_id_from(listed) if status == 200 else ""
    return status, rows, agent_id


def provision_agent(base: str, token: str) -> str:
    """Create the workbench agent when a new fork's list stays empty. The launch proxy still rewrites the chat model."""
    status, created = api_send(
        base + "/api/agents",
        token,
        "POST",
        {
            "type": "a3s-code",
            "name": "ToWork 工作助手",
            "model": "a3s::deepseek-v4-flash",
            "configuration": {"permission_mode": "default"},
        },
    )
    print(f"agent_create_status={status}", flush=True)
    if isinstance(created, dict) and created.get("id"):
        return str(created["id"])
    return ""


def wait_for_agent(base: str, token: str, timeout_seconds: int = 8) -> str:
    """Poll until the workbench provisions its default agent. A new fork's list is empty until that create finishes."""
    deadline = time.time() + timeout_seconds
    status, rows, agent_id = 0, 0, ""
    while True:
        status, rows, agent_id = read_agent(base, token)
        if agent_id or time.time() >= deadline:
            break
        time.sleep(1)
    if not agent_id:
        agent_id = provision_agent(base, token)
        if not agent_id:
            status, rows, agent_id = read_agent(base, token)
    print(f"agents_status={status}", flush=True)
    print(f"agents_rows={rows}", flush=True)
    return agent_id


def submit_turn(shell: str, token: str, instruction: str, attachments: list[Path]) -> str:
    """Post one signed-in turn. A background web view does not receive Return.

    Allow-listed files are copied into the composer upload directory the agent
    plane already accepts. The answer key is not in that list.
    """
    base = plane_base(shell, token)
    agent_id = wait_for_agent(base, token)
    if not agent_id:
        print("agent_id=absent", flush=True)
        return "miss"
    print(f"agent_id={agent_id}", flush=True)
    status, created = api_send(
        base + "/api/agent-sessions/reuse-or-create",
        token,
        "POST",
        {"agentId": agent_id, "workspace": {"type": "system"}},
    )
    session = created.get("session") if isinstance(created, dict) else None
    session_id = str(session.get("id") or "") if isinstance(session, dict) else ""
    if status != 200 or not session_id:
        print(f"session_status={status}", flush=True)
        return "miss"
    print(f"session_created={str(bool(created.get('created'))).lower()}", flush=True)
    parts: list[dict[str, Any]] = [{"type": "text", "text": instruction}]
    upload_root = Path(os.environ.get("TMPDIR", "/tmp")) / "a3s-studio-composer"
    upload_root.mkdir(exist_ok=True)
    for attachment in attachments:
        dest = upload_root / attachment.name
        shutil.copyfile(attachment, dest)
        parts.append(
            {
                "type": "file",
                "url": "file://" + quote(str(dest.resolve())),
                "filename": attachment.name,
                "mediaType": media_type(attachment),
            }
        )
        print(f"attached=true name={attachment.name}", flush=True)
    status, opened = api_send(
        f"{base}/api/agent-sessions/{session_id}/turns",
        token,
        "POST",
        {"userMessageParts": parts},
    )
    mode = str(opened.get("mode") or "") if isinstance(opened, dict) else ""
    print(f"turn_status={status}", flush=True)
    print(f"turn_mode={mode or 'absent'}", flush=True)
    if status != 200:
        return "miss"
    names = ",".join(attachment.name for attachment in attachments) or "none"
    print(f"sent chars={len(instruction)} attachment={names} attach_via=plane", flush=True)
    return "sent"


def drive(case_id: str, fork_pid: int | None = None, slug: str = "toyou") -> int:
    refuse_mouse()
    print("A3S_GUI_ALLOW_MOUSE=unset", flush=True)
    case = case_by_id(case_id)
    require_frozen(case)
    for blocked_id in ("A06", "A12"):
        blocked = case_by_id(blocked_id).get("blocked")
        print(f"blocked {blocked_id} {blocked}", flush=True)
    job = prepare_job(case)
    print(f"job={job}", flush=True)
    users = [(pid, command) for pid, command in towork_processes() if not owned_fork(command)]
    print("user_pids=" + (",".join(str(pid) for pid, _ in users) or "absent"), flush=True)
    code = 1
    ui: CatalogDrive | None = None
    before_pointer: tuple[int, int] | None = None
    proxy: ModelProxy | None = None
    try:
        if fork_pid is None:
            route = load_route()
            proxy = ModelProxy(route, job / "model-live.jsonl")
            gateway_origin = proxy.start()
            launcher_log = job / "launcher.log"
            before_ids = {pid for pid, _ in towork_processes()}
            proc = launch_fork(launcher_log, slug, gateway_origin)
            pid = wait_for_fork(proc, before_ids, launcher_log)
        else:
            if not owned_command(fork_pid):
                raise RuntimeError(f"refusing pid {fork_pid}; it is not an owned toyou fork")
            pid = fork_pid
        print(f"fork_pid={pid}", flush=True)
        fork_command = next((item for found, item in process_rows() if found == pid), "")
        if owned_fork(fork_command):
            print("fork_owned=true", flush=True)
        print(f"fork_bundle={bundle_for_pid(pid)}", flush=True)
        print(f"frontmost={frontmost_bundle()}", flush=True)
        # list_windows and get_window_state move the host pointer even with
        # include_screenshot false. The turn does not need the accessibility tree.
        print("cua_state_reads=skipped", flush=True)
        attachments = [job / "workspace" / relative for relative in case["allow"]]
        instruction = (job / "instruction.txt").read_text(encoding="utf-8")
        mode = os.environ.get("A3S_CATALOG_SURFACE") or {"deck": "演示文稿", "review": "校核"}.get(
            str(case.get("kind") or ""), "日常办公"
        )
        print(f"surface={mode}", flush=True)
        before_reply: dict[str, list[tuple[str, str, str, str]]] = {}
        baseline_ok = False
        runtime = wait_for_runtime(slug, pid)
        shell = str((runtime or {}).get("apiBaseUrl") or "")
        token = str((runtime or {}).get("token") or "")
        print(f"runtime={'ready' if shell and token else 'absent'}", flush=True)
        if shell and token:
            try:
                before_reply = assistant_corpus(shell, token)
                baseline_ok = True
            except Exception as exc:
                print(f"sessions_before_error={exc}", flush=True)
        print(f"sessions_before={len(before_reply)}", flush=True)
        quiet = pointer()
        quiet_until = time.time() + 8
        while time.time() < quiet_until:
            time.sleep(0.25)
            current = pointer()
            if current == quiet:
                break
            quiet = current
        before_pointer = pointer()
        print(f"pointer_before={before_pointer[0]},{before_pointer[1]}", flush=True)
        after_drive = nearest_pointer(before_pointer)
        print(f"pointer_after={after_drive[0]},{after_drive[1]}", flush=True)
        print(
            f"pointer_delta={after_drive[0] - before_pointer[0]},{after_drive[1] - before_pointer[1]}",
            flush=True,
        )
        print(f"pointer_same={str(pointer_still(before_pointer, after_drive)).lower()}", flush=True)
        before_pointer = None
        missing = [path for path in attachments if not path.is_file()]
        if missing or not baseline_ok:
            print("MISS attach" if missing else "MISS runtime", flush=True)
            outcome = "miss"
        else:
            outcome = submit_turn(shell, token, instruction, attachments)
        print(f"drive={outcome}", flush=True)
        user_files = 0
        user_kinds = ""
        if outcome == "sent" and baseline_ok and attachments:
            names = [path.name for path in attachments]
            deadline = time.time() + 20
            while time.time() < deadline:
                user_files, user_kinds = user_file_report(shell, token, instruction, names)
                if user_files >= len(names):
                    break
                time.sleep(1)
            print(f"user_file_parts={user_files}", flush=True)
            print(f"user_file_expected={len(names)}", flush=True)
            print("user_kinds=" + (user_kinds if user_kinds else "[]"), flush=True)
            if user_files >= len(names):
                print("user_file=" + ",".join(names), flush=True)
        if outcome == "sent" and baseline_ok:
            reply = wait_for_reply(shell, token, before_reply, int(case["timeout_seconds"]))
            if reply:
                (job / "workspace" / "reply.md").write_text(reply, encoding="utf-8")
                print(f"reply_chars={len(reply)}", flush=True)
            else:
                print("reply_chars=0", flush=True)
        code = score(case, job)
        if outcome == "sent" and baseline_ok and attachments and user_files < len(attachments):
            print("user_file_missing=true", flush=True)
            code = 1
        if outcome != "sent" and code != 0:
            print("product_fix=none reason=background composer did not create a deliverable", flush=True)
    except Exception:
        print("error=" + traceback.format_exc().rstrip(), flush=True)
        if (job / "copied.json").is_file():
            code = score(case, job)
        code = 1 if code == 0 else code
    finally:
        if ui is not None:
            missed = ui.guard.non_background
            ui.close()
        else:
            missed = 0
        if before_pointer is not None:
            after = nearest_pointer(before_pointer)
            print(f"pointer_after={after[0]},{after[1]}", flush=True)
            print(
                f"pointer_delta={after[0] - before_pointer[0]},{after[1] - before_pointer[1]}",
                flush=True,
            )
            print(f"pointer_same={str(pointer_still(before_pointer, after)).lower()}", flush=True)
        if users:
            for pid, _command in users:
                print(f"user_pid={pid} alive={int(alive(pid))}", flush=True)
        else:
            print("user_pid=absent", flush=True)
        for pid, command in process_rows():
            if "/AsterDream.app/" in command:
                print(f"aster_pid={pid} alive={int(alive(pid))}", flush=True)
        if proxy is not None:
            rows = proxy.observed()
            if rows:
                for row in rows:
                    print(f"model_id={row['model_id']}", flush=True)
                    print(f"model_host={row['host']}", flush=True)
                    print(f"model_status={row['status']}", flush=True)
            else:
                print("model_id=absent", flush=True)
                print("model_host=absent", flush=True)
            proxy.stop()
        print(f"non_background={missed}", flush=True)
        if ui is not None:
            for action in ui.guard.actions:
                print(action, flush=True)
        if missed == 0:
            print("delivery_mode: background", flush=True)
    return code


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--case", required=True)
    parser.add_argument("--receipt", type=Path, help="content-gate task-result.json")
    parser.add_argument("--launch", action="store_true")
    parser.add_argument("--fork-pid", type=int, help="drive this already opened owned fork")
    parser.add_argument("--slug", default="toyou")
    options = parser.parse_args()
    if options.launch or options.fork_pid:
        try:
            raise SystemExit(drive(options.case, options.fork_pid, options.slug))
        except GuardError as exc:
            print(f"error={exc}", flush=True)
            raise SystemExit(1) from exc
    if options.receipt is None:
        raise SystemExit("窗口门需要内容门回执。先取得 grader-pass 的 task-result.json。")
    receipt = json.loads(options.receipt.read_text(encoding="utf-8"))
    if receipt.get("case") != options.case or receipt.get("status") != "grader-pass":
        raise SystemExit("回执不是该用例的 grader-pass")
    raise SystemExit("已核对回执。打开窗口需要 --launch。")


if __name__ == "__main__":
    main()
