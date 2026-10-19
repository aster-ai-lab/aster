"""Two-account team workbench drive. Accessibility background delivery only.

Does not set A3S_GUI_ALLOW_MOUSE, does not move the system pointer, and does
not raise a window. The same-session control 「切换为审批人」 is never used.
"""

from __future__ import annotations

import ctypes
import datetime
import json
import os
import re
import shutil
import signal
import subprocess
import sys
import time
import urllib.error
import urllib.request
import uuid
from contextlib import contextmanager
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "towork"))
from driver import Drive  # noqa: E402

SUPPORT = Path.home() / "Library/Application Support/dev.a3s.studio"
API = "http://127.0.0.1:3000"
MEMBER_APP = "towork-cua-teamadmin"
CREATOR_APP = "toyou"
ORG_NAME = "e2e-gui-pair"
MEMBER_FRAGMENT = "dev.a3s.studio/towork-cua-teamadmin/ToWork.app"
CREATOR_FRAGMENT = "dev.a3s.studio/toyou/ToWork.app"
UUID_RE = re.compile(
    r"[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}",
    re.I,
)
PHONE_RE = re.compile(r"1\d{10}")
TOKEN_RE = re.compile(r"a3s_ms_\S+")
CLICK_ROLES = {
    "AXButton",
    "AXPopUpButton",
    "AXComboBox",
    "AXMenuButton",
    "AXMenuItem",
    "AXRadioButton",
    "AXCheckBox",
}


def log(message: str) -> None:
    text = TOKEN_RE.sub("[token]", PHONE_RE.sub("[phone]", message))
    print(text, flush=True)


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


def front_process() -> tuple[int, str]:
    pid_result = subprocess.run(
        [
            "osascript",
            "-e",
            'tell application "System Events" to unix id of first process whose frontmost is true',
        ],
        capture_output=True,
        text=True,
    )
    name_result = subprocess.run(
        [
            "osascript",
            "-e",
            'tell application "System Events" to name of first process whose frontmost is true',
        ],
        capture_output=True,
        text=True,
    )
    pid_text = pid_result.stdout.strip()
    pid = int(pid_text) if pid_text.isdigit() else 0
    return pid, name_result.stdout.strip()


def restore_front(pid: int) -> tuple[int, str]:
    subprocess.run(
        [
            "osascript",
            "-e",
            f'tell application "System Events" to set frontmost of first process whose unix id is {int(pid)} to true',
        ],
        check=False,
        capture_output=True,
        text=True,
    )
    return front_process()


OWNED: set[int] = set()
OPERATOR: tuple[int, str] = (0, "")
POINTER_ORIGIN: tuple[int, int] | None = None
FRONT_STEALS = 0
OPPOSITE_RECORD: str | None = None
OPPOSITE_ACTIVATED = False
KEYBOARD_HOLDS: dict[int, int] = {}
HELD_WINDOWS: dict[int, list[str]] = {}


def pointer_outcome(origin: tuple[int, int], current: tuple[int, int]) -> str:
    """Return unchanged when the pointer is still at the captured point."""
    if current == origin:
        return "unchanged"
    return "changed"


def front_outcome(
    current: int,
    operator: int,
    owned: set[int],
    restored: int | None = None,
) -> str:
    """Classify who is in front.

    kept: the captured operator is in front, or no operator was captured.
    left: a process this drive does not own is in front.
    restore: an owned window is in front and no restore has been applied yet.
    returned: the restore put the captured operator back in front.
    stayed: an owned window is still in front after a restore.
    """
    if not operator or current == operator:
        return "kept"
    if current not in owned:
        return "left"
    if restored is None:
        return "restore"
    if restored == operator:
        return "returned"
    return "stayed"


def note_front(current: int, operator: int, owned: set[int]) -> str:
    """held when the captured operator is front. stolen when this drive is front."""
    outcome = front_outcome(current, operator, owned, None)
    if outcome == "kept":
        return "held"
    if outcome == "restore":
        return "stolen"
    return "left"


def opposite_observation(control: str, activated: bool, status: str) -> str | None:
    """The opposite-decision line the log may print.

    A click on 拒绝 can leave the item 已通过. When that button is already
    gone or disabled, the log says so and records a fresh read of the same
    item. It does not claim a decision was submitted.
    """
    if activated and status == "已通过":
        return "opposite decision left status=已通过"
    if not activated and control in {"absent", "disabled"} and status == "已通过":
        return f"拒绝 control {control}; reread status=已通过"
    return None


def opposite_accepted(record: str | None, activated: bool) -> bool:
    """A pass needs the observation that matches what the drive actually did."""
    expected = None
    if activated:
        expected = opposite_observation("enabled", True, "已通过")
    elif record is not None and record.startswith("拒绝 control absent"):
        expected = opposite_observation("absent", False, "已通过")
    elif record is not None and record.startswith("拒绝 control disabled"):
        expected = opposite_observation("disabled", False, "已通过")
    return expected is not None and record == expected


def drive_acceptance(
    origin: tuple[int, int],
    current: tuple[int, int],
    front_pid: int,
    operator: int,
    owned: set[int],
    steals: int,
    opposite: str | None,
    activated: bool,
) -> str:
    """Pass only when the pointer, the front process, and the decision check hold.

    The captured operator has to still be front. Putting that process back
    after this drive took the front is not a pass. A claimed opposite
    decision that never activated 拒绝 is not a pass either.
    """
    if pointer_outcome(origin, current) != "unchanged":
        return "pointer"
    if steals or note_front(front_pid, operator, owned) != "held":
        return "front"
    if not opposite_accepted(opposite, activated):
        return "opposite"
    return "pass"


def confirm_pointer(kind: str) -> None:
    """Stop the drive when the pointer is no longer at its captured point.

    A second sample ignores a reading taken while the pointer was between
    two positions. The drive does not move the pointer back.
    """
    origin = POINTER_ORIGIN
    if origin is None:
        return
    if pointer() == origin:
        return
    time.sleep(0.05)
    current = pointer()
    if current == origin:
        return
    raise RuntimeError(f"pointer changed during {kind}: {origin} -> {current}")


class Guard(Drive):
    """Reject any CUA call that would move the pointer or raise a window."""

    def __init__(self, pid: int, window_id: int) -> None:
        last: RuntimeError | None = None
        for attempt in range(4):
            try:
                super().__init__(pid, window_id)
                return
            except RuntimeError as error:
                last = error
                proc = getattr(self, "proc", None)
                if proc is not None and proc.poll() is None:
                    proc.kill()
                    try:
                        proc.wait(timeout=2)
                    except subprocess.TimeoutExpired:
                        pass
                message = str(error)
                if "closed stdout" not in message and "timed out" not in message:
                    raise
                log(f"cua client retry {attempt + 1}: {message[:80]}")
                time.sleep(0.5 * (attempt + 1))
        if last is not None:
            raise last

    def close(self) -> None:
        super().close()
        proc = getattr(self, "proc", None)
        if proc is not None:
            try:
                proc.wait(timeout=2)
            except subprocess.TimeoutExpired:
                pass

    def call(self, name: str, arguments: dict) -> dict:
        if arguments.get("delivery_mode") not in (None, "background"):
            raise RuntimeError(f"refusing delivery {arguments.get('delivery_mode')}")
        if name in {"click", "press_key", "type_text", "hotkey"} and arguments.get("delivery_mode") != "background":
            raise RuntimeError(f"refusing {name} without background delivery")
        if name in {"move_mouse", "drag", "scroll", "raise_window", "set_frontmost"}:
            raise RuntimeError(f"refusing pointer tool {name}")
        result = super().call(name, arguments)
        confirm_pointer(name)
        return result


def windows(pid: int) -> list[tuple[int, bool, str]]:
    helper = os.environ.get("A3S_WINDOW_LIST")
    if helper:
        result = subprocess.run([helper, str(pid)], capture_output=True, text=True)
    else:
        swift = f"""
import CoreGraphics
let info = CGWindowListCopyWindowInfo([.excludeDesktopElements], kCGNullWindowID) as? [[String: Any]] ?? []
for w in info {{
  let owner = w["kCGWindowOwnerPID"] as? Int ?? -1
  let layer = w["kCGWindowLayer"] as? Int ?? -1
  if owner == {int(pid)} && layer == 0 {{
    let id = w["kCGWindowNumber"] as? Int ?? 0
    let on = (w["kCGWindowIsOnscreen"] as? Bool) ?? false
    let name = (w["kCGWindowName"] as? String ?? "").replacingOccurrences(of: "\\t", with: " ")
    print("\\(id)\\t\\(on)\\t\\(name)")
  }}
}}
"""
        result = subprocess.run(["swift", "-e", swift], capture_output=True, text=True)
    rows = []
    for line in result.stdout.splitlines():
        parts = line.split("\t")
        if len(parts) >= 3 and parts[0].isdigit():
            rows.append((int(parts[0]), parts[1] == "true", parts[2]))
    return rows


def label_of(element: dict) -> str:
    return " ".join(str(element.get("label") or element.get("value") or "").split())


def bind(pid: int, title: str, previous: Guard | None = None) -> Guard:
    """Bind a window whose accessibility tree is live. A new window can exist in
    the window server before that tree is published.
    """
    deadline = time.time() + 20
    rows: list[tuple[int, bool, str]] = []
    reason = ""
    window_id = 0
    while time.time() < deadline:
        rows = windows(pid)
        ranked = [row for row in rows if row[2] == title] or [row for row in rows if title and title in row[2]]
        ranked.sort(key=lambda row: (not row[1], row[0]))
        reason = ""
        window_id = 0
        for candidate, _on_screen, _name in ranked:
            count, reason = ax_surface(pid, candidate)
            if count > 0 and not reason:
                window_id = candidate
                break
        if window_id:
            break
        time.sleep(0.4)
    if not window_id:
        if not rows:
            raise RuntimeError(f"pid {pid} has no window {title!r}: {rows}")
        raise RuntimeError(f"pid {pid} has no live window {title!r}: {rows} {reason[:160]}")
    if previous is None:
        return Guard(pid, window_id)
    previous.pid = pid
    previous.window_id = window_id
    previous.refresh()
    return previous


def session(app: str) -> dict:
    data = json.loads((SUPPORT / app / "member-session.json").read_text())
    token = data.get("token") or ""
    if not token.startswith("a3s_ms_"):
        raise RuntimeError(f"{app} has no member session")
    return data


def api(token: str, method: str, path: str, body: dict | None = None):
    payload = None if body is None else json.dumps(body).encode()
    last_error = "no api response"
    raw = ""
    for base in (API, "http://127.0.0.1:8080"):
        for attempt in range(12):
            request = urllib.request.Request(
                base + path,
                data=payload,
                method=method,
                headers={
                    "accept": "application/json",
                    "authorization": f"Bearer {token}",
                    **({"content-type": "application/json"} if payload else {}),
                },
            )
            try:
                with urllib.request.urlopen(request, timeout=20) as response:
                    raw = response.read().decode()
                last_error = ""
                break
            except urllib.error.HTTPError as error:
                detail = PHONE_RE.sub("[phone]", error.read().decode(errors="replace")[:300])
                # A restart answers 500 for a few seconds. Retry reads only.
                if method == "GET" and error.code >= 500 and attempt < 11:
                    last_error = f"HTTP {error.code} {detail}"
                    log(f"{method} {path} retry {attempt + 1} HTTP {error.code}")
                    time.sleep(2)
                    continue
                raise RuntimeError(f"{method} {path} HTTP {error.code} {detail}") from error
            except urllib.error.URLError as error:
                last_error = str(error)
                if "refused" not in last_error:
                    raise RuntimeError(f"{method} {path} {error}") from error
                time.sleep(0.4)
        if raw:
            break
    if not raw and last_error:
        raise RuntimeError(f"{method} {path} {last_error}")
    parsed = json.loads(raw) if raw else None
    if isinstance(parsed, dict) and "data" in parsed and "id" not in parsed and "name" not in parsed:
        return parsed["data"]
    return parsed


def ensure_pair() -> str:
    member = session(MEMBER_APP)
    creator = session(CREATOR_APP)
    log(f"account member name={member.get('display_name')} id={member.get('member_id')} app={MEMBER_APP}")
    log(f"account creator name={creator.get('display_name')} id={creator.get('member_id')} app={CREATOR_APP}")
    if member.get("member_id") == creator.get("member_id"):
        raise RuntimeError("the two sessions are the same member")
    member_orgs = api(member["token"], "GET", "/v1/api/org/members/orgs")
    found = next((row for row in member_orgs if row.get("name") == ORG_NAME and row.get("creator")), None)
    if found is None:
        created = api(
            member["token"],
            "POST",
            "/v1/api/org/members/orgs",
            {"name": ORG_NAME, "industry": "software", "scale": "1-9"},
        )
        org_id = created["id"]
        log(f"created team {ORG_NAME} id={org_id}")
    else:
        org_id = found["id"]
        log(f"reused team {ORG_NAME} id={org_id}")
    creator_orgs = api(creator["token"], "GET", "/v1/api/org/members/orgs")
    if not any(row.get("id") == org_id for row in creator_orgs):
        invite = api(
            member["token"],
            "POST",
            "/v1/api/org/members/invites",
            {"org_id": org_id, "phone": creator.get("phone") or "", "role_code": "super_admin"},
        )
        code = invite.get("code") or ""
        api(creator["token"], "POST", f"/v1/api/org/members/invite-codes/{code}/memberships")
        log("creator accepted the admin invite")
    else:
        log("creator already belongs to the team")
    for label, token in (("member", member["token"]), ("creator", creator["token"])):
        rows = api(token, "GET", f"/v1/api/org/members/work-items/approvers?org={org_id}")
        names = [row.get("name") or row.get("display_name") for row in rows]
        log(f"approvers for {label}: {names}")
        if not names:
            raise RuntimeError(f"{label} has no other approver on {ORG_NAME}")
    return org_id


def pids_for(fragment: str) -> list[int]:
    result = subprocess.run(["pgrep", "-f", fragment], capture_output=True, text=True)
    return [int(line) for line in result.stdout.split() if line.isdigit()]


def main_pid(fragment: str) -> int:
    found = []
    for pid in pids_for(fragment):
        rows = windows(pid)
        if any(row[2] == "ToWork" for row in rows):
            found.append(pid)
    if not found:
        raise RuntimeError(f"no ToWork window for {fragment}")
    return found[0]


