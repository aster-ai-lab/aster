"""Drive one owned ToWork window through cua-driver. No model stubs."""

from __future__ import annotations

import json
import select
import subprocess
import time
from typing import Any


# Another snapshot, including one from a different driver client, supersedes the
# token before the press. Five attempts is the bound already used by the
# team-admin harness. Each attempt addresses the same control again.
_PRESS_ATTEMPTS = 5


def _press_should_retry(message: str) -> bool:
    """A stale or unproven token can be pressed again from a fresh snapshot."""
    return (
        "-25202" in message
        or "element_outside_target_window" in message
        or "element_token is stale" in message
    )


def _incomplete_suffix(message: str, text: str) -> str | None:
    """The unsent tail after a partial insert.

    The driver reports how many characters landed and says to retry only that
    tail. Sending the whole string again would repeat the part already there.
    """
    marker = "delivered "
    start = message.find(marker)
    if start < 0:
        return None
    rest = message[start + len(marker):]
    delivered_text, separator, after = rest.partition(" of ")
    if not separator:
        return None
    total_text = after.split(" ", 1)[0]
    if not delivered_text.isdigit() or not total_text.isdigit():
        return None
    delivered = int(delivered_text)
    total = int(total_text)
    if total != len(text) or delivered < 0 or delivered >= len(text):
        return None
    return text[delivered:]


