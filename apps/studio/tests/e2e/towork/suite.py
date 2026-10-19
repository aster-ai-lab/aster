"""Ordered ToWork cases against one owned window and a real model.

Credential names stay in the environment. This file never prints their values
and never substitutes a recorded completion.
"""

from __future__ import annotations

import fcntl
import json
import os
import subprocess
import sys
import tempfile
import time
from pathlib import Path

import captcha_slide
from driver import Drive

CREDENTIALS = ("ANTHROPIC_AUTH_TOKEN", "ANTHROPIC_BASE_URL")
BUNDLE = "dev.a3s.studio.towork-cua-fork1"
SUPPORT = Path.home() / "Library/Application Support/dev.a3s.studio/towork-cua-fork1"
MESSAGES = SUPPORT / "agent-plane/messages"
RUNTIME = SUPPORT / "agent-plane/runtime"

ORDER = [
    "P0-01", "P0-02", "P0-03", "P0-04", "P0-05", "P0-06", "P0-07", "P0-08",
    "P0-09", "P0-10", "P0-11", "P0-12", "P0-13", "P0-14", "P0-15", "P0-16",
    "W-01", "W-02", "W-03", "W-04", "W-05", "W-06", "W-07", "W-08", "W-09",
    "W-10", "W-11", "W-12", "W-13", "W-14", "W-15", "W-16", "W-17", "W-18",
    "W-19", "W-20", "W-21",
    "M-01", "M-02", "M-03", "M-04", "M-05", "M-06", "M-07", "M-08", "M-09",
    "M-10", "M-11", "M-12",
    "O-01", "O-02", "O-03", "O-04", "O-05", "O-06",
    "K-01", "K-02", "K-03", "K-04", "K-05", "K-06", "K-07",
    "F-01", "F-02", "F-03", "F-04", "F-05",
    "N-01", "C-01",
    "S-01", "S-02", "E-01", "E-02", "P-01", "G-01", "G-02",
    "T-01", "T-02", "T-03", "T-04", "T-05", "T-06", "T-07", "T-08", "T-09",
    "T-10", "T-11", "T-12",
    "X-01", "X-02", "X-03", "X-04", "X-05",
]

SURFACE = [
    "P0-05", "P0-10",
    "K-01", "K-02", "K-03", "K-04", "K-05", "K-06", "K-07",
    "F-01", "F-02", "F-03", "F-04", "F-05",
    "N-01", "C-01",
]

WAVES = {
    "p0": [case for case in ORDER if case.startswith("P0-")],
    "surfaces": SURFACE,
    "waves": [case for case in ORDER if case[:1] in {"P", "W", "M", "O", "S", "E", "G", "T", "X"} or case.startswith("P-")],
    "all": ORDER,
}

CHROME = {
    "ToWork", "RoyLin", "工作", "消息", "知识", "团队", "插件", "平台", "任务", "设置",
    "新建任务", "搜索任务", "智能体", "连接器", "技能", "自动化", "项目", "新建项目",
    "复制", "编辑重发", "删除", "重命名", "归档", "自动", "逐次确认",
}


def require_credentials() -> None:
    missing = [name for name in CREDENTIALS if not os.environ.get(name)]
    if missing:
        print("FAIL closed: missing " + ",".join(missing))
        raise SystemExit(2)
    print("credentials present " + ",".join(CREDENTIALS))


def _bundle_pid() -> int:
    """The owned process id. Stderr stays captured so a dead process does not spam -1719."""
    result = subprocess.run(
        [
            "osascript",
            "-e",
            f'tell application "System Events" to unix id of first process whose bundle identifier is "{BUNDLE}"',
        ],
        capture_output=True,
        text=True,
    )
    pid_text = result.stdout.strip()
    if result.returncode != 0 or not pid_text.isdigit():
        raise RuntimeError("owned ToWork process is not running")
    return int(pid_text)


def _onscreen_window(pid: int) -> str:
    swift = f"""
import CoreGraphics
let info = CGWindowListCopyWindowInfo([.optionOnScreenOnly, .excludeDesktopElements], kCGNullWindowID) as? [[String: Any]] ?? []
for w in info {{
  let owner = w["kCGWindowOwnerPID"] as? Int ?? -1
  let layer = w["kCGWindowLayer"] as? Int ?? -1
  let name = w["kCGWindowName"] as? String ?? ""
  if owner == {pid} && layer == 0 && name == "ToWork" {{
    print(w["kCGWindowNumber"] as? Int ?? 0)
    break
  }}
}}
"""
    result = subprocess.run(["swift", "-e", swift], capture_output=True, text=True)
    return result.stdout.strip() if result.returncode == 0 else ""


def _raise_owned_window(pid: int) -> None:
    """Bring the owned window onto this screen. A window on another space is not listed."""
    script = (
        'tell application "System Events"\n'
        f"  tell (first process whose unix id is {pid})\n"
        "    set visible to true\n"
        "    set frontmost to true\n"
        '    perform action "AXRaise" of window 1\n'
        "  end tell\n"
        "end tell\n"
    )
    subprocess.run(["osascript", "-e", script], capture_output=True, text=True)


def _window_for_pid(pid: int) -> int:
    window = _onscreen_window(pid)
    if not window.isdigit() or window == "0":
        _raise_owned_window(pid)
        time.sleep(0.4)
        window = _onscreen_window(pid)
    if not window.isdigit() or window == "0":
        raise RuntimeError("owned ToWork window is not on screen")
    return int(window)


def owned_target() -> tuple[int, int]:
    pid = _bundle_pid()
    return pid, _window_for_pid(pid)


class Ui(Drive):
    def _with_visible_window(self, method, *args, **kwargs):
        """Retry one key after the owned window can receive it.

        The driver refuses the key when it cannot prove the window is visible,
        and when another window of the same process could receive it. Raising
        or closing that popout does not move the pointer. A second refusal
        still fails the case.
        """
        try:
            return method(*args, **kwargs)
        except RuntimeError as exc:
            message = str(exc)
            if "minimized_or_hidden_window" in message:
                _raise_owned_window(self.pid)
                time.sleep(0.4)
                return method(*args, **kwargs)
            if "same_pid_keyboard_ambiguity" in message:
                _close_extra_windows(self.pid)
                time.sleep(0.3)
                return method(*args, **kwargs)
            raise

    def type_text(self, text: str, element: dict | None = None, **kwargs) -> None:
        self._with_visible_window(super().type_text, text, element, **kwargs)

    def chord(self, keys: list[str], element: dict | None = None, **kwargs) -> None:
        self._with_visible_window(super().chord, keys, element, **kwargs)

    def press_background(
        self,
        key: str,
        element: dict | None = None,
        modifiers: list[str] | None = None,
        **kwargs,
    ) -> None:
        self._with_visible_window(super().press_background, key, element, modifiers, **kwargs)

    def rail(self) -> list[str]:
        self.refresh()
        work = self.find("工作", "AXButton")
        x = (work.get("frame") or {}).get("x", 0)
        rows = []
        for element in self.elements:
            if element.get("role") != "AXButton":
                continue
            frame = element.get("frame") or {}
            label = str(element.get("label") or "")
            if not label or abs(frame.get("x", 9999) - x) > 8:
                continue
            rows.append((frame.get("y", 0), label))
        return [label for _, label in sorted(rows)]

    def open_account(self) -> None:
        self.refresh()
        element = self._account_control()
        frame = element.get("frame") or {}
        # The account menu opens on pointerdown. A synthesized click misses it.
        self.pointer_click(element, dx=float(frame.get("w", 36)) / 2, dy=float(frame.get("h", 36)) / 2)
        time.sleep(0.4)
        self.refresh()

    def _account_control(self) -> dict:
        """Sidebar account control. The visible name follows the signed-in member."""
        names = ("RoyLin", "林书豪")
        named = [
            element
            for element in self.elements
            if element.get("role") in ("AXPopUpButton", "AXButton")
            and any(name in str(element.get("label") or "") for name in names)
        ]
        # A conversation row can contain the member name. The account control is the small rail avatar.
        avatars = [element for element in named if (element.get("frame") or {}).get("w", 9999) <= 64]
        if avatars:
            return avatars[0]
        if named:
            return named[0]
        rail = {"工作", "消息", "知识", "团队", "插件", "平台", "任务", "设置", "文件", "笔记", "组织", "启动台", "工作台"}
        chrome = ("展示方式", "更多选项", "授权模式", "项目文件夹", "工作区文件")
        web = next((element for element in self.elements if element.get("role") == "AXWebArea"), None)
        origin_x = (web.get("frame") or {}).get("x", 0) if web else 0
        candidates = []
        for element in self.elements:
            if element.get("role") not in ("AXPopUpButton", "AXButton"):
                continue
            frame = element.get("frame") or {}
            label = " ".join(str(element.get("label") or "").split())
            if not label or label in rail or any(label.startswith(prefix) for prefix in chrome):
                continue
            if frame.get("x", 9999) - origin_x > 220 or frame.get("w", 9999) > 220:
                continue
            candidates.append(element)
        if not candidates:
            raise AssertionError("account menu is missing")
        return max(candidates, key=lambda element: (element.get("frame") or {}).get("y", 0))

    def _rail_button(self, label: str) -> dict | None:
        exact = [
            element
            for element in self.elements
            if element.get("role") == "AXButton"
            and " ".join(str(element.get("label") or "").split()) == label
        ]
        rail = [element for element in exact if (element.get("frame") or {}).get("w", 9999) <= 64]
        if rail:
            return rail[0]
        return exact[0] if exact else None

    def _exact_button(self, label: str) -> dict | None:
        wanted = " ".join(label.split())
        for element in self.elements:
            if element.get("role") != "AXButton":
                continue
            if " ".join(str(element.get("label") or "").split()) == wanted:
                return element
        return None

    def _settings_chrome(self) -> bool:
        """Settings stays mounted over the work shell, so the rail is not proof of the work page."""
        if self._exact_button("取消搜索") is not None or self._exact_button("返回应用") is not None:
            return True
        return any(
            element.get("role") in ("AXTextField", "AXTextArea", "AXSearchField", "AXComboBox")
            and "搜索设置" in str(element.get("label") or "") + str(element.get("value") or "")
            for element in self.elements
        )

    def _leave_settings(self) -> None:
        """Leave settings without pressing the work rail.

        Escape does not collapse a search page. 取消搜索 restores 返回应用,
        and Space on 返回应用 leaves settings. The first token can belong to a
        settings row that is still settling, so a miss takes a fresh snapshot.
        """
        for _ in range(3):
            if not self._settings_chrome():
                return
            cancel = self._exact_button("取消搜索")
            if cancel is not None:
                self.press_background("space", cancel)
                time.sleep(0.4)
                self.refresh()
            back = self._exact_button("返回应用")
            if back is None:
                return
            self.press_background("space", back)
            time.sleep(0.6)
            self.refresh()
            if not self._settings_chrome():
                return

    def _work_ready(self) -> bool:
        return (
            self._rail_button("工作") is not None
            and self._exact_button("新建任务") is not None
            and not self._settings_chrome()
        )

    def _work_gap(self) -> str:
        parts = []
        if self._rail_button("工作") is None:
            parts.append("rail")
        if self._exact_button("新建任务") is None:
            parts.append("新建任务")
        if self._exact_button("取消搜索") is not None:
            parts.append("取消搜索")
        if self._exact_button("返回应用") is not None:
            parts.append("返回应用")
        return "missing AXButton 工作 " + (" ".join(parts) if parts else "unready")

    def open(self, label: str) -> None:
        """Open a rail app. Exact label, narrow icon. Substring 工作 also matches 工作助手.

        Pressing the rail icon of the page that is already showing remounts it
        into the error boundary, and that press can return -25202. Stay when
        the page's own control is already in the tree.
        """
        self.refresh()
        if label == "设置" and (
            self._exact_button("搜索设置...") is not None or self._settings_chrome()
        ):
            return
        if label == "工作" and self._settings_chrome():
            self._leave_settings()
            if self._settings_chrome():
                raise AssertionError(self._work_gap())
        if label == "工作" and self._work_ready():
            return
        if label == "知识" and _fields(self, "搜索笔记"):
            return
        if label == "团队" and any(
            element.get("role") == "AXButton"
            and " ".join(str(element.get("label") or "").split()) == "云盘"
            and (element.get("frame") or {}).get("w", 0) >= 80
            for element in self.elements
        ):
            return
        target = self._rail_button(label)
        if target is None and label == "团队":
            # The scope tab is also labeled 团队. Select a joined team so the rail entry exists.
            _select_center(self, "团队")
            self.refresh()
            target = self._rail_button(label)
        if target is None:
            self.click(label, "AXButton")
        else:
            frame = target.get("frame") or {}
            self.pixel_click(target, dx=16, dy=min(16.0, float(frame.get("h", 24)) / 2))
            time.sleep(0.5)
            self.refresh()
        if self.has("似乎出现了一些问题"):
            raise AssertionError(f"{label} showed the error boundary")

    def work(self) -> None:
        self._restore_frame()
        self.refresh()
        if self.has("似乎出现了一些问题") and self.has("重新加载"):
            self.pixel_click(self.find("重新加载", "AXButton"))
            time.sleep(0.8)
            self.refresh()
        if self.has("没有匹配的任务") and self._rail_button("工作") is None:
            _dismiss_task_search(self)
            time.sleep(0.4)
            self.refresh()
        # Settings keeps the work rail and the exact 新建任务 button in the tree.
        # Returning while 返回应用 is up makes the next search run against settings.
        # Space leaves. Pressing the work rail remounts the current page.
        if self._settings_chrome():
            self.press("escape")
            time.sleep(0.3)
            self.refresh()
            self._leave_settings()
        deadline = time.time() + 8
        while time.time() < deadline:
            if self._work_ready():
                return
            if self._settings_chrome():
                self._leave_settings()
            time.sleep(0.3)
            self.refresh()
        if self._settings_chrome():
            raise AssertionError(self._work_gap())
        try:
            self.open("工作")
        except AssertionError:
            relaunch(self, "新建任务")
        if self.has("似乎出现了一些问题"):
            relaunch(self, "新建任务")
        deadline = time.time() + 20
        while time.time() < deadline:
            self.refresh()
            if self._work_ready():
                return
            if self._settings_chrome():
                self._leave_settings()
            time.sleep(0.4)
        raise AssertionError(self._work_gap())

    def _composer_area(self) -> dict:
        self.refresh()
        needle = "描述任务"
        controls = []
        for element in self.elements:
            if element.get("role") not in ("AXTextArea", "AXTextField"):
                continue
            blob = " ".join(
                str(element.get("label") or "").split()
                + str(element.get("value") or "").split()
            )
            if needle in blob:
                controls.append(element)
        if not controls:
            controls = [
                element for element in self.elements if element.get("role") == "AXTextArea"
            ]
        if not controls:
            raise AssertionError("composer did not receive the pasted prompt")
        return max(controls, key=lambda element: float((element.get("frame") or {}).get("w", 0)))

    def paste_composer(self, text: str) -> None:
        # Typed insertion drops line breaks, and a second type appends. A multiline
        # prompt is pasted so the accessibility value keeps every line.
        if "\n" in text:
            self.fill_element(self._composer_area(), text)
        else:
            self.fill(text, "描述任务")
        value = str(self._composer_area().get("value") or "")
        if text not in value:
            if "\n" in text:
                self.fill_element(self._composer_area(), text)
            else:
                self.fill(text, "描述任务")
            value = str(self._composer_area().get("value") or "")
        if text not in value:
            raise AssertionError("composer did not receive the pasted prompt")

    def send_work(self, text: str) -> str:
        before = self._message_stamp()
        self.work()
        self.click("新建任务", "AXButton")
        self.wait_for("描述任务", 10)
        self.paste_composer(text)
        self.submit_composer()
        outcome = self._await_turn(text, before, 150)
        print(f"llm endpoint={self._endpoint_host()} model=a3s/auto visible={outcome}")
        return outcome

    def _message_stamp(self) -> float:
        latest = 0.0
        if MESSAGES.is_dir():
            for path in MESSAGES.glob("*.json"):
                latest = max(latest, path.stat().st_mtime)
        return latest

    def _endpoint_host(self) -> str:
        hosts = set()
        if RUNTIME.is_dir():
            for path in RUNTIME.glob("*.acl"):
                for line in path.read_text(errors="ignore").splitlines():
                    if "base_url" in line and "://" in line:
                        hosts.add(line.split("://", 1)[-1].strip().strip('"').split("/", 1)[0])
        return ",".join(sorted(hosts)) or "absent"

    def approval_pending(self) -> bool:
        """The composer asks before a tool runs. The title is 允许此操作？ or 允许 <tool>?."""
        labels = self.labels()
        if any("允许此操作" in label for label in labels):
            return True
        if any(label.startswith("允许") and (label.endswith("?") or label.endswith("？")) for label in labels):
            return True
        # 待确认 is the badge. A bare 允许 also appears in settings, so require 拒绝 beside it.
        has_allow = any(label == "允许" or label.startswith("允许 ") for label in labels)
        has_deny = any(label == "拒绝" or label.startswith("拒绝 ") for label in labels)
        return any("待确认" in label for label in labels) or (has_allow and has_deny)

    def approve_if_pending(self) -> bool:
        if not self.approval_pending():
            return False
        button = None
        for element in self.elements:
            if element.get("role") != "AXButton":
                continue
            label = " ".join(str(element.get("label") or "").split())
            if label == "允许" or label.startswith("允许 "):
                button = element
        if button is None:
            return False
        frame = button.get("frame") or {}
        self.pointer_click(button, dx=float(frame.get("w", 48)) / 2, dy=float(frame.get("h", 32)) / 2)
        time.sleep(0.4)
        return True

    def _await_turn(self, prompt: str, before: float, timeout: float) -> str:
        deadline = time.time() + timeout
        while time.time() < deadline:
            self.refresh()
            visible = self.has(prompt) or any(prompt[:18] in label for label in self.labels())
            if self.approve_if_pending():
                time.sleep(0.4)
            failed = any(token in "\n".join(self.labels()) for token in ("失败", "错误", "无法", "超时"))
            reply = self._assistant_after(before, prompt)
            if visible and failed and reply is None:
                return "failure"
            if visible and reply:
                if reply == prompt:
                    raise AssertionError("assistant text repeated the prompt")
                return "reply"
            time.sleep(1)
        raise AssertionError("no visible reply or explicit failure")

    def _assistant_after(self, before: float, prompt: str) -> str | None:
        if not MESSAGES.is_dir():
            return None
        for path in sorted(MESSAGES.glob("*.json"), key=lambda item: item.stat().st_mtime, reverse=True):
            if path.stat().st_mtime + 1 < before:
                continue
            try:
                rows = json.loads(path.read_text())
            except json.JSONDecodeError:
                continue
            if not isinstance(rows, list):
                continue
            # Compare decoded part text: JSON escapes a multi-line or quoted prompt.
            saw_user = any(
                row.get("role") == "user"
                and any(
                    prompt in str(part.get("text") or "")
                    for part in (row.get("data") or {}).get("parts") or []
                    if isinstance(part, dict)
                )
                for row in rows
            )
            if not saw_user:
                continue
            for row in rows:
                if row.get("role") != "assistant" or row.get("status") not in ("success", "error"):
                    continue
                if row.get("status") == "error":
                    return "error"
                for part in (row.get("data") or {}).get("parts") or []:
                    if not isinstance(part, dict) or part.get("type") == "reasoning":
                        continue
                    text = part.get("text")
                    if text:
                        return str(text)
        return None


SESSION = SUPPORT / "member-session.json"
SESSION_ASIDE = SUPPORT / "member-session.json.p0-aside"
SESSION_BACKUP = SUPPORT / "member-session.json.suite-backup"


def _copy_session(src: Path, dest: Path) -> None:
    """Copy a session file without reading it into a log. Mode stays 0600."""
    data = src.read_bytes()
    temporary = dest.with_name(dest.name + ".tmp")
    temporary.write_bytes(data)
    os.chmod(temporary, 0o600)
    temporary.replace(dest)


def _backup_session() -> None:
    if SESSION.is_file():
        _copy_session(SESSION, SESSION_BACKUP)


def _park_session() -> None:
    """Move only this instance's session aside. The file contents stay unread.

    A second call keeps the first copy. A quitting app can rewrite the file,
    and that rewrite is removed so the next launch still starts signed out.
    """
    if not SESSION.is_file():
        return
    if SESSION_ASIDE.exists():
        SESSION.unlink()
        return
    SESSION.replace(SESSION_ASIDE)


def _restore_session() -> None:
    if not SESSION_ASIDE.is_file():
        return
    if SESSION.exists():
        SESSION.unlink()
    SESSION_ASIDE.replace(SESSION)


def _reinstate_session() -> bool:
    """Put a signed-in file back after the app deletes it. A live park is restored, not copied over."""
    if SESSION.is_file():
        return False
    if SESSION_ASIDE.is_file():
        _restore_session()
        return SESSION.is_file()
    if not SESSION_BACKUP.is_file():
        return False
    _copy_session(SESSION_BACKUP, SESSION)
    return True


def _exact_role(ui: Ui, label: str, role: str) -> dict:
    ui.refresh()
    matches = [
        element
        for element in ui.elements
        if element.get("role") == role and " ".join(str(element.get("label") or "").split()) == label
    ]
    if not matches:
        raise AssertionError(f"missing {role} {label}")
    return matches[0]


def _captcha_frames(ui: Ui) -> tuple[dict, dict, dict]:
    images = [
        element
        for element in ui.elements
        if element.get("role") == "AXImage" and (element.get("frame") or {}).get("w", 0) > 40
    ]
    if not images:
        raise AssertionError("captcha picture is not showing")
    # The piece bitmap includes a 12px pad, so its frame overlaps the background edge.
    overlap = 16

    def _on_picture(background: dict, piece: dict) -> bool:
        bounds = background.get("frame") or {}
        frame = piece.get("frame") or {}
        return (
            frame.get("w", 9999) < bounds.get("w", 0)
            and frame.get("x", -1) >= bounds.get("x", 0) - overlap
            and frame.get("y", -1) >= bounds.get("y", 0) - overlap
            and frame.get("x", 0) <= bounds.get("x", 0) + bounds.get("w", 0)
            and frame.get("y", 0) <= bounds.get("y", 0) + bounds.get("h", 0)
        )

    named_background = next((element for element in images if element.get("label") == "验证背景"), None)
    named_piece = next((element for element in images if element.get("label") == "验证拼图"), None)
    if named_background is not None and named_piece is not None and _on_picture(named_background, named_piece):
        background = named_background
        piece = named_piece
    else:
        background = max(images, key=lambda element: (element.get("frame") or {}).get("w", 0))
        pieces = [
            element
            for element in images
            if element is not background and _on_picture(background, element)
        ]
        if not pieces:
            raise AssertionError("captcha piece is not showing")
        piece = min(pieces, key=lambda element: (element.get("frame") or {}).get("w", 0))
    sliders = [
        element
        for element in ui.elements
        if element.get("role") == "AXSlider"
        or (
            "拖动" in " ".join(str(element.get("label") or "").split())
            and (element.get("frame") or {}).get("w", 9999) <= 80
        )
    ]
    if not sliders:
        raise AssertionError("captcha slider is not showing")
    slider = min(sliders, key=lambda element: (element.get("frame") or {}).get("w", 9999))
    return background, piece, slider