def uses_renderer(pid: int) -> bool:
    """WebKit networking is reparented to launchd, so match a nearby WebKit client."""
    listed = subprocess.run(["lsof", "-nP", "-iTCP:4318"], capture_output=True, text=True)
    connected = set()
    for line in listed.stdout.splitlines()[1:]:
        parts = line.split()
        if len(parts) > 1 and parts[1].isdigit():
            connected.add(int(parts[1]))
    if pid in connected:
        return True
    listing = subprocess.run(["ps", "-ax", "-o", "pid=,command="], capture_output=True, text=True)
    for line in listing.stdout.splitlines():
        parts = line.split(None, 1)
        if len(parts) != 2 or not parts[0].isdigit():
            continue
        other = int(parts[0])
        if other in connected and "WebKit" in parts[1] and abs(other - pid) < 80:
            return True
    return False


def lock_holders(storage: str) -> list[int]:
    """Pids holding this storage id's single-instance lock."""
    temp = os.environ.get("TMPDIR") or "/tmp"
    path = Path(temp) / f"dev.a3s.studio.{storage}.lock"
    listed = subprocess.run(["lsof", "-t", "--", str(path)], capture_output=True, text=True)
    return [int(line) for line in listed.stdout.split() if line.isdigit()]


def launch_env(storage: str) -> list[str]:
    """Brand plus instance. The lock file is named from the storage id, not the brand.

    `towork-cua-teamadmin` is brand `towork-cua` and instance `teamadmin`.
    Passing the storage id as A3S_TENANT_SLUG exits before a window exists.
    """
    if storage == "toyou":
        return ["A3S_TENANT_SLUG=toyou"]
    prefix = "towork-cua-"
    if storage.startswith(prefix) and len(storage) > len(prefix):
        return ["A3S_TENANT_SLUG=towork-cua", f"A3S_STUDIO_INSTANCE={storage[len(prefix):]}"]
    raise RuntimeError(f"unknown studio storage {storage}")


def stop_other_fragment(fragment: str, keep: set[int], label: str, slug: str) -> None:
    """Stop this bundle when it claimed another tenant's lock. Leave every other app."""
    targets = []
    for pid in pids_for(fragment):
        if pid in keep:
            continue
        command = subprocess.run(
            ["ps", "-p", str(pid), "-o", "command="], capture_output=True, text=True
        ).stdout
        if fragment not in command:
            continue
        targets.append(pid)
    for pid in targets:
        log(f"stopping {label} pid={pid} for slug={slug}")
        try:
            os.kill(pid, signal.SIGTERM)
        except ProcessLookupError:
            pass
    deadline = time.time() + 8
    while time.time() < deadline:
        live = []
        for pid in targets:
            try:
                os.kill(pid, 0)
            except ProcessLookupError:
                continue
            live.append(pid)
        if not live:
            return
        time.sleep(0.2)
    for pid in targets:
        try:
            os.kill(pid, 0)
        except ProcessLookupError:
            continue
        log(f"stopping {label} pid={pid} did not exit after term")
        try:
            os.kill(pid, signal.SIGKILL)
        except ProcessLookupError:
            pass
    time.sleep(0.3)


def window_is_live(pid: int, title: str) -> bool:
    """True when an on-screen window of this title exposes an accessibility tree."""
    for window_id, on_screen, name in windows(pid):
        if name != title or not on_screen:
            continue
        count, reason = ax_surface(pid, window_id)
        if count > 0 and not reason:
            return True
    return False


def launch_app(fragment: str, app: Path, label: str, operator: int) -> int:
    """Open one signed-in ToWork in the background under its own tenant slug.

    `open` without A3S_TENANT_SLUG claims the default toyou lock, so the other
    account then exits immediately. Does not write a session file.
    """
    slug = app.parent.name
    env_items = launch_env(slug)
    holders = set(lock_holders(slug))
    running = [pid for pid in pids_for(fragment) if pid in holders]
    if running:
        try:
            pid = main_pid(fragment)
        except RuntimeError:
            pid = 0
        if pid in holders:
            set_minimized(pid, "ToWork", False)
            blocking = close_blocking_applet_windows(pid)
            if blocking:
                log(f"{label} applet windows stayed open {blocking}")
            # The member test app can be started again. The creator app is left running.
            if window_is_live(pid, "ToWork") and not (blocking and slug == MEMBER_APP):
                log(f"{label} window already running pid={pid} slug={slug} renderer_4318={uses_renderer(pid)}")
                OWNED.add(pid)
                return pid
            log(f"{label} window needs a fresh process pid={pid}")
            holders = set()
        else:
            log(f"{label} holds slug={slug} without a ToWork window")
    stop_other_fragment(fragment, holders, label, slug)
    time.sleep(0.4)
    command = ["open", "-g"]
    for item in env_items:
        command.extend(["--env", item])
    log_dir = os.environ.get("A3S_GUI_LAUNCH_LOG_DIR")
    if log_dir:
        command.extend(
            [
                "--stdout",
                str(Path(log_dir) / f"{label}-open.out"),
                "--stderr",
                str(Path(log_dir) / f"{label}-open.err"),
            ]
        )
    command.append(str(app))
    log(f"launch {label} storage={slug} env={','.join(env_items)}")
    subprocess.run(command, check=True)
    confirm_pointer(f"open -g {label}")
    deadline = time.time() + 60
    pid = 0
    while time.time() < deadline:
        try:
            candidate = main_pid(fragment)
        except RuntimeError:
            candidate = 0
        if candidate and candidate in set(lock_holders(slug)) and window_is_live(candidate, "ToWork"):
            pid = candidate
            break
        time.sleep(0.4)
    if not pid:
        seen = pids_for(fragment)
        log(f"{label} slug={slug} lock_holders={lock_holders(slug)} fragment_pids={seen}")
        if log_dir:
            err = Path(log_dir) / f"{label}-open.err"
            if err.exists():
                log(f"{label} launch stderr: {err.read_text(errors='replace')[-500:]}")
        raise RuntimeError(f"{label} window did not become accessible")
    OWNED.add(pid)
    keep_front(operator, f"launch {label}")
    front_pid, front_name = front_process()
    log(
        f"{label} pid={pid} slug={slug} lock_holders={lock_holders(slug)} "
        f"renderer_4318={uses_renderer(pid)} frontmost={front_pid} {front_name}"
    )
    blocking = close_blocking_applet_windows(pid)
    if blocking and slug == MEMBER_APP:
        log(f"{label} applet windows stayed open after launch {blocking}")
        raise RuntimeError(f"{label} could not drop applet windows {blocking}")
    return pid


def refresh_account(pid: int, fragment: str, slug: str, label: str, operator: int) -> int:
    """Follow a replaced process. The tenant lock moves when ToWork is relaunched."""
    if window_is_live(pid, "ToWork"):
        return pid
    for holder in lock_holders(slug):
        if window_is_live(holder, "ToWork"):
            log(f"{label} process changed pid={pid} -> {holder}")
            OWNED.add(holder)
            return holder
    log(f"{label} window is gone; opening it again")
    return launch_app(fragment, SUPPORT / slug / "ToWork.app", label, operator)


def keep_front(operator: int, reason: str) -> None:
    """Fail when this drive is in front. Restoring the operator afterwards is not enough.

    The captured operator has to stay in front for the whole drive. Putting
    that process back still counts as taking the front, so the pass line is
    not printed.
    """
    global FRONT_STEALS
    if not operator:
        return
    current, current_name = front_process()
    sample = note_front(current, operator, OWNED)
    if sample == "held":
        return
    if sample == "stolen":
        FRONT_STEALS += 1
        restore_front(operator)
    raise RuntimeError(
        f"frontmost left the operator during {reason}: {current} {current_name}"
    )


def snapshot(ui: Guard, reason: str) -> None:
    try:
        ui.refresh()
    except Exception as error:
        log(f"snapshot {reason} refresh failed: {error}")
        return
    lines = []
    for element in ui.elements:
        role = element.get("role")
        if role in {"AXMenuBar", "AXMenuBarItem"}:
            continue
        label = label_of(element)
        if not label:
            continue
        if role == "AXMenuItem" and ORG_NAME not in label and label not in {"创建团队", "团队", "个人"}:
            continue
        lines.append(f"{role} {label[:140]}")
    degraded = getattr(ui, "degraded_reason", "") or ""
    log(
        f"snapshot {reason} count={len(lines)} elements={len(ui.elements)} "
        f"degraded={degraded[:140]}"
    )
    for line in lines[:80]:
        log("  " + line)


def _warped_onto(before: tuple[int, int], after: tuple[int, int], frame: dict) -> bool:
    """True when the pointer stopped on the control center.

    A 36-point account control fits inside a 24-point radius, so the operator's
    pointer ending on that control is not a warp. A warp stops within 8 points
    of the center.
    """
    try:
        center_x = float(frame["x"]) + float(frame["w"]) / 2
        center_y = float(frame["y"]) + float(frame["h"]) / 2
    except (KeyError, TypeError, ValueError):
        return False
    def offset(point: tuple[int, int]) -> float:
        dx = point[0] - center_x
        dy = point[1] - center_y
        return (dx * dx + dy * dy) ** 0.5
    end = offset(after)
    start = offset(before)
    return end <= 8 and start - end >= 40


def _inside(point: tuple[int, int], frame: dict) -> bool:
    try:
        x = float(frame["x"])
        y = float(frame["y"])
        w = float(frame["w"])
        h = float(frame["h"])
    except (KeyError, TypeError, ValueError):
        return False
    px, py = point
    return x - 4 <= px <= x + w + 4 and y - 4 <= py <= y + h + 4


def click_element(ui: Guard, element: dict, last: bool = False, action: str = "press") -> None:
    """Press a token from the snapshot just taken. A previous snapshot's token is always stale."""
    wanted = label_of(element)
    role = element.get("role")
    origin = element.get("frame") or {}
    last_error: Exception | None = None
    for attempt in range(12):
        ui.refresh()
        fresh = [
            item
            for item in ui.elements
            if label_of(item) == wanted and (role is None or item.get("role") == role)
        ]
        if not fresh and wanted:
            fresh = [
                item
                for item in ui.elements
                if wanted in label_of(item) and (role is None or item.get("role") == role)
            ]
        if not fresh:
            last_error = RuntimeError(f"missing {role or 'control'} {wanted}")
            time.sleep(0.3)
            continue
        if "x" in origin:
            current = min(
                fresh,
                key=lambda item: abs(float((item.get("frame") or {}).get("x", 0)) - float(origin.get("x", 0)))
                + abs(float((item.get("frame") or {}).get("y", 0)) - float(origin.get("y", 0))),
            )
        else:
            current = fresh[-1] if last else fresh[0]
        token = current.get("element_token")
        if not token:
            raise RuntimeError("background click needs an element token")
        frame = current.get("frame") or {}
        before = pointer()
        try:
            ui.call(
                "click",
                {
                    "pid": ui.pid,
                    "element_token": token,
                    "action": action,
                    "delivery_mode": "background",
                },
            )
        except RuntimeError as error:
            last_error = error
            message = str(error)
            if "-25202" in message:
                log(f"click left the tree label={wanted[:40]}")
                return
            if attempt == 11 or not any(sign in message for sign in ("stale", "-25206")):
                raise
            log(f"click retry {attempt + 1} label={wanted[:40]}")
            continue
        after = pointer()
        if after != before:
            landed = _inside(after, frame)
            log(
                f"pointer during click {before} -> {after} "
                f"frame={frame} on_control={landed} label={label_of(current)[:40]}"
            )
            # The operator moves the pointer during a background press. A warp
            # finishes on the control center; passing across the frame does not.
            if landed and _warped_onto(before, after, frame):
                raise RuntimeError("background click moved the pointer onto the control")
        time.sleep(0.35)
        return
    if last_error:
        raise last_error


def candidates(ui: Guard, text: str, role: str | None = None) -> list[dict]:
    needle = " ".join(text.split())
    matches = []
    for element in ui.elements:
        if needle not in label_of(element):
            continue
        if role and element.get("role") != role:
            continue
        matches.append(element)
    return matches


def click_label(ui: Guard, text: str, role: str | None = None, last: bool = False) -> None:
    ui.refresh()
    matches = candidates(ui, text, role)
    if not matches:
        snapshot(ui, f"missing {text}")
        raise AssertionError(f"missing {role or 'control'} {text}")
    web = next((element for element in ui.elements if element.get("role") == "AXWebArea"), None)
    if web is not None:
        area = web.get("frame") or {}
        inside = []
        for element in matches:
            frame = element.get("frame") or {}
            if frame.get("x", -1) >= area.get("x", 0) - 2 and frame.get("y", -1) >= area.get("y", 0) - 2:
                inside.append(element)
        if inside:
            matches = inside
    exact = [element for element in matches if label_of(element) == " ".join(text.split())]
    pool = exact or matches
    if role is None:
        buttons = [element for element in pool if element.get("role") == "AXButton"]
        if buttons:
            pool = buttons
    chosen = pool[-1] if last else pool[0]
    if last and len(pool) > 1:
        log(f"click last of {len(pool)} label={text}")
    click_element(ui, chosen, last=last)


def member_editor_cancel(elements: list[dict]) -> dict | None:
    """The cancel control on a member editor. Saving that sheet would change a role."""
    if not any("编辑「" in label_of(element) for element in elements):
        return None
    cancels = [
        element
        for element in elements
        if element.get("role") == "AXButton" and label_of(element) == "取消"
    ]
    if not cancels:
        return None
    return cancels[-1]


def dismiss_member_editor(ui: Guard) -> bool:
    """Close an open member editor. The role and department stay as they were."""
    ui.refresh()
    cancel = member_editor_cancel(ui.elements)
    if cancel is None:
        return False
    log("closing the member editor without saving")
    click_element(ui, cancel)
    deadline = time.time() + 2
    while time.time() < deadline:
        time.sleep(0.3)
        ui.refresh()
        if member_editor_cancel(ui.elements) is None:
            return True
    return member_editor_cancel(ui.elements) is None


def click_account(ui: Guard) -> None:
    ui.refresh()
    if dismiss_member_editor(ui):
        ui.refresh()
    names = ("林书豪", "RoyLin")
    named = [
        element
        for element in ui.elements
        if element.get("role") in ("AXPopUpButton", "AXButton")
        and any(name in label_of(element) for name in names)
    ]
    avatars = [element for element in named if (element.get("frame") or {}).get("w", 9999) <= 64]
    target = (avatars or named)[:1]
    if not target:
        snapshot(ui, "account control")
        raise AssertionError("account control is missing")
    click_element(ui, target[0])