class Drive:
    def __init__(self, pid: int, window_id: int) -> None:
        self.pid = pid
        self.window_id = window_id
        self.rebind = None
        self.proc = subprocess.Popen(
            ["cua-driver", "mcp"],
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.DEVNULL,
            text=True,
        )
        self._id = 0
        self.snapshot_id = ""
        self.elements: list[dict[str, Any]] = []
        self._send(
            {
                "jsonrpc": "2.0",
                "id": self._next(),
                "method": "initialize",
                "params": {
                    "protocolVersion": "2025-06-18",
                    "capabilities": {},
                    "clientInfo": {"name": "towork-e2e", "version": "1"},
                },
            }
        )
        assert self.proc.stdin
        self.proc.stdin.write(json.dumps({"jsonrpc": "2.0", "method": "notifications/initialized"}) + "\n")
        self.proc.stdin.flush()

    def close(self) -> None:
        if self.proc.poll() is None:
            self.proc.kill()

    def _next(self) -> int:
        self._id += 1
        return self._id

    def _send(self, payload: dict[str, Any]) -> dict[str, Any]:
        assert self.proc.stdin and self.proc.stdout
        self.proc.stdin.write(json.dumps(payload) + "\n")
        self.proc.stdin.flush()
        ready, _, _ = select.select([self.proc.stdout], [], [], 25)
        if not ready:
            raise RuntimeError("cua-driver timed out")
        line = self.proc.stdout.readline()
        if not line:
            raise RuntimeError("cua-driver closed stdout")
        return json.loads(line)

    def call(self, name: str, arguments: dict[str, Any]) -> dict[str, Any]:
        response = self._send(
            {
                "jsonrpc": "2.0",
                "id": self._next(),
                "method": "tools/call",
                "params": {"name": name, "arguments": arguments},
            }
        )
        if "error" in response:
            raise RuntimeError(json.dumps(response["error"], ensure_ascii=False)[:500])
        result = response.get("result") or {}
        if result.get("isError"):
            text = " ".join(
                item.get("text", "") for item in result.get("content") or [] if isinstance(item, dict)
            )
            raise RuntimeError(text[:500] or "cua tool error")
        return result

    def refresh(self) -> None:
        try:
            result = self._window_state()
        except RuntimeError as exc:
            if not self._window_fault(exc):
                raise
            self._recover_window()
            result = self._window_state()
        structured = result.get("structuredContent") or {}
        self.snapshot_id = str(structured.get("snapshot_id") or "")
        self.elements = list(structured.get("elements") or [])
        self.degraded_reason = str(structured.get("degraded_reason") or "")

    def _window_fault(self, exc: BaseException) -> bool:
        message = str(exc)
        return self.rebind is not None and (
            "not a live window" in message
            or "not one coherent" in message
            or "no live frame" in message
        )

    def _recover_window(self) -> None:
        """Rebind the owned window and undo a tiny frame. A 72-point window hides the work page.

        A relaunch leaves a short gap with no window. "no live frame" is the
        same gap: the stored window id is already gone. Wait and rebind
        instead of failing the case on that stale id.
        """
        if self.rebind is None:
            raise RuntimeError("owned window is not bound")
        deadline = time.time() + 8
        last: BaseException | None = None
        while time.time() < deadline:
            try:
                rebound = self.rebind()
            except Exception as exc:
                last = exc
                time.sleep(0.4)
                continue
            if rebound is None:
                time.sleep(0.4)
                continue
            self.pid, self.window_id = rebound
            try:
                self._restore_frame()
                self._window_state()
            except Exception as exc:
                last = exc
                if isinstance(exc, RuntimeError) and not self._window_fault(exc):
                    if "window stayed" not in str(exc) and "window resize failed" not in str(exc):
                        raise
                time.sleep(0.4)
                continue
            return
        if last is not None:
            raise last
        raise RuntimeError("owned window is not bound")

    def _window_size(self) -> tuple[int, int] | None:
        script = (
            'tell application "System Events" to get size of window 1 of '
            f"(first process whose unix id is {self.pid})"
        )
        completed = subprocess.run(["osascript", "-e", script], capture_output=True, text=True)
        if completed.returncode != 0:
            return None
        parts = [part.strip() for part in completed.stdout.strip().strip("{}").split(",")]
        if len(parts) != 2 or not all(part.isdigit() for part in parts):
            return None
        return int(parts[0]), int(parts[1])

    def _restore_frame(self) -> None:
        size = self._window_size()
        if size is None:
            return
        width, height = size
        if width >= 1000 and height >= 700:
            return
        self.set_window_size(1440, 920)
        time.sleep(0.4)
        if self.rebind is None:
            return
        try:
            rebound = self.rebind()
        except Exception:
            return
        if rebound is not None:
            self.pid, self.window_id = rebound

    def _window_state(self) -> dict[str, Any]:
        return self.call(
            "get_window_state",
            {"pid": self.pid, "window_id": self.window_id, "include_screenshot": False},
        )

    def labels(self) -> list[str]:
        found: list[str] = []
        for element in self.elements:
            label = " ".join(str(element.get("label") or element.get("value") or "").split())
            if label:
                found.append(label)
        return found

    def has(self, text: str) -> bool:
        needle = " ".join(text.split())
        return any(needle in label for label in self.labels())

    def find(self, text: str, role: str | None = None) -> dict[str, Any]:
        needle = " ".join(text.split())
        matches = []
        for element in self.elements:
            label = " ".join(str(element.get("label") or element.get("value") or "").split())
            if needle not in label:
                continue
            if role and element.get("role") != role:
                continue
            matches.append(element)
        if not matches:
            raise AssertionError(f"missing {role or 'control'} {text}")
        return matches[0]

    def _background_click(self, element: dict[str, Any], *, attempt: int = 0) -> None:
        """Accessibility click. Pixel and HID clicks move the system pointer.

        A superseded token is pressed again from a fresh snapshot, up to
        _PRESS_ATTEMPTS. Each later press uses the same role, label, and nearest
        frame. It does not choose another control.
        """
        token = element.get("element_token")
        if not token:
            raise RuntimeError("background click needs an element token")
        try:
            self.call(
                "click",
                {
                    "pid": self.pid,
                    "window_id": self.window_id,
                    "element_token": token,
                    "action": "press",
                    "delivery_mode": "background",
                },
            )
        except RuntimeError as exc:
            if attempt + 1 >= _PRESS_ATTEMPTS or not _press_should_retry(str(exc)):
                raise
            time.sleep(0.4)
            fresh = self._readdress(element)
            if fresh is None and attempt + 2 < _PRESS_ATTEMPTS:
                time.sleep(0.3)
                fresh = self._readdress(element)
            if fresh is None:
                raise
            self._background_click(fresh, attempt=attempt + 1)

    def _readdress(self, element: dict[str, Any]) -> dict[str, Any] | None:
        """Find the same control in a new snapshot. A newer snapshot invalidates the old token."""
        self.refresh()
        role = element.get("role")
        label = " ".join(str(element.get("label") or "").split())
        frame = element.get("frame") or {}
        matches = [
            item
            for item in self.elements
            if item.get("role") == role
            and " ".join(str(item.get("label") or "").split()) == label
            and item.get("element_token")
        ]
        if not matches:
            return None

        def score(item: dict[str, Any]) -> tuple[float, float]:
            other = item.get("frame") or {}
            width = abs(float(other.get("w", 0)) - float(frame.get("w", 0)))
            place = abs(float(other.get("x", 0)) - float(frame.get("x", 0))) + abs(
                float(other.get("y", 0)) - float(frame.get("y", 0))
            )
            return (width, place)

        return min(matches, key=score)

    def pixel_click(self, element: dict[str, Any], dx: float | None = None, dy: float | None = None) -> None:
        """Click without moving the system pointer."""
        del dx, dy
        self._background_click(element)

    def show_menu(self, element: dict[str, Any], *, attempt: int = 0) -> None:
        """Open one element's menu. The action stays on the accessibility path.

        A right-button or pixel click can move the pointer. This call does not
        send a button, a coordinate, or a key. A superseded token is resolved
        again, up to _PRESS_ATTEMPTS, for the same control.
        """
        token = element.get("element_token")
        if not token:
            raise RuntimeError("show menu needs an element token")
        try:
            self.call(
                "click",
                {
                    "pid": self.pid,
                    "window_id": self.window_id,
                    "element_token": token,
                    "action": "show_menu",
                    "delivery_mode": "background",
                },
            )
        except RuntimeError as exc:
            if attempt + 1 >= _PRESS_ATTEMPTS or not _press_should_retry(str(exc)):
                raise
            time.sleep(0.4)
            fresh = self._readdress(element)
            if fresh is None:
                raise
            self.show_menu(fresh, attempt=attempt + 1)

    def _on_activity_bar(self, element: dict[str, Any]) -> bool:
        if element.get("role") not in ("AXButton", "AXPopUpButton"):
            return False
        frame = element.get("frame") or {}
        if float(frame.get("w", 9999)) > 48 or float(frame.get("h", 9999)) > 48:
            return False
        web = next((item for item in self.elements if item.get("role") == "AXWebArea"), None)
        if web is None:
            return False
        origin = float((web.get("frame") or {}).get("x", 0))
        return abs(float(frame.get("x", 9999)) - origin) <= 36

    def _screen_click(self, x: int, y: int) -> None:
        """Refuse a system click. It moves the pointer."""
        del x, y
        raise RuntimeError("system click moves the pointer")

    def pointer_click(self, element: dict[str, Any], dx: float | None = None, dy: float | None = None) -> None:
        """Press a control without moving the system pointer."""
        del dx, dy
        self._background_click(element)

    def _post_pointer(self, event: str, x: int, y: int) -> None:
        del event, x, y
        raise RuntimeError("pointer events move the system pointer")

    def pointer_move(self, x: int, y: int) -> None:
        """Refuse a pointer move. It moves the system pointer and fronts the process."""
        del x, y
        raise RuntimeError("pointer events move the system pointer")

    def pointer_down_at(self, x: int, y: int) -> None:
        del x, y
        raise RuntimeError("pointer events move the system pointer")

    def pointer_drag_at(self, x: int, y: int) -> None:
        self._post_pointer("kCGEventLeftMouseDragged", x, y)

    def pointer_up_at(self, x: int, y: int) -> None:
        self._post_pointer("kCGEventLeftMouseUp", x, y)

    def pointer_drag(self, source: dict[str, Any], target: dict[str, Any], sx: float = 40, sy: float = 16, tx: float = 40, ty: float = 12) -> None:
        """Refuse a drag. Background drag is unavailable, and a foreground drag moves the pointer."""
        del source, target, sx, sy, tx, ty
        raise RuntimeError("background drag is unavailable on macOS")

    def pointer_scroll(self, x: int, y: int, lines: int, move: bool = True) -> None:
        """Refuse a wheel scroll. The background wheel event moves the system pointer."""
        del x, y, lines, move
        raise RuntimeError("background scroll moves the system pointer")

    def click(self, text: str, role: str | None = None) -> None:
        """Pixel-click. An accessibility index click does not reach the web view."""
        self.refresh()
        try:
            element = self.find(text, role)
        except AssertionError:
            self._dismiss_blocker()
            element = self.find(text, role)
        self.pixel_click(element)
        time.sleep(0.3)

    def press_background(
        self,
        key: str,
        element: dict[str, Any] | None = None,
        modifiers: list[str] | None = None,
        *,
        attempt: int = 0,
    ) -> None:
        """Post one key. Background delivery does not front the process or move the pointer."""
        arguments: dict[str, Any] = {
            "pid": self.pid,
            "window_id": self.window_id,
            "key": key,
            "delivery_mode": "background",
        }
        token = (element or {}).get("element_token")
        if token:
            arguments["element_token"] = token
        if modifiers:
            arguments["modifiers"] = modifiers
        try:
            self.call("press_key", arguments)
        except RuntimeError as exc:
            if not _press_should_retry(str(exc)):
                raise
            # A key with no element still fails when another snapshot has
            # superseded the driver's last token. Refresh and send it again,
            # without that token, up to the same attempt bound.
            if element is None:
                if attempt + 1 >= _PRESS_ATTEMPTS:
                    raise
                time.sleep(0.2)
                self.refresh()
                self.press_background(key, None, modifiers, attempt=attempt + 1)
                return
            # The rejected key did not change the control. Address that same control again.
            if attempt + 1 >= _PRESS_ATTEMPTS:
                raise
            time.sleep(0.4)
            fresh = self._readdress(element)
            if fresh is None and attempt + 2 < _PRESS_ATTEMPTS:
                time.sleep(0.3)
                fresh = self._readdress(element)
            if fresh is None:
                raise
            self.press_background(key, fresh, modifiers, attempt=attempt + 1)

    def type_text(self, text: str, element: dict[str, Any] | None = None, *, attempt: int = 0) -> None:
        """Insert text in the background. Omit the element when the field is already focused.

        Addressing a field focuses it again. A knowledge name draft treats that
        focus change as a blur and cancels when the draft was not edited yet.
        """
        arguments: dict[str, Any] = {
            "pid": self.pid,
            "window_id": self.window_id,
            "text": text,
            "delivery_mode": "background",
            "delay_ms": 0,
        }
        token = (element or {}).get("element_token")
        if token:
            arguments["element_token"] = token
        try:
            self.call("type_text", arguments)
        except RuntimeError as exc:
            message = str(exc)
            suffix = _incomplete_suffix(message, text)
            if suffix is not None and attempt + 1 < _PRESS_ATTEMPTS:
                time.sleep(0.15)
                if suffix == text and element is not None:
                    # Nothing landed, so the field was not focused. Address it again.
                    fresh = self._readdress(element)
                    self.type_text(text, fresh if fresh is not None else element, attempt=attempt + 1)
                    return
                # The caret stays after the inserted prefix. Do not address the
                # field again, and do not send the prefix a second time.
                self.type_text(suffix, None, attempt=attempt + 1)
                return
            if element is None or attempt + 1 >= _PRESS_ATTEMPTS or not _press_should_retry(message):
                raise
            time.sleep(0.4)
            fresh = self._readdress(element)
            if fresh is None:
                raise
            self.type_text(text, fresh, attempt=attempt + 1)

    def chord(
        self,
        keys: list[str],
        element: dict[str, Any] | None = None,
        *,
        attempt: int = 0,
    ) -> None:
        """Press a key combination without fronting the window.

        A superseded token is sent again, up to _PRESS_ATTEMPTS, for the same control.
        """
        arguments: dict[str, Any] = {
            "pid": self.pid,
            "window_id": self.window_id,
            "keys": keys,
            "delivery_mode": "background",
        }
        token = (element or {}).get("element_token")
        if token:
            arguments["element_token"] = token
        try:
            self.call("hotkey", arguments)
        except RuntimeError as exc:
            if not _press_should_retry(str(exc)):
                raise
            if element is None:
                if attempt + 1 >= _PRESS_ATTEMPTS:
                    raise
                time.sleep(0.2)
                self.refresh()
                self.chord(keys, None, attempt=attempt + 1)
                return
            if attempt + 1 >= _PRESS_ATTEMPTS:
                raise
            time.sleep(0.4)
            fresh = self._readdress(element)
            if fresh is None and attempt + 2 < _PRESS_ATTEMPTS:
                time.sleep(0.3)
                fresh = self._readdress(element)
            if fresh is None:
                raise
            self.chord(keys, fresh, attempt=attempt + 1)

    def press(self, key: str) -> None:
        self.press_background(key)

    def _front_keystroke(self, script_body: str) -> None:
        """Refuse a fronting keystroke. Background press_key is the path that does not steal focus."""
        del script_body
        raise RuntimeError("fronting the process is not used")

    def _window_point(self, element: dict[str, Any], dx: float = 24, dy: float = 18) -> tuple[int, int]:
        web = next(item for item in self.elements if item.get("role") == "AXWebArea")
        frame = element.get("frame") or {}
        origin = web.get("frame") or {}
        return (
            int(frame["x"] - origin["x"] + dx),
            int(frame["y"] - origin["y"] + dy),
        )

    def focus_field(self, text: str) -> None:
        """Pixel-click a text control. WKWebView ignores AX focus for paste."""
        self.refresh()
        needle = " ".join(text.split())
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
        if not controls and text in ("描述任务", "输入消息"):
            controls = [element for element in self.elements if element.get("role") == "AXTextArea"]
        element = (
            max(controls, key=lambda item: (item.get("frame") or {}).get("w", 0))
            if controls
            else self.find(text)
        )
        frame = element.get("frame") or {}
        # A System Events click does not put the caret in this web view.
        self.pointer_click(
            element,
            dx=max(8.0, float(frame.get("w", 40)) / 2),
            dy=max(8.0, float(frame.get("h", 20)) / 2),
        )
        time.sleep(0.15)

    def menu_click(self, item: str, menu: str = "Edit") -> None:
        """Invoke the application menu through accessibility. A System Events click fronts the process."""
        self.call(
            "invoke_menu",
            {
                "pid": self.pid,
                "window_id": self.window_id,
                "path": [menu, item],
            },
        )

    def type_into(self, element: dict[str, Any], text: str) -> None:
        """Focus one web field, then type without addressing it.

        A WebKit field can omit the window id. Addressing that field is refused,
        and an unfocused field accepts none of the characters. The press focuses
        it. Select-all and the text then go to that focus with no element token.
        A selection flicker still focused the field, so typing continues.
        """
        try:
            self.pixel_click(element)
        except RuntimeError as exc:
            if "stable AXSelected" not in str(exc):
                raise
        time.sleep(0.12)
        self.chord(["cmd", "a"])
        if text:
            self.type_text(text)
        else:
            self.press("delete")

    def fill(self, text: str, field: str) -> None:
        """Insert text into a labeled field. Paste fronts the window, so it is not used."""
        self.refresh()
        needle = " ".join(field.split())
        controls = []
        for element in self.elements:
            if element.get("role") not in ("AXTextArea", "AXTextField", "AXSearchField", "AXComboBox"):
                continue
            blob = " ".join(
                str(element.get("label") or "").split()
                + str(element.get("value") or "").split()
            )
            if needle in blob:
                controls.append(element)
        if not controls and field in ("描述任务", "输入消息"):
            controls = [element for element in self.elements if element.get("role") == "AXTextArea"]
        if not controls:
            raise AssertionError(f"missing field {field}")
        target = max(controls, key=lambda item: (item.get("frame") or {}).get("w", 0))
        self.type_into(target, text)

    def set_shown_value(self, element: dict[str, Any], text: str) -> None:
        """Write the accessibility value. This does not move the pointer or front the window."""
        token = element.get("element_token")
        if not token:
            raise RuntimeError("set value needs an element token")
        self.call(
            "set_value",
            {
                "pid": self.pid,
                "window_id": self.window_id,
                "element_token": token,
                "value": text,
            },
        )

    def fill_element(self, element: dict[str, Any], text: str) -> None:
        """Paste into one control. Typing is not used: the input method rewrites Latin."""
        frame = element.get("frame") or {}
        self.pointer_click(
            element,
            dx=max(8.0, float(frame.get("w", 40)) / 2),
            dy=max(8.0, float(frame.get("h", 20)) / 2),
        )
        time.sleep(0.15)
        self._paste(text)

    def _paste(self, text: str) -> None:
        self.call("clipboard_write", {"text": text})
        self.menu_click("Select All")
        time.sleep(0.08)
        self.menu_click("Paste")
        time.sleep(0.25)

    def grab(self, path: str) -> None:
        """Refresh the tree and write the window PNG. Coordinates match the tree."""
        arguments = {
            "pid": self.pid,
            "window_id": self.window_id,
            "screenshot_out_file": path,
            "include_screenshot": True,
        }
        try:
            result = self.call("get_window_state", arguments)
        except RuntimeError as exc:
            if not self._window_fault(exc):
                raise
            self._recover_window()
            arguments["pid"] = self.pid
            arguments["window_id"] = self.window_id
            result = self.call("get_window_state", arguments)
        structured = result.get("structuredContent") or {}
        self.snapshot_id = str(structured.get("snapshot_id") or "")
        self.elements = list(structured.get("elements") or [])
        self.degraded_reason = str(structured.get("degraded_reason") or "")

    def _dismiss_blocker(self) -> None:
        """Close a dialog this suite opened. Never choose 退出登录."""
        self.press("escape")
        time.sleep(0.25)
        self.refresh()
        if self.has("退出登录"):
            self.press("escape")
            time.sleep(0.25)
            self.refresh()
        # The sidebar always shows 新建项目. Only the open dialog has this sentence,
        # and only an actual 取消 button may be clicked.
        if not self.has("项目是一个独立的工作文件夹"):
            return
        cancels = [
            element
            for element in self.elements
            if element.get("role") == "AXButton"
            and " ".join(str(element.get("label") or "").split()) == "取消"
        ]
        if not cancels:
            return
        self.pixel_click(cancels[0])
        time.sleep(0.3)
        self.refresh()

    def _click_send(self) -> bool:
        sends = [
            element
            for element in self.elements
            if element.get("role") == "AXButton" and str(element.get("label") or "") == "发送"
        ]
        fields = [
            element
            for element in self.elements
            if element.get("role") in ("AXTextArea", "AXTextField")
        ]
        if not sends or not fields:
            return False
        field = max(fields, key=lambda element: (element.get("frame") or {}).get("w", 0))
        field_y = (field.get("frame") or {}).get("y", 0)
        target = min(sends, key=lambda element: abs((element.get("frame") or {}).get("y", 0) - field_y))
        frame = target.get("frame") or {}
        # WKWebView does not deliver an AX element click to the send control.
        self.pointer_click(target, dx=float(frame.get("w", 32)) / 2, dy=float(frame.get("h", 32)) / 2)
        return True

    def submit_composer(self) -> None:
        """Pixel-click send. The control appears only after the live composer attaches."""
        clicked = False
        deadline = time.time() + 8
        while time.time() < deadline:
            self.refresh()
            if self._click_send():
                clicked = True
                break
            time.sleep(0.3)
        if not clicked:
            self.press_background("return")
            return
        time.sleep(1.5)
        self.refresh()
        fields = [
            element
            for element in self.elements
            if element.get("role") in ("AXTextArea", "AXTextField")
        ]
        if not fields:
            return
        field = max(fields, key=lambda element: (element.get("frame") or {}).get("w", 0))
        value = str(field.get("value") or "")
        if value and "运行中" not in value and len(value) > 12:
            self._click_send()

    def set_window_size(self, width: int, height: int) -> None:
        script = (
            'tell application "System Events"\n'
            f"  tell (first process whose unix id is {self.pid}) to set size of window 1 to {{{width}, {height}}}\n"
            "end tell"
        )
        completed = subprocess.run(["osascript", "-e", script], capture_output=True, text=True)
        if completed.returncode != 0:
            raise RuntimeError((completed.stderr or "window resize failed").strip()[:300])
        deadline = time.time() + 4
        while time.time() < deadline:
            size = self._window_size()
            if size and abs(size[0] - width) <= 8 and abs(size[1] - height) <= 8:
                time.sleep(0.35)
                return
            # The main window's minimum is wider than a 720-point request.
            # A wider frame with the requested height is that clamp, not a miss.
            if size and size[0] + 8 >= width and abs(size[1] - height) <= 8:
                time.sleep(0.35)
                return
            time.sleep(0.2)
        raise RuntimeError(f"window stayed {self._window_size()} after resize to {width}x{height}")

    def press_return(self, element: dict[str, Any]) -> None:
        self.press_background("return", element)

    def wait_for(self, text: str, timeout: float = 20) -> None:
        deadline = time.time() + timeout
        while time.time() < deadline:
            self.refresh()
            if self.has(text):
                return
            time.sleep(0.5)
        raise AssertionError(f"timed out waiting for {text}")

    def wait_until_gone(self, text: str, timeout: float = 10) -> None:
        deadline = time.time() + timeout
        while time.time() < deadline:
            self.refresh()
            if not self.has(text):
                return
            time.sleep(0.4)
        raise AssertionError(f"timed out waiting for {text} to leave")
