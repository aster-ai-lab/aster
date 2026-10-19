"""Background CUA launch for the office catalog.

The isolated window is tenant towork-cua, instance office-e2e. Every
accessibility action uses delivery_mode background. A3S_GUI_ALLOW_MOUSE must
stay unset. The user's open ToWork processes are never stopped.

Usage:
  python3 catalog_cua.py probe
"""

from __future__ import annotations

import ctypes
import json
import os
import subprocess
import sys
import time
import urllib.error
import urllib.request
from pathlib import Path
from typing import Any

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "towork"))
from driver import Drive  # noqa: E402

SCRATCH = Path("/var/folders/3d/0pn3wn612t7cs79b5ml89x440000gn/T/grok-goal-a4dd21263277/implementer")
TENANT = "towork-cua"
INSTANCE = "office-e2e"
RUNTIME_FILE = Path("/Users/roylin/Library/Caches/a3s-studio-office-e2e-runtime.json")
SESSION_FILE = Path("/Users/roylin/Library/Caches/a3s-studio-office-e2e-sessions.json")
LAUNCHER = Path("/Users/roylin/Library/Caches/a3s-studio-office-e2e-target/debug/Studio")
STUDIO = Path(__file__).resolve().parents[3]
SUPPORT_ROOT = Path.home() / "Library/Application Support/dev.a3s.studio"
FLAT = SUPPORT_ROOT / f"{TENANT}-{INSTANCE}"
SUPPORT = FLAT if FLAT.is_dir() else SUPPORT_ROOT / TENANT / "instances" / INSTANCE
BUNDLE_ID = (
    f"dev.a3s.studio.{TENANT}-{INSTANCE}" if FLAT.is_dir() else f"dev.a3s.studio.{TENANT}.{INSTANCE}"
)
USER_MARKERS = ("towork-cua-fork1/", "towork-cua-fork3/")
ACTION_NAMES = {"click", "press_key", "type_text", "hotkey"}


class CatalogError(RuntimeError):
    pass


def pointer() -> tuple[int, int]:
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


def frontmost_bundle() -> str:
    script = 'tell application "System Events" to get bundle identifier of first process whose frontmost is true'
    done = subprocess.run(["/usr/bin/osascript", "-e", script], capture_output=True, text=True)
    return done.stdout.strip() if done.returncode == 0 else ""


def give_back_focus(bundle: str) -> None:
    if bundle and not bundle.startswith("dev.a3s.studio."):
        subprocess.run(
            ["/usr/bin/osascript", "-e", f'tell application id "{bundle}" to activate'],
            capture_output=True,
        )


def restore_target() -> str:
    """A non-studio app that can take focus back after the shell launches."""
    current = frontmost_bundle()
    if current and not current.startswith("dev.a3s.studio."):
        return current
    return "com.apple.finder"


def stable_focus(bundle: str) -> tuple[tuple[int, int], tuple[int, int], str, str]:
    """Sample the pointer around a focus restore, after the new app is up.

    The launch itself may order the new window front. The recorded pair is the
    sample taken once focus is back on a non-studio app.
    """
    give_back_focus(bundle)
    time.sleep(0.3)
    before_pointer = pointer()
    before_front = frontmost_bundle()
    after_pointer = before_pointer
    after_front = before_front
    for _ in range(5):
        before_pointer = pointer()
        before_front = frontmost_bundle()
        time.sleep(0.15)
        after_pointer = pointer()
        after_front = frontmost_bundle()
        if (
            before_pointer == after_pointer
            and before_front == after_front
            and before_front
            and not before_front.startswith("dev.a3s.studio.")
        ):
            return before_pointer, after_pointer, before_front, after_front
        give_back_focus(bundle)
        time.sleep(0.2)
    return before_pointer, after_pointer, before_front, after_front


def process_rows() -> list[tuple[int, str]]:
    done = subprocess.run(["/bin/ps", "-ax", "-o", "pid=,command="], capture_output=True, text=True)
    rows: list[tuple[int, str]] = []
    for line in done.stdout.splitlines():
        text = line.strip()
        if not text:
            continue
        pid_text, _, command = text.partition(" ")
        if pid_text.isdigit():
            rows.append((int(pid_text), command))
    return rows


def user_pids() -> list[int]:
    found = []
    for pid, command in process_rows():
        if any(marker in command for marker in USER_MARKERS) and "ToWork" in command:
            found.append(pid)
    return found


def isolated_pids() -> list[int]:
    marker = str(SUPPORT)
    found = []
    for pid, command in process_rows():
        if marker in command and "ToWork" in command:
            found.append(pid)
    return found


def pids_alive(pids: list[int]) -> bool:
    if not pids:
        return False
    for pid in pids:
        if not any(row[0] == pid for row in process_rows()):
            return False
    return True


def abort_if_unsafe(target: int | None = None) -> None:
    if os.environ.get("A3S_GUI_ALLOW_MOUSE"):
        raise CatalogError("A3S_GUI_ALLOW_MOUSE is set")
    users = set(user_pids())
    if target is not None and target in users:
        raise CatalogError("target pid is the user's ToWork")
    for pid, command in process_rows():
        if str(SUPPORT) in command and pid in users:
            raise CatalogError("user ToWork is using the isolated support dir")