def team_workbench_button(elements: list[dict]) -> dict | None:
    """The team sidebar entry, not the activity-bar icon that uses the same word.

    The sidebar row is a full-width text button. The activity bar is a square
    icon further left. A wide row wins. When every frame is square, the row
    is the one further right.
    """
    matches = [
        element
        for element in elements
        if element.get("role") == "AXButton" and label_of(element) == "工作台"
    ]
    if not matches:
        return None

    def width(element: dict) -> float:
        try:
            return float((element.get("frame") or {}).get("w", 0))
        except (TypeError, ValueError):
            return 0.0

    def origin_x(element: dict) -> float:
        try:
            return float((element.get("frame") or {}).get("x", 0))
        except (TypeError, ValueError):
            return 0.0

    wide = [element for element in matches if width(element) > 80]
    if wide:
        return max(wide, key=width)
    return max(matches, key=origin_x)


def applet_tile(elements: list[dict], applet: str) -> dict | None:
    """The workbench tile. A same-named row is about 36px tall and does not open a window."""
    tiles = []
    for element in elements:
        if element.get("role") != "AXButton" or label_of(element) != applet:
            continue
        try:
            height = float((element.get("frame") or {}).get("h", 0))
        except (TypeError, ValueError):
            height = 0.0
        if height >= 60:
            tiles.append(element)
    if not tiles:
        return None
    return min(
        tiles,
        key=lambda element: float((element.get("frame") or {}).get("y", 9999) or 9999),
    )


def click_team_workbench(ui: Guard) -> None:
    ui.refresh()
    chosen = team_workbench_button(ui.elements)
    if chosen is None:
        snapshot(ui, "missing 工作台")
        raise AssertionError("missing AXButton 工作台")
    frame = chosen.get("frame") or {}
    log(f"team workbench button w={frame.get('w')} x={frame.get('x')}")
    click_element(ui, chosen)


def click_rail(ui: Guard, text: str) -> None:
    """Press the leftmost narrow activity-bar entry. The account menu uses the same word."""
    ui.refresh()
    matches = []
    for element in ui.elements:
        if element.get("role") != "AXButton" or label_of(element) != text:
            continue
        frame = element.get("frame") or {}
        if float(frame.get("w", 9999)) <= 80:
            matches.append(element)
    if not matches:
        snapshot(ui, f"rail {text}")
        raise AssertionError(f"missing rail {text}")
    matches.sort(key=lambda element: float((element.get("frame") or {}).get("x", 0)))
    click_element(ui, matches[0])


def click_narrow(ui: Guard, text: str) -> None:
    ui.refresh()
    exact = [
        element
        for element in ui.elements
        if element.get("role") == "AXButton"
        and label_of(element) == text
        and (element.get("frame") or {}).get("w", 9999) <= 72
    ]
    if exact:
        click_element(ui, exact[0])
        return
    click_label(ui, text, "AXButton")


def team_heading(ui: Guard, org_name: str) -> bool:
    return any(
        element.get("role") == "AXHeading" and org_name in label_of(element) for element in ui.elements
    )


def org_menu_row(ui: Guard, org_name: str) -> dict | None:
    for element in ui.elements:
        if element.get("role") not in CLICK_ROLES:
            continue
        label = label_of(element)
        if label == org_name or label.startswith(org_name + " "):
            return element
    return None


def click_menu_scope(ui: Guard, text: str) -> bool:
    """Press the account-menu scope tab that sits beside 个人. The activity rail uses the same word."""
    ui.refresh()
    matches = [
        element
        for element in ui.elements
        if label_of(element) == text and element.get("role") in CLICK_ROLES
    ]
    personal = [
        element
        for element in ui.elements
        if label_of(element) == "个人" and element.get("role") in CLICK_ROLES
    ]
    paired = []
    for element in matches:
        box = _box(element)
        if box is None:
            continue
        _ex, ey, _ew, eh = box
        for other in personal:
            other_box = _box(other)
            if other_box is None:
                continue
            _ox, oy, _ow, oh = other_box
            if abs(ey - oy) < 12 and abs(eh - oh) < 16:
                paired.append(element)
                break
    if not paired:
        log(f"menu scope {text} is not open")
        return False
    paired.sort(key=lambda element: float((element.get("frame") or {}).get("x", 0)))
    click_element(ui, paired[0])
    return True


def wait_org_row(ui: Guard, org_name: str, seconds: float = 8) -> bool:
    """Refresh first. A snapshot can outlast the deadline and still contain the row."""
    deadline = time.time() + seconds
    while True:
        ui.refresh()
        if org_menu_row(ui, org_name) is not None:
            return True
        if time.time() >= deadline:
            return False
        time.sleep(0.3)


def account_menu_open(ui: Guard) -> bool:
    """The center switcher is open. A second avatar press closes it.

    Only the dialog title counts. 团队 is also the activity rail, and a joined
    label list makes shorter phrases look present.
    """
    return any(label_of(element) == "切换中心" for element in ui.elements)


def retry_team_menu(ui: Guard, org_name: str, operator: int) -> None:
    """The account menu shows 无法加载团队 when the list request fails. Press 重试."""
    for attempt in range(4):
        ui.refresh()
        if org_menu_row(ui, org_name) is not None:
            return
        labels = " ".join(ui.labels())
        if not account_menu_open(ui) and "无法加载团队" not in labels:
            claim_key_window(ui.pid, "ToWork", operator)
            keep_front(operator, "team menu")
            click_account(ui)
            time.sleep(0.8)
            ui.refresh()
            labels = " ".join(ui.labels())
        if "无法加载团队" not in labels:
            return
        log(f"team menu failed to load; retry {attempt + 1}")
        click_label(ui, "重试", "AXButton")
        keep_front(operator, "team menu retry")
        time.sleep(1.0)
        if wait_org_row(ui, org_name, 8):
            return


def org_page_failed(ui: Guard) -> bool:
    return any(
        element.get("role") == "AXHeading" and "出错了，请重试" in label_of(element)
        for element in ui.elements
    )


def retry_org_page(ui: Guard, org_name: str, operator: str) -> bool:
    """The team rail failed to load the org list. Press 重试 until the heading shows."""
    for attempt in range(4):
        ui.refresh()
        if team_heading(ui, org_name):
            return True
        if not org_page_failed(ui):
            return False
        log(f"org list failed; retrying the team page attempt={attempt + 1}")
        click_label(ui, "重试", "AXButton")
        keep_front(operator, "org retry")
        deadline = time.time() + 8
        while time.time() < deadline:
            ui.refresh()
            if team_heading(ui, org_name):
                return True
            if not org_page_failed(ui):
                break
            time.sleep(0.4)
    ui.refresh()
    return team_heading(ui, org_name)


def click_org(ui: Guard, org_name: str) -> None:
    """Press the team row. The menu can close between the check and this refresh."""
    deadline = time.time() + 3
    while True:
        ui.refresh()
        if team_heading(ui, org_name):
            log(f"team heading {org_name} before the row click")
            return
        matches = []
        for element in ui.elements:
            if element.get("role") not in CLICK_ROLES:
                continue
            label = label_of(element)
            if label == org_name or label.startswith(org_name + " "):
                matches.append(element)
        if matches:
            chosen = matches[0]
            log(f"select team row role={chosen.get('role')} label={label_of(chosen)[:80]}")
            click_element(ui, chosen)
            return
        if time.time() >= deadline:
            break
        time.sleep(0.25)
    snapshot(ui, "org row")
    raise AssertionError(f"missing team row {org_name}")


def dismiss_verification(ui: Guard) -> None:
    """Close an empty personal-verification dialog. Do not submit an identity."""
    blob = " ".join(ui.labels())
    if "个人认证" in blob and "提交认证" in blob:
        log("dismissing personal verification dialog")
        click_label(ui, "取消", "AXButton")
        time.sleep(0.8)


def invite_sheet_open(elements: list[dict]) -> bool:
    """The invite dialog is up. A loading body drops the phone and link labels.

    The members page also says 邀请. That page has no Close control. The dialog
    keeps Close while its body is 加载中 or still lists rows.
    """
    blob = " ".join(label_of(element) for element in elements)
    has_close = any(
        element.get("role") == "AXButton" and label_of(element) == "Close" for element in elements
    )
    if not has_close:
        return False
    if "手机号邀请" in blob or "加入链接" in blob:
        return True
    return "邀请" in blob and ("加载中" in blob or ("第 " in blob and "条" in blob))


def _invite_close(ui: Guard) -> dict | None:
    closes = []
    for element in ui.elements:
        if element.get("role") != "AXButton" or label_of(element) != "Close":
            continue
        frame = element.get("frame") or {}
        try:
            x = float(frame.get("x", 0))
            width = float(frame.get("w", 99))
        except (TypeError, ValueError):
            continue
        if width <= 20 and x < 120:
            continue
        closes.append(element)
    if not closes:
        return None
    return closes[-1]


def _invite_sheet_gone(ui: Guard, seconds: float) -> bool:
    deadline = time.time() + seconds
    while True:
        ui.refresh()
        if not invite_sheet_open(ui.elements):
            return True
        if time.time() >= deadline:
            return False
        time.sleep(0.3)


def dismiss_invite_dialog(ui: Guard) -> bool:
    """Close the team invite sheet. It hides the shell, including the account control.

    The sheet's close control is labeled Close. The window traffic light is 关闭
    and sits at the top left; leave that button alone. The body can drop 手机号邀请
    while Close is still on screen, so missing those words is not a closed sheet.
    """
    ui.refresh()
    if not invite_sheet_open(ui.elements):
        return False
    close = _invite_close(ui)
    if close is None:
        return False
    log("closing the invite dialog")
    click_element(ui, close)
    if _invite_sheet_gone(ui, 1.5):
        return True
    ui.refresh()
    close = _invite_close(ui)
    if close is not None:
        log("invite dialog still open after press; confirming")
        click_element(ui, close, action="confirm")
        if _invite_sheet_gone(ui, 1.5):
            return True
    log("invite dialog still open; sending escape")
    try:
        claim_key_window(ui.pid, "ToWork", OPERATOR[0] or front_process()[0])
        ui.press_background("escape")
    except RuntimeError as error:
        log(f"invite escape refused: {str(error)[:160]}")
    return _invite_sheet_gone(ui, 1.5)


def open_team(ui: Guard, org_name: str, operator: str) -> None:
    deadline = time.time() + 24
    while time.time() < deadline:
        ui.refresh()
        dismiss_verification(ui)
        if dismiss_invite_dialog(ui):
            continue
        if dismiss_member_editor(ui):
            continue
        ui.refresh()
        if member_editor_cancel(ui.elements) is not None:
            time.sleep(0.4)
            continue
        if ui.has("林书豪") or ui.has("RoyLin"):
            break
        time.sleep(0.4)
    else:
        snapshot(ui, "account wait")
        raise AssertionError("account control did not appear")
    # The applet reads the shell center, not this window's heading. Selecting the
    # row publishes that center even when the heading is already the team.
    # A second press on the avatar closes the menu, so only press while it is shut.
    log("publishing the team center")
    if True:
        opened = False
        for attempt in range(4):
            ui.refresh()
            if org_menu_row(ui, org_name) is not None or account_menu_open(ui):
                opened = True
                break
            claim_key_window(ui.pid, "ToWork", operator)
            keep_front(operator, "account menu")
            click_account(ui)
            deadline = time.time() + 4
            while time.time() < deadline:
                time.sleep(0.25)
                ui.refresh()
                if org_menu_row(ui, org_name) is not None or account_menu_open(ui):
                    opened = True
                    break
            if opened:
                log(f"account menu open attempt={attempt + 1}")
                break
            log(f"account menu stayed closed attempt={attempt + 1}")
        if org_menu_row(ui, org_name) is None and account_menu_open(ui):
            labels = " ".join(ui.labels())
            if "加入团队" in labels or "创建团队" not in labels:
                if click_menu_scope(ui, "团队"):
                    wait_org_row(ui, org_name, 6)
        if org_menu_row(ui, org_name) is None:
            retry_team_menu(ui, org_name, operator)
        published = False
        for select_try in range(3):
            if org_menu_row(ui, org_name) is None and not account_menu_open(ui):
                claim_key_window(ui.pid, "ToWork", operator)
                keep_front(operator, "account menu")
                click_account(ui)
                wait_org_row(ui, org_name, 4)
            if org_menu_row(ui, org_name) is None and account_menu_open(ui):
                if click_menu_scope(ui, "团队"):
                    wait_org_row(ui, org_name, 4)
            if org_menu_row(ui, org_name) is None:
                retry_team_menu(ui, org_name, operator)
            if org_menu_row(ui, org_name) is None:
                if team_heading(ui, org_name):
                    published = True
                    log("team menu stayed closed; heading already shows the team")
                    break
                log(f"team row missing on select attempt {select_try + 1}")
                continue
            try:
                click_org(ui, org_name)
            except AssertionError:
                log(f"team row vanished on select attempt {select_try + 1}")
                continue
            keep_front(operator, "team select")
            time.sleep(0.8)
            log("published team center")
            published = True
            break
        if not published and not team_heading(ui, org_name):
            log("account menu did not list the team; opening the team rail")
        deadline = time.time() + 12
        while True:
            ui.refresh()
            if team_heading(ui, org_name):
                break
            if time.time() >= deadline:
                break
            time.sleep(0.3)
        if not team_heading(ui, org_name):
            log("opening the team rail after the menu")
            click_rail(ui, "团队")
            keep_front(operator, "team rail")
            deadline = time.time() + 12
            while True:
                ui.refresh()
                if team_heading(ui, org_name):
                    break
                if time.time() >= deadline:
                    break
                time.sleep(0.3)
        if not team_heading(ui, org_name) and not retry_org_page(ui, org_name, operator):
            log("org list stayed failed; selecting the team again")
            retry_team_menu(ui, org_name, operator)
            if org_menu_row(ui, org_name) is not None:
                click_org(ui, org_name)
                keep_front(operator, "team select")
                time.sleep(0.8)
            click_rail(ui, "团队")
            keep_front(operator, "team rail")
            retry_org_page(ui, org_name, operator)
        if not team_heading(ui, org_name):
            snapshot(ui, "team page")
            raise AssertionError(f"team page is not {org_name}")
        log(f"team heading {org_name}")
    if not ui.has("工作台"):
        click_narrow(ui, "团队")
        time.sleep(0.6)
        ui.refresh()
    click_team_workbench(ui)
    deadline = time.time() + 8
    while True:
        ui.refresh()
        if any(element.get("role") == "AXButton" and label_of(element) == "审批" for element in ui.elements):
            break
        if time.time() >= deadline:
            snapshot(ui, "workbench")
            raise AssertionError("workbench did not show 审批")
        time.sleep(0.3)
    keep_front(operator, "workbench")