def _drag_captcha(ui: Ui) -> None:
    from PIL import Image

    handle, path = tempfile.mkstemp(prefix="towork-captcha-", suffix=".png")
    os.close(handle)
    shot = Path(path)
    try:
        ui.grab(str(shot))
        background, piece, slider = _captcha_frames(ui)
        bounds = background.get("frame") or {}
        left, top = ui._window_point(background, dx=0, dy=0)
        image = Image.open(shot).convert("RGB")
        width, height = image.size
        box = (
            max(0, left),
            max(0, top),
            min(width, left + int(bounds.get("w", 0))),
            min(height, top + int(bounds.get("h", 0))),
        )
        if box[2] - box[0] < 40 or box[3] - box[1] < 40:
            raise AssertionError("captcha picture is outside the window shot")
        crop = image.crop(box)
        piece_frame = piece.get("frame") or {}
        ratio = (float(piece_frame.get("y", 0)) - float(bounds.get("y", 0))) / max(1.0, float(bounds.get("h", 1)))
        slot = captcha_slide.answer_x(crop, ratio)
        slider_frame = slider.get("frame") or {}
        handle_w = float(slider_frame.get("w", 56))
        travel = float(bounds.get("w", 0)) + 2 - handle_w
        if travel <= 8:
            raise AssertionError("captcha track has no travel")
        fraction = slot / float(captcha_slide.IMAGE_W - captcha_slide.PIECE_W)
        # A slider drag posts pointer events and moves the system cursor.
        raise RuntimeError(f"background drag is unavailable on macOS ({fraction:.2f})")
    finally:
        shot.unlink(missing_ok=True)


def _phone_landed(ui: Ui) -> bool:
    return any(
        "13800138000" in " ".join(str(element.get("value") or "").split())
        for element in ui.elements
        if element.get("role") == "AXTextField"
    )


def _bad_password(ui: Ui) -> None:
    ui.click("密码登录", "AXRadioButton")
    deadline = time.time() + 8
    phone = secret = None
    while time.time() < deadline:
        ui.refresh()
        phones = [
            element
            for element in ui.elements
            if element.get("role") == "AXTextField"
            and "手机号" in " ".join(str(element.get("label") or "").split())
            and (element.get("frame") or {}).get("w", 0) > 40
        ]
        secrets = [
            element
            for element in ui.elements
            if element.get("role") == "AXTextField" and " ".join(str(element.get("label") or "").split()) == "密码"
        ]
        if phones and secrets:
            secret = secrets[0]
            secret_y = (secret.get("frame") or {}).get("y", 0)
            phone = min(phones, key=lambda element: abs((element.get("frame") or {}).get("y", 0) - secret_y))
            break
        time.sleep(0.3)
    if phone is None or secret is None:
        raise AssertionError("password login fields are not showing")
    ui.fill_element(phone, "13800138000")
    ui.fill_element(secret, "00000000")
    ui.refresh()
    if not _phone_landed(ui):
        ui.fill_element(phone, "13800138000")
        ui.fill_element(secret, "00000000")
        ui.refresh()
    if not _phone_landed(ui):
        raise AssertionError("phone field did not receive the number")
    ui.pixel_click(_exact_role(ui, "登录", "AXButton"))
    deadline = time.time() + 25
    retried = False
    while time.time() < deadline:
        ui.refresh()
        if ui.has("邮箱或密码不正确"):
            if ui.has("新建任务"):
                raise AssertionError("bad password left the login page")
            return
        if ui.has("请输入正确的"):
            raise AssertionError("login rejected the phone before the captcha")
        if ui.has("密码至少"):
            raise AssertionError("login rejected the password before the captcha")
        if ui.has("暂时无法签发挑战"):
            if retried:
                raise AssertionError("slide challenge was not issued")
            ui.pixel_click(_exact_role(ui, "重试", "AXButton"))
            retried = True
            time.sleep(2.0)
            continue
        ready = ui.has("向右拖动") and not ui.has("正在准备") and not ui.has("校验中")
        if ready:
            pictured = False
            for _ in range(8):
                ui.refresh()
                pictured = any(
                    element.get("role") == "AXImage" and (element.get("frame") or {}).get("w", 0) > 40
                    for element in ui.elements
                )
                if pictured:
                    break
                time.sleep(0.3)
            _drag_captcha(ui)
            time.sleep(1.6)
        else:
            time.sleep(0.4)
    raise AssertionError("bad password did not stay on the login page with 邮箱或密码不正确")


def p0_01(ui: Ui) -> None:
    failure: Exception | None = None
    try:
        _park_session()
        relaunch(ui, "密码登录", repark=True)
        if not (ui.has("密码登录") or ui.has("手机号登录")):
            raise AssertionError("cold start is already signed in; login surface is not showing")
        _bad_password(ui)
    except Exception as exc:
        failure = exc
    finally:
        _restore_session()
        try:
            relaunch(ui, "工作")
        except Exception as exc:
            if failure is None:
                failure = exc
            else:
                failure = RuntimeError(f"{failure}; session restore: {exc}")
    if failure is not None:
        raise failure


def p0_02(ui: Ui) -> None:
    ui.refresh()
    if ui.has("密码登录"):
        raise AssertionError("still on the login page")
    ui.work()
    if not (ui.has("描述任务") or ui.has("新建任务")):
        raise AssertionError("work page has no composer")


def p0_03(ui: Ui) -> None:
    ui.work()
    ui.open("消息")
    ui.wait_for("消息", 8)
    relaunch(ui)
    ui.wait_for("消息", 20)
    if ui.has("似乎出现了一些问题"):
        raise AssertionError("restored page is the error boundary")


def _pid_alive(pid: int) -> bool:
    try:
        os.kill(pid, 0)
    except OSError:
        return False
    return True


def _bundle_executable(pid: int) -> str:
    result = subprocess.run(["ps", "-p", str(pid), "-o", "comm="], capture_output=True, text=True)
    path = result.stdout.strip()
    if result.returncode != 0 or not path.endswith("/Contents/MacOS/ToWork"):
        raise AssertionError("owned process is not the ToWork bundle")
    return path


def _tenant_lock_held(slug: str = "towork-cua-fork1") -> bool:
    """The running bundle holds this file. A second launch must see the same lock."""
    path = Path(tempfile.gettempdir()) / f"dev.a3s.studio.{slug}.lock"
    if not path.is_file():
        return False
    fd = os.open(path, os.O_RDWR)
    try:
        try:
            fcntl.flock(fd, fcntl.LOCK_EX | fcntl.LOCK_NB)
        except OSError:
            return True
        fcntl.flock(fd, fcntl.LOCK_UN)
        return False
    finally:
        os.close(fd)


def _pid_windows(pid: int) -> list[tuple[int, str]]:
    swift = f"""
import CoreGraphics
let info = CGWindowListCopyWindowInfo([.optionOnScreenOnly, .excludeDesktopElements], kCGNullWindowID) as? [[String: Any]] ?? []
for w in info {{
  let owner = w["kCGWindowOwnerPID"] as? Int ?? -1
  let layer = w["kCGWindowLayer"] as? Int ?? -1
  if owner == {pid} && layer == 0 {{
    let number = w["kCGWindowNumber"] as? Int ?? 0
    let name = (w["kCGWindowName"] as? String ?? "").replacingOccurrences(of: "\\t", with: " ")
    print("\\(number)\\t\\(name)")
  }}
}}
"""
    result = subprocess.run(["swift", "-e", swift], capture_output=True, text=True)
    windows = []
    if result.returncode != 0:
        return windows
    for line in result.stdout.splitlines():
        number, _, name = line.partition("\t")
        if number.isdigit() and int(number) > 0:
            windows.append((int(number), name))
    return windows


def _raise_named_window(pid: int, name: str) -> None:
    safe = name.replace("\\", "\\\\").replace('"', '\\"')
    script = (
        'tell application "System Events"\n'
        f"  tell (first process whose unix id is {pid})\n"
        "    set frontmost to true\n"
        f'    perform action "AXRaise" of (first window whose name is "{safe}")\n'
        "  end tell\n"
        "end tell\n"
    )
    subprocess.run(["osascript", "-e", script], capture_output=True, text=True)


def _close_preview(pid: int, window_id: int, name: str) -> None:
    """Close a file editor. Its traffic-light stand-in is the 关闭 button."""
    preview = Drive(pid, window_id)
    try:
        deadline = time.time() + 8
        while time.time() < deadline:
            try:
                preview.refresh()
            except Exception:
                time.sleep(0.3)
                continue
            buttons = [
                element
                for element in preview.elements
                if element.get("role") == "AXButton"
                and element.get("label") == "关闭"
                and (element.get("frame") or {}).get("w", 99) <= 40
            ]
            if buttons:
                preview.pixel_click(buttons[-1], dx=12, dy=12)
                return
            time.sleep(0.3)
    finally:
        preview.close()


def p0_04(ui: Ui) -> None:
    """A second launch of this bundle may only raise the owned window.

    The dev launcher stops the previous bundle so a rebuild can replace it.
    The product path is the bundle itself: the tenant lock asks the running
    window to show, and the new process exits.
    """
    original = ui.pid
    if not _tenant_lock_held():
        raise AssertionError("the owned window is not holding the tenant lock")
    executable = _bundle_executable(original)
    before = _towork_pids()
    env = os.environ.copy()
    env["A3S_TENANT_SLUG"] = "towork-cua"
    env["A3S_STUDIO_INSTANCE"] = "fork1"
    env.pop("A3S_STUDIO_FORK_ALLOC", None)
    child = subprocess.Popen(
        [executable],
        env=env,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
    )
    try:
        code = child.wait(timeout=20)
    except subprocess.TimeoutExpired:
        child.kill()
        raise AssertionError("a second same-tenant launch stayed up")
    if not _pid_alive(original):
        try:
            pid, window = owned_target()
            ui.pid = pid
            ui.window_id = window
            ui.refresh()
        except Exception:
            pass
        raise AssertionError("a second same-tenant launch replaced the owned window")
    ui.pid = original
    ui.window_id = _window_for_pid(original)
    ui.refresh()
    if not ui.has("工作"):
        raise AssertionError("the existing window is not the work shell")
    extra = _towork_pids() - before - {original}
    if extra or code != 0:
        raise AssertionError("a second same-tenant window stayed up")


def p0_05(ui: Ui) -> None:
    ui.work()
    order = ui.rail()
    needed = ["工作", "知识", "消息", "团队", "插件"]
    positions = []
    for label in needed:
        if label not in order:
            raise AssertionError(f"rail {order} is missing {label}")
        positions.append(order.index(label))
    if positions != sorted(positions):
        raise AssertionError(f"rail order {order}")
    for label in ("文件", "笔记", "编码搭档"):
        if label in order:
            raise AssertionError(f"{label} is on the default rail")
    for label, marker in (("工作", "新建任务"), ("消息", "消息"), ("知识", "搜索笔记"), ("插件", "插件")):
        ui.open(label)
        ui.wait_for(marker, 8)


def p0_06(ui: Ui) -> None:
    stamp = str(int(time.time()))
    prompt = f"Reply with one short word. marker {stamp}"
    ui.send_work(prompt)


_SIDE_CHECKS = ("置顶", "取消置顶", "收起侧边栏", "隐藏侧边栏", "文件", "浏览器", "状态")


def _form_boxes(ui: Ui) -> list[dict]:
    """OpenUI checkboxes. Sidebar pin boxes also say 置顶 and are not the form."""
    return [
        element
        for element in ui.elements
        if element.get("role") == "AXCheckBox"
        and str(element.get("label") or "") not in _SIDE_CHECKS
        and "置顶" not in str(element.get("label") or "")
    ]


def p0_07(ui: Ui) -> None:
    stamp = str(int(time.time()))
    source = "\n".join(
        [
            'root = Card("Confirm", [row, go])',
            "row = Grid(2, [include, name])",
            'include = Checkbox("include", "Include", true)',
            'name = Input("name", "Name")',
            'go = Button("submit", "Continue")',
        ]
    )
    prompt = (
        f"marker {stamp}. Call ask_user_form one time. "
        "Put exactly these lines in the form argument and do not wrap them in a fence:\n"
        f"{source}"
    )
    before = ui._message_stamp()
    ui.work()
    ui.click("新建任务", "AXButton")
    time.sleep(0.4)
    ui.paste_composer(prompt)
    ui.submit_composer()
    deadline = time.time() + 150
    while time.time() < deadline:
        ui.refresh()
        if ui.approve_if_pending():
            time.sleep(0.4)
            continue
        if _form_boxes(ui):
            break
        time.sleep(1)
    else:
        raise AssertionError("the openui form did not render a checkbox")
    ui.refresh()
    box = _form_boxes(ui)[0]
    inputs = [
        element
        for element in ui.elements
        if element.get("role") in ("AXTextField", "AXTextArea") and "描述任务" not in str(element.get("label") or "")
    ]
    if not inputs:
        raise AssertionError("the openui form did not render the neighboring input")
    neighbor = min(inputs, key=lambda element: abs((element.get("frame") or {}).get("y", 0) - (box.get("frame") or {}).get("y", 0)))
    box_frame = box.get("frame") or {}
    field_frame = neighbor.get("frame") or {}
    # A form takes one field per row, even when the source groups the fields in a Grid.
    if field_frame.get("y", 0) <= box_frame.get("y", 0) + box_frame.get("h", 0) or abs(field_frame.get("x", 0) - box_frame.get("x", 0)) > 4:
        raise AssertionError(f"the input is not stacked under the checkbox on one edge box={box_frame} field={field_frame}")
    if str(box.get("value")) not in ("1", "true"):
        raise AssertionError(f"checkbox checked state is not filled: {box.get('value')}")
    if ui.has("这里不能继续这个任务"):
        raise AssertionError("submit did not continue the turn")
    ui.click("Continue", "AXButton")
    follow = time.time() + 40
    while time.time() < follow:
        ui.refresh()
        if ui.has("回答没有发出"):
            raise AssertionError("the host refused the form submit")
        if ui.approve_if_pending():
            time.sleep(0.4)
            continue
        reply = ui._assistant_after(before, prompt)
        if reply and reply != prompt and not ui.has("这里不能继续这个任务"):
            print(f"llm endpoint={ui._endpoint_host()} model=a3s/auto visible=reply")
            return
        time.sleep(1)
    raise AssertionError("submit did not continue the turn")


def p0_08(ui: Ui) -> None:
    _direct_message(ui, f"e2e-note {int(time.time())}")


def p0_09(ui: Ui) -> None:
    text = f"e2e-both {int(time.time())}"
    _direct_message(ui, text)
    ui.refresh()
    if not ui.has(text):
        raise AssertionError("sender cannot see the message")
    raise AssertionError("the other member is not signed in on this window")


def p0_10(ui: Ui) -> None:
    stamp = f"e2e-{int(time.time())}"
    stem = _create_knowledge_note(ui, f"{stamp}.md")
    ui.open("工作")
    _show_note(ui, stem)


def _drive_file_buttons(ui: Ui) -> list[dict]:
    return [
        element
        for element in ui.elements
        if element.get("role") == "AXButton"
        and "." in str(element.get("label") or "")
        and (element.get("frame") or {}).get("w", 0) > 80
    ]


def _show_team_drive(ui: Ui) -> None:
    ui.open("团队")
    try:
        ui.wait_for("云盘", 10)
    except AssertionError:
        ui.open("团队")
        ui.wait_for("云盘", 10)
    ui.click("云盘", "AXButton")
    time.sleep(0.8)
    ui.refresh()
    if ui.has("似乎出现了一些问题"):
        raise AssertionError("file page error boundary")


def _heading_labels(ui: Ui) -> list[str]:
    ui.refresh()
    return [
        " ".join(str(element.get("label") or "").split())
        for element in ui.elements
        if element.get("role") == "AXHeading" and str(element.get("label") or "").strip()
    ]


def _switch_joined_team(ui: Ui, label: str) -> None:
    """Select one joined-team row by its exact menu label."""
    _open_center_menu(ui)
    ui.refresh()
    anchor = ui._account_control().get("frame") or {}
    row = next(
        (
            element
            for element in ui.elements
            if element.get("role") == "AXMenuItem"
            and _in_account_menu(element, anchor)
            and " ".join(str(element.get("label") or "").split()) == label
        ),
        None,
    )
    if row is None:
        ui.press("escape")
        return
    frame = row.get("frame") or {}
    ui.pixel_click(row, dx=float(frame.get("w", 40)) / 2, dy=float(frame.get("h", 20)) / 2)
    time.sleep(0.8)
    ui.press("escape")
    time.sleep(0.3)


def _use_drive_with_a_file(ui: Ui) -> list[dict]:
    """Open a team drive that lists a file. An empty selected team is not the preview."""
    _show_team_drive(ui)
    found = _drive_file_buttons(ui)
    if found:
        return found
    _open_center_menu(ui)
    ui.refresh()
    try:
        labels = [
            " ".join(str(element.get("label") or "").split())
            for element in _joined_team_rows(ui)
        ]
    except AssertionError:
        labels = []
    ui.press("escape")
    time.sleep(0.2)
    # Member rows are the stable joined teams. Creator rows from earlier probes
    # are tried after them. The order inside one rank stays the menu order.
    labels.sort(key=lambda label: (0 if label.endswith("成员") else 1))
    seen = set()
    for label in labels:
        if label in seen:
            continue
        seen.add(label)
        headings = _heading_labels(ui)
        if any(label == heading or label.startswith(heading + " ") for heading in headings):
            continue
        _switch_joined_team(ui, label)
        _show_team_drive(ui)
        found = _drive_file_buttons(ui)
        if found:
            return found
    return []


def _activate_listed_file(ui: Ui, label: str) -> None:
    """Open a listed file. AX press focuses the row; Return runs its action.

    Press alone does not open the editor. Return is addressed to the same row
    after a fresh snapshot, because the press invalidates the previous token.
    """
    def matches() -> list[dict]:
        return [
            element
            for element in ui.elements
            if element.get("role") == "AXButton"
            and " ".join(str(element.get("label") or "").split()) == label
            and (element.get("frame") or {}).get("w", 0) > 80
        ]

    ui.refresh()
    found = matches()
    if not found:
        raise AssertionError("file preview did not open")
    ui.pixel_click(found[-1])
    time.sleep(0.2)
    ui.refresh()
    found = matches()
    if not found:
        raise AssertionError("file preview did not open")
    ui.press_background("return", found[-1])


def p0_11(ui: Ui) -> None:
    files = _use_drive_with_a_file(ui)
    if not files:
        raise AssertionError("file preview did not open")
    chosen = " ".join(str(files[-1].get("label") or "").split())
    stem = chosen.split(".")[0]
    before = {number for number, _name in _pid_windows(ui.pid)}
    before_labels = set(ui.labels())
    _activate_listed_file(ui, chosen)
    opened: tuple[int, str] | None = None
    deadline = time.time() + 8
    while time.time() < deadline:
        for number, name in _pid_windows(ui.pid):
            if number not in before and stem in name:
                opened = (number, name)
                break
        if opened:
            break
        ui.refresh()
        arrived = "\n".join(label for label in ui.labels() if label not in before_labels)
        if any(token in arrived for token in ("预览", "下载", "用默认应用打开", "返回")):
            return
        time.sleep(0.4)
    if opened is None:
        raise AssertionError("file preview did not open")
    _close_preview(ui.pid, opened[0], opened[1])
    deadline = time.time() + 4
    while time.time() < deadline:
        if not any(number == opened[0] for number, _name in _pid_windows(ui.pid)):
            return
        time.sleep(0.3)
    raise AssertionError("file preview stayed open")


def _hover_click(ui: Ui, label: str) -> None:
    """Press a row action. It stays visibility-hidden until the row is focused.

    A pointer hover would move the system cursor. The button is still in the
    accessibility tree, and an accessibility press reaches its click handler.
    """
    ui.refresh()
    matches = [
        element
        for element in ui.elements
        if element.get("role") == "AXButton"
        and " ".join(str(element.get("label") or "").split()) == label
    ]
    if not matches:
        raise AssertionError(f"missing control {label}")
    _activate(ui, matches[0])


def p0_12(ui: Ui) -> None:
    """The creator of a team this case owns moves its member into a new department."""
    name = None
    try:
        name = _create_team(ui)
        _team_nav(ui, "成员")
        time.sleep(0.6)
        ui.refresh()
        if not ui.has("成员"):
            raise AssertionError("member directory is not visible")
        department = f"e2e-dept-{int(time.time())}"
        root = next(
            (
                element
                for element in ui.elements
                if element.get("role") == "AXButton"
                and " ".join(str(element.get("label") or "").split()) == "全员"
            ),
            None,
        )
        if root is None:
            raise AssertionError("department tree did not show the root")
        # The selected row already shows 新建子部门. Clicking that row again
        # toggles AXSelected and the accessibility press rejects the flicker.
        adds = [
            element
            for element in ui.elements
            if element.get("role") == "AXButton"
            and " ".join(str(element.get("label") or "").split()) == "新建子部门"
        ]
        if not adds:
            _activate(ui, root)
            time.sleep(0.35)
            ui.refresh()
            adds = [
                element
                for element in ui.elements
                if element.get("role") == "AXButton"
                and " ".join(str(element.get("label") or "").split()) == "新建子部门"
            ]
        if not adds:
            raise AssertionError("department create is missing")
        _activate(ui, adds[0])
        time.sleep(0.4)
        ui.wait_for("新建部门", 6)
        _fill_last(ui, "名称", department)
        _click_exact(ui, "保存", last=True)
        deadline = time.time() + 8
        while time.time() < deadline:
            ui.refresh()
            created = [
                element
                for element in ui.elements
                if element.get("role") == "AXButton"
                and " ".join(str(element.get("label") or "").split()) == department
            ]
            if created and not ui.has("新建部门"):
                break
            time.sleep(0.3)
        else:
            raise AssertionError("department was not created")
        _hover_click(ui, "编辑")
        time.sleep(0.4)
        ui.refresh()
        if not ui.has("编辑「"):
            raise AssertionError("member department editor did not open")
        current = next(
            (
                element
                for element in ui.elements
                if element.get("role") == "AXStaticText"
                and " ".join(str(element.get("label") or "").split()) == "全员"
                and float((element.get("frame") or {}).get("y", 0)) > 400
            ),
            None,
        )
        if current is None:
            raise AssertionError("member department editor did not open")
        _activate(ui, current)
        time.sleep(0.45)
        ui.refresh()
        if not ui.has("搜索部门"):
            _activate(ui, current)
            time.sleep(0.45)
            ui.refresh()
        if not ui.has("搜索部门"):
            raise AssertionError("new department is not in the member editor")
        ui.fill(department, "搜索部门")
        time.sleep(0.35)
        ui.refresh()
        choice = next(
            (
                element
                for element in ui.elements
                if element.get("role") == "AXButton"
                and department in str(element.get("label") or "")
                and float((element.get("frame") or {}).get("y", 0)) > 400
            ),
            None,
        )
        if choice is None:
            raise AssertionError("new department is not in the member editor")
        # Press the department row itself. A coordinate release on the list
        # overhang hits the dialog backdrop and closes the editor.
        if not any(
            element.get("role") == "AXButton" and element.get("label") == "取消" for element in ui.elements
        ):
            raise AssertionError("new department is not in the member editor")
        _activate(ui, choice)
        time.sleep(0.35)
        ui.refresh()
        if not ui.has("编辑「"):
            raise AssertionError("member department editor closed before save")
        shown = any(
            element.get("role") == "AXStaticText"
            and " ".join(str(element.get("label") or "").split()) == department
            and 420 < float((element.get("frame") or {}).get("y", 0)) < 520
            and float((element.get("frame") or {}).get("h", 99)) < 30
            for element in ui.elements
        )
        if ui.has("搜索部门") or not shown:
            raise AssertionError("new department is not in the member editor")
        saves = _buttons_exact(ui, "保存")
        if not saves:
            raise AssertionError("member department did not change")
        _activate(ui, saves[-1])
        deadline = time.time() + 8
        while time.time() < deadline:
            ui.refresh()
            if ui.has("保存失败"):
                raise AssertionError("member department save failed")
            moved = [
                element
                for element in ui.elements
                if element.get("role") == "AXStaticText"
                and "e2e-dept-" in str(element.get("label") or "")
                and float((element.get("frame") or {}).get("x", 0)) > 1100
            ]
            if moved and not ui.has("编辑「"):
                return
            time.sleep(0.3)
        raise AssertionError("member department did not change")
    finally:
        ui.press("escape")
        time.sleep(0.2)
        if name:
            _dissolve_team(ui, name)