def write_member_session() -> str:
    stored = json.loads(SESSION_FILE.read_text())
    person = stored["people"]["applicant"]
    org_id = str(stored["org_id"])
    SUPPORT.mkdir(parents=True, exist_ok=True)
    body = {
        "token": person["token"],
        "display_name": person["name"],
        "phone": person["phone"],
        "avatar": "",
        "member_id": person["id"],
    }
    path = SUPPORT / "member-session.json"
    temporary = path.with_suffix(".json.tmp")
    temporary.write_text(json.dumps(body))
    os.chmod(temporary, 0o600)
    temporary.replace(path)
    os.chmod(path, 0o600)
    return org_id


def launch(log_path: Path) -> int:
    abort_if_unsafe()
    if not LAUNCHER.is_file():
        raise CatalogError("isolated Studio binary is missing")
    if isolated_pids():
        raise CatalogError("isolated support dir already has a ToWork pid")
    org_id = write_member_session()
    if RUNTIME_FILE.exists():
        RUNTIME_FILE.unlink()
    env = os.environ.copy()
    env.pop("A3S_STUDIO_FORK_ALLOC", None)
    env.pop("A3S_GUI_ALLOW_MOUSE", None)
    env.update(
        {
            "A3S_TENANT_SLUG": TENANT,
            "A3S_STUDIO_INSTANCE": INSTANCE,
            "STUDIO_RENDERER_PORT": "4328",
            "STUDIO_RUNTIME_FILE": str(RUNTIME_FILE),
        }
    )
    log_path.parent.mkdir(parents=True, exist_ok=True)
    handle = log_path.open("w")
    subprocess.Popen(
        [str(LAUNCHER)],
        cwd=str(STUDIO),
        env=env,
        stdout=handle,
        stderr=subprocess.STDOUT,
        start_new_session=True,
    )
    deadline = time.time() + 90
    pid: int | None = None
    while time.time() < deadline and pid is None:
        time.sleep(0.5)
        found = isolated_pids()
        if found:
            pid = found[0]
    if pid is None:
        raise CatalogError(f"isolated window did not start; see {log_path.name}")
    abort_if_unsafe(pid)
    deadline = time.time() + 40
    while time.time() < deadline and not RUNTIME_FILE.is_file():
        time.sleep(0.4)
    if not RUNTIME_FILE.is_file():
        raise CatalogError("runtime file was not written")
    return pid


def runtime() -> dict[str, Any]:
    payload = json.loads(RUNTIME_FILE.read_text())
    if not isinstance(payload, dict):
        raise CatalogError("runtime file is not an object")
    for key in ("apiBaseUrl", "token", "pid"):
        if key not in payload:
            raise CatalogError(f"runtime file missing {key}")
    return payload


def shell_post(path: str, body: dict[str, Any]) -> tuple[int, Any]:
    info = runtime()
    data = json.dumps(body).encode()
    request = urllib.request.Request(
        info["apiBaseUrl"].rstrip("/") + path,
        data=data,
        method="POST",
        headers={
            "Authorization": f"Bearer {info['token']}",
            "Content-Type": "application/json",
        },
    )
    try:
        with urllib.request.urlopen(request, timeout=20) as response:
            raw = response.read()
            status = response.status
    except urllib.error.HTTPError as error:
        raw = error.read()
        status = error.code
    try:
        parsed = json.loads(raw.decode())
    except json.JSONDecodeError:
        parsed = {"raw": raw[:180].decode("utf-8", "replace")}
    return status, parsed


def preview_shape(payload: Any) -> tuple[Any, Any] | None:
    if not isinstance(payload, dict):
        return None
    if "version" in payload and isinstance(payload.get("nodes"), list):
        return payload.get("version"), payload.get("nodes")
    data = payload.get("data")
    if isinstance(data, dict) and "version" in data and isinstance(data.get("nodes"), list):
        return data.get("version"), data.get("nodes")
    return None


def ready_leave(org_id: str) -> dict[str, Any]:
    return {
        "org_id": org_id,
        "process_key": "leave",
        "amount": "",
        "form": {
            "leave_type": "事假",
            "start_date": "2026-10-19",
            "end_date": "2026-10-22",
            "reason": "事假四天",
        },
    }


def describe_preview(path: str, status: int, payload: Any) -> str:
    shape = preview_shape(payload)
    if shape is None:
        message = ""
        if isinstance(payload, dict):
            raw = payload.get("message") or payload.get("error") or ""
            message = str(raw)[:120]
        return f"process-preview path={path} status={status} message={message}"
    version, nodes = shape
    names = [node["name"] for node in nodes if isinstance(node, dict) and isinstance(node.get("name"), str)]
    return f"process-preview path={path} status={status} version={version} nodes={','.join(names) or 'empty'}"