def day_needle(day: datetime.date) -> str:
    return f"{day.year}年{day.month}月{day.day}日"


def click_opener(ui: Guard, field: str) -> None:
    ui.refresh()
    buttons = [
        element
        for element in ui.elements
        if element.get("role") == "AXButton" and field in label_of(element)
    ]
    if not buttons:
        buttons = [
            element
            for element in ui.elements
            if element.get("role") == "AXButton" and "选择日期" in label_of(element)
        ]
        if field == "结束日期" and len(buttons) > 1:
            buttons = [buttons[-1]]
        elif buttons:
            buttons = [buttons[0]]
    if not buttons:
        snapshot(ui, f"date {field}")
        raise AssertionError(f"missing date opener {field}")
    click_element(ui, buttons[0])


def calendar_open(ui: Guard) -> bool:
    return any(
        element.get("role") == "AXButton" and label_of(element) == "上个月" for element in ui.elements
    )


def _box(element: dict) -> tuple[float, float, float, float] | None:
    frame = element.get("frame") or {}
    try:
        return float(frame["x"]), float(frame["y"]), float(frame["w"]), float(frame["h"])
    except (KeyError, TypeError, ValueError):
        return None


def find_day(ui: Guard, day: datetime.date) -> dict:
    needle = day_needle(day)
    number = str(day.day)
    roles = {"AXButton", "AXCell"}
    buttons = [
        element
        for element in ui.elements
        if element.get("role") in roles and needle in label_of(element)
    ]
    if not buttons:
        buttons = [
            element
            for element in ui.elements
            if element.get("role") in roles and label_of(element) == number and element.get("enabled") is not False
        ]
    tables = [box for element in ui.elements if element.get("role") == "AXTable" and (box := _box(element))]
    if tables:
        boxed = []
        for button in buttons:
            point = _frame_point(button)
            if point is None:
                continue
            px, py = point
            if any(x <= px <= x + w and y <= py <= y + h for x, y, w, h in tables):
                boxed.append(button)
        if boxed:
            buttons = boxed
    day_buttons = [button for button in buttons if button.get("role") == "AXButton"]
    if day_buttons:
        buttons = day_buttons
    if not buttons:
        snapshot(ui, f"day {needle}")
        raise AssertionError(f"missing day {needle}")
    return buttons[0]


def click_day(ui: Guard, day: datetime.date) -> dict:
    ui.refresh()
    chosen = find_day(ui, day)
    click_element(ui, chosen)
    return chosen


def calendar_closed(ui: Guard, seconds: float) -> bool:
    """True when the day grid closes on its own. Closing it from the opener does not select a day."""
    deadline = time.time() + seconds
    while time.time() < deadline:
        ui.refresh()
        if not calendar_open(ui):
            return True
        time.sleep(0.25)
    return False


def press_day_key(ui: Guard, day: datetime.date, key: str) -> None:
    """Send Enter or Space to the day button. The button selects on those keys."""
    ui.refresh()
    chosen = find_day(ui, day)
    token = chosen.get("element_token")
    if not token:
        raise RuntimeError("day button has no element token")
    result = ui.call(
        "press_key",
        {
            "pid": ui.pid,
            "window_id": ui.window_id,
            "element_token": token,
            "key": key,
            "delivery_mode": "background",
        },
    )
    structured = result.get("structuredContent") or {}
    window_name, role, _value = focused_control(ui.pid)
    log(
        f"date key={key} effect={structured.get('effect')} "
        f"focused={window_name} role={role}"
    )


def settle_calendar(ui: Guard, field: str) -> None:
    """A single-day popover stays up when the day press does not select. Close it before submit."""
    deadline = time.time() + 3
    while time.time() < deadline:
        ui.refresh()
        if not calendar_open(ui):
            return
        time.sleep(0.3)
    log("closing the date calendar")
    click_opener(ui, field)
    ui.refresh()


def date_control_text(ui: Guard, field: str) -> str:
    """Visible date-button text. The accessible name stays the label even after a day is chosen."""
    buttons = [
        element
        for element in ui.elements
        if element.get("role") == "AXButton" and field in label_of(element)
    ]
    if not buttons:
        return ""
    button = max(buttons, key=lambda item: (item.get("frame") or {}).get("w", 0))
    parts = [
        str(button.get("value") or ""),
        str(button.get("description") or ""),
        str(button.get("title") or ""),
        label_of(button),
    ]
    return " ".join(part for part in parts if part)


def date_shows(ui: Guard, field: str) -> bool:
    """True when the field or any control shows today's ISO day."""
    ui.refresh()
    shown = date_control_text(ui, field)
    if re.search(r"\d{4}-\d{2}-\d{2}", shown):
        log(f"date field={field} shows {shown}")
        return True
    today = datetime.date.today().isoformat()
    for element in ui.elements:
        blob = " ".join(
            str(element.get(key) or "") for key in ("label", "value", "title", "description")
        )
        if today in blob or today in label_of(element):
            log(f"date field={field} shows {today}")
            return True
    return False


def pick_today(ui: Guard, field: str) -> bool:
    """Press the plain Today button. The single-day grid does not take an AX press."""
    ui.refresh()
    matches = [
        element
        for element in ui.elements
        if element.get("role") == "AXButton" and label_of(element) in {"今天", "Today"}
    ]
    if not matches:
        log(f"date field={field} has no today button")
        return False
    click_element(ui, matches[0])
    deadline = time.time() + 2
    while time.time() < deadline:
        if date_shows(ui, field):
            log(f"date field={field} selected by today")
            return True
        time.sleep(0.25)
    log(f"date field={field} today did not store a day shown={date_control_text(ui, field)!r}")
    return False


def pick_day(ui: Guard, field: str) -> None:
    """A selected single day closes its popover. The button name stays the field label.

    A pointer press on this grid often does not run the day action. The Today
    button does. Enter and Space remain the fallback once the applet window is
    the process key window.
    """
    if pick_today(ui, field):
        return
    today = datetime.date.today()
    if not calendar_open(ui):
        click_opener(ui, field)
    target_title = next((row[2] for row in windows(ui.pid) if row[0] == ui.window_id), "")
    operator = OPERATOR[0] or front_process()[0]
    if target_title:
        claim_key_window(ui.pid, target_title, operator)
    for attempt in range(4):
        ui.refresh()
        if not calendar_open(ui):
            click_opener(ui, field)
            ui.refresh()
        if not calendar_open(ui):
            raise AssertionError(f"date {field} calendar did not open")
        click_day(ui, today)
        if day_chosen(ui, field):
            log(f"date field={field} selected by press")
            return
        for key in ("space", "return"):
            try:
                press_day_key(ui, today, key)
            except RuntimeError as error:
                log(f"date key {key} refused: {str(error)[:160]}")
                continue
            log(f"date field={field} key={key} attempt={attempt + 1}")
            if day_chosen(ui, field):
                log(f"date field={field} selected by {key}")
                return
    raise AssertionError(f"date {field} did not select")


def day_chosen(ui: Guard, field: str) -> bool:
    """True when an open day grid closes itself or the field shows an ISO day.

    The grid is open when this is called. Closing it from the opener is not a
    selection. A grid that closes on its own stored the day. An ISO day on the
    control is that stored day even if the grid is still up.
    """
    ui.refresh()
    if not calendar_open(ui):
        return True
    if calendar_closed(ui, 1.2):
        return True
    ui.refresh()
    shown = date_control_text(ui, field)
    if not re.search(r"\d{4}-\d{2}-\d{2}", shown):
        return False
    log(f"date field={field} shows {shown}")
    if calendar_open(ui):
        log("closing the date calendar after the day was stored")
        click_opener(ui, field)
    return True


def pick_range(ui: Guard) -> None:
    today = datetime.date.today()
    tomorrow = today + datetime.timedelta(days=1)
    click_opener(ui, "开始日期")
    click_day(ui, today)
    ui.refresh()
    if not any(day_needle(tomorrow) in label_of(element) for element in ui.elements):
        click_opener(ui, "结束日期")
    click_day(ui, tomorrow)


def _choice_opener(ui: Guard, field: str) -> dict | None:
    matches = []
    for element in ui.elements:
        if element.get("role") not in CLICK_ROLES:
            continue
        label = label_of(element)
        if label == field or label.startswith(field + " "):
            matches.append(element)
    if not matches and len(field) > 2:
        matches = [
            element
            for element in ui.elements
            if element.get("role") in CLICK_ROLES and field in label_of(element)
        ]
    return matches[0] if matches else None


def _frame_of(element: dict) -> dict:
    return element.get("frame") or {}


def _frame_contains(outer: dict, inner: dict) -> bool:
    """True when inner sits inside outer. A menu row below the field does not."""
    try:
        left = float(outer["x"])
        top = float(outer["y"])
        right = left + float(outer["w"])
        bottom = top + float(outer["h"])
        inner_left = float(inner["x"])
        inner_top = float(inner["y"])
        inner_right = inner_left + float(inner["w"])
        inner_bottom = inner_top + float(inner["h"])
    except (KeyError, TypeError, ValueError):
        return False
    return (
        inner_left >= left - 2
        and inner_top >= top - 2
        and inner_right <= right + 2
        and inner_bottom <= bottom + 2
    )


def _menu_row(ui: Guard, field: str, option: str) -> dict | None:
    """A menu row outside the field. WebKit exposes the option as static text.

    The selected value drawn inside the closed field is not a row. A menu bar
    item with no frame is not a row either.
    """
    opener = _choice_opener(ui, field)
    opener_frame = _frame_of(opener) if opener else {}
    rows = []
    for element in ui.elements:
        role = element.get("role")
        if role not in CLICK_ROLES and role != "AXStaticText":
            continue
        if label_of(element) != option:
            continue
        frame = _frame_of(element)
        if not frame.get("w") or not frame.get("h"):
            continue
        if opener_frame and (
            frame == opener_frame or _frame_contains(opener_frame, frame)
        ):
            continue
        rows.append(element)
    if opener_frame and rows:
        rows.sort(
            key=lambda element: abs(
                float(_frame_of(element).get("y", 0)) - float(opener_frame.get("y", 0))
            )
        )
    return rows[0] if rows else None


def _choice_selected(ui: Guard, field: str, option: str) -> bool:
    """The field's own label shows the option. An open row does not cancel that."""
    opener = _choice_opener(ui, field)
    if opener is None:
        return False
    blob = " ".join(
        str(opener.get(key) or "") for key in ("label", "value", "title", "description")
    )
    if option in blob:
        return True
    frame = _frame_of(opener)
    for element in ui.elements:
        if element.get("role") in CLICK_ROLES or option not in label_of(element):
            continue
        if frame and _frame_contains(frame, _frame_of(element)):
            return True
    return False


def _poll_choice(ui: Guard, field: str, option: str, seconds: float) -> str:
    """Wait until the field shows the option or the menu row is visible."""
    deadline = time.time() + seconds
    while True:
        ui.refresh()
        if _choice_selected(ui, field, option):
            return "selected"
        if _menu_row(ui, field, option) is not None:
            return "row"
        if time.time() >= deadline:
            return ""
        time.sleep(0.2)


def choose(ui: Guard, field: str, option: str) -> None:
    """Open a menu and press one row. A closed field keeps the selected label."""
    deadline = time.time() + 16
    last: Exception | None = None
    while time.time() < deadline:
        ui.refresh()
        if _choice_selected(ui, field, option):
            log(f"chose {field}={option}")
            return
        row = _menu_row(ui, field, option)
        if row is None:
            opener = _choice_opener(ui, field)
            if opener is None:
                last = AssertionError(f"missing choice {field}")
                time.sleep(0.25)
                continue
            try:
                click_element(ui, opener)
            except RuntimeError as error:
                last = error
                log(f"choice {field}={option} retry: {str(error)[:120]}")
                continue
            seen = _poll_choice(ui, field, option, 1.6)
            if seen == "selected":
                log(f"chose {field}={option}")
                return
            if seen == "row":
                continue
            last = AssertionError(f"missing option {option}")
            continue
        try:
            log(f"choice row {field}={option} role={row.get('role')}")
            click_element(ui, row)
        except RuntimeError as error:
            last = error
            log(f"choice {field}={option} retry: {str(error)[:120]}")
            continue
        if _poll_choice(ui, field, option, 1.6) == "selected":
            log(f"chose {field}={option}")
            return
        log(f"choice {field}={option} still open")
    snapshot(ui, f"choice {field}={option}")
    if last:
        raise last
    raise AssertionError(f"missing option {option}")


def allow_permission(ui: Guard) -> None:
    ui.refresh()
    if ui.has("允许") and ui.has("申请使用以下权限") or (ui.has("允许") and ui.has("用户信息授权")):
        click_label(ui, "允许", "AXButton")
        time.sleep(0.4)
        ui.refresh()


def close_other_on_screen(pid: int, keep: int) -> None:
    """Close extra on-screen windows so background typing can prove its target."""
    for window_id, on_screen, title in windows(pid):
        if not on_screen or window_id == keep or title in {"ToWork", ""}:
            continue
        if "iris" in title.lower() or title.startswith("3DR"):
            continue
        log(f"closing extra window {title} id={window_id}")
        extra = Guard(pid, window_id)
        try:
            extra.refresh()
            close = [
                element
                for element in extra.elements
                if element.get("role") == "AXButton" and label_of(element) == "关闭"
            ]
            if not close or not close[0].get("element_token"):
                if press_named_button(pid, title, "关闭"):
                    continue
                log(f"extra window {title} has no close control")
                continue
            extra.call(
                "click",
                {
                    "pid": pid,
                    "element_token": close[0]["element_token"],
                    "action": "press",
                    "delivery_mode": "background",
                },
            )
        except Exception as error:
            log(f"extra window {title} stayed open: {error}")
        finally:
            extra.close()


def ax_surface(pid: int, window_id: int) -> tuple[int, str]:
    """Return a live element count. A compositor ghost has no accessibility window."""
    ui = Guard(pid, window_id)
    try:
        try:
            result = ui.call("get_window_state", {"pid": pid, "window_id": window_id})
        except RuntimeError as error:
            return 0, str(error)[:160]
        structured = result.get("structuredContent") or {}
        elements = structured.get("elements") or []
        count = structured.get("element_count")
        if not isinstance(count, int):
            count = len(elements)
        reason = structured.get("degraded_reason") or ""
        return count, str(reason)
    finally:
        ui.close()