def _choose_appearance(ui: Ui, label: str) -> None:
    ui.open("设置")
    try:
        ui.wait_for("外观", 10)
    except AssertionError:
        ui.open("设置")
        ui.wait_for("外观", 10)
    ui.click("外观", "AXButton")
    time.sleep(0.4)
    ui.refresh()
    element = next(
        (
            item
            for item in ui.elements
            if item.get("role") == "AXRadioButton" and item.get("label") == label
        ),
        None,
    )
    if element is None:
        raise AssertionError(f"appearance choice {label} is missing")
    frame = element.get("frame") or {}
    ui.pixel_click(element, dx=float(frame.get("w", 40)) / 2, dy=float(frame.get("h", 20)) / 2)
    time.sleep(0.5)


def p0_13(ui: Ui) -> None:
    try:
        _choose_appearance(ui, "深色")
        ui.refresh()
        chosen = next(
            (
                item
                for item in ui.elements
                if item.get("role") == "AXRadioButton" and item.get("label") == "深色"
            ),
            None,
        )
        if chosen is None or str(chosen.get("value")) not in ("1", "true"):
            raise AssertionError("appearance did not switch to 深色")
        for label in ("工作", "消息", "知识"):
            ui.open(label)
            if ui.has("似乎出现了一些问题"):
                raise AssertionError(f"appearance broke {label}")
        relaunch(ui)
        _choose_appearance(ui, "深色")
        ui.refresh()
        kept = next(
            (
                item
                for item in ui.elements
                if item.get("role") == "AXRadioButton" and item.get("label") == "深色"
            ),
            None,
        )
        if kept is None or str(kept.get("value")) not in ("1", "true"):
            raise AssertionError("appearance did not survive restart")
    finally:
        _choose_appearance(ui, "跟随系统")


def p0_14(ui: Ui) -> None:
    ui.refresh()
    try:
        ui.open_account()
    except AssertionError as exc:
        raise AssertionError("account menu is missing") from exc
    if not any("退出登录" in label for label in ui.labels()):
        raise AssertionError("logout is not in the account menu")
    ui.press("escape")
    # Leave the session in place. Logging out would block the rest of the order
    # because this environment has no member password.
    raise AssertionError("logout returns to login only after a password is available to sign back in")


_MENU_CHROME = {
    "个人",
    "团队",
    "个人中心",
    "创建团队",
    "加入团队",
    "退出登录",
    "切换中心",
    "重试",
    "重新登录",
}
# Closed role suffixes on a joined-team row. The org name itself is not matched.
_TEAM_ROLE_SUFFIXES = ("创建人", "管理员", "成员")


def _in_account_menu(element: dict, anchor: dict) -> bool:
    """Account-menu body. The window and the menu bar share the accessibility tree."""
    frame = element.get("frame") or {}
    x, y, w, h = (frame.get(key) for key in ("x", "y", "w", "h"))
    if not all(isinstance(value, (int, float)) for value in (x, y, w, h)):
        return False
    if not (60 <= float(w) <= 260 and 16 <= float(h) <= 48):
        return False
    ax = float(anchor.get("x", 0))
    ay = float(anchor.get("y", 0))
    near_x = ax - 280 <= float(x) <= ax + 320
    near_y = ay - 80 <= float(y) <= ay + 520
    return near_x and near_y


def _center_menu_open(ui: Ui) -> bool:
    return ui.has("切换中心") and (
        ui.has("创建团队") or ui.has("加入团队") or ui.has("个人中心") or ui.has("还没有团队") or ui.has("无法加载团队")
    )


def _open_center_menu(ui: Ui) -> None:
    """Open the account menu. The avatar toggles, so a second click reopens a menu the first click closed."""
    ui.refresh()
    if _center_menu_open(ui):
        return
    for attempt in range(3):
        if attempt == 2:
            ui.press("escape")
            time.sleep(0.2)
            ui.refresh()
            if _center_menu_open(ui):
                return
        ui.open_account()
        deadline = time.time() + 3
        while time.time() < deadline:
            ui.refresh()
            if _center_menu_open(ui):
                return
            time.sleep(0.25)
    raise AssertionError("account center menu did not open")


def _click_menu_label(ui: Ui, label: str) -> None:
    """Click a control in the open account menu. A rail icon can share the word."""
    ui.refresh()
    anchor = ui._account_control().get("frame") or {}
    if label in ("团队", "个人"):
        roles = ("AXRadioButton",)
    elif label == "个人中心":
        roles = ("AXMenuItem", "AXButton")
    else:
        roles = ("AXRadioButton", "AXButton", "AXMenuItem")
    matches = [
        element
        for element in ui.elements
        if element.get("role") in roles
        and " ".join(str(element.get("label") or "").split()) == label
        and _in_account_menu(element, anchor)
    ]
    if not matches:
        raise AssertionError(f"missing control {label}")
    target = min(matches, key=lambda element: (element.get("frame") or {}).get("w", 9999))
    frame = target.get("frame") or {}
    ui.pixel_click(target, dx=float(frame.get("w", 20)) / 2, dy=float(frame.get("h", 20)) / 2)
    time.sleep(0.35)


def _joined_team_rows(ui: Ui) -> list[dict]:
    anchor_element = ui._account_control()
    anchor = anchor_element.get("frame") or {}
    account = " ".join(str(anchor_element.get("label") or "").split())
    rows = []
    for element in ui.elements:
        if element.get("role") != "AXMenuItem":
            continue
        if not _in_account_menu(element, anchor):
            continue
        frame = element.get("frame") or {}
        if frame.get("h", 0) < 28:
            continue
        label = " ".join(str(element.get("label") or "").split())
        if not label or label in _MENU_CHROME:
            continue
        if account and (label == account or label.startswith(account + " ")):
            continue
        if not any(suffix in label for suffix in _TEAM_ROLE_SUFFIXES):
            continue
        rows.append(element)
    if not rows:
        raise AssertionError("team center has no joined team")
    return sorted(rows, key=lambda element: (element.get("frame") or {}).get("y", 0))


def _joined_team_row(ui: Ui) -> dict:
    return _joined_team_rows(ui)[0]


def _press_team_retry(ui: Ui) -> None:
    """Press the team-list retry beside the load error. The control is narrower than a center row."""
    error = next(
        (
            element
            for element in ui.elements
            if "无法加载团队" in " ".join(str(element.get("label") or "").split())
        ),
        None,
    )
    if error is None:
        return
    origin = error.get("frame") or {}
    error_x = float(origin.get("x", 0))
    error_y = float(origin.get("y", 0))
    retry = None
    for element in ui.elements:
        if element.get("role") != "AXButton":
            continue
        if " ".join(str(element.get("label") or "").split()) != "重试":
            continue
        frame = element.get("frame") or {}
        if abs(float(frame.get("y", -9999)) - error_y) > 24:
            continue
        if float(frame.get("x", -9999)) < error_x:
            continue
        retry = element
        break
    if retry is None:
        return
    ui.pixel_click(retry)


def _select_center(ui: Ui, label: str) -> None:
    """Switch the account center. 团队/个人 on the menu are scope tabs, not the center."""
    _open_center_menu(ui)
    if label == "个人":
        if not ui.has("个人中心"):
            _click_menu_label(ui, "个人")
            time.sleep(0.3)
        _click_menu_label(ui, "个人中心")
    elif label == "团队":
        # The team tab click can dismiss the menu before the joined rows appear.
        # Reopen it and read again. An empty team list still fails closed.
        row = None
        deadline = time.time() + 12
        while row is None and time.time() < deadline:
            ui.refresh()
            if not _center_menu_open(ui):
                _open_center_menu(ui)
                ui.refresh()
            on_team_tab = ui.has("创建团队") or ui.has("还没有团队") or ui.has("无法加载团队")
            if not on_team_tab:
                _click_menu_label(ui, "团队")
                time.sleep(0.45)
                continue
            if ui.has("无法加载团队"):
                _press_team_retry(ui)
                time.sleep(0.8)
                continue
            if ui.has("还没有团队"):
                raise AssertionError("team center has no joined team")
            try:
                row = _joined_team_row(ui)
            except AssertionError:
                time.sleep(0.4)
        if row is None:
            raise AssertionError("team center has no joined team")
        frame = row.get("frame") or {}
        ui.pixel_click(row, dx=float(frame.get("w", 40)) / 2, dy=float(frame.get("h", 20)) / 2)
    else:
        raise AssertionError(f"unknown center {label}")
    time.sleep(0.8)
    ui.press("escape")
    time.sleep(0.3)
    deadline = time.time() + 6
    order = ui.rail()
    while time.time() < deadline:
        if label == "团队" and ("团队" in order or "组织" in order):
            return
        if label == "个人" and "团队" not in order and "组织" not in order:
            return
        time.sleep(0.4)
        order = ui.rail()
    if label == "团队":
        raise AssertionError(f"team center did not show the team rail: {order}")
    raise AssertionError(f"personal center still shows the team rail: {order}")


def p0_15(ui: Ui) -> None:
    try:
        _select_center(ui, "个人")
        ui.refresh()
        order = ui.rail()
        for hidden in ("组织", "日程", "平台", "团队"):
            if hidden in order:
                raise AssertionError(f"personal center still shows {hidden}: {order}")
        if "插件" not in order:
            raise AssertionError(f"plugins left the personal rail: {order}")
    finally:
        _select_center(ui, "团队")


def p0_16(ui: Ui) -> None:
    try:
        ui.work()
        ui.set_window_size(720, 800)
        time.sleep(0.6)
        ui.open("连接器")
        time.sleep(0.6)
        ui.refresh()
        if ui.has("似乎出现了一些问题"):
            raise AssertionError("connector page failed")
        if ui.has("描述任务"):
            raise AssertionError("narrow window left the connector catalog on the composer")
        if not ui.has("连接器"):
            raise AssertionError("connector catalog did not open")
    finally:
        # A 720-wide frame hides 新建任务. Put the frame back even when the
        # catalog assertion already failed, and do not leave the next case there.
        try:
            ui.set_window_size(1440, 920)
            ui.press("escape")
            ui.work()
        except Exception:
            try:
                relaunch(ui, "新建任务")
            except Exception:
                return


def w_01(ui: Ui) -> None:
    ui.work()
    ui.click("新建任务", "AXButton")
    time.sleep(0.4)
    ui.refresh()
    if ui.has("pong") and ui.has("ToBot"):
        raise AssertionError("old turn stayed on the canvas")
    if not ui.has("描述任务"):
        raise AssertionError("composer is not back")


def w_02(ui: Ui) -> None:
    ui.work()
    ui.click("新建任务", "AXButton")
    time.sleep(0.3)
    before = ui._message_stamp()
    ui.submit_composer()
    time.sleep(1)
    if ui._message_stamp() > before + 0.2:
        raise AssertionError("empty send created a turn")
    ui.refresh()
    if not ui.has("描述任务"):
        raise AssertionError("composer is not editable")


def _home(ui: Ui) -> None:
    ui.work()
    ui.click("新建任务", "AXButton")
    time.sleep(0.4)
    ui.refresh()


def _posture_popup(ui: Ui) -> dict:
    _home(ui)
    popup = next(
        (
            element
            for element in ui.elements
            if element.get("role") == "AXPopUpButton" and str(element.get("label") or "").startswith("授权模式")
        ),
        None,
    )
    if popup is None:
        raise AssertionError("permission posture is not visible")
    return popup


def _choose_posture(ui: Ui, label: str) -> None:
    popup = _posture_popup(ui)
    if label in str(popup.get("label") or ""):
        return
    item = None
    for _ in range(2):
        # A press on this popup does not open the menu. Space does.
        ui.press_background("space", popup)
        time.sleep(0.6)
        ui.refresh()
        item = next(
            (
                element
                for element in ui.elements
                if element.get("role") == "AXMenuItem"
                and " ".join(str(element.get("label") or "").split()).startswith(label)
            ),
            None,
        )
        if item is not None:
            break
        ui.press("escape")
        time.sleep(0.2)
        ui.refresh()
        popup = next(
            (
                element
                for element in ui.elements
                if element.get("role") == "AXPopUpButton" and str(element.get("label") or "").startswith("授权模式")
            ),
            popup,
        )
    if item is None:
        ui.press("escape")
        raise AssertionError(f"posture {label} is missing")
    # The row's accessible name starts with the posture and then its description.
    ui.press_background("space", item)
    time.sleep(0.4)


_MODEL_CHROME = ("RoyLin", "林书豪", "展示方式", "更多选项", "授权模式", "项目文件夹", "工作区文件")


def _model_trigger(ui: Ui) -> dict:
    for element in ui.elements:
        if element.get("role") != "AXPopUpButton":
            continue
        label = str(element.get("label") or "")
        if any(label.startswith(prefix) for prefix in _MODEL_CHROME):
            continue
        return element
    raise AssertionError("model picker is not visible")


def _open_model_picker(ui: Ui) -> None:
    _home(ui)
    trigger = _model_trigger(ui)
    frame = trigger.get("frame") or {}
    ui.pixel_click(trigger, dx=max(8.0, float(frame.get("w", 40)) / 2), dy=float(frame.get("h", 20)) / 2)
    time.sleep(0.55)
    ui.refresh()


def _restore_auto_model(ui: Ui) -> None:
    try:
        _open_model_picker(ui)
        if ui.has("关闭模型选择"):
            auto = next(
                (
                    element
                    for element in ui.elements
                    if "自动选择模型" in str(element.get("label") or "")
                ),
                None,
            )
            if auto is not None:
                ui.pixel_click(auto, dx=40, dy=8)
                time.sleep(0.4)
    finally:
        ui.press("escape")


def w_03(ui: Ui) -> None:
    try:
        _open_model_picker(ui)
        rows = []
        for element in ui.elements:
            label = " ".join(str(element.get("label") or "").split())
            if element.get("role") == "AXStaticText" and "×" in label:
                rows.append((element, label.split("×", 1)[0].strip()))
        if not rows:
            raise AssertionError("model list has no second model to switch to")
        element, name = rows[0]
        ui.pixel_click(element, dx=40, dy=8)
        time.sleep(0.6)
        ui.click("新建任务", "AXButton")
        time.sleep(0.4)
        ui.refresh()
        trigger = _model_trigger(ui)
        shown = " ".join(str(trigger.get("label") or "").split())
        if name[:12] not in shown:
            raise AssertionError(f"next task does not show {name}: {shown}")
    finally:
        _restore_auto_model(ui)


def w_04(ui: Ui) -> None:
    try:
        _choose_posture(ui, "仅规划")
        ui.refresh()
        popup = _posture_popup(ui)
        if "仅规划" not in str(popup.get("label") or ""):
            raise AssertionError("posture did not switch to 仅规划")
        relaunch(ui)
        popup = _posture_popup(ui)
        if "仅规划" not in str(popup.get("label") or ""):
            raise AssertionError("posture did not survive restart")
    finally:
        try:
            _choose_posture(ui, "逐次确认")
        except Exception:
            ui.press("escape")


def _checkbox(ui: Ui, label: str) -> dict:
    box = next(
        (
            element
            for element in ui.elements
            if element.get("role") == "AXCheckBox" and element.get("label") == label
        ),
        None,
    )
    if box is None:
        raise AssertionError(f"plus menu is missing {label}")
    return box


def w_05(ui: Ui) -> None:
    _home(ui)
    if ui.has("研究模式"):
        raise AssertionError("research mode is in the composer")
    ui.pixel_click(ui.find("更多选项", "AXPopUpButton"))
    time.sleep(0.4)
    ui.refresh()
    originals = {}
    try:
        for label in ("自动审查", "产物追溯"):
            box = _checkbox(ui, label)
            originals[label] = str(box.get("value"))
            ui.pixel_click(box)
            time.sleep(0.3)
            ui.refresh()
            if not ui.has(label):
                ui.pixel_click(ui.find("更多选项", "AXPopUpButton"))
                time.sleep(0.3)
                ui.refresh()
            flipped = _checkbox(ui, label)
            if str(flipped.get("value")) == originals[label]:
                raise AssertionError(f"{label} did not toggle")
        if ui.has("研究模式"):
            raise AssertionError("research mode is in the plus menu")
    finally:
        ui.refresh()
        for label, value in originals.items():
            if not ui.has(label):
                if ui.has("更多选项"):
                    ui.pixel_click(ui.find("更多选项", "AXPopUpButton"))
                    time.sleep(0.3)
                    ui.refresh()
            if not ui.has(label):
                continue
            current = _checkbox(ui, label)
            if str(current.get("value")) != value:
                ui.pixel_click(current)
                time.sleep(0.2)
                ui.refresh()
        ui.press("escape")


def _e2e_project_dirs() -> list[Path]:
    """Suite project folders only. The selected composer root is one of these."""
    path = SUPPORT / "agent-plane/sessions.json"
    if not path.is_file():
        return []
    try:
        rows = json.loads(path.read_text())
    except json.JSONDecodeError:
        return []
    if not isinstance(rows, list):
        return []
    found: list[Path] = []
    seen: set[str] = set()
    for row in rows:
        if not isinstance(row, dict):
            continue
        workspace = row.get("workspace")
        if not isinstance(workspace, dict):
            continue
        name = str(workspace.get("name") or "")
        raw = workspace.get("path")
        if not name.startswith("e2e-proj-") or not isinstance(raw, str) or "e2e-proj-" not in raw:
            continue
        directory = Path(raw)
        if not directory.is_dir():
            continue
        key = str(directory)
        if key in seen:
            continue
        seen.add(key)
        found.append(directory)
    return found


def _paste_without_replacing(ui: Ui, text: str) -> None:
    """Paste at the caret. Select All would delete the file chip."""
    ui.call("clipboard_write", {"text": text})
    ui.menu_click("Paste")


def _file_chip(ui: Ui, filename: str) -> dict | None:
    return next(
        (
            element
            for element in ui.elements
            if element.get("role") == "AXPopUpButton" and str(element.get("label") or "") == filename
        ),
        None,
    )


def _workspace_search(ui: Ui, filename: str) -> dict | None:
    for element in ui.elements:
        if element.get("role") != "AXTextField":
            continue
        label = str(element.get("label") or "")
        value = str(element.get("value") or "")
        if "搜索文件" in label or filename in value or filename in label:
            return element
    return None


def _open_workspace_files(ui: Ui, filename: str) -> dict:
    for _attempt in range(4):
        ui.refresh()
        field = _workspace_search(ui, filename)
        if field is not None:
            return field
        target = next(
            (
                element
                for element in ui.elements
                if element.get("role") == "AXPopUpButton" and str(element.get("label") or "") == "工作区文件"
            ),
            None,
        )
        if target is None:
            time.sleep(0.25)
            continue
        ui.pixel_click(target)
        time.sleep(0.45)
    raise AssertionError("workspace file picker did not open")


def _file_row(ui: Ui, filename: str, field: dict) -> dict | None:
    """The workspace option whose accessible name is the file. A coordinate under the search field is not a row."""
    rows = []
    for element in ui.elements:
        if element is field or element.get("element_token") == field.get("element_token"):
            continue
        if element.get("role") != "AXStaticText" or not element.get("element_token"):
            continue
        label = " ".join(str(element.get("label") or "").split())
        if label == filename or label.startswith(filename + " "):
            rows.append(element)
    if not rows:
        return None
    exact = [element for element in rows if " ".join(str(element.get("label") or "").split()) == filename]
    chosen = exact or rows
    return min(chosen, key=lambda element: float((element.get("frame") or {}).get("y") or 0))


def _suite_project_shown(ui: Ui, project: str) -> bool:
    """The composer trigger shows the workspace name only after that row is selected."""
    return any(
        element.get("role") == "AXPopUpButton"
        and str(element.get("label") or "").startswith("项目文件夹")
        and project in " ".join(str(element.get("label") or "").split())
        for element in ui.elements
    )


def _select_suite_project(ui: Ui, project: str) -> None:
    """Point the composer at one suite workspace. A blank task keeps the previous workspace."""
    ui.refresh()
    popup = next(
        (
            element
            for element in ui.elements
            if element.get("role") == "AXPopUpButton" and str(element.get("label") or "").startswith("项目文件夹")
        ),
        None,
    )
    if popup is None:
        raise AssertionError("project list did not open")
    ui.press_background("space", popup)
    time.sleep(0.5)
    ui.refresh()
    search = next(
        (
            element
            for element in ui.elements
            if element.get("role") == "AXTextField"
            and "搜索工作目录"
            in " ".join(str(element.get("label") or "").split() + str(element.get("value") or "").split())
        ),
        None,
    )
    if search is None:
        raise AssertionError("project list did not open")
    ui.type_into(search, project)
    # The work sidebar repeats the project name. The list row shares the search
    # field's left edge. Space on the sidebar namesake does not select it.
    # The delete control has a different name.
    button = None
    for _ in range(6):
        time.sleep(0.35)
        ui.refresh()
        field = next(
            (
                element
                for element in ui.elements
                if element.get("role") == "AXTextField"
                and project in " ".join(str(element.get("label") or "").split() + str(element.get("value") or "").split())
            ),
            None,
        )
        origin = float((field.get("frame") or {}).get("x", -9999)) if field else -9999
        aligned = [
            element
            for element in ui.elements
            if element.get("role") == "AXButton"
            and " ".join(str(element.get("label") or "").split()) == project
            and element.get("element_token")
            and abs(float((element.get("frame") or {}).get("x", -9999)) - origin) < 80
        ]
        if aligned:
            button = max(aligned, key=lambda element: float((element.get("frame") or {}).get("w") or 0))
            break
    if button is None:
        ui.press("escape")
        raise AssertionError("suite project is not in the list")
    ui.pixel_click(button)
    time.sleep(0.45)
    ui.refresh()
    if _suite_project_shown(ui, project):
        return
    ui.press("escape")
    raise AssertionError("suite project was not selected")


def _attach_workspace_file(ui: Ui, filename: str) -> None:
    """Select the file option, then press 添加. Apply stays disabled until that row is selected."""
    field = _open_workspace_files(ui, filename)
    # Menu paste stays in the composer. The search field is already open, and
    # typing reaches it after the field is focused. The file name stays literal.
    ui.type_into(field, filename)
    row = None
    for _ in range(4):
        time.sleep(0.45)
        ui.refresh()
        field = _workspace_search(ui, filename)
        if field is None:
            continue
        kept = str(field.get("value") or "") == filename or str(field.get("label") or "") == filename
        if not kept:
            continue
        if ui.has("没有匹配的文件") or ui.has("请先选择项目文件夹") or ui.has("先选择项目文件夹"):
            continue
        row = _file_row(ui, filename, field)
        if row is not None:
            break
    if field is None:
        raise AssertionError("workspace search did not keep the file name")
    if row is None and (
        ui.has("没有匹配的文件") or ui.has("请先选择项目文件夹") or ui.has("先选择项目文件夹")
    ):
        raise AssertionError("workspace search did not list the file")
    if row is None:
        raise AssertionError("workspace file row was not selected")
    ui.pixel_click(row)
    time.sleep(0.4)
    ui.refresh()
    chosen = [
        element
        for element in ui.elements
        if str(element.get("label") or "").startswith("添加") and "1" in str(element.get("label") or "")
    ]
    if not chosen:
        raise AssertionError("workspace file row was not selected")
    ui.pixel_click(chosen[0])
    time.sleep(0.5)
    ui.refresh()
    if not ui.has(filename) or _file_chip(ui, filename) is None:
        raise AssertionError("attached file name is not visible")