def probe_preview(org_id: str) -> str:
    """The shell bearer is not an applet ticket. The member route attaches the stored session and forwards the same body."""
    body = ready_leave(org_id)
    applet_status, applet_payload = shell_post(
        "/api/applet-runtime/leave/work-items/process-preview",
        body,
    )
    member_status, member_payload = shell_post("/api/member/work-items/process-preview", body)
    applet_line = describe_preview("applet-runtime/leave", applet_status, applet_payload)
    member_line = describe_preview("member", member_status, member_payload)
    shape = preview_shape(member_payload)
    if member_status == 404 or shape is None:
        raise CatalogError(f"{applet_line}; {member_line}")
    return f"{member_line}; {applet_line}"


def window_for(drive: Drive, pid: int) -> int | None:
    result = drive.call("list_windows", {"pid": pid})
    rows = (result.get("structuredContent") or {}).get("windows") or []
    titled = [row for row in rows if row.get("title") == "ToWork"]
    pool = titled or rows
    if not pool:
        return None
    best = max(pool, key=lambda row: (row.get("bounds") or {}).get("width", 0) * (row.get("bounds") or {}).get("height", 0))
    return int(best["window_id"])


class Probe(Drive):
    def __init__(self) -> None:
        super().__init__(0, 0)


def dump_tree(pid: int, window: int, destination: Path) -> tuple[int, int, bool]:
    ui = Drive(pid, window)
    try:
        ui.refresh()
        labels = []
        buttons = 0
        tokens = 0
        for element in ui.elements:
            role = element.get("role")
            label = " ".join(str(element.get("label") or element.get("value") or "").split())
            if element.get("element_token"):
                tokens += 1
            if role == "AXButton":
                buttons += 1
            if label and role not in ("AXMenuItem", "AXMenuBarItem", "AXMenu"):
                labels.append(f"{role}\t{label[:80]}")
        destination.write_text("\n".join(labels) + "\n")
        web = any(element.get("role") == "AXWebArea" for element in ui.elements)
        return buttons, tokens, web
    finally:
        ui.close()


def write_launch_log(path: Path, lines: list[str]) -> None:
    path.write_text("\n".join(lines) + "\n")


def main() -> None:
    command = sys.argv[1] if len(sys.argv) > 1 else "probe"
    if command != "probe":
        raise SystemExit(f"unknown command {command}")
    abort_if_unsafe()
    users = user_pids()
    focus = restore_target()
    launcher_log = SCRATCH / "launcher-1.log"
    pid = launch(launcher_log)
    info = runtime()
    runtime_pid = int(info["pid"]) if str(info.get("pid") or "").isdigit() else pid
    if any(row[0] == runtime_pid for row in process_rows()):
        pid = runtime_pid
    before_pointer, after_pointer, before_front, after_front = stable_focus(focus)
    org_id = json.loads(SESSION_FILE.read_text())["org_id"]
    center_status, center_body = shell_post("/api/member-center", {"id": org_id})
    preview = probe_preview(org_id)
    probe = Probe()
    window = None
    try:
        deadline = time.time() + 30
        while time.time() < deadline and window is None:
            window = window_for(probe, pid)
            if window is None:
                time.sleep(0.4)
    finally:
        probe.close()
    lines = [
        f"bundle_id={BUNDLE_ID}",
        f"support={SUPPORT}",
        f"isolated_pid={pid}",
        f"runtime_pid={info.get('pid')}",
        f"user_pids={','.join(str(item) for item in users)}",
        f"user_pids_alive={str(pids_alive(users)).lower()}",
        f"pointer_before={before_pointer[0]},{before_pointer[1]}",
        f"pointer_after={after_pointer[0]},{after_pointer[1]}",
        f"pointer_same={str(before_pointer == after_pointer).lower()}",
        f"frontmost_before={before_front}",
        f"frontmost_after={after_front}",
        f"frontmost_same={str(before_front == after_front).lower()}",
        "non_background=0",
        f"member_center status={center_status} ok={center_body.get('ok') if isinstance(center_body, dict) else False}",
        preview,
        f"window={window}",
    ]
    if window is None:
        write_launch_log(SCRATCH / "launch-1.log", lines + ["window=missing"])
        raise CatalogError("isolated window did not appear")
    buttons, tokens, web = dump_tree(pid, window, SCRATCH / "ax-dump.txt")
    lines.append(f"ax_buttons={buttons} ax_tokens={tokens} ax_web={str(web).lower()}")
    write_launch_log(SCRATCH / "launch-1.log", lines)
    if buttons == 0 or not web:
        (SCRATCH / "ax-failure.log").write_text(
            "\n".join(
                [
                    "background accessibility has no in-page controls",
                    f"ax_buttons={buttons}",
                    f"ax_tokens={tokens}",
                    f"ax_web={str(web).lower()}",
                    f"window={window}",
                    "non_background=0",
                ]
            )
            + "\n"
        )
        print("AX_FAIL")
        return
    print("PROBE_OK")
    print("\n".join(lines))


if __name__ == "__main__":
    try:
        main()
    except CatalogError as error:
        print(f"FAIL {error}")
        raise SystemExit(1)