def wait_for_window(pid: int, title: str, before: set[int], timeout: float = 25) -> int:
    """Bind an on-screen window that currently exposes an accessibility tree."""
    deadline = time.time() + timeout
    rejected: set[int] = set()
    while True:
        rows = windows(pid)
        titled = [row for row in rows if row[2] == title and row[1]]
        titled.sort(key=lambda row: (row[0] in before, row[0] in rejected, -row[0]))
        if titled:
            window_id = titled[0][0]
            count, reason = ax_surface(pid, window_id)
            if count > 0 and not reason:
                log(f"window {title} id={window_id} on_screen=True elements={count}")
                return window_id
            rejected.add(window_id)
            log(f"window {title} id={window_id} ax rejected elements={count} reason={reason[:120]}")
        if time.time() >= deadline:
            break
        time.sleep(0.3)
    raise RuntimeError(f"{title} window did not open: {windows(pid)}")


def close_build_overlay(ui: Guard, operator: int) -> None:
    """Escape closes the dev-server error sheet. It does not move the pointer."""
    if not ui.has("Build failed"):
        return
    target_title = next((row[2] for row in windows(ui.pid) if row[0] == ui.window_id), "")
    log("closing renderer build overlay")
    if target_title:
        ensure_isolated(ui.pid, target_title)
        claim_key_window(ui.pid, target_title, operator)
    try:
        ui.call(
            "press_key",
            {
                "pid": ui.pid,
                "window_id": ui.window_id,
                "key": "escape",
                "delivery_mode": "background",
            },
        )
    except RuntimeError as error:
        log(f"build overlay dismiss failed: {str(error)[:180]}")
        return
    keep_front(operator, "build overlay")
    deadline = time.time() + 4
    while time.time() < deadline:
        time.sleep(0.3)
        ui.refresh()
        if not ui.has("Build failed"):
            log("renderer build overlay closed")
            return
    log("renderer build overlay stayed open")


PLACEHOLDER_VALUES = {
    "例如：采购申请",
    "写清要审批的事项",
    "选择请假类型",
    "写清请假原因",
    "例如：上海",
    "写清出差事由",
    "0.00",
    "选择类别",
    "写清报销事由",
    "可选，关联已批准的出差",
    "例如：周例会",
    "选择时间",
    "例如：三楼会议室",
    "写清会议议程",
    "例如：采购合同",
    "选择印章",
    "1–99",
    "写清用印事由",
}


def draft_text(element: dict) -> str:
    """User-entered field text. A label or a catalog placeholder is an empty control."""
    value = str(element.get("value") or "").strip()
    if not value or value == label_of(element) or value in PLACEHOLDER_VALUES:
        return ""
    return value


def value_holds(element: dict, text: str) -> bool:
    """True when entered text contains text. A placeholder that merely contains the word does not."""
    entered = draft_text(element)
    return bool(entered) and text in entered


def dirty_values(ui: Guard) -> list[str]:
    found = []
    for element in ui.elements:
        if element.get("role") not in ("AXTextArea", "AXTextField"):
            continue
        text = draft_text(element)
        if text:
            found.append(text[:40])
    return found


def field_text_count(ui: Guard) -> int:
    return len(dirty_values(ui))


def prepare_form(ui: Guard) -> None:
    """Reload a reused applet that still shows an old draft or a failed submit."""
    wait_for_empty_fields(ui)
    ui.refresh()
    if not ui.has("提交失败") and field_text_count(ui) == 0:
        return
    log(f"refreshing applet form residue={dirty_values(ui)[:3]}")
    click_label(ui, "刷新", "AXButton")
    deadline = time.time() + 12
    while time.time() < deadline:
        time.sleep(0.4)
        ui.refresh()
        ready = form_marker_present(ui)
        if ready and not ui.has("提交失败") and field_text_count(ui) == 0:
            log("applet form refreshed")
            return
    snapshot(ui, "refresh")
    log(f"applet form still has text {dirty_values(ui)[:3]}")
    raise RuntimeError("applet form did not refresh")


LOST_TREE = "applet window lost its accessibility tree"
FORM_MARKERS = ("标题", "请假类型", "目的地", "金额", "会议主题", "文件名称", "提交")


def form_markers_visible(labels: list[str]) -> bool:
    """A published office form. A window title alone is not a form."""
    blob = "\n".join(labels)
    return any(marker in blob for marker in FORM_MARKERS)


def form_marker_present(ui: Guard) -> bool:
    return form_markers_visible(ui.labels())


def lost_window(ui: Guard) -> bool:
    """An off-screen compositor ghost has a CG id and an empty accessibility tree."""
    reason = getattr(ui, "degraded_reason", "") or ""
    return any(token in reason for token in ("unresolved", "not a live window", "no live frame"))


def wait_for_empty_fields(ui: Guard) -> None:
    """A reused applet reloads after it is shown again. Type into the fresh form.

    A vanished accessibility tree is not an empty form. The caller opens the applet again.
    """

    def filled() -> int:
        return field_text_count(ui)

    ui.refresh()
    if lost_window(ui):
        raise RuntimeError(LOST_TREE)
    if filled() == 0 and form_marker_present(ui):
        return
    deadline = time.time() + 8
    while time.time() < deadline:
        time.sleep(0.4)
        ui.refresh()
        if lost_window(ui):
            raise RuntimeError(LOST_TREE)
        if filled() == 0 and form_marker_present(ui):
            log("applet form reloaded empty")
            return
    if lost_window(ui) or not ui.elements or not form_marker_present(ui):
        raise RuntimeError(LOST_TREE)
    log("applet form kept previous text")


def submit_button(elements: list[dict]) -> dict | None:
    matches = [
        element
        for element in elements
        if element.get("role") == "AXButton" and label_of(element) == "提交"
    ]
    return matches[0] if matches else None


def approver_list_empty(blob: str) -> bool:
    """The empty-team sentence. A ready approver line uses the full-width colon."""
    return "没有其他审批人" in blob and "审批人：" not in blob


def submit_click_allowed(button: dict | None, blob: str, waited: float) -> bool:
    """Press 提交 only after the approver gate has enabled it.

    The empty-team sentence also contains 审批人, without the full-width colon
    used by the ready line. A control whose enabled flag is still missing can
    be pressed after it has stayed on screen.
    """
    if button is None or approver_list_empty(blob):
        return False
    enabled = button.get("enabled")
    if enabled is False:
        return False
    if enabled is True or "审批人：" in blob:
        return True
    return waited >= 5


def submit_applet(pid: int, applet: str, fill, operator: str) -> str:
    ui: Guard | None = None
    item_id = ""
    try:
        for open_try in range(3):
            main = bind(pid, "ToWork")
            try:
                main.refresh()
                if applet_tile(main.elements, applet) is None:
                    dismiss_invite_dialog(main)
                    main.refresh()
                if applet_tile(main.elements, applet) is None:
                    wide = team_workbench_button(main.elements)
                    frame = (wide or {}).get("frame") or {}
                    try:
                        wide_enough = float(frame.get("w", 0)) > 80
                    except (TypeError, ValueError):
                        wide_enough = False
                    if wide is not None and wide_enough:
                        log(f"opening 工作台 for {applet}")
                        click_element(main, wide)
                        keep_front(operator, f"workbench {applet}")
                        time.sleep(0.6)
                        main.refresh()
                if applet_tile(main.elements, applet) is None:
                    log(f"workbench lost {applet}; selecting the team again")
                    open_team(main, ORG_NAME, operator)
                before = {row[0] for row in windows(pid)}
                main.refresh()
                tile = applet_tile(main.elements, applet)
                if tile is None:
                    log(f"missing {applet} on open attempt {open_try + 1}")
                    if open_try == 2:
                        raise AssertionError(f"missing AXButton {applet}")
                    continue
                frame = tile.get("frame") or {}
                log(f"applet tile {applet} h={frame.get('h')} y={frame.get('y')}")
                click_element(main, tile)
                keep_front(operator, f"open {applet}")
            finally:
                main.close()
            try:
                window = wait_for_window(pid, applet, before)
            except RuntimeError as error:
                log(f"{applet} window did not stay open: {error}")
                if open_try == 2:
                    raise
                continue
            close_other_on_screen(pid, window)
            ui = Guard(pid, window)
            try:
                deadline = time.time() + 20
                lost = 0
                while True:
                    ui.refresh()
                    if lost_window(ui):
                        lost += 1
                        if lost >= 2:
                            raise RuntimeError(LOST_TREE)
                        time.sleep(0.3)
                        continue
                    lost = 0
                    if ui.has("Build failed"):
                        close_build_overlay(ui, operator)
                        continue
                    if ui.has("允许") or form_marker_present(ui):
                        break
                    if time.time() >= deadline:
                        break
                    time.sleep(0.3)
                allow_permission(ui)
                if not form_marker_present(ui) and not ui.has("允许"):
                    log(f"{applet} form had no fields; opening it again")
                    ui.close()
                    ui = None
                    close_titled_window(pid, applet)
                    if open_try == 2:
                        raise RuntimeError(f"{applet} form did not publish")
                    continue
                prepare_form(ui)
                with keyboard_hold(pid, operator):
                    fill(ui)
                item_id = ""
                clicks = 0
                empty_reloads = 0
                last_click = 0.0
                seen_button: float | None = None
                deadline = time.time() + 36
                while time.time() < deadline:
                    ui.refresh()
                    if calendar_open(ui):
                        log("closing the date calendar before submit")
                        for field in ("日期", "开始日期", "结束日期"):
                            if any(
                                element.get("role") == "AXButton" and label_of(element) == field
                                for element in ui.elements
                            ):
                                click_opener(ui, field)
                                break
                        ui.refresh()
                    blob = "\n".join(ui.labels())
                    found = UUID_RE.findall(blob)
                    if found and ("已提交" in blob or "消息中心" in blob):
                        item_id = found[-1]
                        break
                    if approver_list_empty(blob):
                        if empty_reloads >= 2:
                            snapshot(ui, f"{applet} no approver")
                            raise RuntimeError(f"{applet} submit refused")
                        empty_reloads += 1
                        log(f"{applet} approver list empty; reloading the form")
                        click_label(ui, "刷新", "AXButton")
                        time.sleep(1.2)
                        prepare_form(ui)
                        with keyboard_hold(pid, operator):
                            fill(ui)
                        clicks = 0
                        last_click = 0.0
                        seen_button = None
                        deadline = time.time() + 36
                        continue
                    button = submit_button(ui.elements)
                    now = time.time()
                    if button is None:
                        seen_button = None
                    elif seen_button is None:
                        seen_button = now
                    waited = 0.0 if seen_button is None else now - seen_button
                    if (
                        button is not None
                        and submit_click_allowed(button, blob, waited)
                        and clicks < 3
                        and now - last_click >= 4
                    ):
                        click_element(ui, button)
                        clicks += 1
                        last_click = now
                        log(f"{applet} submit click {clicks} enabled={button.get('enabled')}")
                    time.sleep(0.4)
                if not item_id:
                    snapshot(ui, f"{applet} refused")
                    raise RuntimeError(f"{applet} submit refused")
                break
            except RuntimeError as error:
                ui.close()
                ui = None
                if LOST_TREE not in str(error) or open_try == 2:
                    raise
                log(f"{applet} window dropped; opening it again")
        if ui is None or not item_id:
            snapshot(ui, f"{applet} no item") if ui is not None else None
            raise RuntimeError(f"{applet} did not show an item id")
        log(f"submitted {applet} item={item_id}")
        keep_front(operator, f"submit {applet}")
        return item_id
    finally:
        if ui is not None:
            ui.close()


def _frame_point(element: dict) -> tuple[float, float] | None:
    frame = element.get("frame") or {}
    try:
        return float(frame.get("x")), float(frame.get("y"))
    except (TypeError, ValueError):
        return None


def field_controls(ui: Guard, field: str) -> list[dict]:
    """Text controls for one office label.

    WebKit sometimes exposes the visible label only as nearby static text.
    The input then keeps its placeholder, which does not contain the label.
    """
    needle = " ".join(field.split())
    controls = []
    text_roles = ("AXTextArea", "AXTextField")
    for element in ui.elements:
        if element.get("role") not in text_roles:
            continue
        blob = " ".join(
            str(element.get("label") or "").split() + str(element.get("value") or "").split()
        )
        if needle in blob:
            controls.append(element)
    if controls:
        return controls
    anchors = [
        element
        for element in ui.elements
        if element.get("role") == "AXStaticText"
        and (label_of(element) == needle or label_of(element).startswith(needle + " "))
        and _frame_point(element) is not None
    ]
    if not anchors:
        return []
    anchor = anchors[0]
    ax, ay = _frame_point(anchor) or (0.0, 0.0)
    below = []
    for element in ui.elements:
        if element.get("role") not in text_roles:
            continue
        point = _frame_point(element)
        if point is None:
            continue
        x, y = point
        if y + 8 < ay or abs(x - ax) > 280:
            continue
        below.append((y - ay, abs(x - ax), element))
    if not below:
        return []
    below.sort(key=lambda item: (item[0], item[1]))
    return [below[0][2]]


def protected_window(title: str) -> bool:
    return "iris" in title.lower() or title.startswith("3DR")


def ax_window_states(pid: int) -> list[tuple[str, str]]:
    """Every System Events window of this pid, as (name, minimized flag)."""
    script = (
        'tell application "System Events"\n'
        f"  tell (first process whose unix id is {int(pid)})\n"
        '    set report to ""\n'
        "    repeat with w in windows\n"
        '      set n to ""\n'
        "      try\n"
        "        set n to name of w as text\n"
        "      end try\n"
        '      set m to "unknown"\n'
        "      try\n"
        '        set m to (value of attribute "AXMinimized" of w) as text\n'
        "      end try\n"
        '      set report to report & m & tab & n & linefeed\n'
        "    end repeat\n"
        "    return report\n"
        "  end tell\n"
        "end tell"
    )
    result = subprocess.run(["osascript", "-e", script], capture_output=True, text=True)
    if result.returncode != 0:
        detail = " ".join((result.stderr or result.stdout).split())
        raise RuntimeError(f"cannot list accessibility windows: {detail[:180]}")
    rows: list[tuple[str, str]] = []
    for line in result.stdout.splitlines():
        if not line.strip():
            continue
        flag, sep, name = line.partition("\t")
        if not sep:
            continue
        rows.append((name, flag.strip().lower()))
    return rows


def applescript_string(value: str) -> str:
    return '"' + value.replace("\\", "\\\\").replace('"', '\\"') + '"'