def _remove_workspace_file(ui: Ui, filename: str) -> None:
    """The chip's remove control sits inside the token button, so the tree has no separate button.

    Pressing the chip selects that token. Backspace deletes the selected token.
    """
    ui.refresh()
    token = _file_chip(ui, filename)
    if token is None:
        raise AssertionError("attached file chip is missing")
    ui.pixel_click(token)
    time.sleep(0.25)
    ui.press("backspace")
    time.sleep(0.35)
    ui.refresh()
    if ui.has(filename):
        raise AssertionError("removing the file left its name visible")


def _fresh_composer(ui: Ui) -> None:
    ui.work()
    ui.refresh()
    wide = [
        element
        for element in ui.elements
        if element.get("role") == "AXButton"
        and " ".join(str(element.get("label") or "").split()) == "新建任务"
        and float((element.get("frame") or {}).get("w", 0)) > 80
    ]
    if wide:
        frame = wide[0].get("frame") or {}
        ui.pointer_click(wide[0], dx=float(frame.get("w", 80)) / 2, dy=float(frame.get("h", 32)) / 2)
        time.sleep(0.45)
        ui.refresh()
    if not ui.has("描述任务"):
        raise AssertionError("composer is not open")
    if not ui.has("工作区文件"):
        raise AssertionError("attach control is missing")


def w_06(ui: Ui) -> None:
    _fresh_composer(ui)
    filename = f"e2e-file-{int(time.time())}.txt"
    directories = _e2e_project_dirs()
    if not directories:
        raise AssertionError("no suite project folder is available")
    project = directories[0].parent.name
    if not project.startswith("e2e-proj-"):
        raise AssertionError("no suite project folder is available")
    for directory in directories:
        (directory / filename).write_text("e2e\n", encoding="utf-8")
    _select_suite_project(ui, project)
    prompt = "请只用一句话确认收到。"
    try:
        _attach_workspace_file(ui, filename)
        _remove_workspace_file(ui, filename)
        _attach_workspace_file(ui, filename)
        token = _file_chip(ui, filename)
        if token is None:
            raise AssertionError("attached file chip is missing")
        areas = [element for element in ui.elements if element.get("role") == "AXTextArea"]
        area = None
        if areas:
            area = max(areas, key=lambda element: float((element.get("frame") or {}).get("w", 0)))
            ui.press_background("end", area)
        time.sleep(0.15)
        _paste_without_replacing(ui, prompt)
        time.sleep(0.35)
        ui.refresh()
        if filename in prompt or not ui.has(filename) or not ui.has(prompt):
            raise AssertionError("composer lost the file name or the prompt")
        before = ui._message_stamp()
        # The send control is a native button named 发送. Addressed Return on the
        # WebKit field is refused, and an untargeted Return does not submit.
        ui.submit_composer()
        outcome = ui._await_turn(prompt, before, 150)
        print(f"llm endpoint={ui._endpoint_host()} model=a3s/auto visible={outcome}")
        ui.refresh()
        if not ui.has(filename):
            raise AssertionError("sent turn hid the file name")
    finally:
        ui.press("escape")


def _attach(ui: Ui) -> None:
    """Bind the owned window. Relaunch once if the process died between cases."""
    try:
        ui._restore_frame()
        ui.refresh()
        return
    except Exception:
        pass
    try:
        ui.pid, ui.window_id = owned_target()
        ui._restore_frame()
        ui.refresh()
        return
    except Exception:
        pass
    relaunch(ui, "工作")


def _signed_out(ui: Ui) -> bool:
    ui.refresh()
    return ui.has("密码登录") and not ui.has("新建任务")


def _recover(ui: Ui) -> None:
    """Close a dialog left by the previous case. Never choose 退出登录.

    A deleted session file boots the next launch on the login card. Put the
    run's own copy back and relaunch so later cases still reach 工作.
    """
    try:
        if not SESSION.is_file():
            _reinstate_session()
        _attach(ui)
        _close_extra_windows(ui.pid)
        _cancel_create_dialog(ui)
        if _signed_out(ui) and _reinstate_session():
            relaunch(ui, "工作")
        if ui.has("关闭模型选择"):
            ui.pixel_click(ui.find("关闭模型选择", "AXButton"))
            time.sleep(0.2)
            ui.refresh()
        ui.press("escape")
        time.sleep(0.15)
        ui.refresh()
        if ui.has("退出登录"):
            ui.press("escape")
            time.sleep(0.15)
            ui.refresh()
        if ui.has("项目是一个独立的工作文件夹") and ui.has("取消"):
            ui.pixel_click(ui.find("取消", "AXButton"))
            time.sleep(0.3)
        if ui.has("似乎出现了一些问题") and ui.has("重新加载"):
            ui.pixel_click(ui.find("重新加载", "AXButton"))
            time.sleep(0.8)
            ui.refresh()
        if ui.has("似乎出现了一些问题") and not ui.has("密码登录"):
            relaunch(ui, "新建任务")
            return
        try:
            ui.find("工作", "AXButton")
        except AssertionError:
            if not ui.has("密码登录"):
                relaunch(ui, "新建任务")
            return
        order = ui.rail()
        if "团队" not in order and "组织" not in order and not getattr(ui, "_team_center_failed", False):
            try:
                _select_center(ui, "团队")
            except AssertionError:
                ui._team_center_failed = True
    except Exception:
        return


def _close_project_dialog(ui: Ui) -> None:
    ui.refresh()
    if ui.has("项目是一个独立的工作文件夹") and ui.has("取消"):
        ui.pixel_click(ui.find("取消", "AXButton"))
        time.sleep(0.4)


def w_07(ui: Ui) -> None:
    ui.work()
    name = f"e2e-proj-{int(time.time())}"
    try:
        ui.click("新建项目", "AXButton")
        time.sleep(0.4)
        ui.refresh()
        if not ui.has("项目名称"):
            raise AssertionError("new project did not open a name field")
        ui.press_return(ui.find("项目名称", "AXTextField"))
        time.sleep(0.4)
        ui.refresh()
        if not ui.has("项目是一个独立的工作文件夹"):
            raise AssertionError("empty project name was not refused")
        ui.fill(name, "项目名称")
        ui.refresh()
        field = ui.find("项目名称", "AXTextField")
        if name not in str(field.get("value") or ""):
            raise AssertionError("project name field did not receive the name")
        creates = [
            element
            for element in ui.elements
            if element.get("role") == "AXButton" and element.get("label") == "创建"
        ]
        if not creates:
            raise AssertionError("project create did not offer a name-only confirm")
        ui.pixel_click(creates[0])
        time.sleep(0.8)
        ui.refresh()
        if ui.has("项目是一个独立的工作文件夹"):
            raise AssertionError("name-only create left the dialog open")
        if not ui.has(name):
            raise AssertionError("name-only create did not show the project")
        _fill_project(ui, name)
        if not ui.has("展开显示"):
            raise AssertionError("expand past four is not shown")
    finally:
        _close_project_dialog(ui)


def _inbox_tasks(ui: Ui, project_x: float) -> list[dict]:
    return [
        element
        for element in ui.elements
        if element.get("role") == "AXButton"
        and (
            str(element.get("label") or "").startswith("marker ")
            or str(element.get("label") or "").startswith("Reply with")
        )
        and (element.get("frame") or {}).get("w", 0) > 100
        and (element.get("frame") or {}).get("x", 9999) <= project_x + 8
    ]


def _drag_onto(ui: Ui, source: dict, target: dict) -> None:
    """Task moves are pointer drags. Background drag is unavailable and would move the cursor."""
    del ui, source, target
    raise RuntimeError("background drag is unavailable on macOS")


def _fill_project(ui: Ui, name: str) -> None:
    """Drop five existing end-to-end tasks into the new project so the expand row can show."""
    for _ in range(5):
        ui.refresh()
        if ui.has("展开显示"):
            return
        project = next(
            (
                element
                for element in ui.elements
                if element.get("role") == "AXButton" and str(element.get("label") or "").startswith(name)
            ),
            None,
        )
        if project is None:
            return
        rows = _inbox_tasks(ui, float((project.get("frame") or {}).get("x", 0)))
        if not rows:
            break
        _drag_onto(ui, rows[0], project)
    deadline = time.time() + 4
    while time.time() < deadline:
        ui.refresh()
        if ui.has("展开显示"):
            return
        time.sleep(0.3)


def _visible_band(ui: Ui) -> tuple[float, float]:
    web = next(element for element in ui.elements if element.get("role") == "AXWebArea")
    frame = web.get("frame") or {}
    top = float(frame.get("y", 0)) + 80
    bottom = float(frame.get("y", 0)) + float(frame.get("h", 800)) - 30
    return top, bottom


def _in_band(element: dict, top: float, bottom: float) -> bool:
    y = float((element.get("frame") or {}).get("y", -1))
    return top <= y <= bottom


_SIDEBAR_NAV = ("智能体", "连接器", "技能", "自动化")


def _scrollport_top(ui: Ui) -> float:
    """The project list scrolls under the workbench nav. Rows reported above that cover the nav."""
    top = 0.0
    for element in ui.elements:
        if element.get("role") != "AXButton":
            continue
        label = " ".join(str(element.get("label") or "").split())
        if label not in _SIDEBAR_NAV:
            continue
        frame = element.get("frame") or {}
        if float(frame.get("w", 0)) < 80:
            continue
        top = max(top, float(frame.get("y", 0)) + float(frame.get("h", 0)))
    return top


def _in_scrollport(ui: Ui, element: dict) -> bool:
    return float((element.get("frame") or {}).get("y", -1)) >= _scrollport_top(ui) - 2


def _project_buttons(ui: Ui) -> list[dict]:
    return [
        element
        for element in ui.elements
        if element.get("role") == "AXButton" and str(element.get("label") or "").startswith("e2e-proj-")
    ]


def _task_buttons(ui: Ui) -> list[dict]:
    rows = []
    for element in ui.elements:
        if element.get("role") != "AXButton" or (element.get("frame") or {}).get("w", 0) <= 100:
            continue
        label = str(element.get("label") or "")
        if "marker" in label or label.startswith("Reply with") or label.startswith("e2e-renamed-"):
            rows.append(element)
    return rows


def _project_expanded(ui: Ui, project: dict) -> bool:
    """A collapsed project row is about 36px. The next row starts just after it.

    A fixed 80px window treats the following project's task as this project's child.
    """
    py = float((project.get("frame") or {}).get("y", 0))
    following = [
        float((element.get("frame") or {}).get("y", 0))
        for element in _project_buttons(ui)
        if float((element.get("frame") or {}).get("y", 0)) > py + 4
    ]
    if following:
        return min(following) - py > 48
    # The inbox heading always sits under the last project. Distance to it is not expansion.
    limit = py + 220
    for element in ui.elements:
        label = " ".join(str(element.get("label") or "").split())
        frame = element.get("frame") or {}
        if label != "任务" or element.get("role") != "AXHeading" or float(frame.get("w", 0)) <= 48:
            continue
        hy = float(frame.get("y", 0))
        if hy > py + 4:
            limit = hy
            break
    return any(py + 8 < float((task.get("frame") or {}).get("y", 0)) < limit for task in _task_buttons(ui))


def _collapse_one_project(ui: Ui, skip_name: str) -> bool:
    """Fold the lowest open project in view. Skip the project that holds the dragged row."""
    ui.refresh()
    top, bottom = _visible_band(ui)
    choices = []
    for project in _project_buttons(ui):
        label = " ".join(str(project.get("label") or "").split())
        if skip_name and skip_name in label:
            continue
        if not _in_band(project, top, bottom) or not _in_scrollport(ui, project) or not _project_expanded(ui, project):
            continue
        choices.append(project)
    if not choices:
        return False
    target = max(choices, key=lambda element: float((element.get("frame") or {}).get("y", 0)))
    # A System Events click does not run this row's toggle. A pointer press does.
    ui.pointer_click(target, dx=40, dy=16)
    time.sleep(0.18)
    return True


def _collapse_expanded_projects(ui: Ui) -> None:
    """Fold one open project so an inbox row and a project row can share the window."""
    _collapse_one_project(ui, "")


def _reveal_project_tasks(ui: Ui, project_name: str) -> bool:
    """Open the preview overflow when the moved task is past the first four rows."""
    ui.refresh()
    projects = [
        element
        for element in _project_buttons(ui)
        if project_name and project_name in " ".join(str(element.get("label") or "").split()) and _in_scrollport(ui, element)
    ]
    if not projects:
        return False
    py = float((projects[0].get("frame") or {}).get("y", 0))
    expands = []
    for element in ui.elements:
        if element.get("role") != "AXButton":
            continue
        if " ".join(str(element.get("label") or "").split()) != "展开显示":
            continue
        y = float((element.get("frame") or {}).get("y", 0))
        if py < y < py + 180 and _in_scrollport(ui, element):
            expands.append(element)
    if not expands:
        return False
    ui.pointer_click(expands[0], dx=24, dy=12)
    time.sleep(0.2)
    return True


def _sidebar_scroll(ui: Ui, lines: int) -> None:
    ui.refresh()
    top, bottom = _visible_band(ui)
    web = next(element for element in ui.elements if element.get("role") == "AXWebArea")
    frame = web.get("frame") or {}
    ui.pointer_scroll(int(float(frame.get("x", 0)) + 140), int((top + bottom) / 2), lines)
    time.sleep(0.3)


def _buttons_named(ui: Ui, title: str) -> list[dict]:
    return [
        element
        for element in ui.elements
        if element.get("role") == "AXButton" and title in " ".join(str(element.get("label") or "").split())
    ]


def _inbox_drop(ui: Ui, project_x: float, top: float, bottom: float, skip: str) -> dict | None:
    for element in _task_buttons(ui):
        label = " ".join(str(element.get("label") or "").split())
        frame = element.get("frame") or {}
        if skip and skip in label:
            continue
        if (
            float(frame.get("x", 9999)) <= project_x + 2
            and _in_band(element, top, bottom)
            and _in_scrollport(ui, element)
        ):
            return element
    for element in ui.elements:
        label = " ".join(str(element.get("label") or "").split())
        frame = element.get("frame") or {}
        if label != "任务" or not _in_band(element, top, bottom) or not _in_scrollport(ui, element):
            continue
        # The rail icon is a 36px control on the activity bar. The inbox heading sits in the sidebar.
        if float(frame.get("w", 9999)) <= 48 and float(frame.get("x", 9999)) < project_x - 20:
            continue
        if float(frame.get("x", 0)) <= project_x + 40:
            return element
    return None


def _expand_for_tasks(ui: Ui) -> None:
    """A collapsed project hides its rows. Open one on screen, or scroll toward the list."""
    ui.refresh()
    top, bottom = _visible_band(ui)
    projects = [
        element
        for element in _project_buttons(ui)
        if _in_band(element, top, bottom) and _in_scrollport(ui, element)
    ]
    collapsed = [element for element in projects if not _project_expanded(ui, element)]
    if collapsed:
        ui.pointer_click(collapsed[-1], dx=40, dy=16)
        time.sleep(0.25)
        return
    _sidebar_scroll(ui, -4)


def _inbox_heading(ui: Ui) -> dict | None:
    """The inbox heading sits in the sidebar. The rail icon named 任务 is 36px wide."""
    found = [
        element
        for element in ui.elements
        if element.get("role") == "AXHeading"
        and " ".join(str(element.get("label") or "").split()) == "任务"
        and float((element.get("frame") or {}).get("w", 0)) > 48
    ]
    return found[0] if found else None


def _project_name(element: dict) -> str:
    return " ".join(str(element.get("label") or "").split())


def _reveal_nested_task(ui: Ui) -> dict | None:
    """Open the project closest to the inbox until one end-to-end row is on screen.

    Archive removes the row nearest the inbox, so the next row can sit many
    empty projects higher. An empty project does not grow. A wide gap means
    the children are present but not in this snapshot, so that project stays
    open. One missed toggle is retried before the project is skipped.
    """
    opened: set[str] = set()
    retried: set[str] = set()
    empty: set[str] = set()
    for _ in range(80):
        ui.refresh()
        top, bottom = _visible_band(ui)
        tasks = [
            element
            for element in _task_buttons(ui)
            if _in_band(element, top, bottom) and _in_scrollport(ui, element)
        ]
        if tasks:
            return max(tasks, key=lambda element: float((element.get("frame") or {}).get("y", 0)))
        # Accessibility keeps rows that have scrolled above the list. The empty
        # projects beside the inbox are not those rows. Scroll to the rows.
        port = _scrollport_top(ui)
        above_port = [
            element
            for element in _task_buttons(ui)
            if float((element.get("frame") or {}).get("y", 0)) < port
        ]
        if above_port:
            _sidebar_scroll(ui, 8)
            continue
        heading = _inbox_heading(ui)
        if heading is None or float((heading.get("frame") or {}).get("y", 0)) > bottom:
            _sidebar_scroll(ui, -6)
            continue
        hy = float((heading.get("frame") or {}).get("y", 0))
        if hy < _scrollport_top(ui):
            _sidebar_scroll(ui, 5)
            continue
        above = [
            element
            for element in _project_buttons(ui)
            if _in_scrollport(ui, element)
            and float((element.get("frame") or {}).get("y", 0)) < hy - 2
            and _project_name(element) not in empty
        ]
        if not above:
            _sidebar_scroll(ui, 5)
            continue
        nearest = max(above, key=lambda element: float((element.get("frame") or {}).get("y", 0)))
        name = _project_name(nearest)
        py = float((nearest.get("frame") or {}).get("y", 0))
        under = [
            element
            for element in _task_buttons(ui)
            if py < float((element.get("frame") or {}).get("y", 0)) < hy
        ]
        if name not in opened:
            opened.add(name)
            if not _project_expanded(ui, nearest):
                ui.pointer_click(nearest, dx=40, dy=16)
                time.sleep(0.25)
            continue
        if under:
            y = float((max(under, key=lambda element: float((element.get("frame") or {}).get("y", 0))).get("frame") or {}).get("y", 0))
            _sidebar_scroll(ui, -4 if y > bottom else 4)
            continue
        following = [
            float((element.get("frame") or {}).get("y", 0))
            for element in _project_buttons(ui)
            if float((element.get("frame") or {}).get("y", 0)) > py + 4
        ]
        gap = min(following) - py if following else hy - py
        if gap > 48:
            _sidebar_scroll(ui, -4)
            continue
        if not _project_expanded(ui, nearest) and name not in retried:
            retried.add(name)
            ui.pointer_click(nearest, dx=40, dy=16)
            time.sleep(0.25)
            continue
        empty.add(name)
        if _project_expanded(ui, nearest):
            ui.pointer_click(nearest, dx=40, dy=16)
            time.sleep(0.2)
    return None


def _prepare_inbox_task(ui: Ui) -> None:
    """Drag a nested row onto the inbox heading when that heading is already beside it.

    A long project list puts the heading below the window. Dragging across that
    distance scrolls the sidebar instead of moving the row.
    """
    ui.refresh()
    top, bottom = _visible_band(ui)
    projects = _project_buttons(ui)
    if not projects:
        return
    project_x = min(float((element.get("frame") or {}).get("x", 0)) for element in projects)
    if any(
        float((element.get("frame") or {}).get("x", 9999)) <= project_x + 2
        and _in_band(element, top, bottom)
        and _in_scrollport(ui, element)
        for element in _task_buttons(ui)
    ):
        return
    heading = _inbox_heading(ui)
    if heading is None or not _in_band(heading, top, bottom) or not _in_scrollport(ui, heading):
        return
    hy = float((heading.get("frame") or {}).get("y", 0))
    nested = [
        element
        for element in _task_buttons(ui)
        if _in_band(element, top, bottom)
        and _in_scrollport(ui, element)
        and hy - 160 < float((element.get("frame") or {}).get("y", 0)) < hy - 4
        and float((element.get("frame") or {}).get("x", 0)) > project_x + 2
    ]
    if not nested:
        return
    ui.pointer_drag(max(nested, key=lambda element: float((element.get("frame") or {}).get("y", 0))), heading)
    time.sleep(0.6)


def w_08(ui: Ui) -> None:
    ui.work()
    _prepare_inbox_task(ui)
    source = None
    target = None
    for _ in range(8):
        ui.refresh()
        top, bottom = _visible_band(ui)
        projects = _project_buttons(ui)
        tasks = _task_buttons(ui)
        if not projects:
            raise AssertionError("no project and task pair to drag")
        if not tasks:
            _expand_for_tasks(ui)
            continue
        project_x = min(float((element.get("frame") or {}).get("x", 0)) for element in projects)
        inbox = [
            element
            for element in tasks
            if float((element.get("frame") or {}).get("x", 9999)) <= project_x + 2
        ]
        visible_projects = [
            element for element in projects if _in_band(element, top, bottom) and _in_scrollport(ui, element)
        ]
        visible_inbox = [
            element for element in inbox if _in_band(element, top, bottom) and _in_scrollport(ui, element)
        ]
        if visible_projects and visible_inbox:
            source = visible_inbox[0]
            # Drop on the project nearest the inbox so the return drag still fits on screen.
            target = max(visible_projects, key=lambda element: float((element.get("frame") or {}).get("y", 0)))
            break
        if not inbox:
            heading = _inbox_heading(ui)
            heading_y = float((heading.get("frame") or {}).get("y", bottom + 1)) if heading else bottom + 1
            _sidebar_scroll(ui, -4 if heading_y > bottom else 4)
            continue
        lowest = max(inbox, key=lambda element: float((element.get("frame") or {}).get("y", 0)))
        lowest_y = float((lowest.get("frame") or {}).get("y", 0))
        # Negative lines reveal content below the fold. The inbox sits under the projects.
        _sidebar_scroll(ui, -3 if lowest_y > bottom else 3)
    if source is None or target is None:
        raise AssertionError("no project and task pair to drag")
    project_name = " ".join(str(target.get("label") or "").split())
    ui.pointer_drag(source, target)
    time.sleep(0.6)
    ui.refresh()
    title = " ".join(str(source.get("label") or "").split())[:48]
    under = _buttons_named(ui, title)
    if not under:
        raise AssertionError("drag did not keep the task visible")
    project_x = float((target.get("frame") or {}).get("x", 0))
    nested_now = [
        element
        for element in under
        if float((element.get("frame") or {}).get("x", 9999)) > project_x + 2 and _in_scrollport(ui, element)
    ]
    if not nested_now:
        nested_now = [
            element for element in under if float((element.get("frame") or {}).get("x", 9999)) > project_x + 2
        ]
    if not nested_now:
        raise AssertionError("task did not leave the inbox for the project")
    task_y = float((nested_now[0].get("frame") or {}).get("y", 0))
    hosts = [
        element
        for element in _project_buttons(ui)
        if float((element.get("frame") or {}).get("y", 0)) < task_y - 2
    ]
    if hosts:
        project_name = " ".join(
            str(max(hosts, key=lambda element: float((element.get("frame") or {}).get("y", 0))).get("label") or "").split()
        )
    back_source = None
    back_target = None
    for _ in range(12):
        ui.refresh()
        top, bottom = _visible_band(ui)
        projects = _project_buttons(ui)
        if projects:
            project_x = min(float((element.get("frame") or {}).get("x", 0)) for element in projects)
        nested = [
            element
            for element in _buttons_named(ui, title)
            if _in_band(element, top, bottom)
            and _in_scrollport(ui, element)
            and float((element.get("frame") or {}).get("x", 0)) > project_x + 2
        ]
        drop = _inbox_drop(ui, project_x, top, bottom, title)
        if nested and drop is not None:
            drop_y = float((drop.get("frame") or {}).get("y", 0))
            back_source = min(
                nested, key=lambda element: abs(float((element.get("frame") or {}).get("y", 0)) - drop_y)
            )
            back_target = drop
            break
        if _reveal_project_tasks(ui, project_name):
            continue
        if _collapse_one_project(ui, project_name):
            continue
        located = [
            element
            for element in _buttons_named(ui, title)
            if float((element.get("frame") or {}).get("x", 0)) > project_x + 2
        ]
        anchor = float((located[0].get("frame") or {}).get("y", top - 1)) if located else top - 1
        _sidebar_scroll(ui, 3 if anchor < _scrollport_top(ui) else -2)
    if back_source is None or back_target is None:
        raise AssertionError("task did not return to the inbox")
    ui.pointer_drag(back_source, back_target)
    time.sleep(0.6)
    ui.refresh()
    projects = _project_buttons(ui)
    if projects:
        project_x = min(float((element.get("frame") or {}).get("x", 0)) for element in projects)
    returned = [
        element
        for element in _buttons_named(ui, title)
        if float((element.get("frame") or {}).get("x", 9999)) <= project_x + 2
    ]
    if not returned:
        raise AssertionError("task did not return to the inbox")