def minimize_names(pid: int, names: list[str], minimized: bool) -> None:
    """Set AXMinimized on every window whose name is in names. Iris stays untouched."""
    unique = [name for name in dict.fromkeys(names) if not protected_window(name)]
    if not unique:
        return
    flag = "true" if minimized else "false"
    literals = ", ".join(applescript_string(name) for name in unique)
    script = (
        'tell application "System Events"\n'
        f"  tell (first process whose unix id is {int(pid)})\n"
        f"    set targets to {{{literals}}}\n"
        "    repeat with w in windows\n"
        '      set n to ""\n'
        "      try\n"
        "        set n to name of w as text\n"
        "      end try\n"
        "      if targets contains n then\n"
        "        try\n"
        f'          set value of attribute "AXMinimized" of w to {flag}\n'
        "        end try\n"
        "      end if\n"
        "    end repeat\n"
        "  end tell\n"
        "end tell"
    )
    result = subprocess.run(["osascript", "-e", script], capture_output=True, text=True)
    if result.returncode != 0:
        detail = " ".join((result.stderr or result.stdout).split())
        raise RuntimeError(f"window minimize={minimized} failed for {unique}: {detail[:180]}")
    log(f"windows {unique} minimized={minimized}")


def set_minimized(pid: int, title: str, minimized: bool) -> bool:
    """Minimize every window of this name. A minimized accessibility window is not a keyboard destination."""
    if protected_window(title):
        log(f"window {title} left visible")
        return False
    try:
        minimize_names(pid, [title], minimized)
        ready = "true" if minimized else "false"
        return any(name == title and flag == ready for name, flag in ax_window_states(pid))
    except RuntimeError as error:
        log(str(error)[:180])
        return False


_CF = ctypes.CDLL("/System/Library/Frameworks/CoreFoundation.framework/CoreFoundation")
_AX = ctypes.CDLL("/System/Library/Frameworks/ApplicationServices.framework/ApplicationServices")
_CF_UTF8 = 0x08000100
_CF.CFStringCreateWithCString.argtypes = [ctypes.c_void_p, ctypes.c_char_p, ctypes.c_uint32]
_CF.CFStringCreateWithCString.restype = ctypes.c_void_p
_CF.CFRelease.argtypes = [ctypes.c_void_p]
_CF.CFGetTypeID.argtypes = [ctypes.c_void_p]
_CF.CFGetTypeID.restype = ctypes.c_ulong
_CF.CFStringGetTypeID.restype = ctypes.c_ulong
_CF.CFArrayGetTypeID.restype = ctypes.c_ulong
_CF.CFStringGetLength.argtypes = [ctypes.c_void_p]
_CF.CFStringGetLength.restype = ctypes.c_long
_CF.CFStringGetCString.argtypes = [ctypes.c_void_p, ctypes.c_char_p, ctypes.c_long, ctypes.c_uint32]
_CF.CFStringGetCString.restype = ctypes.c_byte
_CF.CFArrayGetCount.argtypes = [ctypes.c_void_p]
_CF.CFArrayGetCount.restype = ctypes.c_long
_CF.CFArrayGetValueAtIndex.argtypes = [ctypes.c_void_p, ctypes.c_long]
_CF.CFArrayGetValueAtIndex.restype = ctypes.c_void_p
_AX.AXUIElementCreateApplication.argtypes = [ctypes.c_int32]
_AX.AXUIElementCreateApplication.restype = ctypes.c_void_p
_AX.AXUIElementCopyAttributeValue.argtypes = [
    ctypes.c_void_p,
    ctypes.c_void_p,
    ctypes.POINTER(ctypes.c_void_p),
]
_AX.AXUIElementCopyAttributeValue.restype = ctypes.c_int32
_AX.AXUIElementPerformAction.argtypes = [ctypes.c_void_p, ctypes.c_void_p]
_AX.AXUIElementPerformAction.restype = ctypes.c_int32
_AX_STRING = _CF.CFStringGetTypeID()
_AX_ARRAY = _CF.CFArrayGetTypeID()


def _cf_string(text: str):
    return _CF.CFStringCreateWithCString(None, text.encode(), _CF_UTF8)


def _cf_text(value) -> str:
    if not value or _CF.CFGetTypeID(value) != _AX_STRING:
        return ""
    size = _CF.CFStringGetLength(value) * 4 + 1
    buf = ctypes.create_string_buffer(size)
    if _CF.CFStringGetCString(value, buf, size, _CF_UTF8) != 1:
        return ""
    return buf.value.decode()


def _ax_copy(element, name: str):
    key = _cf_string(name)
    try:
        out = ctypes.c_void_p()
        err = _AX.AXUIElementCopyAttributeValue(element, key, ctypes.byref(out))
        if err != 0 or not out.value:
            return None
        return out.value
    finally:
        _CF.CFRelease(key)


def _ax_label(element) -> str:
    for attribute in ("AXTitle", "AXDescription"):
        value = _ax_copy(element, attribute)
        if not value:
            continue
        text = _cf_text(value)
        _CF.CFRelease(value)
        if text:
            return text
    return ""


def _press_walk(element, name: str, depth: int) -> bool:
    """AXPress one named button. The pointer stays where it is."""
    if depth > 40:
        return False
    role_ref = _ax_copy(element, "AXRole")
    role = _cf_text(role_ref) if role_ref else ""
    if role_ref:
        _CF.CFRelease(role_ref)
    if role == "AXButton" and _ax_label(element) == name:
        action = _cf_string("AXPress")
        try:
            return _AX.AXUIElementPerformAction(element, action) == 0
        finally:
            _CF.CFRelease(action)
    children = _ax_copy(element, "AXChildren")
    if not children or _CF.CFGetTypeID(children) != _AX_ARRAY:
        if children:
            _CF.CFRelease(children)
        return False
    try:
        for index in range(_CF.CFArrayGetCount(children)):
            if _press_walk(_CF.CFArrayGetValueAtIndex(children, index), name, depth + 1):
                return True
        return False
    finally:
        _CF.CFRelease(children)


def press_named_button(pid: int, title: str, name: str) -> bool:
    """Press a button inside one window. This does not move the pointer or raise the app."""
    if protected_window(title) or not title:
        return False
    app = _AX.AXUIElementCreateApplication(int(pid))
    windows = _ax_copy(app, "AXWindows")
    try:
        if not windows or _CF.CFGetTypeID(windows) != _AX_ARRAY:
            return False
        for index in range(_CF.CFArrayGetCount(windows)):
            window = _CF.CFArrayGetValueAtIndex(windows, index)
            if _ax_label(window) != title:
                continue
            if _press_walk(window, name, 0):
                log(f"pressed {name} on {title}")
                return True
        return False
    finally:
        if windows:
            _CF.CFRelease(windows)
        _CF.CFRelease(app)


def _window_blocking(pid: int, title: str) -> bool:
    """An on-screen window that is not minimized still receives background keys."""
    flags = {name: flag for name, flag in ax_window_states(pid)}
    return any(
        name == title and on_screen and flags.get(name) != "true"
        for _window_id, on_screen, name in windows(pid)
    )


def _cua_close(pid: int, title: str, action: str) -> None:
    for window_id, on_screen, name in windows(pid):
        if name != title or not on_screen:
            continue
        extra = Guard(pid, window_id)
        try:
            extra.refresh()
            close = [
                element
                for element in extra.elements
                if element.get("role") == "AXButton"
                and label_of(element) == "关闭"
                and element.get("element_token")
            ]
            if not close:
                labels = [
                    label_of(element)
                    for element in extra.elements
                    if element.get("role") == "AXButton" and label_of(element)
                ]
                log(f"no 关闭 on {title} buttons={labels[:16]}")
                return
            chosen = min(
                close,
                key=lambda element: (
                    float((element.get("frame") or {}).get("y", 0) or 0),
                    float((element.get("frame") or {}).get("w", 999) or 999),
                ),
            )
            extra.call(
                "click",
                {
                    "pid": pid,
                    "element_token": chosen["element_token"],
                    "action": action,
                    "delivery_mode": "background",
                },
            )
            log(f"pressed 关闭 on {title} via {action}")
        except Exception as error:
            log(f"close {title} via {action} failed: {str(error)[:140]}")
        finally:
            extra.close()
        return


def close_titled_window(pid: int, title: str) -> bool:
    """Close one applet window. A press that leaves the window up does not count.

    Applet windows drop the miniaturize style, and a press on 关闭 can report
    success without destroying the window. Confirm, then Space and Return on
    that button. The caller restarts the member app when this still returns false.
    """
    if protected_window(title) or not title or title == "ToWork":
        return False
    if not _window_blocking(pid, title):
        return True
    press_named_button(pid, title, "关闭")
    time.sleep(0.35)
    if not _window_blocking(pid, title):
        return True
    for action in ("press", "confirm"):
        _cua_close(pid, title, action)
        time.sleep(0.35)
        if not _window_blocking(pid, title):
            return True
    for window_id, on_screen, name in windows(pid):
        if name != title or not on_screen:
            continue
        extra = Guard(pid, window_id)
        try:
            extra.refresh()
            close = [
                element
                for element in extra.elements
                if element.get("role") == "AXButton"
                and label_of(element) == "关闭"
                and element.get("element_token")
            ]
            if not close:
                break
            claim_key_window(pid, title, OPERATOR[0] or front_process()[0])
            for key in ("space", "return"):
                try:
                    extra.press_background(key, close[0])
                    log(f"close key={key} on {title}")
                except RuntimeError as error:
                    log(f"close key {key} refused on {title}: {str(error)[:120]}")
                    continue
                time.sleep(0.35)
                if not _window_blocking(pid, title):
                    return True
        finally:
            extra.close()
        break
    if _window_blocking(pid, title):
        log(f"{title} stayed open after close")
        return False
    return True


def close_blocking_applet_windows(pid: int) -> list[str]:
    """Drop extra on-screen applet windows so background typing has one destination.

    Office applet windows remove the miniaturizable style bit, so AXMinimized does
    not stick. The page's 关闭 control still destroys the window. A window with no
    such control stays in the returned list.
    """
    blocking = []
    for _window_id, on_screen, title in windows(pid):
        if not on_screen or not title or title == "ToWork" or protected_window(title):
            continue
        blocking.append(title)
    if not blocking:
        return []
    unique = list(dict.fromkeys(blocking))
    minimize_names(pid, unique, True)
    time.sleep(0.2)
    flags = {name: flag for name, flag in ax_window_states(pid)}
    remain = []
    for title in unique:
        if flags.get(title) == "true":
            continue
        if close_titled_window(pid, title):
            time.sleep(0.3)
            continue
        remain.append(title)
    flags = {name: flag for name, flag in ax_window_states(pid)}
    visible = {name for _wid, on_screen, name in windows(pid) if on_screen and name}
    return [title for title in remain if title in visible and flags.get(title) != "true"]


def ensure_isolated(pid: int, target_title: str) -> bool:
    """Minimize every sibling accessibility window and read AXMinimized back before typing.

    Returns whether a window's minimized state changed. Callers must take a new
    element token after that, because the accessibility tree was rebuilt.
    """
    held = HELD_WINDOWS.setdefault(pid, [])
    last: list[tuple[str, str]] = []
    mutated = False
    for _attempt in range(5):
        last = ax_window_states(pid)
        hidden_target = [
            name for name, flag in last if name == target_title and flag == "true"
        ]
        if hidden_target:
            minimize_names(pid, [target_title], False)
            mutated = True
            time.sleep(0.2)
            continue
        visible = {name for _wid, on_screen, name in windows(pid) if on_screen and name}
        pending = [
            name
            for name, flag in last
            if name
            and name in visible
            and name != target_title
            and not protected_window(name)
            and flag != "true"
        ]
        if not pending:
            log(f"keyboard isolated target={target_title} ax={last}")
            return mutated
        unique = list(dict.fromkeys(pending))
        minimize_names(pid, unique, True)
        for name in unique:
            if name not in held:
                held.append(name)
        mutated = True
        time.sleep(0.25)
        flags = {name: flag for name, flag in ax_window_states(pid)}
        for name in unique:
            if flags.get(name) == "true" or name == "ToWork" or protected_window(name):
                continue
            if close_titled_window(pid, name):
                mutated = True
        time.sleep(0.25)
    log(f"keyboard siblings remained target={target_title} ax={last} cg={windows(pid)}")
    raise RuntimeError(f"keyboard siblings stayed eligible for {target_title}")


def focused_control(pid: int) -> tuple[str, str, str]:
    """Read the process key window and focused element. Does not change focus."""
    script = (
        'tell application "System Events"\n'
        f"  tell (first process whose unix id is {int(pid)})\n"
        '    set windowName to ""\n'
        "    try\n"
        '      set windowName to name of (value of attribute "AXFocusedWindow") as text\n'
        "    end try\n"
        '    set roleName to ""\n'
        '    set valueName to ""\n'
        "    try\n"
        '      set fe to value of attribute "AXFocusedUIElement"\n'
        "      try\n"
        "        set roleName to role of fe as text\n"
        "      end try\n"
        "      try\n"
        "        set valueName to value of fe as text\n"
        "      end try\n"
        "    end try\n"
        "    return windowName & linefeed & roleName & linefeed & valueName\n"
        "  end tell\n"
        "end tell"
    )
    result = subprocess.run(["osascript", "-e", script], capture_output=True, text=True)
    if result.returncode != 0:
        detail = " ".join((result.stderr or result.stdout).split())
        return "", "unread", detail[:80]
    parts = result.stdout.splitlines()
    window_name = parts[0].strip() if parts else ""
    role = parts[1].strip() if len(parts) > 1 else ""
    value = parts[2].strip() if len(parts) > 2 else ""
    return window_name, role, value[:48]


def claim_key_window(pid: int, title: str, operator: int) -> None:
    """Make the named window this process's key window without activating the app.

    Background keystrokes are delivered to the process key window. Minimizing
    the main ToWork window leaves that minimized window key, and the applet
    then accepts none of the characters. AXFocused marks the key window.
    AXMain activates the application, so this claim does not set it, and the
    claim is not repeated after the original front process is restored.
    """
    literal = applescript_string(title)
    script = (
        'tell application "System Events"\n'
        f"  tell (first process whose unix id is {int(pid)})\n"
        "    set changed to 0\n"
        "    repeat with w in windows\n"
        '      set n to ""\n'
        "      try\n"
        "        set n to name of w as text\n"
        "      end try\n"
        f"      if n is {literal} then\n"
        "        set mini to false\n"
        "        try\n"
        '          set mini to value of attribute "AXMinimized" of w\n'
        "        end try\n"
        "        if mini is false then\n"
        "          try\n"
        '            set value of attribute "AXFocused" of w to true\n'
        "            set changed to changed + 1\n"
        "          end try\n"
        "        end if\n"
        "      end if\n"
        "    end repeat\n"
        "    return changed as text\n"
        "  end tell\n"
        "end tell"
    )
    before_pid, _before_name = front_process()
    result = subprocess.run(["osascript", "-e", script], capture_output=True, text=True)
    confirm_pointer("claim key window")
    changed = result.stdout.strip() if result.returncode == 0 else "failed"
    if result.returncode != 0:
        detail = " ".join((result.stderr or result.stdout).split())
        log(f"key window claim failed target={title}: {detail[:160]}")
    after_pid, after_name = front_process()
    if after_pid != before_pid and operator:
        outcome = front_outcome(after_pid, operator, OWNED, None)
        if outcome == "restore":
            keep_front(operator, "claim key window")
        elif outcome == "left":
            log(f"frontmost left the drive during claim key window: {after_pid} {after_name}")
    window_name, role, value = focused_control(pid)
    log(
        f"key window target={title} changed={changed} "
        f"focused={window_name} role={role} value={value!r}"
    )


def restore_held(pid: int, operator: int) -> None:
    names = [name for name in HELD_WINDOWS.pop(pid, []) if not protected_window(name)]
    if names:
        try:
            minimize_names(pid, names, False)
        except RuntimeError as error:
            log(f"restore windows failed: {error}")
        log(f"restored windows {names}")
    if operator:
        keep_front(operator, "restore keyboard windows")


@contextmanager
def keyboard_hold(pid: int, operator: int):
    """Keep sibling windows minimized across every field of one form."""
    KEYBOARD_HOLDS[pid] = KEYBOARD_HOLDS.get(pid, 0) + 1
    try:
        yield
    finally:
        KEYBOARD_HOLDS[pid] = KEYBOARD_HOLDS.get(pid, 0) - 1
        if KEYBOARD_HOLDS[pid] <= 0:
            KEYBOARD_HOLDS.pop(pid, None)
            restore_held(pid, operator)


def clipboard_previous(read, pause=lambda: None, attempts: int = 4) -> str:
    """Text to restore after a paste. A missing text flavor restores as empty.

    One read can miss while the next read returns a string. The value itself
    is never logged.
    """
    seen = "missing"
    for index in range(attempts):
        value = read()
        if isinstance(value, str):
            return value
        seen = type(value).__name__
        if index + 1 < attempts:
            pause()
    log(f"clipboard text unavailable type={seen}")
    return ""


def paste_after_ambiguity(message: str, paste_attempts: int, limit: int = 4) -> bool:
    """Process-scoped keys stay refused while a sibling window is eligible."""
    return "ambiguity" in message and paste_attempts < limit


def paste_into_focused(ui: Guard, text: str) -> None:
    """Menu-paste when process-scoped keys are refused. The clipboard is put back."""

    def read_once() -> object:
        result = ui.call("clipboard_read", {"include_text": True})
        structured = result.get("structuredContent") or {}
        if not isinstance(structured, dict):
            return None
        return structured.get("text")

    previous = clipboard_previous(read_once, pause=lambda: time.sleep(0.2))
    try:
        ui.call("clipboard_write", {"text": text})
        last: RuntimeError | None = None
        for _attempt in range(4):
            try:
                ui.call(
                    "invoke_menu",
                    {"pid": ui.pid, "window_id": ui.window_id, "path": ["Edit", "Select All"]},
                )
                time.sleep(0.12)
                ui.call(
                    "invoke_menu",
                    {"pid": ui.pid, "window_id": ui.window_id, "path": ["Edit", "Paste"]},
                )
                return
            except RuntimeError as error:
                last = error
                if "stably key" not in str(error) and "frontmost" not in str(error):
                    raise
                time.sleep(0.4)
        if last is not None:
            raise last
    finally:
        ui.call("clipboard_write", {"text": previous})


def fill_field(ui: Guard, text: str, field: str) -> None:
    """Type into one field. Sibling accessibility windows stay minimized for the whole form."""
    operator = OPERATOR[0] or front_process()[0]
    target_title = next((row[2] for row in windows(ui.pid) if row[0] == ui.window_id), "")
    if not target_title:
        raise RuntimeError(f"typing window {ui.window_id} has no title")
    outer = KEYBOARD_HOLDS.get(ui.pid, 0) == 0
    KEYBOARD_HOLDS[ui.pid] = KEYBOARD_HOLDS.get(ui.pid, 0) + 1
    try:
        _fill_field(ui, text, field, operator, target_title)
    finally:
        KEYBOARD_HOLDS[ui.pid] = KEYBOARD_HOLDS.get(ui.pid, 0) - 1
        if outer:
            KEYBOARD_HOLDS.pop(ui.pid, None)
            restore_held(ui.pid, operator)


def release_keyboard_hold(pid: int, target_title: str) -> None:
    """Unminimize after a vanished tree. The hold is restored again when typing ends."""
    names = [name for name in HELD_WINDOWS.get(pid, []) if name and not protected_window(name)]
    if target_title and target_title not in names:
        names.append(target_title)
    if not names:
        return
    try:
        minimize_names(pid, names, False)
    except RuntimeError as error:
        log(f"restore after lost tree failed: {error}")


def _fill_field(ui: Guard, text: str, field: str, operator: int, target_title: str) -> None:
    last_error: Exception | None = None
    focused = False
    paste_attempts = 0
    for attempt in range(12):
        ui.refresh()
        controls = field_controls(ui, field)
        if not controls:
            if lost_window(ui):
                reason = getattr(ui, "degraded_reason", "") or ""
                log(f"field {field} lost tree attempt={attempt} reason={reason[:140]}")
                release_keyboard_hold(ui.pid, target_title)
                focused = False
                if attempt >= 8:
                    raise RuntimeError(LOST_TREE)
                time.sleep(0.4)
                continue
            if ui.has("Build failed"):
                close_build_overlay(ui, operator)
                time.sleep(0.4)
                continue
            last_error = AssertionError(f"missing field {field}")
            if attempt == 11:
                snapshot(ui, f"missing {field}")
            time.sleep(0.4)
            continue
        target = max(controls, key=lambda item: (item.get("frame") or {}).get("w", 0))
        if value_holds(target, text):
            log(f"typed field={field} effect=readback")
            return
        if ensure_isolated(ui.pid, target_title):
            focused = False
        if not focused:
            claim_key_window(ui.pid, target_title, operator)
            ui.refresh()
            controls = field_controls(ui, field)
            if not controls:
                if lost_window(ui):
                    reason = getattr(ui, "degraded_reason", "") or ""
                    log(f"field {field} lost tree attempt={attempt} reason={reason[:140]}")
                    release_keyboard_hold(ui.pid, target_title)
                    focused = False
                    if attempt >= 8:
                        raise RuntimeError(LOST_TREE)
                    time.sleep(0.4)
                    continue
                last_error = AssertionError(f"missing field {field}")
                time.sleep(0.3)
                continue
            target = max(controls, key=lambda item: (item.get("frame") or {}).get("w", 0))
            if value_holds(target, text):
                log(f"typed field={field} effect=readback")
                return
            log(
                f"focus target field={field} role={target.get('role')} "
                f"label={label_of(target)[:40]} frame={target.get('frame')}"
            )
            try:
                if label_of(target):
                    click_element(ui, target)
                else:
                    token = target.get("element_token")
                    if not token:
                        raise RuntimeError(f"field {field} has no element token")
                    ui.call(
                        "click",
                        {
                            "pid": ui.pid,
                            "element_token": token,
                            "action": "press",
                            "delivery_mode": "background",
                        },
                    )
            except RuntimeError as error:
                log(f"focus field={field} failed: {str(error)[:160]}")
            focused = True
            keep_front(operator, f"focus {field}")
            window_name, role, value = focused_control(ui.pid)
            log(f"focus field={field} window={window_name} role={role} value={value!r}")
            time.sleep(0.3)
            continue
        token = target.get("element_token")
        if not token:
            raise RuntimeError(f"field {field} has no element token")
        log(
            f"type target field={field} role={target.get('role')} "
            f"label={label_of(target)[:40]} value_len={len(str(target.get('value') or ''))} "
            f"frame={target.get('frame')}"
        )
        try:
            result = ui.call(
                "type_text",
                {
                    "pid": ui.pid,
                    "window_id": ui.window_id,
                    "element_token": token,
                    "text": text,
                    "delivery_mode": "background",
                    "delay_ms": 30,
                },
            )
        except RuntimeError as error:
            last_error = error
            message = str(error)
            if paste_after_ambiguity(message, paste_attempts):
                paste_attempts += 1
                try:
                    paste_into_focused(ui, text)
                except RuntimeError as paste_error:
                    last_error = paste_error
                    log(f"paste field={field} failed {str(paste_error)[:160]}")
                else:
                    time.sleep(0.4)
                    ui.refresh()
                    controls = field_controls(ui, field)
                    target = (
                        max(controls, key=lambda item: (item.get("frame") or {}).get("w", 0))
                        if controls
                        else None
                    )
                    if target is not None and value_holds(target, text):
                        log(f"typed field={field} effect=paste")
                        keep_front(operator, f"paste {field}")
                        return
                    log(f"paste unread field={field}")
            window_name, role, value = focused_control(ui.pid)
            log(
                f"type retry {attempt + 1} field={field} window={window_name} "
                f"role={role} value={value!r} error={message[:180]}"
            )
            if attempt == 11 or not any(
                sign in message for sign in ("stale", "ambiguity", "refused", "outside", "incomplete")
            ):
                snapshot(ui, f"type {field}")
                raise
            focused = False
            continue
        structured = result.get("structuredContent") or {}
        time.sleep(0.4)
        ui.refresh()
        controls = field_controls(ui, field)
        target = (
            max(controls, key=lambda item: (item.get("frame") or {}).get("w", 0)) if controls else None
        )
        if target is not None and value_holds(target, text):
            effect = structured.get("effect") or structured.get("route") or "readback"
            log(f"typed field={field} effect={effect}")
            return
        delivery = structured.get("delivery") or {}
        raw = str((target or {}).get("value") or "")
        log(
            f"type unread field={field} effect={structured.get('effect')} "
            f"route={structured.get('route')} delivered={delivery.get('delivered_count')} "
            f"value_len={len(draft_text(target) if target else '')} raw_len={len(raw)}"
        )
        last_error = RuntimeError(f"field {field} did not keep the text")
        focused = False
    if last_error:
        raise last_error
    raise AssertionError(f"missing field {field}")


def fill_approval(ui: Guard, title: str) -> None:
    fill_field(ui, title, "标题")
    fill_field(ui, "双账户端到端", "说明")


def fill_leave(ui: Guard, marker: str) -> None:
    choose(ui, "请假类型", "事假")
    pick_range(ui)
    fill_field(ui, marker, "事由")


def fill_travel(ui: Guard, marker: str) -> None:
    fill_field(ui, marker, "目的地")
    pick_range(ui)
    fill_field(ui, "双账户出差", "事由")


def fill_expense(ui: Guard, marker: str) -> None:
    fill_field(ui, "18.60", "金额")
    choose(ui, "类别", "交通")
    pick_day(ui, "日期")
    fill_field(ui, marker, "事由")


def fill_meeting(ui: Guard, marker: str) -> None:
    fill_field(ui, marker, "会议主题")
    pick_day(ui, "日期")
    choose(ui, "开始", "09:00")
    choose(ui, "结束", "10:00")
    fill_field(ui, "东侧厅", "地点")
    fill_field(ui, "对齐进度", "议程")


def fill_seal(ui: Guard, marker: str) -> None:
    fill_field(ui, marker, "文件名称")
    choose(ui, "印章", "公章")
    fill_field(ui, "1", "份数")
    fill_field(ui, "双账户用印", "事由")


def fill_marked(ui: Guard, applet: str, marker: str) -> None:
    if applet == "审批":
        fill_approval(ui, marker)
    elif applet == "请假":
        fill_leave(ui, marker)
    elif applet == "出差":
        fill_travel(ui, marker)
    elif applet == "报销":
        fill_expense(ui, marker)
    elif applet == "会议":
        fill_meeting(ui, marker)
    elif applet == "用印":
        fill_seal(ui, marker)
    else:
        raise RuntimeError(f"unknown applet {applet}")


APPLETS = ("审批", "请假", "出差", "报销", "会议", "用印")


def inbox_box_visible(elements: list[dict], box: str) -> bool:
    """A folder label. The applet window title is 待办 and does not count."""
    for element in elements:
        if element.get("role") in {"AXWindow", "AXWebArea"}:
            continue
        if box in label_of(element):
            return True
    return False


def inbox_count(ui: Guard, box: str) -> int:
    deadline = time.time() + 8
    while True:
        ui.refresh()
        for element in ui.elements:
            if element.get("role") != "AXButton":
                continue
            label = label_of(element)
            if label == box or label.startswith(box):
                numbers = re.findall(r"\d+", label)
                if numbers:
                    return int(numbers[-1])
        if inbox_box_visible(ui.elements, box):
            return 0
        if time.time() >= deadline:
            break
        time.sleep(0.3)
    snapshot(ui, f"inbox {box}")
    raise AssertionError(f"missing inbox box {box}")


def open_inbox(pid: int, operator: str) -> Guard:
    main = bind(pid, "ToWork")
    try:
        main.refresh()
        if not any(
            element.get("role") == "AXButton" and "待办" in label_of(element) for element in main.elements
        ):
            log("workbench not showing; opening 工作台")
            click_team_workbench(main)
            time.sleep(0.6)
            main.refresh()
        before = {row[0] for row in windows(pid)}
        click_label(main, "待办", "AXButton")
        keep_front(operator, "open inbox")
    finally:
        main.close()
    window = wait_for_window(pid, "待办", before)
    # Leave the form window up until this tree is readable. Closing it drops
    # the new applet's accessibility window.
    ui = Guard(pid, window)
    deadline = time.time() + 20
    logged = False
    while True:
        ui.refresh()
        reason = str(getattr(ui, "degraded_reason", "") or "")
        if reason or not ui.elements:
            log(f"inbox window {window} waiting for a tree")
            ui.close()
            time.sleep(0.4)
            try:
                window = wait_for_window(pid, "待办", set(), timeout=6)
            except RuntimeError:
                if time.time() >= deadline:
                    break
                ui = Guard(pid, window)
                continue
            ui = Guard(pid, window)
            logged = False
            if time.time() >= deadline:
                break
            continue
        if ui.has("展开导航"):
            log("inbox nav is collapsed")
            click_label(ui, "展开导航", "AXButton")
            time.sleep(0.4)
            ui.refresh()
        if inbox_box_visible(ui.elements, "已发起"):
            close_other_on_screen(pid, window)
            keep_front(operator, "open inbox")
            ui.refresh()
            return ui
        if not logged:
            buttons = [
                label_of(element)
                for element in ui.elements
                if element.get("role") == "AXButton" and label_of(element)
            ]
            roles: dict[str, int] = {}
            for element in ui.elements:
                role = str(element.get("role") or "?")
                roles[role] = roles.get(role, 0) + 1
            log(f"inbox elements={len(ui.elements)} roles={roles} buttons={buttons[:24]}")
            logged = True
        if time.time() >= deadline:
            break
        time.sleep(0.35)
    snapshot(ui, "inbox")
    ui.close()
    raise RuntimeError("inbox did not show boxes")