def _task_search_field(ui: Ui) -> dict | None:
    for element in ui.elements:
        if element.get("role") not in ("AXTextField", "AXTextArea", "AXSearchField", "AXComboBox"):
            continue
        label = " ".join(str(element.get("label") or "").split())
        if "搜索任务" in label:
            return element
    return None


def _dismiss_task_search(ui: Ui) -> None:
    """Close the task search overlay. An untargeted Escape does not reach its field."""
    field = _task_search_field(ui)
    if field is None:
        ui.press("escape")
        return
    ui.press_background("escape", field)


def w_09(ui: Ui) -> None:
    ui.work()
    before = ui.rail()
    ui.click("搜索任务", "AXButton")
    time.sleep(0.4)
    ui.fill("zzzz-no-such-task", "搜索")
    time.sleep(0.6)
    ui.refresh()
    # The search overlay replaces the accessibility tree, so the rail is read after Escape.
    if not ui.has("没有匹配的任务"):
        raise AssertionError("search did not show an empty result")
    _dismiss_task_search(ui)
    deadline = time.time() + 6
    while time.time() < deadline:
        time.sleep(0.3)
        ui.refresh()
        if ui.has("没有匹配的任务"):
            _dismiss_task_search(ui)
            continue
        if ui._rail_button("工作") is not None and ui.has("搜索任务"):
            break
    else:
        if ui._rail_button("工作") is None:
            raise AssertionError(ui._work_gap())
        raise AssertionError("escape did not return to the search button")
    if ui.rail() != before:
        raise AssertionError("search changed the rail")
    if not ui.has("搜索任务"):
        raise AssertionError("escape did not return to the search button")


def _e2e_task(ui: Ui) -> dict:
    ui.work()
    for _ in range(14):
        ui.refresh()
        top, bottom = _visible_band(ui)
        rows = [
            element
            for element in _task_buttons(ui)
            if _in_band(element, top, bottom) and _in_scrollport(ui, element)
        ]
        if not rows:
            if _reveal_nested_task(ui) is None:
                raise AssertionError("no end-to-end task row to act on")
            continue
        inbox_x = min(float((element.get("frame") or {}).get("x", 9999)) for element in rows)
        inbox = [
            element
            for element in rows
            if float((element.get("frame") or {}).get("x", 9999)) <= inbox_x + 2
        ]
        pool = inbox or rows
        named = [
            element
            for element in pool
            if str(element.get("label") or "").startswith(("marker ", "e2e-", "Reply with"))
        ]
        # A project row can share the e2e- prefix. Only a row that shows 归档
        # is a task this case may rename or archive.
        actionable = []
        for element in named:
            try:
                _row_action(ui, element, "归档")
            except AssertionError:
                continue
            actionable.append(element)
        if not actionable:
            raise AssertionError("no end-to-end task row to act on")
        return max(actionable, key=lambda element: float((element.get("frame") or {}).get("y", 0)))
    raise AssertionError("no end-to-end task row to act on")


def _task_stamp(title: str) -> str:
    """Stable id shared by the sidebar row and the settings archive row.

    A truncated title keeps only the first digits of a marker. Those six
    digits also occur in a newer marker, so the archive search uses the
    longest full digit run instead.
    """
    text = title.replace("…", "").strip()
    runs: list[str] = []
    current: list[str] = []
    for char in text:
        if char.isdigit():
            current.append(char)
            continue
        if len(current) >= 8:
            runs.append("".join(current))
        current = []
    if len(current) >= 8:
        runs.append("".join(current))
    if runs:
        return max(runs, key=len)
    if "marker " in text:
        digits = []
        for char in text.split("marker ", 1)[1]:
            if char.isdigit():
                digits.append(char)
            else:
                break
        if len(digits) >= 6:
            return "marker " + "".join(digits)
    return text[:24]


def _space_archive_confirm(ui: Ui) -> bool:
    """Space the wide confirm. An accessibility press closes the popover and does not archive."""
    ui.refresh()
    prompt = next(
        (
            element
            for element in ui.elements
            if "归档这个任务" in " ".join(str(element.get("label") or "").split())
        ),
        None,
    )
    if prompt is None:
        return False
    prompt_y = float((prompt.get("frame") or {}).get("y", 0))
    confirms = [
        element
        for element in ui.elements
        if element.get("role") == "AXButton"
        and element.get("label") == "归档"
        and (element.get("frame") or {}).get("w", 0) > 40
        and element.get("element_token")
        and abs(float((element.get("frame") or {}).get("y", 0)) - prompt_y) < 160
    ]
    if not confirms:
        return False
    ui.press_background(
        "space",
        min(confirms, key=lambda element: abs(float((element.get("frame") or {}).get("y", 0)) - prompt_y)),
    )
    return True


def _hover_row(ui: Ui, row: dict) -> None:
    """The row action is pressed by the caller. Resting the pointer would move the cursor."""
    del ui, row


def _row_action(ui: Ui, row: dict, label: str) -> dict:
    frame = row.get("frame") or {}
    action = next(
        (
            element
            for element in ui.elements
            if element.get("role") == "AXButton"
            and element.get("label") == label
            and float((element.get("frame") or {}).get("w", 0)) <= 40
            and abs((element.get("frame") or {}).get("y", 0) - frame.get("y", 0)) < 20
        ),
        None,
    )
    if action is None:
        raise AssertionError(f"{label} is not on the task row")
    return action


def _archive_lists(ui: Ui, stamp: str) -> bool:
    """The system settings item 归档 sits with 关于. A row action is only 24px wide."""
    for _ in range(2):
        ui.open("设置")
        ui.wait_for("关于", 8)
        ui.refresh()
        about = ui.find("关于", "AXButton")
        about_x = float((about.get("frame") or {}).get("x", 0))
        nav = [
            element
            for element in ui.elements
            if element.get("role") == "AXButton"
            and element.get("label") == "归档"
            and abs(float((element.get("frame") or {}).get("x", 0)) - about_x) < 40
            and float((element.get("frame") or {}).get("w", 0)) > 40
        ]
        if not nav:
            return False
        frame = nav[0].get("frame") or {}
        ui.pointer_click(nav[0], dx=float(frame.get("w", 80)) / 2, dy=float(frame.get("h", 32)) / 2)
        time.sleep(0.6)
        ui.refresh()
        if not ui.has("搜索已归档任务"):
            continue
        ui.fill(stamp, "搜索已归档任务")
        time.sleep(0.6)
        ui.refresh()
        if stamp in " ".join(ui.labels()):
            return True
        time.sleep(0.8)
    return False


def _open_archive_confirm(ui: Ui, row: dict) -> dict:
    """Hover until the row's archive icon asks for confirmation.

    The icon is 24px and can sit just past the title frame. The settings item
    named 归档 is wide and is not this control.
    """
    title = " ".join(str(row.get("label") or "").split())[:48]
    origin_y = float((row.get("frame") or {}).get("y", 0))
    for _ in range(2):
        _hover_row(ui, row)
        ui.refresh()
        rows = _buttons_named(ui, title) or [row]
        row = min(rows, key=lambda element: abs(float((element.get("frame") or {}).get("y", 0)) - origin_y))
        origin_y = float((row.get("frame") or {}).get("y", 0))
        try:
            action = _row_action(ui, row, "归档")
        except AssertionError:
            continue
        frame = action.get("frame") or {}
        row_frame = row.get("frame") or {}
        if float(frame.get("w", 0)) > 40:
            continue
        # The 24px icon sits just past the title frame (measured 43px past the
        # title's right edge). The settings item is wide or a column away.
        center = float(frame.get("x", 0)) + float(frame.get("w", 0)) / 2
        if center > float(row_frame.get("x", 0)) + float(row_frame.get("w", 0)) + 80:
            continue
        # A pointer press on this icon starts a row drag. Space on the icon
        # from this snapshot opens the confirm. A later snapshot drops the token.
        ui.press_background("space", action)
        time.sleep(0.45)
        ui.refresh()
        if ui.has("归档这个任务"):
            return row
    small = sorted(
        {
            " ".join(str(element.get("label") or "").split())
            for element in ui.elements
            if element.get("role") == "AXButton" and float((element.get("frame") or {}).get("w", 0)) <= 40
        }
    )
    raise AssertionError("archive did not ask for confirmation" + (f" buttons={small[:8]}" if small else ""))


def w_10(ui: Ui) -> None:
    row = _e2e_task(ui)
    title = " ".join(str(row.get("label") or "").split())
    stamp = _task_stamp(title)
    _open_archive_confirm(ui, row)
    if not _space_archive_confirm(ui):
        raise AssertionError("archive confirm is missing")
    # A row that has scrolled above the list can remain in the accessibility
    # tree after the visible row is gone. Only a row still on screen counts.
    # Match the archived label itself. A short marker prefix also matches a newer task.
    deadline = time.time() + 4
    still = []
    while time.time() < deadline:
        time.sleep(0.4)
        ui.refresh()
        top, bottom = _visible_band(ui)
        still = [
            element
            for element in ui.elements
            if element.get("role") == "AXButton"
            and " ".join(str(element.get("label") or "").split()) == title
            and _in_band(element, top, bottom)
            and _in_scrollport(ui, element)
        ]
        if not still:
            break
        _space_archive_confirm(ui)
    if still:
        raise AssertionError("archive did not remove the task from the work list")
    if not _archive_lists(ui, stamp):
        raise AssertionError("settings archive does not list the task")


def _open_rename(ui: Ui, row: dict) -> dict:
    """Press the row's rename button. A double-click and a pointer hover both move the cursor."""
    title = " ".join(str(row.get("label") or "").split())[:48]
    origin_y = float((row.get("frame") or {}).get("y", 0))
    ui.refresh()
    rows = _buttons_named(ui, title)
    if rows:
        row = min(rows, key=lambda element: abs(float((element.get("frame") or {}).get("y", 0)) - origin_y))
    action = _row_action(ui, row, "重命名")
    ui.pixel_click(action)
    time.sleep(0.35)
    return row


def w_11(ui: Ui) -> None:
    row = _e2e_task(ui)
    original = " ".join(str(row.get("label") or "").split())
    _open_rename(ui, row)
    ui.refresh()
    if not any(
        element.get("role") == "AXTextField" and "重命名" in str(element.get("label") or "")
        for element in ui.elements
    ):
        raise AssertionError("rename field did not open")
    ui.press("escape")
    time.sleep(0.3)
    ui.refresh()
    if original[:18] not in " ".join(ui.labels()):
        raise AssertionError("escape changed the task name")
    row = _e2e_task(ui)
    _open_rename(ui, row)
    time.sleep(0.3)
    renamed = f"e2e-renamed-{int(time.time())}"
    ui.refresh()
    field = next(
        (
            element
            for element in ui.elements
            if element.get("role") == "AXTextField" and "重命名" in str(element.get("label") or "")
        ),
        None,
    )
    if field is None or not field.get("element_token"):
        raise AssertionError("rename field did not receive the name")
    # Addressing this field is refused: WebKit gives it no window id.
    # Focus it, type without a token, then commit with an untargeted Return
    # while that focus is still the input. Addressing Return does not commit.
    ui.type_into(field, renamed)
    ui.press("return")
    deadline = time.time() + 6
    while time.time() < deadline:
        ui.refresh()
        saved = any(
            element.get("role") == "AXButton" and renamed in str(element.get("label") or "")
            for element in ui.elements
        )
        if saved:
            return
        time.sleep(0.3)
    raise AssertionError("enter did not save the task name")


def w_12(ui: Ui) -> None:
    ui.work()
    ui.click("智能体", "AXButton")
    time.sleep(0.6)
    ui.refresh()
    if ui.has("似乎出现了一些问题"):
        raise AssertionError("expert page failed")
    if not (ui.has("描述任务") or ui.has("暂无") or ui.has("智能体")):
        raise AssertionError("expert catalog hid the composer and the empty state")


def w_13(ui: Ui) -> None:
    ui.work()
    ui.click("连接器", "AXButton")
    time.sleep(0.7)
    ui.refresh()
    if ui.has("似乎出现了一些问题"):
        raise AssertionError("connector page failed")
    if not ui.has("连接器"):
        raise AssertionError("connector catalog did not open")


def w_14(ui: Ui) -> None:
    ui.work()
    ui.click("技能", "AXButton")
    time.sleep(0.7)
    ui.refresh()
    if ui.has("似乎出现了一些问题"):
        raise AssertionError("skill page failed")


def w_15(ui: Ui) -> None:
    ui.work()
    ui.click("自动化", "AXButton")
    time.sleep(0.7)
    ui.refresh()
    if ui.has("似乎出现了一些问题"):
        raise AssertionError("automation page failed")
    ui.click("新建任务", "AXButton")
    time.sleep(0.4)
    ui.refresh()
    if not ui.has("描述任务"):
        raise AssertionError("automation still covers the canvas")


def w_16(ui: Ui) -> None:
    ui.work()
    ui.click("自动化", "AXButton")
    time.sleep(0.5)
    ui.refresh()
    if not (ui.has("自动化") or ui.has("定时") or ui.has("新建")):
        raise AssertionError("automation list is not visible")


def _pin_installed_home(ui: Ui) -> None:
    ui.open("插件")
    ui.wait_for("写作", 10)
    for _ in range(6):
        ui.refresh()
        pins = [
            element
            for element in ui.elements
            if element.get("role") == "AXButton" and element.get("label") == "固定到首页"
        ]
        if not pins:
            return
        ui.pixel_click(pins[0], dx=14, dy=14)
        time.sleep(0.35)


def w_17(ui: Ui) -> None:
    _pin_installed_home(ui)
    ui.work()
    ui.click("新建任务", "AXButton")
    time.sleep(0.4)
    ui.refresh()
    if not any(token in "\n".join(ui.labels()) for token in ("编制方案", "创作PPT", "写作", "PDF")):
        raise AssertionError("installed home zones are not on the work page")


def w_18(ui: Ui) -> None:
    ui.work()
    ui.click("新建任务", "AXButton")
    time.sleep(0.3)
    ui.refresh()
    if not ui.has("编制方案"):
        raise AssertionError("plan card is not on the home page")
    ui.click("编制方案")
    time.sleep(0.8)
    ui.refresh()
    if ui.has("似乎出现了一些问题"):
        raise AssertionError("plan editor failed")
    if ui.has("暂未开放") or ui.has("未开放"):
        raise AssertionError("plan editor did not open")
    blob = "\n".join(ui.labels())
    if "预览" not in blob or not any(token in blob for token in ("源码", "源代码", "编辑")):
        raise AssertionError("plan source and preview are not both visible")


def w_19(ui: Ui) -> None:
    ui.work()
    ui.click("新建任务", "AXButton")
    time.sleep(0.3)
    ui.refresh()
    for label in ("创作PPT", "开发应用", "设计界面"):
        if not ui.has(label):
            raise AssertionError(f"missing create card {label}")
        ui.click(label)
        time.sleep(0.4)
        ui.refresh()
        if not any(token in "\n".join(ui.labels()) for token in ("未开放", "暂未", "即将")):
            raise AssertionError(f"{label} did not explain that it is closed")


def w_20(ui: Ui) -> None:
    ui.work()
    ui.click("新建任务", "AXButton")
    time.sleep(0.3)
    ui.refresh()
    if not any(token in "\n".join(ui.labels()) for token in ("PDF", "转换")):
        raise AssertionError("conversion entry is not on the home page")
    raise AssertionError("conversion success and unsupported-file failure were not both shown")


def w_21(ui: Ui) -> None:
    ui.work()
    ui.refresh()
    marker = next((label for label in ui.labels() if "Reply with" in label or "marker" in label), "")
    ui.open("消息")
    ui.work()
    if marker and not ui.has(marker[:12]):
        raise AssertionError("returning to work dropped the open session")


def m_01(ui: Ui) -> None:
    ui.open("消息")
    ui.wait_for("选择一个会话", 10)


def m_02(ui: Ui) -> None:
    _open_tobot(ui)
    if not ui.has("输入消息") and not ui.has("ToBot"):
        raise AssertionError("conversation did not open")


def m_03(ui: Ui) -> None:
    _open_tobot(ui)
    if not ui.has("输入消息"):
        raise AssertionError("message composer is missing")


def m_04(ui: Ui) -> None:
    _open_tobot(ui)


def m_05(ui: Ui) -> None:
    _open_tobot(ui)


def m_06(ui: Ui) -> None:
    _open_tobot(ui)


def m_07(ui: Ui) -> None:
    _open_tobot(ui)


def _colleague_boxes(ui: Ui) -> list[dict]:
    ui.refresh()
    return [
        element
        for element in ui.elements
        if element.get("role") == "AXCheckBox"
        and float((element.get("frame") or {}).get("w", 0)) > 0
        and "ToBot" not in str(element.get("label") or "")
    ]


def _open_group_picker(ui: Ui) -> None:
    _close_group_picker(ui)
    ui.open("消息")
    ui.wait_for("发起会话", 8)
    # The new-conversation control is a popup, not a plain button.
    # Press it once. A second press closes the menu before the entry is read.
    ui.click("发起会话")
    time.sleep(0.4)
    ui.refresh()
    item = next(
        (
            element
            for element in ui.elements
            if element.get("role") in ("AXMenuItem", "AXButton")
            and "发起群聊" in " ".join(str(element.get("label") or "").split())
        ),
        None,
    )
    if item is None:
        raise AssertionError("group entry is missing")
    ui.pixel_click(item, dx=24, dy=10)
    time.sleep(0.5)
    ui.refresh()


def _group_dialog_open(ui: Ui) -> bool:
    return any(
        element.get("role") == "AXHeading" and element.get("label") == "发起群聊" for element in ui.elements
    )


def _close_group_picker(ui: Ui) -> None:
    """Close the group dialog. 取消 and 关闭 both receive AXPress; either one can dismiss it."""
    ui.refresh()
    for label, max_w in (("取消", 400.0), ("关闭", 40.0)):
        if not _group_dialog_open(ui):
            return
        matches = [
            element
            for element in ui.elements
            if element.get("role") == "AXButton"
            and " ".join(str(element.get("label") or "").split()) == label
            and float((element.get("frame") or {}).get("w", 0)) <= max_w
        ]
        if not matches:
            continue
        ui.pixel_click(matches[-1])
        time.sleep(0.35)
        ui.refresh()
    if _group_dialog_open(ui):
        ui.press("escape")
        time.sleep(0.3)


def m_08(ui: Ui) -> None:
    try:
        _open_group_picker(ui)
        if not ui.has("发起群聊"):
            raise AssertionError("group create dialog did not open")
        colleague = [
            " ".join(str(element.get("label") or "").split())
            for element in ui.elements
            if element.get("role") in ("AXButton", "AXCheckBox", "AXStaticText")
            and "ToBot" in str(element.get("label") or "")
        ]
        if colleague:
            raise AssertionError("ToBot is listed as a colleague")
    finally:
        _close_group_picker(ui)


def _open_group_with_colleague(ui: Ui) -> None:
    """Open the group picker on a joined team that lists a colleague.

    The topmost team can be a one-member org. Its picker says 没有匹配的成员.
    Try each other joined team until a checkbox is listed. Do not create a team.
    """
    _open_group_picker(ui)
    deadline = time.time() + 4
    while not _colleague_boxes(ui) and time.time() < deadline:
        time.sleep(0.4)
    if _colleague_boxes(ui):
        return
    if not ui.has("没有匹配的成员"):
        return
    _close_group_picker(ui)
    _open_center_menu(ui)
    if not (ui.has("创建团队") or ui.has("还没有团队") or ui.has("无法加载团队")):
        _click_menu_label(ui, "团队")
        ui.refresh()
    pending = [
        " ".join(str(row.get("label") or "").split())
        for row in _joined_team_rows(ui)
        if str(row.get("value")) != "1"
    ]
    ui.press("escape")
    time.sleep(0.2)
    for label in pending:
        _open_center_menu(ui)
        if not (ui.has("创建团队") or ui.has("还没有团队") or ui.has("无法加载团队")):
            _click_menu_label(ui, "团队")
            ui.refresh()
        clicked = False
        for _ in range(3):
            try:
                row = next(
                    item
                    for item in _joined_team_rows(ui)
                    if " ".join(str(item.get("label") or "").split()) == label and str(item.get("value")) != "1"
                )
            except (AssertionError, StopIteration):
                break
            frame = row.get("frame") or {}
            try:
                ui.pixel_click(row, dx=float(frame.get("w", 40)) / 2, dy=float(frame.get("h", 20)) / 2)
            except RuntimeError:
                time.sleep(0.3)
                _open_center_menu(ui)
                if not (ui.has("创建团队") or ui.has("还没有团队") or ui.has("无法加载团队")):
                    try:
                        _click_menu_label(ui, "团队")
                    except AssertionError:
                        break
                    ui.refresh()
                continue
            clicked = True
            break
        if not clicked:
            ui.press("escape")
            time.sleep(0.2)
            continue
        time.sleep(0.6)
        ui.press("escape")
        time.sleep(0.2)
        try:
            # Switching a team leaves the messages page. Open it again.
            _open_group_picker(ui)
        except (AssertionError, RuntimeError):
            continue
        if ui.has("没有匹配的成员"):
            _close_group_picker(ui)
            continue
        deadline = time.time() + 6
        while not _colleague_boxes(ui) and time.time() < deadline:
            if ui.has("没有匹配的成员"):
                break
            time.sleep(0.4)
        if _colleague_boxes(ui):
            return
        _close_group_picker(ui)


def m_09(ui: Ui) -> None:
    """Check and uncheck one colleague. Do not create the group."""
    try:
        _open_group_with_colleague(ui)
        if not ui.has("发起群聊"):
            raise AssertionError("group entry is missing")
        boxes = _colleague_boxes(ui)
        if not boxes:
            raise AssertionError("member add and remove did not both complete")

        def box_named(name: str) -> dict:
            ui.refresh()
            match = next(
                (
                    element
                    for element in ui.elements
                    if element.get("role") == "AXCheckBox"
                    and " ".join(str(element.get("label") or "").split()) == name
                ),
                None,
            )
            if match is None:
                raise AssertionError("member add and remove did not both complete")
            return match

        toggled = False
        for _round in range(3):
            if toggled:
                break
            boxes = _colleague_boxes(ui)
            for box in boxes:
                label = " ".join(str(box.get("label") or "").split())
                before = str(box.get("value"))
                try:
                    ui.pointer_click(box)
                except RuntimeError:
                    break
                time.sleep(0.35)
                try:
                    checked = box_named(label)
                except AssertionError:
                    break
                if str(checked.get("value")) == before:
                    continue
                try:
                    ui.pointer_click(checked)
                except RuntimeError:
                    break
                time.sleep(0.35)
                restored = box_named(label)
                if str(restored.get("value")) != before:
                    raise AssertionError("member add and remove did not both complete")
                toggled = True
                break
        if not toggled:
            raise AssertionError("member add and remove did not both complete")
    finally:
        _close_group_picker(ui)


def m_10(ui: Ui) -> None:
    _open_tobot(ui)
    stamp = str(int(time.time()))
    prompt = f"Reply with one short word only. marker {stamp}"
    ui.fill(prompt, "输入消息")
    ui.submit_composer()
    deadline = time.time() + 60
    while time.time() < deadline:
        ui.refresh()
        if ui.has(stamp) and (ui.has("ToBot") or any(token in "\n".join(ui.labels()) for token in ("失败", "错误"))):
            print(f"llm endpoint={ui._endpoint_host()} model=a3s/auto visible=reply")
            return
        time.sleep(1)
    raise AssertionError("ToBot produced neither a reply nor an explicit failure")


def m_11(ui: Ui) -> None:
    _open_tobot(ui)
    if not ui.has("ToBot"):
        raise AssertionError("ToBot profile entry is missing")


def m_12(ui: Ui) -> None:
    ui.open("消息")
    if ui.has("似乎出现了一些问题"):
        raise AssertionError("messages empty state is an error boundary")


def _team_nav(ui: Ui, label: str) -> None:
    """Open the team sidebar item once that sidebar has rendered. One missed click is retried."""
    ui.open("团队")
    try:
        ui.wait_for(label, 8)
    except AssertionError:
        ui.open("团队")
        ui.wait_for(label, 8)
    ui.click(label, "AXButton")


def o_01(ui: Ui) -> None:
    _team_nav(ui, "成员")
    time.sleep(0.6)
    ui.refresh()
    if not ui.has("成员"):
        raise AssertionError("member directory did not open")


def _buttons_exact(ui: Ui, label: str, min_w: float = 0) -> list[dict]:
    ui.refresh()
    return [
        element
        for element in ui.elements
        if element.get("role") == "AXButton"
        and " ".join(str(element.get("label") or "").split()) == label
        and (element.get("frame") or {}).get("w", 0) >= min_w
    ]


def _view_buttons(ui: Ui, label: str) -> list[dict]:
    """Exact buttons whose center is on the page.

    A hidden control can share the label and sit later in the tree. Its frame
    is outside the web view, so it is not the dialog button.
    """
    web = next((item for item in ui.elements if item.get("role") == "AXWebArea"), None)
    view = (web.get("frame") if web else None) or {}
    found = []
    for element in _buttons_exact(ui, label):
        if not _shown(element):
            continue
        frame = element.get("frame") or {}
        if view:
            try:
                cx = float(frame["x"]) + float(frame["w"]) / 2
                cy = float(frame["y"]) + float(frame["h"]) / 2
                left = float(view["x"]) - 48
                top = float(view["y"]) - 120
                right = float(view["x"]) + float(view["w"]) + 48
                bottom = float(view["y"]) + float(view["h"]) + 120
            except (KeyError, TypeError, ValueError):
                continue
            if not (left <= cx <= right and top <= cy <= bottom):
                continue
        found.append(element)
    return found


def _activate(ui: Ui, element: dict) -> None:
    """Press a control. Space is the fallback when the selection readback flickers.

    The accessibility press rejects a click that toggles AXSelected. Space still
    runs the control, and it does not move the pointer.
    """
    try:
        ui.pixel_click(element)
    except RuntimeError as exc:
        if "stable AXSelected" not in str(exc):
            raise
        ui.press_background("space", element)


def _click_exact(ui: Ui, label: str, min_w: float = 0, last: bool = False) -> None:
    matches = _buttons_exact(ui, label, min_w)
    if not matches:
        raise AssertionError(f"missing control {label}")
    target = matches[-1] if last else max(matches, key=lambda element: (element.get("frame") or {}).get("w", 0))
    _activate(ui, target)


def _fields(ui: Ui, label: str) -> list[dict]:
    ui.refresh()
    return [
        element
        for element in ui.elements
        if element.get("role") in ("AXTextField", "AXTextArea") and element.get("label") == label
    ]


def _wait_field(ui: Ui, label: str, timeout: float = 8) -> dict:
    deadline = time.time() + timeout
    while time.time() < deadline:
        found = _fields(ui, label)
        if found:
            return found[-1]
        time.sleep(0.15)
    raise AssertionError(f"missing field {label}")


def _type_focused(ui: Ui, text: str) -> None:
    """Type into the field that already has focus. Do not address it again."""
    ui.type_text(text)


def _receive_focused(ui: Ui, label: str, text: str) -> dict:
    """Replace the focused field and require the accessibility value to show it.

    A markdown name keeps its extension outside the selection. Select-all would
    delete that extension, so a new name is typed into the selection directly.
    An empty name deletes the selected stem, then moves to the end and deletes
    the extension. Select-all is not used: it either misses that extension or
    dismisses the untouched draft. Forward delete does not remove the
    extension. Return stays unpressed until the accessibility value is empty.
    A backspace after the value is already empty removes the draft, so the
    clear stops at the first empty read.
    """
    current = _fields(ui, label)
    shown = str(current[-1].get("value") or "") if current else ""
    opened = shown
    # An untouched draft cancels if focus leaves after it has been open a
    # moment. The empty clear has to land before that, so it does not wait.
    if text:
        time.sleep(0.3)
    if text and shown.endswith(".md"):
        _type_focused(ui, text)
    elif text:
        ui.chord(["cmd", "a"])
        _type_focused(ui, text)
    else:
        # The stem is selected and the extension is not. One backspace removes
        # the stem and leaves the caret in front of the extension. Forward
        # delete does not move that caret, so End then backspace removes the
        # extension. Select-all is omitted on purpose. End and the extension
        # backspaces run only after the stem key has changed the value. Until
        # then the draft is untouched, and a run of keys dismisses it.
        found = current
        for _ in range(3):
            ui.press("backspace")
            time.sleep(0.15)
            found = _fields(ui, label)
            if not found:
                shown = ""
                break
            shown = str(found[-1].get("value") or "")
            if shown != opened:
                break
        if found and shown.strip() and shown != opened:
            ui.press("end")
            time.sleep(0.12)
            previous = shown
            for _ in range(max(4, len(shown) + 2)):
                ui.press("backspace")
                time.sleep(0.1)
                found = _fields(ui, label)
                if not found:
                    break
                current_value = str(found[-1].get("value") or "")
                if not current_value.strip() or current_value == previous:
                    break
                previous = current_value
    found = _fields(ui, label)
    after = str(found[-1].get("value") or "") if found else ""
    if not text:
        print(f"name-clear opened={opened!r} before={shown!r} shown={after!r} fields={len(found)}", flush=True)
    if not found or (text and text not in after) or (not text and after.strip()):
        # Escape only cancels the draft when that field is focused.
        if found:
            ui.pixel_click(found[-1])
            time.sleep(0.1)
        ui.press("escape")
        raise AssertionError(
            f"{label} did not receive the text; opened={opened!r} before={shown!r} shown={after!r} fields={len(found)}"
        )
    return found[-1]


def _close_name_draft(ui: Ui) -> None:
    """Cancel a leftover name draft. It only listens while it is focused."""
    found = _fields(ui, "名称")
    if not found:
        return
    ui.pixel_click(found[-1])
    time.sleep(0.15)
    ui.press("escape")
    time.sleep(0.2)


def _set_last(ui: Ui, label: str, text: str) -> dict:
    """The name field is already focused. Addressing it again cancels the draft."""
    _wait_field(ui, label)
    return _receive_focused(ui, label, text)


def _fill_last(ui: Ui, label: str, text: str) -> None:
    field = _wait_field(ui, label)
    ui.chord(["cmd", "a"], field)
    if text:
        ui.type_text(text)
    else:
        ui.press("delete")
    found = _fields(ui, label)
    shown = str(found[-1].get("value") or "") if found else ""
    if not found or (text and text not in shown):
        raise AssertionError(f"{label} did not receive the text")


def _dissolve_listed_team(ui: Ui, name: str) -> None:
    """Dissolve one named team. A create that misses 职位 can leave it unselected.

    Toyou stays because the row must contain this case's name. A failure here
    does not replace the case error.
    """
    try:
        pending = [
            element
            for element in ui.elements
            if element.get("role") == "AXTextField"
            and element.get("label") == "名称"
            and name in str(element.get("value") or "")
        ]
        if pending:
            cancel = _view_buttons(ui, "取消")
            if cancel:
                ui.pixel_click(cancel[-1])
                time.sleep(0.3)
        _select_named_team(ui, name)
        if not _has_created_name(ui, name):
            return
        _dissolve_team(ui, name)
    except Exception:
        return


def _cancel_create_dialog(ui: Ui) -> None:
    """Dismiss 创建团队. Space activates 取消. Do not press 创建."""
    try:
        ui.refresh()
    except Exception:
        return
    open_dialog = any(
        element.get("role") == "AXHeading"
        and " ".join(str(element.get("label") or "").split()) == "创建团队"
        for element in ui.elements
    )
    if not open_dialog:
        return
    cancel = _view_buttons(ui, "取消")
    if cancel:
        try:
            ui.press_background("space", cancel[-1])
        except Exception:
            return
        time.sleep(0.4)
        ui.refresh()
    still_open = any(
        element.get("role") == "AXHeading"
        and " ".join(str(element.get("label") or "").split()) == "创建团队"
        for element in ui.elements
    )
    if still_open:
        try:
            ui.press("escape")
        except Exception:
            return
        time.sleep(0.3)


def _create_heading_open(ui: Ui) -> bool:
    return any(
        element.get("role") == "AXHeading"
        and " ".join(str(element.get("label") or "").split()) == "创建团队"
        for element in ui.elements
    )


def _name_field_holds(ui: Ui, name: str) -> bool:
    return any(
        element.get("role") == "AXTextField"
        and element.get("label") == "名称"
        and name in str(element.get("value") or "")
        for element in ui.elements
    )


def _created_name_forms(name: str) -> tuple[str, ...]:
    """Forms of one created team name that the accessibility tree may expose.

    A heading and a menu row capitalize the first character. The rest of the
    typed name stays as entered. The stored name, and the dissolve confirm,
    keep the original characters.
    """
    folded = " ".join(name.split())
    if not folded:
        return ()
    titled = folded[0].upper() + folded[1:]
    if titled == folded:
        return (folded,)
    return (folded, titled)


def _label_has_created_name(label: str, name: str) -> bool:
    shown = " ".join(label.split())
    return any(form in shown for form in _created_name_forms(name))


def _has_created_name(ui: Ui, name: str) -> bool:
    return any(_label_has_created_name(label, name) for label in ui.labels())


def _quoted_team_name(label: str, fallback: str) -> str:
    """The dissolve dialog asks for the stored name between 「 and 」."""
    prefix = "输入团队名称「"
    start = label.find(prefix)
    if start < 0:
        return fallback
    start += len(prefix)
    end = label.find("」", start)
    if end <= start:
        return fallback
    return label[start:end]


def _team_menu_row(ui: Ui, name: str) -> dict | None:
    anchor = ui._account_control().get("frame") or {}
    rows = [
        element
        for element in ui.elements
        if element.get("role") in ("AXMenuItem", "AXButton", "AXRadioButton")
        and _label_has_created_name(str(element.get("label") or ""), name)
        and _in_account_menu(element, anchor)
        and _shown(element)
    ]
    return next((element for element in rows if element.get("role") == "AXMenuItem"), None) or (
        rows[0] if rows else None
    )


def _select_named_team(ui: Ui, name: str) -> None:
    """Choose the team this case created. A visible 云盘 button can be the previous team.

    The row is a menu item. Escape before that item's action lands closes the
    menu and the center stays on the previous team. A menu row sometimes has no
    window id until the menu is opened again, so a refused press reopens it.
    """
    refused = False
    for _attempt in range(3):
        _open_center_menu(ui)
        ui.refresh()
        row = _team_menu_row(ui, name)
        if row is None:
            ui.press("escape")
            raise AssertionError("created team is not in the account menu")
        try:
            ui.pixel_click(row)
        except RuntimeError as exc:
            if "element_outside_target_window" not in str(exc):
                raise
            refused = True
            ui.press("escape")
            time.sleep(0.4)
            continue
        deadline = time.time() + 8
        while time.time() < deadline:
            ui.refresh()
            if not _center_menu_open(ui) and _has_created_name(ui, name):
                return
            time.sleep(0.3)
        # One accepted click can close the menu without switching the center.
        # Escape only while the menu is still open, then try the row again.
        if _center_menu_open(ui):
            ui.press("escape")
            time.sleep(0.3)
        continue
    if refused:
        raise AssertionError("created team is not in the account menu")
    raise AssertionError("created team did not become the selected center")


def _create_team(ui: Ui) -> str:
    try:
        return _create_team_body(ui)
    except Exception:
        _cancel_create_dialog(ui)
        raise


def _create_team_body(ui: Ui) -> str:
    """Open a team this case owns. The signed-in member is its creator.

    The create dialog closes before the team page shows 职位. Open 团队 after
    the name field is gone, and dissolve the named team if that page never arrives.
    A second on-screen window makes the name keystrokes fail closed, so close
    those windows before the dialog opens.
    """
    _close_extra_windows(ui.pid)
    if _editor_names(ui.pid):
        raise AssertionError("document window blocks the drive keys")
    name = f"e2e-team-{int(time.time())}"
    _open_center_menu(ui)
    _click_menu_label(ui, "创建团队")
    time.sleep(0.5)
    # The create dialog is a normal field. Addressing it does not cancel a draft.
    # Typing without the token delivers nothing when the field is not yet focused.
    field = _wait_field(ui, "名称")
    time.sleep(0.2)
    ui.type_text(name, field)
    found = _fields(ui, "名称")
    shown = str(found[-1].get("value") or "") if found else ""
    if name not in shown:
        field = _wait_field(ui, "名称")
        ui.chord(["cmd", "a"], field)
        ui.type_text(name, field)
        found = _fields(ui, "名称")
        shown = str(found[-1].get("value") or "") if found else ""
    if name not in shown:
        ui.press("escape")
        raise AssertionError("名称 did not receive the text")
    name_fields = [item for item in _fields(ui, "名称") if name in str(item.get("value") or "")]
    if not name_fields:
        raise AssertionError("名称 did not receive the text")
    # Enter in the name field submits the form. Space on 创建 can land in the
    # focused field, change the name, and leave the dialog open.
    ui.press_background("return", name_fields[-1])
    retried_submit = False
    submitted = time.time()
    dialog_deadline = submitted + 12
    while time.time() < dialog_deadline:
        ui.refresh()
        if ui.has("创建失败，请稍后重试") or ui.has("这个名称已被使用") or ui.has("请填写名称"):
            break
        # A cleared field is not a closed dialog. The title stays until submit finishes.
        if not _create_heading_open(ui) and not _name_field_holds(ui, name):
            break
        # Enter can miss a field that was not focused. Space is the second submit.
        # Skip it while 创建 is busy, so a slow create is not sent twice.
        if not retried_submit and time.time() > submitted + 8 and _create_heading_open(ui):
            again = _view_buttons(ui, "创建")
            if any(item.get("busy") or item.get("enabled") is False for item in again):
                time.sleep(0.3)
                continue
            retried_submit = True
            dialog_deadline = time.time() + 8
            held = [item for item in _fields(ui, "名称") if name in str(item.get("value") or "")]
            if again and held:
                field_y = float((held[-1].get("frame") or {}).get("y", 0))
                again_submit = min(
                    again,
                    key=lambda element: abs(float((element.get("frame") or {}).get("y", 0)) - field_y),
                )
                ui.press_background("space", again_submit)
        time.sleep(0.3)
    else:
        _cancel_create_dialog(ui)
        _dissolve_listed_team(ui, name)
        raise AssertionError("create dialog stayed open")
    if _create_heading_open(ui) or _name_field_holds(ui, name):
        _cancel_create_dialog(ui)
        raise AssertionError("create dialog stayed open")
    # choose() already selects the new center. Load that page before a menu
    # click, which can close without switching.
    _show_created_team(ui, name)
    if _created_team_visible(ui, name):
        return name
    try:
        _select_named_team(ui, name)
    except AssertionError:
        pass
    _show_created_team(ui, name)
    if _created_team_visible(ui, name):
        return name
    try:
        _select_named_team(ui, name)
    except AssertionError:
        pass
    _show_created_team(ui, name)
    deadline = time.time() + 12
    while time.time() < deadline:
        ui.refresh()
        if ui.has("创建失败，请稍后重试") or ui.has("这个名称已被使用"):
            break
        if ui.has("职位") and _has_created_name(ui, name):
            return name
        time.sleep(0.4)
    _dissolve_listed_team(ui, name)
    raise AssertionError("created team did not open creator navigation")


def _created_team_visible(ui: Ui, name: str) -> bool:
    ui.refresh()
    return ui.has("职位") and _has_created_name(ui, name)


def _show_created_team(ui: Ui, name: str) -> None:
    """Load the new center's team page.

    The previous center can leave 云盘 on screen. open("团队") treats that
    button as the destination and does not press the rail.
    """
    ui.refresh()
    if ui.has("职位") and _has_created_name(ui, name):
        return
    target = ui._rail_button("团队")
    if target is None:
        ui.open("团队")
        return
    ui.pixel_click(target)
    time.sleep(0.5)


def _wait_exact(ui: Ui, label: str, min_w: float = 0, timeout: float = 8) -> dict:
    """Wait for an exact button. The rail icon shares short labels and is narrower."""
    deadline = time.time() + timeout
    while time.time() < deadline:
        matches = _buttons_exact(ui, label, min_w)
        if matches:
            return max(matches, key=lambda element: (element.get("frame") or {}).get("w", 0))
        time.sleep(0.25)
    raise AssertionError(f"missing control {label}")


def _editor_names(pid: int) -> list[str]:
    """On-screen popout titles. A hidden editor is not eligible for background typing."""
    return [name for _number, name in _pid_windows(pid) if name and name != "ToWork"]


def _close_extra_windows(pid: int) -> None:
    """Close owned popouts. A second on-screen window makes background typing fail closed.

    The web 关闭 button is the control. System Events is only the fallback when
    that button is missing, and a slow query does not hang the case.
    """
    for window, name in list(_pid_windows(pid)):
        if not name or name == "ToWork":
            continue
        _close_preview(pid, window, name)
        if any(number == window for number, _name in _pid_windows(pid)):
            # A cold web window exposes no 关闭 until it is raised.
            _raise_named_window(pid, name)
            time.sleep(0.4)
            if any(number == window for number, _name in _pid_windows(pid)):
                _close_preview(pid, window, name)
        if any(number == window for number, _name in _pid_windows(pid)):
            _press_window_button(pid, name, "关闭")


def _press_window_button(pid: int, window_name: str, button_name: str) -> bool:
    safe_window = window_name.replace("\\", "\\\\").replace('"', '\\"')
    safe_button = button_name.replace("\\", "\\\\").replace('"', '\\"')
    script = f"""
tell application "System Events"
  tell (first process whose unix id is {int(pid)})
    if not (exists window "{safe_window}") then return "absent"
    set nodes to entire contents of window "{safe_window}"
    repeat with i from 1 to (count of nodes)
      set el to item i of nodes
      try
        set r to role of el as string
        set n to name of el as string
      on error
        set r to ""
        set n to ""
      end try
      if r is "AXButton" and n is "{safe_button}" then
        perform action "AXPress" of el
        return "pressed"
      end if
    end repeat
    return "missing"
  end tell
end tell
"""
    try:
        result = subprocess.run(["osascript", "-e", script], capture_output=True, text=True, timeout=8)
    except subprocess.TimeoutExpired:
        return False
    return result.returncode == 0 and result.stdout.strip() == "pressed"


def _dissolve_team(ui: Ui, name: str) -> None:
    """Dissolve only the team this case created, then return to a joined team."""
    _close_extra_windows(ui.pid)
    ui.open("团队")
    _wait_exact(ui, "设置", min_w=80)
    _click_exact(ui, "设置", min_w=80)
    time.sleep(0.6)
    ui.refresh()
    if not _has_created_name(ui, name):
        raise AssertionError("dissolve is not on the team this case created")
    # A new team renders the danger action after the org policy arrives.
    deadline = time.time() + 15
    while time.time() < deadline and not _buttons_exact(ui, "解散团队"):
        time.sleep(0.4)
    if not _buttons_exact(ui, "解散团队"):
        raise AssertionError("missing control 解散团队")
    _click_exact(ui, "解散团队")
    time.sleep(0.4)
    ui.refresh()
    field = next(
        (
            element
            for element in ui.elements
            if element.get("role") == "AXTextField" and "以确认" in str(element.get("label") or "")
        ),
        None,
    )
    if field is None:
        raise AssertionError("dissolve confirm did not ask for the team name")
    label = str(field.get("label") or "")
    # The dialog compares the field with the stored name. That name is the
    # quoted text, which can differ in the first character from the typed name.
    expected = _quoted_team_name(label, name)
    ui.chord(["cmd", "a"], field)
    ui.type_text(expected, field)
    shown = _fields(ui, label)
    shown_value = str(shown[-1].get("value") or "") if shown else ""
    if shown_value != expected:
        raise AssertionError("dissolve confirm did not receive the team name")
    confirms = _view_buttons(ui, "解散团队")
    field_y = float((field.get("frame") or {}).get("y", 0))
    near = [
        element
        for element in confirms
        if abs(float((element.get("frame") or {}).get("y", 0)) - field_y) <= 220
    ]
    if not near:
        raise AssertionError("missing control 解散团队")
    # The settings row uses the same label and clears the typed name. The dialog
    # confirm is the button beside the field. Space runs that confirm.
    confirm = min(
        near,
        key=lambda element: abs(float((element.get("frame") or {}).get("y", 0)) - field_y),
    )
    ui.press_background("space", confirm)
    deadline = time.time() + 10
    while time.time() < deadline:
        ui.refresh()
        if ui.has("团队已解散") or not _has_created_name(ui, name):
            break
        time.sleep(0.4)
    else:
        ui.refresh()
        later = _fields(ui, label)
        later_value = str(later[-1].get("value") or "") if later else ""
        print(
            f"dissolve-stuck expected={expected!r} field={later_value!r} "
            f"toast={ui.has('团队已解散')} failed={ui.has('删除失败')}",
            flush=True,
        )
        raise AssertionError("created team did not dissolve")
    try:
        _select_center(ui, "团队")
    except AssertionError:
        return


def _switch_away_from(ui: Ui, name: str) -> None:
    """Leave the named team for another joined team. The other name is not guessed."""
    _open_center_menu(ui)
    ui.refresh()
    if not _has_created_name(ui, name):
        _click_menu_label(ui, "团队")
        time.sleep(0.4)
        ui.refresh()
    anchor = ui._account_control().get("frame") or {}
    rows = []
    for element in ui.elements:
        if element.get("role") != "AXMenuItem" or not _in_account_menu(element, anchor):
            continue
        label = " ".join(str(element.get("label") or "").split())
        if not label or _label_has_created_name(label, name) or label in _MENU_CHROME:
            continue
        if not any(suffix in label for suffix in _TEAM_ROLE_SUFFIXES):
            continue
        rows.append((label, element))
    if not rows:
        raise AssertionError("team switcher has no other team")
    stable = [
        element
        for label, element in rows
        if not label.startswith("e2e-team-") and not label.startswith("E2e-team-")
    ]
    row = stable[0] if stable else rows[0][1]
    frame = row.get("frame") or {}
    ui.pixel_click(row, dx=float(frame.get("w", 40)) / 2, dy=float(frame.get("h", 20)) / 2)
    time.sleep(0.8)
    ui.press("escape")
    time.sleep(0.3)


def _save_named(ui: Ui, opener: str, text: str) -> None:
    ui.click(opener)
    time.sleep(0.4)
    ui.wait_for(opener, 6)
    _fill_last(ui, "名称", text)
    _click_exact(ui, "保存", last=True)
    ui.wait_for(text, 8)


def o_02(ui: Ui) -> None:
    name = None
    try:
        name = _create_team(ui)
        _team_nav(ui, "职位")
        time.sleep(0.4)
        position = f"e2e-pos-{int(time.time())}"
        _save_named(ui, "新建职位", position)
        _team_nav(ui, "角色")
        time.sleep(0.4)
        role = f"e2e-role-{int(time.time())}"
        _save_named(ui, "新建角色", role)
    finally:
        if name:
            _dissolve_team(ui, name)