def open_detail(ui: Guard, title: str) -> None:
    click_label(ui, title)
    deadline = time.time() + 8
    while True:
        ui.refresh()
        if ui.has("打开侧边栏"):
            click_label(ui, "打开侧边栏", "AXButton")
            time.sleep(0.4)
            ui.refresh()
        if field_controls(ui, "审批意见") or any(
            element.get("role") == "AXButton" and label_of(element) == "同意" for element in ui.elements
        ):
            return
        if time.time() >= deadline:
            break
        time.sleep(0.3)
    snapshot(ui, "detail")
    raise AssertionError(f"detail did not open for {title}")


def assert_no_self_approval(ui: Guard, title: str) -> None:
    click_label(ui, "已发起")
    time.sleep(0.4)
    open_detail(ui, title)
    if any(label_of(element) == "同意" for element in ui.elements):
        snapshot(ui, "self approval")
        raise AssertionError("member can approve their own item")
    log("self-approval refused")


def item_status(ui: Guard, title: str) -> str:
    ui.refresh()
    blobs = [label_of(element) for element in ui.elements if title in label_of(element)]
    blob = " ".join(blobs)
    if "已通过" in blob and "已拒绝" not in blob:
        return "已通过"
    if "已拒绝" in blob and "已通过" not in blob:
        return "已拒绝"
    if "已通过" in blob:
        return "已通过"
    if "审批中" in blob:
        return "审批中"
    return "unknown"


def reject_control_state(elements: list[dict]) -> str:
    """enabled when 拒绝 can be pressed. disabled or absent when it cannot."""
    buttons = [
        element
        for element in elements
        if element.get("role") == "AXButton" and label_of(element) == "拒绝"
    ]
    if not buttons:
        return "absent"
    if all(element.get("enabled") is False for element in buttons):
        return "disabled"
    return "enabled"


def decide(ui: Guard, title: str) -> None:
    ui.refresh()
    if not ui.has(title):
        click_label(ui, "待办", "AXButton")
        time.sleep(0.4)
    open_detail(ui, title)
    approve = [
        element
        for element in ui.elements
        if element.get("role") == "AXButton" and label_of(element) == "同意"
    ]
    if approve and all(element.get("enabled") is False for element in approve):
        log("empty comment refused")
    else:
        if approve:
            click_element(ui, approve[0])
            ui.refresh()
        if "同意后" in " ".join(ui.labels()):
            snapshot(ui, "empty comment dialog")
            raise AssertionError("empty comment opened the confirm dialog")
        log("empty comment refused")
    fill_field(ui, "同意这条申请", "审批意见")
    click_label(ui, "同意", "AXButton")
    time.sleep(0.4)
    ui.refresh()
    if "同意后" not in " ".join(ui.labels()):
        snapshot(ui, "confirm missing")
        raise AssertionError("commented approve did not open the confirm dialog")
    click_label(ui, "同意", "AXButton", last=True)
    if read_decided_status(ui, title) != "已通过":
        snapshot(ui, "decision")
        raise AssertionError("decision did not reach 已通过")
    log("item terminal status=已通过")
    ui.refresh()
    control = reject_control_state(ui.elements)
    activated = False
    if control == "enabled":
        reject = next(
            element
            for element in ui.elements
            if element.get("role") == "AXButton" and label_of(element) == "拒绝"
        )
        click_element(ui, reject)
        activated = True
        log("拒绝 activated")
        time.sleep(0.3)
        ui.refresh()
        if "拒绝后" in " ".join(ui.labels()):
            click_label(ui, "拒绝", "AXButton", last=True)
            time.sleep(0.6)
            ui.refresh()
    else:
        log(f"拒绝 control {control}")
    status = read_decided_status(ui, title)
    record = opposite_observation(control, activated, status)
    if record is None:
        snapshot(ui, "opposite")
        raise AssertionError(
            f"opposite decision was not checked: control={control} activated={activated} status={status}"
        )
    global OPPOSITE_RECORD, OPPOSITE_ACTIVATED
    OPPOSITE_RECORD = record
    OPPOSITE_ACTIVATED = activated
    log(record)


def read_decided_status(ui: Guard, title: str) -> str:
    """A decided card leaves 待办. Read 已通过 from 已办 when the current box no longer lists it."""
    deadline = time.time() + 12
    opened_done = False
    while time.time() < deadline:
        status = item_status(ui, title)
        if status != "unknown":
            return status
        if not opened_done and any(
            element.get("role") == "AXButton" and label_of(element).startswith("已办") for element in ui.elements
        ):
            click_label(ui, "已办", "AXButton")
            opened_done = True
            time.sleep(0.5)
            continue
        time.sleep(0.4)
    return item_status(ui, title)


def inbox_box_button(elements: list[dict], box: str) -> dict | None:
    """The folder chip. The inbox window title is 待办, and that window cannot be pressed."""
    buttons = []
    for element in elements:
        if element.get("role") != "AXButton":
            continue
        label = label_of(element)
        if label == box or label.startswith(box + " "):
            buttons.append(element)
    if not buttons:
        return None
    return min(
        buttons,
        key=lambda element: float((element.get("frame") or {}).get("w", 9999) or 9999),
    )


def first_inbox_box(box: str, pull, pause) -> dict | None:
    """Return the folder chip from the first tree that contains it.

    One accessibility snapshot can omit the chip while the next snapshot of
    the same window contains it. pull returns (elements, retry). pause runs
    only between misses.
    """
    while True:
        elements, retry = pull()
        chosen = inbox_box_button(elements, box)
        if chosen is not None or not retry:
            return chosen
        pause()


def visible(ui: Guard, box: str, title: str) -> None:
    deadline = time.time() + 8
    missed = False

    def pull():
        ui.refresh()
        return ui.elements, time.time() < deadline

    def pause():
        nonlocal missed
        if not missed:
            log(f"inbox box {box} absent from this tree")
            missed = True
        time.sleep(0.4)

    chosen = first_inbox_box(box, pull, pause)
    if chosen is None:
        snapshot(ui, f"missing inbox box {box}")
        raise AssertionError(f"missing inbox box {box}")
    if missed:
        log(f"inbox box {box} appeared after refresh")
    log(f"inbox box {box} label={label_of(chosen)[:40]} w={(chosen.get('frame') or {}).get('w')}")
    click_element(ui, chosen)
    time.sleep(0.6)
    ui.refresh()
    deadline = time.time() + 8
    while time.time() < deadline:
        if ui.has(title):
            log(f"visible box={box} title={title}")
            return
        time.sleep(0.4)
        ui.refresh()
    snapshot(ui, f"missing {title} in {box}")
    raise AssertionError(f"{title} missing from {box}")


def drive(pass_name: str, probe: bool = False) -> None:
    if os.environ.get("A3S_GUI_ALLOW_MOUSE"):
        raise RuntimeError("A3S_GUI_ALLOW_MOUSE must stay unset")
    if shutil.which("cua-driver") is None:
        raise RuntimeError("cua-driver is not installed")
    operator, operator_name = front_process()
    global OPERATOR, POINTER_ORIGIN
    OPERATOR = (operator, operator_name)
    origin = pointer()
    POINTER_ORIGIN = origin
    log(f"pass={pass_name} allow_mouse=unset frontmost={operator} {operator_name} pointer={origin}")
    log("切换为审批人 was not used")
    org_id = ensure_pair()
    log(f"team id={org_id}")
    member_pid = launch_app(
        MEMBER_FRAGMENT, SUPPORT / MEMBER_APP / "ToWork.app", "member", operator
    )
    log(f"member pid={member_pid} renderer_4318={uses_renderer(member_pid)}")
    set_minimized(member_pid, "ToWork", False)
    keep_front(operator, "show member")
    creator_pid = launch_app(
        CREATOR_FRAGMENT, SUPPORT / CREATOR_APP / "ToWork.app", "creator", operator
    )
    OWNED.add(creator_pid)
    set_minimized(creator_pid, "ToWork", False)
    keep_front(operator, "show creator")
    member_locks = lock_holders(MEMBER_APP)
    creator_locks = lock_holders(CREATOR_APP)
    log(f"lock member={member_locks} creator={creator_locks}")
    if member_pid not in member_locks or creator_pid not in creator_locks:
        raise RuntimeError("tenant locks are not held by the two account windows")
    if set(member_locks) & set(creator_locks):
        raise RuntimeError("the two accounts share one single-instance lock")
    # One accessibility client at a time. A second snapshot invalidates element tokens.
    member = bind(member_pid, "ToWork")
    try:
        open_team(member, ORG_NAME, operator)
    finally:
        member.close()
    creator = None
    finished = False
    try:
        creator = bind(creator_pid, "ToWork")
        open_team(creator, ORG_NAME, operator)
        front_pid, front_name = front_process()
        log(f"probe workbench visible pointer={pointer()} frontmost={front_pid} {front_name}")
        if probe:
            log("probe stopped before submit")
            return
        if creator is not None:
            creator.close()
            creator = None
        approval_title = f"审批-{uuid.uuid4().hex[:8]}"
        approval_id = submit_applet(
            member_pid,
            "审批",
            lambda ui: fill_approval(ui, approval_title),
            operator,
        )
        log(f"one item id {approval_id}")
        member_inbox = open_inbox(member_pid, operator)
        sent = inbox_count(member_inbox, "已发起")
        log(f"member sent {sent}")
        assert_no_self_approval(member_inbox, approval_title)
        member_inbox.close()
        creator_pid = refresh_account(
            creator_pid, CREATOR_FRAGMENT, CREATOR_APP, "creator", operator
        )
        creator_inbox = open_inbox(creator_pid, operator)
        todo = inbox_count(creator_inbox, "待办")
        log(f"creator todo {todo}")
        decide(creator_inbox, approval_title)
        visible(creator_inbox, "已办", approval_title)
        creator_inbox.close()
        markers = {"审批": approval_title}
        for applet in APPLETS[1:]:
            marker = f"gui-{applet}-{uuid.uuid4().hex[:8]}"
            markers[applet] = marker
            member_pid = refresh_account(
                member_pid, MEMBER_FRAGMENT, MEMBER_APP, "member", operator
            )
            submit_applet(
                member_pid,
                applet,
                lambda ui, applet=applet, marker=marker: fill_marked(ui, applet, marker),
                operator,
            )
        creator_markers = {}
        for applet in APPLETS:
            marker = f"gui-creator-{applet}-{uuid.uuid4().hex[:8]}"
            creator_markers[applet] = marker
            creator_pid = refresh_account(
                creator_pid, CREATOR_FRAGMENT, CREATOR_APP, "creator", operator
            )
            submit_applet(
                creator_pid,
                applet,
                lambda ui, applet=applet, marker=marker: fill_marked(ui, applet, marker),
                operator,
            )
            log(f"creator submitted {applet} marker={marker}")
        member_pid = refresh_account(
            member_pid, MEMBER_FRAGMENT, MEMBER_APP, "member", operator
        )
        member_inbox = open_inbox(member_pid, operator)
        visible(member_inbox, "已发起", approval_title)
        for marker in creator_markers.values():
            visible(member_inbox, "待办", marker)
        member_inbox.close()
        creator_pid = refresh_account(
            creator_pid, CREATOR_FRAGMENT, CREATOR_APP, "creator", operator
        )
        creator_inbox = open_inbox(creator_pid, operator)
        for applet, marker in markers.items():
            if applet == "审批":
                continue
            visible(creator_inbox, "待办", marker)
        creator_inbox.close()
        current_point = pointer()
        if pointer_outcome(origin, current_point) != "unchanged":
            raise RuntimeError(
                f"pointer changed outside a refused call: {origin} -> {current_point}"
            )
        log(f"pointer unchanged {origin}")
        keep_front(operator, "pass end")
        front_pid, front_name = front_process()
        log(f"frontmost={front_pid} {front_name} expected={operator} {operator_name}")
        acceptance = drive_acceptance(
            origin,
            pointer(),
            front_pid,
            operator,
            OWNED,
            FRONT_STEALS,
            OPPOSITE_RECORD,
            OPPOSITE_ACTIVATED,
        )
        if acceptance == "pointer":
            raise RuntimeError(
                f"pointer changed outside a refused call: {origin} -> {pointer()}"
            )
        if acceptance == "opposite":
            raise RuntimeError("opposite decision was not checked")
        if acceptance != "pass":
            raise RuntimeError(
                f"frontmost stayed on the drive: {front_pid} {front_name}"
            )
        finished = True
    finally:
        if creator is not None:
            creator.close()
        keep_front(operator, "cleanup")
    if not finished:
        return
    front_pid, front_name = front_process()
    current_point = pointer()
    acceptance = drive_acceptance(
        origin,
        current_point,
        front_pid,
        operator,
        OWNED,
        FRONT_STEALS,
        OPPOSITE_RECORD,
        OPPOSITE_ACTIVATED,
    )
    if acceptance == "pointer":
        raise RuntimeError(
            f"pointer changed outside a refused call: {origin} -> {current_point}"
        )
    if acceptance == "opposite":
        raise RuntimeError("opposite decision was not checked")
    if acceptance != "pass":
        raise RuntimeError(f"frontmost stayed on the drive: {front_pid} {front_name}")
    log(f"frontmost={front_pid} {front_name} expected={operator} {operator_name}")
    log(f"pass={pass_name} office round trip passed")


def main() -> None:
    mode = sys.argv[1] if len(sys.argv) > 1 else "first"
    drive(mode, probe=mode == "probe")


if __name__ == "__main__":
    try:
        main()
    except Exception as error:
        log(f"FAIL {error}")
        raise