def o_03(ui: Ui) -> None:
    ui.open("团队")
    ui.wait_for("成员", 8)
    if not (ui.has("邀请") or ui.has("成员")):
        raise AssertionError("invite entry is not visible to this member")


def _select_member_team(ui: Ui) -> None:
    """Open a joined team whose row role is 成员. Usage stays on the creator."""
    _open_center_menu(ui)
    row = None
    deadline = time.time() + 12
    while row is None and time.time() < deadline:
        ui.refresh()
        if not _center_menu_open(ui):
            _open_center_menu(ui)
            ui.refresh()
        on_team_tab = ui.has("创建团队") or ui.has("还没有团队") or ui.has("无法加载团队")
        if not on_team_tab:
            _click_menu_label(ui, "团队")
            time.sleep(0.45)
            continue
        if ui.has("无法加载团队"):
            _press_team_retry(ui)
            time.sleep(0.8)
            continue
        if ui.has("还没有团队"):
            raise AssertionError("team center has no joined team")
        try:
            joined = _joined_team_rows(ui)
        except AssertionError:
            time.sleep(0.4)
            continue
        members = [
            item
            for item in joined
            if " ".join(str(item.get("label") or "").split()).endswith("成员")
        ]
        if not members:
            raise AssertionError("member team is not in the center menu")
        row = members[0]
    if row is None:
        raise AssertionError("member team is not in the center menu")
    frame = row.get("frame") or {}
    ui.pixel_click(row, dx=float(frame.get("w", 40)) / 2, dy=float(frame.get("h", 20)) / 2)
    time.sleep(0.8)
    ui.press("escape")
    time.sleep(0.3)
    deadline = time.time() + 6
    order = ui.rail()
    while time.time() < deadline:
        if "团队" in order or "组织" in order:
            return
        time.sleep(0.4)
        order = ui.rail()
    raise AssertionError(f"team center did not show the team rail: {order}")


def o_04(ui: Ui) -> None:
    _select_member_team(ui)
    ui.open("团队")
    ui.wait_for("成员", 8)
    member_hides_usage = not any(
        element.get("role") == "AXButton" and element.get("label") == "用量" for element in ui.elements
    )
    if not member_hides_usage:
        raise AssertionError("member team sidebar shows the creator usage entry")
    name = None
    try:
        name = _create_team(ui)
        ui.open("团队")
        ui.wait_for("用量", 8)
        if not _buttons_exact(ui, "用量"):
            raise AssertionError("creator team sidebar hides usage")
        _click_exact(ui, "设置", min_w=80)
        time.sleep(0.4)
        ui.refresh()
        if not _buttons_exact(ui, "用量"):
            raise AssertionError("creator usage entry left the team sidebar")
    finally:
        if name:
            _dissolve_team(ui, name)


def o_05(ui: Ui) -> None:
    name = None
    try:
        name = _create_team(ui)
        _team_nav(ui, "用量")
        time.sleep(0.5)
        ui.refresh()
        for label in ("全团队", "部门", "成员"):
            radios = [
                element
                for element in ui.elements
                if element.get("role") == "AXRadioButton"
                and " ".join(str(element.get("label") or "").split()) == label
            ]
            if not radios:
                raise AssertionError("usage filters did not open")
        department = next(
            element
            for element in ui.elements
            if element.get("role") == "AXRadioButton"
            and " ".join(str(element.get("label") or "").split()) == "部门"
        )
        frame = department.get("frame") or {}
        ui.pixel_click(department, dx=float(frame.get("w", 20)) / 2, dy=float(frame.get("h", 20)) / 2)
        time.sleep(0.4)
        ui.refresh()
        picker = next(
            (
                element
                for element in ui.elements
                if element.get("role") in ("AXPopUpButton", "AXButton", "AXComboBox")
                and "选择部门" in str(element.get("label") or "")
            ),
            None,
        )
        if picker is None:
            raise AssertionError("department usage filter did not open")
        frame = picker.get("frame") or {}
        ui.pixel_click(picker, dx=float(frame.get("w", 20)) / 2, dy=float(frame.get("h", 20)) / 2)
        time.sleep(0.4)
        ui.refresh()
        if not ui.has("搜索部门"):
            raise AssertionError("department usage filter did not open")
    finally:
        if name:
            _dissolve_team(ui, name)


def o_06(ui: Ui) -> None:
    name = None
    try:
        name = _create_team(ui)
        for label in ("消息", "知识"):
            ui.open(label)
            if ui.has("似乎出现了一些问题"):
                raise AssertionError(f"{label} failed on the created team")
        ui.open("团队")
        extra = _open_mail(ui)
        try:
            surface = extra or ui
            if surface.has("似乎出现了一些问题"):
                raise AssertionError("邮件 failed on the created team")
        finally:
            _close_mail(ui, extra)
        ui.open("团队")
        ui.click("日程", "AXButton")
        time.sleep(0.5)
        ui.refresh()
        if ui.has("似乎出现了一些问题"):
            raise AssertionError("日程 failed on the created team")
        _switch_away_from(ui, name)
        ui.refresh()
        if _has_created_name(ui, name):
            raise AssertionError("team switch stayed on the created team")
        for label in ("消息", "知识"):
            ui.open(label)
            if ui.has("似乎出现了一些问题"):
                raise AssertionError(f"{label} failed after the team switch")
        ui.open("团队")
        extra = _open_mail(ui)
        try:
            surface = extra or ui
            if surface.has("似乎出现了一些问题"):
                raise AssertionError("邮件 failed after the team switch")
        finally:
            _close_mail(ui, extra)
        ui.open("团队")
        ui.click("日程", "AXButton")
        time.sleep(0.5)
        ui.refresh()
        if ui.has("似乎出现了一些问题"):
            raise AssertionError("日程 failed after the team switch")
    finally:
        if name and _has_created_name(ui, name):
            _dissolve_team(ui, name)
        else:
            try:
                _select_center(ui, "团队")
            except Exception:
                return


def _open_marker(ui: Ui, label: str, marker: str, timeout: float = 8) -> None:
    """Open a rail app and wait. One missed click is retried."""
    ui.open(label)
    try:
        ui.wait_for(marker, timeout)
    except AssertionError:
        ui.open(label)
        ui.wait_for(marker, timeout)


def _shown(element: dict) -> bool:
    frame = element.get("frame") or {}
    return float(frame.get("w", 0)) > 1 and float(frame.get("h", 0)) > 1


def _page_heading(ui: Ui, text: str) -> bool:
    """A page title such as 知识 is a short heading in the content column.

    The activity bar repeats those words as buttons. A heading counts when it
    sits to the right of that bar, even when the text itself is narrower than
    the bar.
    """
    ui.refresh()
    web = next((item for item in ui.elements if item.get("role") == "AXWebArea"), None)
    origin = float((web.get("frame") or {}).get("x", 0)) if web else 0
    return any(
        element.get("role") in ("AXHeading", "AXStaticText")
        and _shown(element)
        and " ".join(str(element.get("label") or "").split()) == text
        and float((element.get("frame") or {}).get("x", 0)) > origin + 72
        for element in ui.elements
    )


def _open_knowledge(ui: Ui) -> None:
    """Open the personal vault. Retry one rail press that did not land.

    A second press on the vault that is already showing remounts the error
    boundary, so the retry runs only when that page is still closed.
    """
    ui.open("知识")
    try:
        ui.wait_for("搜索笔记", 10)
    except AssertionError:
        if _page_heading(ui, "知识") or ui.has("编写本地工作区里的笔记"):
            raise
        ui.open("知识")
        ui.wait_for("搜索笔记", 10)


def _show_note(ui: Ui, stem: str) -> None:
    """Return to the vault and require the saved note. One missed press is retried."""
    ui.open("知识")
    try:
        ui.wait_for(stem, 10)
    except AssertionError:
        if _page_heading(ui, "知识") or _fields(ui, "搜索笔记"):
            raise
        ui.open("知识")
        ui.wait_for(stem, 10)


def _knowledge(ui: Ui) -> None:
    _open_knowledge(ui)
    ui.refresh()
    if not ui.has("编写本地工作区里的笔记"):
        raise AssertionError("knowledge page is not the personal vault")
    if any(_shown(element) and str(element.get("label") or "") in ("本地", "云端") for element in ui.elements):
        raise AssertionError("local/cloud switch is visible")


def _click_named(ui: Ui, label: str) -> None:
    """Press the exact control. The drive chooser is a popup button, not a plain button."""
    ui.refresh()
    matches = [
        element
        for element in ui.elements
        if element.get("role") in ("AXButton", "AXPopUpButton") and element.get("label") == label
    ]
    if not matches:
        raise AssertionError(f"missing control {label}")
    ui.pixel_click(max(matches, key=lambda element: float((element.get("frame") or {}).get("w", 0))))


def _exact(ui: Ui, text: str) -> bool:
    ui.refresh()
    return any(
        _shown(element) and " ".join(str(element.get("label") or "").split()) == text
        for element in ui.elements
    )


def _body_matches(ui: Ui) -> bool:
    """The search group 正文. The paragraph style control draws the same word."""
    ui.refresh()
    ribbon = next(
        (
            element.get("frame") or {}
            for element in ui.elements
            if element.get("role") == "AXComboBox" and element.get("label") == "段落样式"
        ),
        None,
    )

    def inside_ribbon(frame: dict) -> bool:
        if not ribbon:
            return False
        x = float(frame.get("x", 0))
        y = float(frame.get("y", 0))
        width = float(frame.get("w", 0))
        height = float(frame.get("h", 0))
        return (
            x >= float(ribbon.get("x", 0)) - 2
            and y >= float(ribbon.get("y", 0)) - 2
            and x + width <= float(ribbon.get("x", 0)) + float(ribbon.get("w", 0)) + 2
            and y + height <= float(ribbon.get("y", 0)) + float(ribbon.get("h", 0)) + 2
        )

    for element in ui.elements:
        if not _shown(element):
            continue
        if " ".join(str(element.get("label") or "").split()) != "正文":
            continue
        if element.get("role") == "AXComboBox" or inside_ribbon(element.get("frame") or {}):
            continue
        return True
    return False


def _note_row(ui: Ui, stem: str) -> dict | None:
    ui.refresh()
    return next(
        (
            element
            for element in ui.elements
            if element.get("role") == "AXButton" and element.get("label") == stem
        ),
        None,
    )


def _create_knowledge_note(ui: Ui, title: str) -> str:
    _knowledge(ui)
    stem = title[:-3] if title.endswith(".md") else title
    last: Exception | None = None
    for _attempt in range(2):
        _click_named(ui, "新建文件")
        time.sleep(0.35)
        try:
            field = _set_last(ui, "名称", stem)
        except AssertionError as exc:
            last = exc
            time.sleep(0.2)
            continue
        ui.press_background("return", field)
        deadline = time.time() + 8
        while time.time() < deadline:
            if _note_row(ui, stem) is not None:
                return stem
            time.sleep(0.3)
        ui.press("escape")
        last = AssertionError(f"note {stem} was not saved")
    raise last or AssertionError(f"note {stem} was not saved")


def _open_note(ui: Ui, stem: str) -> dict:
    row = _note_row(ui, stem)
    if row is None:
        raise AssertionError(f"note {stem} is not in the tree")
    ui.pixel_click(row)
    try:
        return _wait_field(ui, "Markdown 编辑区")
    except AssertionError:
        # One row press can leave the tree selected and the editor closed.
        row = _note_row(ui, stem)
        if row is None:
            raise
        ui.pixel_click(row)
        return _wait_field(ui, "Markdown 编辑区")


def _save_note_body(ui: Ui, stem: str, body: str) -> None:
    editor = _open_note(ui, stem)
    ui.type_text(body, editor)
    editor = _wait_field(ui, "Markdown 编辑区")
    ui.chord(["cmd", "s"], editor)
    time.sleep(0.7)


def _launchpad(ui: Ui, label: str) -> None:
    before = ui.rail()
    ui.refresh()
    opener = next(
        (
            element
            for element in ui.elements
            if element.get("role") == "AXButton" and element.get("label") == "启动台"
        ),
        None,
    )
    if opener is None:
        raise AssertionError("launchpad button is missing")
    ui.pixel_click(opener)
    time.sleep(0.6)
    ui.wait_for(label, 8)
    _click_named(ui, label)
    time.sleep(0.8)
    ui.refresh()
    if ui.rail() != before and label in ui.rail() and label not in before:
        raise AssertionError(f"launchpad added {label} to the rail")


def k_01(ui: Ui) -> None:
    _knowledge(ui)
    if not _page_heading(ui, "知识"):
        raise AssertionError("knowledge heading is missing")
    if not ui.has("笔记"):
        raise AssertionError("personal vault tree is missing")
    if ui.has("似乎出现了一些问题"):
        raise AssertionError("knowledge page failed")


def k_02(ui: Ui) -> None:
    stem = _create_knowledge_note(ui, f"e2e-k2-{int(time.time())}.md")
    _team_nav(ui, "云盘")
    time.sleep(0.7)
    ui.refresh()
    if ui.has("似乎出现了一些问题"):
        raise AssertionError("team drive failed")
    if not _page_heading(ui, "云盘"):
        raise AssertionError("team drive did not replace the knowledge page")
    if _page_heading(ui, "知识"):
        raise AssertionError("team drive is still the personal knowledge page")
    _show_note(ui, stem)
    if not ui.has("搜索笔记"):
        raise AssertionError("personal knowledge did not return")


def _search_notes(ui: Ui, text: str) -> None:
    """Replace the note search. Select-all from a token does not clear this field."""
    field = _wait_field(ui, "搜索笔记")
    ui.pixel_click(field)
    time.sleep(0.15)
    ui.chord(["cmd", "a"])
    if text:
        _type_focused(ui, text)
    else:
        ui.press("backspace")
    time.sleep(0.15)

    def shown_value() -> str:
        found = _fields(ui, "搜索笔记")
        return " ".join(str(found[-1].get("value") or "").split()) if found else ""

    shown = shown_value()
    if shown == text or (not text and shown in ("", "搜索笔记")):
        return
    ui.press("end")
    previous = shown
    for _ in range(max(64, len(shown) + 8)):
        ui.press("backspace")
        current = shown_value()
        if current in ("", "搜索笔记") or current == previous:
            break
        previous = current
    if text:
        _type_focused(ui, text)
    shown = shown_value()
    if text and shown != text or not text and shown not in ("", "搜索笔记"):
        raise AssertionError("搜索笔记 did not receive the text")


def k_03(ui: Ui) -> None:
    token = f"e2ebody{int(time.time())}"
    stem = _create_knowledge_note(ui, f"e2e-k3-{int(time.time())}.md")
    _save_note_body(ui, stem, token)
    _knowledge(ui)
    _search_notes(ui, stem)
    deadline = time.time() + 6
    while time.time() < deadline and _note_row(ui, stem) is None:
        time.sleep(0.3)
    if _note_row(ui, stem) is None:
        raise AssertionError("filename search dropped the note")
    if _body_matches(ui):
        raise AssertionError("filename search listed the note under 正文")
    _search_notes(ui, token)
    deadline = time.time() + 6
    while time.time() < deadline and not (_body_matches(ui) and _note_row(ui, stem) is not None):
        time.sleep(0.3)
    if not _body_matches(ui) or _note_row(ui, stem) is None:
        _search_notes(ui, "")
        raise AssertionError("body-only search did not list 正文")
    _search_notes(ui, "")


def k_04(ui: Ui) -> None:
    _knowledge(ui)
    _close_name_draft(ui)
    for reserved, typed, message in (
        ("index.md", "index", "不能使用这个保留名称"),
        ("log.md", "log", "不能使用这个保留名称"),
    ):
        _click_named(ui, "新建文件")
        field = _set_last(ui, "名称", typed)
        ui.press_background("return", field)
        time.sleep(0.4)
        ui.refresh()
        if not ui.has(message):
            ui.press("escape")
            raise AssertionError(f"{reserved} was not refused")
        ui.press("escape")
        time.sleep(0.2)
    refused = False
    empty_detail = "名称 field did not open"
    for _attempt in range(2):
        _click_named(ui, "新建文件")
        try:
            _receive_focused(ui, "名称", "")
        except AssertionError as exc:
            empty_detail = str(exc)
            time.sleep(0.2)
            continue
        # Addressing the field again blurs an untouched draft and cancels it.
        ui.press("return")
        time.sleep(0.4)
        ui.refresh()
        if ui.has("名称不能为空"):
            refused = True
            break
        empty_detail = "return on an empty name did not show 名称不能为空"
        ui.press("escape")
        time.sleep(0.2)
    if not refused:
        ui.press("escape")
        raise AssertionError(f"an empty name was not refused; {empty_detail}")
    ui.press("escape")
    time.sleep(0.2)
    folder = f"e2e-folder-{int(time.time())}"
    _click_named(ui, "新建文件夹")
    field = _set_last(ui, "名称", folder)
    ui.press_background("return", field)
    ui.wait_for(folder, 8)
    row = _note_row(ui, folder)
    if row is None:
        raise AssertionError("folder row is missing")
    ui.pixel_click(row)
    ui.press_background("f10", row, ["shift"])
    time.sleep(0.4)
    ui.refresh()
    menu_items = [
        element
        for element in ui.elements
        if element.get("role") in ("AXButton", "AXMenuItem")
        and element.get("label") == "删除"
        and _shown(element)
    ]
    if not menu_items:
        ui.press("escape")
        raise AssertionError("folder menu did not open")
    ui.pixel_click(menu_items[0])
    time.sleep(0.3)
    ui.refresh()
    if not ui.has("会移到废纸篓"):
        ui.press("escape")
        raise AssertionError("delete did not say the item moves to the trash")
    confirms = [
        element
        for element in ui.elements
        if element.get("role") == "AXButton" and element.get("label") == "删除" and _shown(element)
    ]
    if not confirms:
        ui.press("escape")
        raise AssertionError("delete confirm is missing")
    ui.pixel_click(confirms[-1])
    time.sleep(0.6)
    ui.refresh()
    if _note_row(ui, folder) is not None:
        raise AssertionError("confirmed delete left the folder in the tree")


def k_05(ui: Ui) -> None:
    token = f"e2ekept{int(time.time())}"
    stem = _create_knowledge_note(ui, f"e2e-k5-{int(time.time())}.md")
    _save_note_body(ui, stem, token)
    ui.open("工作")
    # One rail press back to the vault can miss. Retry only while that page is closed.
    _show_note(ui, stem)
    _open_note(ui, stem)
    ui.refresh()
    if not ui.has(token):
        raise AssertionError("saved body did not stay after leaving the page")
    relaunch(ui, "知识")
    ui.wait_for(stem, 12)
    if not ui.has("搜索笔记"):
        raise AssertionError("restart did not restore the personal vault")
    _open_note(ui, stem)
    ui.refresh()
    if not ui.has(token):
        raise AssertionError("saved body did not stay after restart")


def _reading(ui: Ui) -> set[str]:
    return {label for label in ("链接", "反向链接", "标签", "待办") if _exact(ui, label)}


def k_06(ui: Ui) -> None:
    stamp = int(time.time())
    target = _create_knowledge_note(ui, f"e2e-k6a-{stamp}.md")
    source = _create_knowledge_note(ui, f"e2e-k6b-{stamp}.md")
    _open_note(ui, source)
    if _reading(ui):
        raise AssertionError("an unsaved note already showed reading headings")
    editor = _wait_field(ui, "Markdown 编辑区")
    ui.type_text(f"[[{target}]]\n#e2etag\n- [ ] e2etodo\n", editor)
    time.sleep(0.4)
    if _reading(ui):
        raise AssertionError("unsaved text changed the reading rail")
    editor = _wait_field(ui, "Markdown 编辑区")
    ui.chord(["cmd", "s"], editor)
    time.sleep(0.7)
    _save_note_body(ui, target, f"[[{source}]]\n")
    ui.open("工作")
    ui.open("知识")
    ui.wait_for(source, 10)
    _open_note(ui, source)
    deadline = time.time() + 8
    missing = {"链接", "反向链接", "标签", "待办"}
    while time.time() < deadline:
        missing = {"链接", "反向链接", "标签", "待办"} - _reading(ui)
        if not missing:
            break
        time.sleep(0.4)
    if missing:
        raise AssertionError("saved note did not show " + ", ".join(sorted(missing)))


def k_07(ui: Ui) -> None:
    _knowledge(ui)
    ui.refresh()
    button = next(
        (
            element
            for element in ui.elements
            if element.get("role") == "AXButton" and element.get("label") == "知识编译"
        ),
        None,
    )
    if button is None:
        raise AssertionError("compile action is missing")
    if str(button.get("value") or "") in ("1", "true") or button.get("busy"):
        raise AssertionError("compile is already running")
    print("compile not pressed")


def _drive_row(ui: Ui, stem: str) -> dict | None:
    ui.refresh()
    matches = [
        element
        for element in ui.elements
        if element.get("role") == "AXButton" and stem in " ".join(str(element.get("label") or "").split())
    ]
    return matches[-1] if matches else None


def _drive_file(ui: Ui, stem: str) -> None:
    _team_nav(ui, "云盘")
    ui.wait_for("新建文件", 12)
    _click_named(ui, "新建文件")
    ui.wait_for("新建文档", 8)
    _click_named(ui, "文档")
    # The name step focuses itself after paint. A key without the field token
    # lands before that focus and leaves the default stem in place.
    field = _wait_field(ui, "文件名")
    time.sleep(0.2)
    ui.chord(["cmd", "a"], field)
    ui.type_text(stem, field)
    named = _fields(ui, "文件名")
    if not named or stem not in str(named[-1].get("value") or ""):
        ui.press("escape")
        raise AssertionError("文件名 did not receive the text")
    # A hidden 创建 can sit later in the tree than the dialog's submit button.
    confirms = _view_buttons(ui, "创建")
    if not confirms:
        raise AssertionError("missing control 创建")
    ui.pixel_click(confirms[-1])
    deadline = time.time() + 12
    while time.time() < deadline:
        if _drive_row(ui, stem) is not None:
            # The new document opens its own window. Background typing then
            # refuses to guess which window receives the keys.
            _close_extra_windows(ui.pid)
            return
        time.sleep(0.3)
    ui.refresh()
    dialog = "create dialog still open" if ui.has("新建文档") else "create dialog closed"
    fields = _fields(ui, "文件名")
    shown = str(fields[-1].get("value") or "") if fields else ""
    alert = next(
        (
            " ".join(str(element.get("label") or "").split())
            for element in ui.elements
            if any(
                phrase in str(element.get("label") or "")
                for phrase in ("创建没有完成", "名称不能为空", "已存在同名", "名称不能")
            )
        ),
        "",
    )
    detail = dialog
    if shown:
        detail += f"; 文件名={shown}"
    if alert:
        detail += f"; {alert}"
    raise AssertionError(f"drive file {stem} was not created; {detail}")


def f_01(ui: Ui) -> None:
    _team_nav(ui, "云盘")
    time.sleep(0.7)
    ui.refresh()
    if ui.has("似乎出现了一些问题"):
        raise AssertionError("drive failed")
    if not _page_heading(ui, "云盘"):
        raise AssertionError("team drive heading is missing")
    ui.open("知识")
    ui.wait_for("搜索笔记", 8)


def _press_labeled(ui: Ui, label: str) -> None:
    """Press one visible button or checkbox. A drag is not a substitute."""
    ui.refresh()
    matches = [
        element
        for element in ui.elements
        if element.get("role") in ("AXButton", "AXCheckBox", "AXPopUpButton")
        and element.get("label") == label
        and _shown(element)
    ]
    if not matches:
        raise AssertionError(f"missing control {label}")
    ui.pixel_click(max(matches, key=lambda element: float((element.get("frame") or {}).get("w", 0))))


def _upload_drive_text(ui: Ui, name: str, body: str) -> None:
    """Upload through the path-free form. The drop target stays unused."""
    _team_nav(ui, "云盘")
    ui.wait_for("上传", 12)
    _press_labeled(ui, "上传")
    field = _wait_field(ui, "上传文件名")
    ui.type_text(name, field)
    typed = _fields(ui, "上传文件名")
    if not typed or name not in str(typed[-1].get("value") or ""):
        raise AssertionError("上传文件名 did not receive the text")
    content = _wait_field(ui, "上传内容")
    ui.type_text(body, content)
    _press_labeled(ui, "开始上传")
    deadline = time.time() + 12
    while time.time() < deadline:
        if _drive_row(ui, name) is not None:
            return
        time.sleep(0.3)
    raise AssertionError(f"uploaded file {name} is not listed")


def f_02(ui: Ui) -> None:
    name = _create_team(ui)
    case_error: BaseException | None = None
    try:
        stamp = int(time.time())
        uploaded = f"e2e-up-{stamp}.txt"
        _upload_drive_text(ui, uploaded, f"e2e-upload-{stamp}")
        if not ui.has("已上传 1 个文件"):
            raise AssertionError("upload did not report the file")
        if _drive_row(ui, uploaded) is None:
            raise AssertionError("uploaded file is not listed")
    except Exception as exc:
        case_error = exc
    try:
        _dissolve_team(ui, name)
    except Exception as exc:
        if case_error is None:
            raise
        raise AssertionError(f"{case_error}; dissolve: {exc}") from exc
    if case_error is not None:
        raise case_error


def _rename_accepted(shown: str, renamed: str) -> bool:
    """The rename field keeps the extension and replaces the selected stem."""
    if shown == renamed:
        return True
    base, dot, _ext = shown.rpartition(".")
    return bool(dot) and base == renamed


def _confirm_drive_delete(ui: Ui, label: str) -> None:
    deadline = time.time() + 4
    while time.time() < deadline:
        ui.refresh()
        if ui.has("会移到废纸篓"):
            break
        time.sleep(0.25)
    else:
        raise AssertionError("delete did not say the item moves to the trash")
    confirms = [
        element
        for element in ui.elements
        if element.get("role") == "AXButton" and element.get("label") == "删除" and _shown(element)
    ]
    if not confirms:
        raise AssertionError("delete confirm is missing")
    ui.pixel_click(confirms[-1])
    time.sleep(0.6)
    ui.refresh()
    if _drive_row(ui, label) is not None:
        raise AssertionError("confirmed delete left the file in the drive")


def f_03(ui: Ui) -> None:
    name = _create_team(ui)
    stem = f"e2e-ren-{int(time.time())}"
    renamed = f"{stem}b"
    case_error: BaseException | None = None
    try:
        _drive_file(ui, stem)
        if _drive_row(ui, stem) is None:
            raise AssertionError("created drive file is not listed")
        # The file is in the list. F2 on the folder-tree root clears the selection.
        _close_extra_windows(ui.pid)
        if _editor_names(ui.pid):
            raise AssertionError("document window blocks the drive keys")
        row = _drive_row(ui, stem)
        if row is None:
            raise AssertionError("created drive file is not listed")
        ui.press_background("f2", row)
        field = None
        deadline = time.time() + 8
        while time.time() < deadline:
            current = _fields(ui, "重命名")
            if current and stem in str(current[-1].get("value") or ""):
                field = current[-1]
                break
            time.sleep(0.15)
        if field is None:
            raise AssertionError("rename did not open the created file")
        # F2 already selected the stem. Addressing the field again blurs an
        # untouched rename and cancels it, so the new name goes to the focused field.
        ui.type_text(renamed)
        found = _fields(ui, "重命名")
        shown = str(found[-1].get("value") or "") if found else ""
        if not _rename_accepted(shown, renamed):
            if found:
                ui.press_background("escape")
            raise AssertionError("重命名 did not receive the text")
        ui.press_background("return")
        deadline = time.time() + 8
        while time.time() < deadline:
            if _drive_row(ui, renamed) is not None:
                break
            time.sleep(0.3)
        else:
            ui.press("escape")
            raise AssertionError("renamed drive file is not listed")
        if _drive_row(ui, renamed) is None:
            raise AssertionError("renamed drive file is not listed")
        _close_extra_windows(ui.pid)
        if _editor_names(ui.pid):
            raise AssertionError("document window blocks the drive keys")
        row = _drive_row(ui, renamed)
        if row is None:
            raise AssertionError("renamed drive file is not listed")
        ui.press_background("delete", row)
        _confirm_drive_delete(ui, renamed)
    except Exception as exc:
        case_error = exc
    try:
        _dissolve_team(ui, name)
    except Exception as exc:
        if case_error is None:
            raise
        raise AssertionError(f"{case_error}; dissolve: {exc}") from exc
    if case_error is not None:
        raise case_error


def f_04(ui: Ui) -> None:
    name = _create_team(ui)
    case_error: BaseException | None = None
    try:
        stamp = int(time.time())
        first = f"e2e-sel-a-{stamp}.txt"
        second = f"e2e-sel-b-{stamp}.txt"
        _upload_drive_text(ui, first, "a")
        _upload_drive_text(ui, second, "b")
        _press_labeled(ui, f"选择 {first}")
        _press_labeled(ui, f"选择 {second}")
        ui.wait_for("2 个已选", 8)
    except Exception as exc:
        case_error = exc
    try:
        _dissolve_team(ui, name)
    except Exception as exc:
        if case_error is None:
            raise
        raise AssertionError(f"{case_error}; dissolve: {exc}") from exc
    if case_error is not None:
        raise case_error


def f_05(ui: Ui) -> None:
    name = _create_team(ui)
    case_error: BaseException | None = None
    try:
        stamp = int(time.time())
        handed = f"e2e-hand-{stamp}.txt"
        _upload_drive_text(ui, handed, "handoff")
        _press_labeled(ui, f"选择 {handed}")
        _press_labeled(ui, "交给任务")
        ui.wait_for("已交给任务", 8)
        if not ui.has(handed):
            raise AssertionError("task handoff did not name the file")
    except Exception as exc:
        case_error = exc
    try:
        _dissolve_team(ui, name)
    except Exception as exc:
        if case_error is None:
            raise
        raise AssertionError(f"{case_error}; dissolve: {exc}") from exc
    if case_error is not None:
        raise case_error


def n_01(ui: Ui) -> None:
    _launchpad(ui, "笔记")
    ui.refresh()
    if ui.has("似乎出现了一些问题"):
        raise AssertionError("notes page failed")
    if not (ui.has("新建笔记") or ui.has("暂无笔记")):
        raise AssertionError("launchpad did not open the notes app")


def c_01(ui: Ui) -> None:
    _launchpad(ui, "编码搭档")
    ui.refresh()
    if ui.has("似乎出现了一些问题"):
        raise AssertionError("code tool error boundary")
    if not any(ui.has(label) for label in ("暂无工具", "Claude Code", "Codex", "Gemini", "编码搭档")):
        raise AssertionError("code tool did not open from the launchpad")


def s_01(ui: Ui) -> None:
    ui.open("团队")
    ui.click("日程", "AXButton")
    time.sleep(0.6)
    ui.refresh()
    if ui.has("似乎出现了一些问题"):
        raise AssertionError("calendar error boundary")
    if not ui.has("日程"):
        raise AssertionError("calendar did not open")


def s_02(ui: Ui) -> None:
    if "日程" in ui.rail():
        raise AssertionError("calendar is on the icon rail")


def _inbox_button(ui: Ui) -> dict:
    """The task bell sits with 设置. A rail item can share the word 任务."""
    ui.refresh()
    settings = [
        element
        for element in ui.elements
        if element.get("role") == "AXButton"
        and " ".join(str(element.get("label") or "").split()) == "设置"
        and float((element.get("frame") or {}).get("w", 9999)) <= 64
    ]
    tasks = [
        element
        for element in ui.elements
        if element.get("role") == "AXButton"
        and " ".join(str(element.get("label") or "").split()) == "任务"
        and float((element.get("frame") or {}).get("w", 9999)) <= 64
    ]
    if not tasks:
        raise AssertionError("missing AXButton 任务")
    if settings:
        anchor_y = float((max(settings, key=lambda element: float((element.get("frame") or {}).get("y", 0))).get("frame") or {}).get("y", 0))
        near = [
            element
            for element in tasks
            if abs(float((element.get("frame") or {}).get("y", 0)) - anchor_y) < 80
        ]
        if near:
            return min(near, key=lambda element: abs(float((element.get("frame") or {}).get("y", 0)) - anchor_y))
    return max(tasks, key=lambda element: float((element.get("frame") or {}).get("y", 0)))


def _mail_tab_label(label: str) -> bool:
    """The mail lane is labeled 邮件. An unread count may follow it."""
    text = " ".join(label.split())
    if text == "邮件":
        return True
    if not text.startswith("邮件"):
        return False
    rest = text[2:].strip()
    return bool(rest) and rest.isdigit()


def _mail_tabs(ui: Ui) -> list[dict]:
    return [
        element
        for element in ui.elements
        if element.get("role") in ("AXButton", "AXRadioButton", "AXTab", "AXCheckBox", "AXPopUpButton")
        and _mail_tab_label(str(element.get("label") or ""))
    ]


def _select_mail_tab(ui: Ui) -> None:
    deadline = time.time() + 6
    while time.time() < deadline:
        ui.refresh()
        tabs = _mail_tabs(ui)
        if tabs:
            _activate(ui, tabs[0])
            time.sleep(0.5)
            ui.refresh()
            return
        time.sleep(0.3)
    seen = sorted(
        {
            f"{element.get('role')}:{str(element.get('label') or '')[:40]}"
            for element in ui.elements
            if "邮件" in " ".join(str(element.get("label") or "").split())
        }
    )
    raise AssertionError("missing AXButton 邮件" + (f" saw {seen}" if seen else ""))


def _close_mail(ui: Ui, extra: Ui | None) -> None:
    """Close the inbox window. A popup in the work window closes with Escape."""
    if extra is None:
        ui.press("escape")
        time.sleep(0.2)
        return
    extra.close()
    _close_extra_windows(ui.pid)


def _open_mail(ui: Ui) -> Ui | None:
    """Open the 邮件 lane. Return the inbox window's driver, or None for an in-window popup.

    The inbox button ignores an accessibility press. Space opens a window titled
    任务, and that window's web view is absent from the first snapshot. The popup
    is only the surface when no window appears.
    """
    for number, name in list(_pid_windows(ui.pid)):
        if name == "任务":
            _close_preview(ui.pid, number, name)
    before = {number for number, _name in _pid_windows(ui.pid)}
    ui.press_background("space", _inbox_button(ui))
    spawned: tuple[int, str] | None = None
    deadline = time.time() + 8
    while time.time() < deadline and spawned is None:
        for number, name in _pid_windows(ui.pid):
            if number not in before and name:
                spawned = (number, name)
                break
        if spawned is None:
            time.sleep(0.25)
    if spawned is None:
        _select_mail_tab(ui)
        return None
    _raise_named_window(ui.pid, spawned[1])
    extra = Ui(ui.pid, spawned[0])
    deadline = time.time() + 8
    ready = False
    while time.time() < deadline:
        try:
            extra.refresh()
        except Exception:
            time.sleep(0.3)
            continue
        if any(element.get("role") == "AXWebArea" for element in extra.elements):
            ready = True
            break
        time.sleep(0.3)
    if not ready:
        extra.close()
        _close_extra_windows(ui.pid)
        raise AssertionError("missing AXButton 邮件")
    try:
        _select_mail_tab(extra)
    except Exception:
        extra.close()
        _close_extra_windows(ui.pid)
        raise
    return extra


def e_01(ui: Ui) -> None:
    extra = _open_mail(ui)
    try:
        surface = extra or ui
        if surface.has("似乎出现了一些问题"):
            raise AssertionError("mail error boundary")
    finally:
        _close_mail(ui, extra)


def e_02(ui: Ui) -> None:
    extra = _open_mail(ui)
    try:
        surface = extra or ui
        if surface.has("似乎出现了一些问题"):
            raise AssertionError("mail error boundary")
        if not (surface.has("写邮件") or surface.has("发信") or surface.has("收件")):
            raise AssertionError("mail compose entry is missing")
    finally:
        _close_mail(ui, extra)


def p_01(ui: Ui) -> None:
    """平台 is on the rail only for a directory admin of the official team.

    A member table column or a team-nav 职位 is not that admin. Hidden is the
    result for every other signed-in member.
    """
    if "平台" not in ui.rail():
        return
    ui.open("平台")
    if ui.has("似乎出现了一些问题"):
        raise AssertionError("platform error boundary")


def g_01(ui: Ui) -> None:
    ui.open("插件")
    ui.wait_for("写作", 8)
    ui.click("返回应用") if ui.has("返回应用") else None


def g_02(ui: Ui) -> None:
    ui.open("插件")
    ui.wait_for("写作", 8)
    ui.work()


def t_01(ui: Ui) -> None:
    _settings(ui, "通用")


def t_02(ui: Ui) -> None:
    _settings(ui, "外观")
    for label in ("工作", "消息", "知识"):
        ui.open(label)


def t_03(ui: Ui) -> None:
    _settings(ui, "快捷键")


def t_04(ui: Ui) -> None:
    _settings(ui, "用量")


def t_05(ui: Ui) -> None:
    _settings(ui, "渠道")


def t_06(ui: Ui) -> None:
    _settings(ui, "数据")
    if not (ui.has("清除缓存") or ui.has("应用数据") or ui.has("数据")):
        raise AssertionError("data page has no management entry")


def t_07(ui: Ui) -> None:
    _settings(ui, "执行轨迹")
    ui.refresh()
    if "分析轨迹" not in "\n".join(ui.labels()) and "执行轨迹" not in "\n".join(ui.labels()):
        raise AssertionError("traces page is missing its title")


def t_08(ui: Ui) -> None:
    _settings(ui, "环境依赖")


def t_09(ui: Ui) -> None:
    _settings(ui, "归档")


def t_10(ui: Ui) -> None:
    _settings(ui, "关于")
    ui.refresh()
    blob = "\n".join(ui.labels())
    if "ToWork" not in blob:
        raise AssertionError("about page does not name ToWork")


def t_11(ui: Ui) -> None:
    ui.open("设置")
    ui.wait_for("搜索设置", 8)
    # The closed control is a button labeled with the placeholder. The field
    # appears after that button is pressed.
    opened = any(
        element.get("role") in ("AXTextField", "AXTextArea", "AXSearchField", "AXComboBox")
        and "搜索设置" in " ".join(str(element.get("label") or "").split() + str(element.get("value") or "").split())
        for element in ui.elements
    )
    if not opened:
        ui.click("搜索设置", "AXButton")
        time.sleep(0.4)
    ui.fill("归档", "搜索设置")
    time.sleep(0.5)
    ui.refresh()
    typed = any(
        element.get("role") in ("AXTextField", "AXTextArea", "AXSearchField", "AXComboBox")
        and "归档" in str(element.get("value") or "")
        for element in ui.elements
    )
    if not typed:
        raise AssertionError("settings search missed 归档")
    ui.fill("zzzz-no-setting", "搜索设置")
    time.sleep(0.5)
    ui.refresh()


def t_12(ui: Ui) -> None:
    ui.open("设置")
    blob = "\n".join(ui.labels())
    for label in ("偏好", "连接", "开发者", "系统"):
        if label not in blob:
            raise AssertionError(f"settings group {label} is missing")
    if "自主进化" in blob:
        raise AssertionError("evolution is in the settings sidebar")


def x_01(ui: Ui) -> None:
    ui.work()
    ui.click("搜索任务", "AXButton")
    time.sleep(0.3)
    ui.press("escape")
    time.sleep(0.3)
    ui.refresh()
    if not ui.has("搜索任务"):
        raise AssertionError("escape did not return focus to search")


def x_02(ui: Ui) -> None:
    order = ui.rail()
    needed = ["工作", "知识", "消息", "团队", "插件"]
    positions = [order.index(label) if label in order else -1 for label in needed]
    if any(position < 0 for position in positions) or positions != sorted(positions):
        raise AssertionError(f"rail restore needs a pointer drag: {order}")
    for label in ("文件", "笔记", "编码搭档"):
        if label in order:
            raise AssertionError(f"{label} is on the default rail")


def x_03(ui: Ui) -> None:
    ui.work()
    ui.open("消息")
    ui.refresh()
    if ui.has("Reply with the single word pong"):
        raise AssertionError("work task text is in the message list")


def x_04(ui: Ui) -> None:
    raise AssertionError("disconnect is not posted")


def x_05(ui: Ui) -> None:
    ui.work()
    if ui.has("似乎出现了一些问题"):
        raise AssertionError("work route is blank")


CASES = {
    "P0-01": p0_01, "P0-02": p0_02, "P0-03": p0_03, "P0-04": p0_04, "P0-05": p0_05,
    "P0-06": p0_06, "P0-07": p0_07, "P0-08": p0_08, "P0-09": p0_09, "P0-10": p0_10,
    "P0-11": p0_11, "P0-12": p0_12, "P0-13": p0_13, "P0-14": p0_14, "P0-15": p0_15,
    "P0-16": p0_16,
    "W-01": w_01, "W-02": w_02, "W-03": w_03, "W-04": w_04, "W-05": w_05, "W-06": w_06,
    "W-07": w_07, "W-08": w_08, "W-09": w_09, "W-10": w_10, "W-11": w_11, "W-12": w_12,
    "W-13": w_13, "W-14": w_14, "W-15": w_15, "W-16": w_16, "W-17": w_17, "W-18": w_18,
    "W-19": w_19, "W-20": w_20, "W-21": w_21,
    "M-01": m_01, "M-02": m_02, "M-03": m_03, "M-04": m_04, "M-05": m_05, "M-06": m_06,
    "M-07": m_07, "M-08": m_08, "M-09": m_09, "M-10": m_10, "M-11": m_11, "M-12": m_12,
    "O-01": o_01, "O-02": o_02, "O-03": o_03, "O-04": o_04, "O-05": o_05, "O-06": o_06,
    "K-01": k_01, "K-02": k_02, "K-03": k_03, "K-04": k_04, "K-05": k_05, "K-06": k_06, "K-07": k_07,
    "F-01": f_01, "F-02": f_02, "F-03": f_03, "F-04": f_04, "F-05": f_05,
    "N-01": n_01, "C-01": c_01, "S-01": s_01, "S-02": s_02, "E-01": e_01, "E-02": e_02,
    "P-01": p_01, "G-01": g_01, "G-02": g_02,
    "T-01": t_01, "T-02": t_02, "T-03": t_03, "T-04": t_04, "T-05": t_05, "T-06": t_06,
    "T-07": t_07, "T-08": t_08, "T-09": t_09, "T-10": t_10, "T-11": t_11, "T-12": t_12,
    "X-01": x_01, "X-02": x_02, "X-03": x_03, "X-04": x_04, "X-05": x_05,
}


def _settings(ui: Ui, label: str) -> None:
    ui.open("设置")
    try:
        ui.wait_for(label, 10)
    except AssertionError:
        ui.open("设置")
        ui.wait_for(label, 10)
    ui.click(label, "AXButton")
    time.sleep(0.4)
    ui.refresh()
    if not ui.has(label):
        raise AssertionError(f"settings page {label} did not open")
    if ui.has("似乎出现了一些问题"):
        raise AssertionError(f"settings page {label} hit the error boundary")


def _open_tobot(ui: Ui) -> None:
    """ToBot stays in the personal inbox. A team inbox does not list it."""
    ui.open("消息")
    ui.refresh()
    if not ui.has("ToBot"):
        _select_center(ui, "个人")
        ui.open("消息")
        ui.refresh()
    ui.click("ToBot")
    time.sleep(0.6)
    ui.refresh()


def _direct_message(ui: Ui, text: str) -> None:
    ui.open("消息")
    ui.refresh()
    if ui.has("关闭"):
        ui.pixel_click(ui.find("关闭", "AXButton"))
        time.sleep(0.4)
        ui.refresh()
    rows = [
        element
        for element in ui.elements
        if element.get("role") == "AXButton"
        and "Admin" in " ".join(str(element.get("label") or "").split())
        and (element.get("frame") or {}).get("w", 9999) < 400
    ]
    if not rows:
        raise AssertionError("the other member is not in the conversation list")
    ui.pixel_click(rows[0], dx=30)
    ui.wait_for("输入消息", 10)
    ui.fill(text, "输入消息")
    ui.submit_composer()
    ui.wait_for(text, 15)


def _towork_pids() -> set[int]:
    result = subprocess.run(["pgrep", "-f", "towork-cua-fork1/ToWork.app"], capture_output=True, text=True)
    return {int(line) for line in result.stdout.split() if line.isdigit()}


def relaunch(ui: Ui, ready: str = "工作", repark: bool = False) -> None:
    pid = ui.pid
    subprocess.run(
        ["osascript", "-e", f'tell application "System Events" to tell (first process whose unix id is {pid}) to set visible to true'],
        check=False,
        capture_output=True,
        text=True,
    )
    # Quit only this owned pid. A dead pid's kill message stays out of the case log.
    subprocess.run(["kill", str(pid)], check=False, capture_output=True, text=True)
    deadline = time.time() + 15
    while time.time() < deadline:
        try:
            os.kill(pid, 0)
        except OSError:
            break
        time.sleep(0.3)
    if repark:
        _park_session()
    env = os.environ.copy()
    env.pop("A3S_STUDIO_FORK_ALLOC", None)
    env.update({"A3S_TENANT_SLUG": "towork-cua", "A3S_STUDIO_INSTANCE": "fork1"})
    binary = SUPPORT / "ToWork.app/Contents/MacOS/ToWork"
    subprocess.Popen(
        [str(binary)],
        env=env,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
    )
    deadline = time.time() + 40
    saw_window = False
    while time.time() < deadline:
        try:
            new_pid, window = owned_target()
        except Exception:
            time.sleep(1)
            continue
        saw_window = True
        ui.pid = new_pid
        ui.window_id = window
        try:
            ui.refresh()
        except Exception:
            time.sleep(1)
            continue
        if ui.has(ready):
            if ready != "密码登录":
                _backup_session()
            return
        time.sleep(1)
    if not saw_window:
        raise AssertionError("owned window did not return after restart")
    raise AssertionError(f"owned window did not show {ready} after restart")


def run(wave: str) -> int:
    require_credentials()
    selected = WAVES[wave]
    pid, window = owned_target()
    print(f"owned pid={pid} window={window} bundle={BUNDLE}")
    ui = Ui(pid, window)
    ui.rebind = owned_target
    _backup_session()
    # An open popout makes an untargeted key fail before any case starts.
    _close_extra_windows(pid)
    try:
        ui.press("escape")
    except RuntimeError as exc:
        if "same_pid_keyboard_ambiguity" not in str(exc):
            raise
    time.sleep(0.2)
    failed = 0
    try:
        for case_id in selected:
            try:
                _recover(ui)
            except Exception as exc:
                failed += 1
                print(f"FAIL {case_id} {exc}")
                sys.stdout.flush()
                continue
            started = time.time()
            try:
                CASES[case_id](ui)
            except Exception as exc:
                failed += 1
                print(f"FAIL {case_id} {exc}")
            else:
                print(f"PASS {case_id} {time.time() - started:.1f}s")
            sys.stdout.flush()
    finally:
        ui.close()
    print(f"done passed={len(selected) - failed} failed={failed}")
    return 1 if failed else 0


def main() -> None:
    wave = "all"
    if len(sys.argv) > 1:
        if sys.argv[1] == "--fail-closed":
            for name in CREDENTIALS:
                os.environ.pop(name, None)
            require_credentials()
            return
        wave = sys.argv[1]
    raise SystemExit(run(wave))


if __name__ == "__main__":
    main()
