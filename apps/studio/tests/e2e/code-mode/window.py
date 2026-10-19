#!/usr/bin/env python3
"""Code-mode window gate. Background clicks only. A3S_GUI_ALLOW_MOUSE stays unset.

Two fresh signed-in launches each check the homepage, then the placement
sends on a temporary git repository. A third launch checks the changes panel.
The local ACP peer in agent.py answers the send. It does not call a model.
"""

from __future__ import annotations

import argparse
import fcntl
import json
import os
import re
import shutil
import signal
import sqlite3
import subprocess
import sys
import tempfile
import time
import traceback
import uuid
from datetime import datetime, timezone
from pathlib import Path
from typing import Any

HERE = Path(__file__).resolve().parent
CATALOG = HERE.parent / "task-catalog"
sys.path.insert(0, str(CATALOG))
sys.path.insert(0, str(HERE.parent / "towork"))

from cua_guard import refuse_mouse  # noqa: E402
from window import (  # noqa: E402
    CatalogDrive,
    alive,
    find_exact,
    launch_fork,
    pointer,
    process_rows,
    runtime_for,
    towork_processes,
    wait_for_fork,
    wait_for_window,
)

AGENT = HERE / "agent.py"
PORT_DEFAULT = 4344
SLUG = "toyou"
TOPIC_ASCII = "fix login"
TOPIC_CJK = "修复登录"
ASCII_BRANCH = re.compile(r"^code/fix-login-[a-z0-9]{4}$")
CJK_BRANCH = re.compile(r"^code/task-[a-z0-9]{4}$")
NEED_PROJECT = "先选择一个项目文件夹，再创建工作树"
EMPTY_TURN = "这一回合还没有改文件"
BRANCH_EXISTS = "这个分支已经存在"
WRITE_MARKER = "code-mode-e2e-write"
NOTE_NAME = "e2e-note.txt"
NOTE_LINE = "e2e-note-line"


class GateFailure(RuntimeError):
    pass


def norm(value: object) -> str:
    return " ".join(str(value or "").split())


def texts_of(elements: list[dict[str, Any]]) -> list[str]:
    found: list[str] = []
    for element in elements:
        for key in ("label", "value", "description", "title", "help"):
            raw = element.get(key)
            if isinstance(raw, str) and raw.strip():
                found.append(norm(raw))
    return found


def fixture_root(prefix: str) -> Path:
    """Short directory so the changes header can expose the whole cwd."""
    return Path(tempfile.mkdtemp(prefix=prefix, dir="/tmp"))


def path_visible(texts: list[str], path: Path) -> bool:
    candidates = {str(path), str(path.resolve())}
    for text in texts:
        if any(candidate in text for candidate in candidates):
            return True
        if path.name in text and any(candidate.startswith(text) and len(text) >= len(path.name) + 1 for candidate in candidates):
            return True
    return False


def path_debug(ui: CatalogDrive, path: Path) -> str:
    interesting = [
        text
        for text in texts_of(ui.elements)
        if path.name in text
        or text.startswith("/")
        or text in {"变更", "请选择模型", "无法创建工作树", NEED_PROJECT, "这一回合还没有改文件", "创建拉取请求"}
    ]
    return " | ".join(interesting[:12])


def brief(element: dict[str, Any]) -> dict[str, str]:
    kept: dict[str, str] = {}
    for key, value in element.items():
        if key in {"element_token", "children", "frame"}:
            continue
        text = norm(value) if not isinstance(value, (dict, list)) else json.dumps(value, ensure_ascii=False)
        kept[str(key)] = text[:180]
    return kept


def radio_on(element: dict[str, Any]) -> bool:
    for key in ("value", "checked", "selected", "aria-checked", "aria_checked"):
        raw = element.get(key)
        if raw in (True, 1, "1", "true", "True", "on"):
            return True
    traits = element.get("traits")
    if isinstance(traits, str) and "selected" in traits.lower():
        return True
    if isinstance(traits, list) and any("selected" in str(item).lower() for item in traits):
        return True
    return False


def workspace_group(elements: list[dict[str, Any]]) -> bool:
    for element in elements:
        if norm(element.get("label")) != "工作区":
            continue
        if element.get("role") in {"AXRadioGroup", "AXGroup"}:
            return True
    return False


class Log:
    def __init__(self, path: Path) -> None:
        self.path = path
        self.handle = path.open("w", encoding="utf-8")

    def line(self, text: str) -> None:
        print(text, flush=True)
        self.handle.write(text + "\n")
        self.handle.flush()

    def close(self) -> None:
        self.handle.close()


def git_lines(path: Path) -> list[str]:
    if not path.is_file():
        return []
    return path.read_text(encoding="utf-8", errors="replace").splitlines()


def worktree_adds(lines: list[str]) -> list[str]:
    return [line for line in lines if line.startswith("worktree add ") or " worktree add " in f" {line}"]


def has_dash_b(line: str) -> bool:
    return "-b" in line.split()


def branch_after_flag(line: str) -> str:
    parts = line.split()
    if "-b" not in parts:
        return ""
    index = parts.index("-b")
    if index + 1 >= len(parts):
        return ""
    return parts[index + 1]


def existing_checkout(line: str) -> tuple[str, str] | None:
    """`worktree add <dest> <branch>` with no -b. Returns dest, branch."""
    parts = line.split()
    if parts[:2] != ["worktree", "add"] or "-b" in parts or len(parts) < 4:
        return None
    return parts[2], parts[3]


def parent_names(path: Path) -> set[str]:
    return {item.name for item in path.iterdir()} if path.is_dir() else set()


def lock_held(slug: str) -> bool:
    path = Path(tempfile.gettempdir()) / f"dev.a3s.studio.{slug}.lock"
    path.touch()
    handle = path.open("a+")
    try:
        fcntl.flock(handle.fileno(), fcntl.LOCK_EX | fcntl.LOCK_NB)
    except BlockingIOError:
        return True
    fcntl.flock(handle.fileno(), fcntl.LOCK_UN)
    handle.close()
    return False


def next_free_fork() -> str:
    """The same lowest free forkN that `just fork toyou` will claim."""
    for number in range(1, 31):
        if not lock_held(f"toyou-fork{number}"):
            if number < 3:
                raise GateFailure(f"refusing to store a project on toyou-fork{number}")
            return f"fork{number}"
    raise GateFailure("no free fork for the project list")


def fork_localstorage(fork_id: str, port: int) -> Path:
    if fork_id in {"fork1", "fork2"}:
        raise GateFailure(f"refusing the look window profile {fork_id}")
    bundle = Path.home() / "Library/WebKit" / f"dev.a3s.studio.toyou.{fork_id}"
    port_le = int(port).to_bytes(2, "little")
    found: list[Path] = []
    if bundle.is_dir():
        for origin in bundle.rglob("origin"):
            try:
                data = origin.read_bytes()
            except OSError:
                continue
            if b"127.0.0.1" not in data or port_le not in data:
                continue
            db = origin.parent / "LocalStorage" / "localstorage.sqlite3"
            if db.is_file():
                found.append(db)
    if not found:
        raise GateFailure(f"localstorage for 127.0.0.1:{port} missing under {fork_id}")
    found.sort(key=lambda item: item.stat().st_mtime, reverse=True)
    chosen = found[0]
    text = str(chosen)
    if "toyou-fork1" in text or "toyou-fork2" in text or "toyou.fork1" in text or "toyou.fork2" in text:
        raise GateFailure("refusing the look window profile")
    return chosen


def reset_saved_task(cursor: sqlite3.Cursor) -> None:
    """Open the homepage. A restored task hides the mode radios."""
    found = cursor.execute("SELECT value FROM ItemTable WHERE key='cs_cache_persist'").fetchone()
    if not found:
        return
    try:
        data = json.loads(bytes(found[0]).decode("utf-16-le"))
    except (UnicodeError, json.JSONDecodeError):
        return
    if not isinstance(data, dict):
        return
    data["ui.agent.last_used_session_id"] = None
    data["ui.agent.last_used_workspace_id"] = None
    encoded = json.dumps(data, ensure_ascii=False, separators=(",", ":")).encode("utf-16-le")
    cursor.execute("UPDATE ItemTable SET value=? WHERE key='cs_cache_persist'", (encoded,))


def write_project_row(db: Path, repo: Path) -> str:
    """Replace the page project list with this fixture. The app must be closed."""
    project_id = str(uuid.uuid4())
    stamp = datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%S.%f")[:-3] + "Z"
    row = {
        "id": project_id,
        "name": repo.name,
        "path": str(repo),
        "type": "user",
        "orderKey": "a0",
        "createdAt": stamp,
        "updatedAt": stamp,
    }
    payload = json.dumps([row], ensure_ascii=False).encode("utf-16-le")
    connection = sqlite3.connect(db)
    try:
        cursor = connection.cursor()
        cursor.execute(
            "INSERT INTO ItemTable(key, value) VALUES(?, ?) ON CONFLICT(key) DO UPDATE SET value=excluded.value",
            ("a3s.workbench.projects", payload),
        )
        cursor.execute("DELETE FROM ItemTable WHERE key=?", ("workbench.pending_project:v1",))
        cursor.execute("DELETE FROM ItemTable WHERE key=?", ("workbench.code-session.v1",))
        reset_saved_task(cursor)
        connection.commit()
        cursor.execute("PRAGMA wal_checkpoint(TRUNCATE)")
    finally:
        connection.close()
    for suffix in ("-wal", "-shm"):
        extra = Path(str(db) + suffix)
        if extra.is_file() and extra.stat().st_size == 0:
            extra.unlink()
    return project_id


def store_fixture_project(repo: Path, port: int) -> str:
    fork_id = next_free_fork()
    db = fork_localstorage(fork_id, port)
    write_project_row(db, repo)
    print(f"project_store={fork_id}", flush=True)
    print("project_panel=false", flush=True)
    return fork_id


def make_fixture(root: Path) -> Path:
    repo = root / root.name
    repo.mkdir()
    run = lambda args, cwd=repo: subprocess.run(
        ["/usr/bin/git", *args], cwd=cwd, check=True, capture_output=True, text=True
    )
    run(["init", "-b", "main"])
    run(["config", "user.email", "code-mode-e2e@example.com"])
    run(["config", "user.name", "code-mode-e2e"])
    (repo / "README.md").write_text("hello\n", encoding="utf-8")
    run(["add", "README.md"])
    run(["commit", "-m", "init"])
    run(["branch", "release"])
    return repo


def remove_fixture(repo: Path, log: Log) -> None:
    parent = repo.parent
    created = [repo]
    if parent.is_dir():
        for item in parent.iterdir():
            if item == repo or item.name.startswith(repo.name + "-"):
                created.append(item)
    seen: list[Path] = []
    for path in created:
        resolved = path.resolve()
        if parent.resolve() not in resolved.parents and resolved != parent.resolve():
            raise GateFailure(f"refusing to delete outside the fixture parent: {resolved}")
        if resolved in seen:
            continue
        seen.append(resolved)
        if path.is_dir():
            shutil.rmtree(path)
        elif path.exists():
            path.unlink()
        log.line(f"deleted={resolved}")
    log.line("other_worktrees_removed=0")


def git_branches(repo: Path) -> set[str]:
    done = subprocess.run(
        ["/usr/bin/git", "-C", str(repo), "for-each-ref", "--format=%(refname:short)", "refs/heads"],
        capture_output=True,
        text=True,
    )
    if done.returncode != 0:
        raise GateFailure(done.stderr.strip() or "git branch failed")
    return {line.strip() for line in done.stdout.splitlines() if line.strip()}


def head_name(repo: Path) -> str:
    done = subprocess.run(
        ["/usr/bin/git", "-C", str(repo), "rev-parse", "--abbrev-ref", "HEAD"],
        capture_output=True,
        text=True,
        check=True,
    )
    return done.stdout.strip()


def wrapper_dir(root: Path) -> Path:
    directory = root / "bin"
    directory.mkdir(parents=True, exist_ok=True)
    script = directory / "git"
    script.write_text(
        "#!/bin/sh\n"
        'if [ -n "${CODE_MODE_GIT_LOG:-}" ]; then printf "%s\\n" "$*" >> "$CODE_MODE_GIT_LOG"; fi\n'
        'exec /usr/bin/git "$@"\n',
        encoding="utf-8",
    )
    script.chmod(0o755)
    return directory


def page_urls(launcher_log: Path) -> tuple[str, str]:
    if not launcher_log.is_file():
        return "", ""
    page = ""
    web = ""
    for line in launcher_log.read_text(encoding="utf-8", errors="replace").splitlines():
        if "studio-page-load " in line:
            page = line.split("studio-page-load ", 1)[1].strip()
        elif "studio-webview-url " in line:
            web = line.split("studio-webview-url ", 1)[1].strip()
    return page, web


def settle_pointer() -> tuple[int, int]:
    quiet = pointer()
    deadline = time.time() + 2
    while time.time() < deadline:
        time.sleep(0.2)
        current = pointer()
        if current == quiet:
            return current
        quiet = current
    return pointer()


def stop_launch(proc: subprocess.Popen[str], pid: int) -> None:
    if proc.poll() is None:
        try:
            os.killpg(proc.pid, signal.SIGTERM)
        except (ProcessLookupError, PermissionError):
            pass
    deadline = time.time() + 12
    while time.time() < deadline and (alive(pid) or proc.poll() is None):
        time.sleep(0.3)
    if proc.poll() is None:
        try:
            os.killpg(proc.pid, signal.SIGKILL)
        except (ProcessLookupError, PermissionError):
            pass


def refresh(ui: CatalogDrive) -> None:
    ui.refresh()


def wait_exact(ui: CatalogDrive, text: str, role: str | None, timeout: float) -> dict[str, Any]:
    deadline = time.time() + timeout
    while time.time() < deadline:
        refresh(ui)
        found = find_exact(ui.elements, text, role)
        if found is not None:
            return found
        time.sleep(0.35)
    raise GateFailure(f"missing {role or 'control'} {text}")


def click_exact(ui: CatalogDrive, text: str, role: str | None = None, timeout: float = 15) -> None:
    element = wait_exact(ui, text, role, timeout)
    ui.pixel_click(element)


def name_field(ui: CatalogDrive) -> dict[str, Any]:
    fields = [
        element
        for element in ui.elements
        if element.get("role") == "AXTextField" and element.get("element_token")
    ]
    branched = [element for element in fields if norm(element.get("value")).startswith("code/")]
    if len(branched) == 1:
        return branched[0]
    if len(fields) == 1:
        return fields[0]
    raise GateFailure(f"branch name field ambiguous ({len(fields)})")


def branch_button_value(ui: CatalogDrive) -> str:
    refresh(ui)
    for element in ui.elements:
        if element.get("role") == "AXButton" and norm(element.get("label")) == "分支":
            return norm(element.get("value"))
    return ""


def composer(ui: CatalogDrive) -> dict[str, Any]:
    """The field next to the project tray. The page fallback is a different, larger box."""
    areas = [
        element
        for element in ui.elements
        if element.get("role") in {"AXTextArea", "AXTextField"} and element.get("element_token")
    ]
    if not areas:
        raise GateFailure("composer missing")
    trigger = project_trigger(ui)
    if trigger is not None:
        origin = frame_center(trigger)

        def distance(element: dict[str, Any]) -> float:
            point = frame_center(element)
            return (point[0] - origin[0]) ** 2 + (point[1] - origin[1]) ** 2

        return min(areas, key=distance)

    def area(element: dict[str, Any]) -> float:
        frame = element.get("frame") or {}
        return float(frame.get("w") or 0) * float(frame.get("h") or 0)

    return max(areas, key=area)


def set_composer(ui: CatalogDrive, text: str) -> None:
    # Focus starts the rich editor. Keystrokes during that swap never reach the draft.
    area = composer(ui)
    ui.pixel_click(area)
    time.sleep(0.8)
    refresh(ui)
    area = composer(ui)
    ui.pixel_click(area)
    ui.chord(["cmd", "a"], area)
    ui.press_background("delete", area)
    if text:
        ui.type_text(text, area)
    time.sleep(0.35)
    refresh(ui)
    if text and text not in composer_value(ui):
        area = composer(ui)
        ui.pixel_click(area)
        ui.type_text(text, area)
        time.sleep(0.35)
        refresh(ui)
    return composer(ui)


def composer_value(ui: CatalogDrive) -> str:
    area = composer(ui)
    return norm(area.get("value") or area.get("label"))


def element_names(element: dict[str, Any]) -> list[str]:
    names: list[str] = []
    for key in ("label", "value", "description", "title", "help"):
        raw = element.get(key)
        if isinstance(raw, str) and norm(raw):
            names.append(norm(raw))
    return names


def frame_x(element: dict[str, Any]) -> float:
    frame = element.get("frame") or {}
    try:
        return float(frame.get("x") or 0)
    except (TypeError, ValueError):
        return 0.0


def project_trigger(ui: CatalogDrive, name: str | None = None) -> dict[str, Any] | None:
    """The composer project button. The sidebar uses the same folder name farther left."""
    labeled: list[dict[str, Any]] = []
    named: list[dict[str, Any]] = []
    for element in ui.elements:
        if element.get("role") not in {"AXButton", "AXPopUpButton", "AXMenuButton"} or not element.get("element_token"):
            continue
        texts = element_names(element)
        if any(text in {"项目文件夹", "选择项目文件夹"} or text.startswith("项目文件夹 ") for text in texts):
            labeled.append(element)
        elif name and frame_x(element) > 240 and any(text == name for text in texts):
            named.append(element)
    pool = labeled or named
    if not pool:
        return None
    return max(pool, key=frame_x)


def project_shows(trigger: dict[str, Any], name: str) -> bool:
    return any(name in text for text in element_names(trigger))


def project_cleared(trigger: dict[str, Any]) -> bool:
    names = element_names(trigger)
    if "选择项目文件夹" in names:
        return True
    return "项目文件夹" in names and not any(text.startswith("项目文件夹 ") for text in names)


def button_dump(ui: CatalogDrive) -> str:
    rows: list[str] = []
    for element in ui.elements:
        if element.get("role") not in {"AXButton", "AXPopUpButton", "AXMenuButton", "AXRadioButton"}:
            continue
        text = " / ".join(element_names(element))[:90]
        if text:
            rows.append(f"{element.get('role')}:{text}")
    return " | ".join(rows[:30])


def control_dump(ui: CatalogDrive) -> str:
    """Names and frames for the composer controls a failed send needs."""
    rows: list[str] = []
    for element in ui.elements:
        role = element.get("role")
        if role not in {"AXButton", "AXPopUpButton", "AXMenuButton", "AXRadioButton", "AXTextArea", "AXTextField"}:
            continue
        text = " / ".join(element_names(element))[:70]
        if not text:
            continue
        if not any(piece in text for piece in ("发送", "语音", "项目", "分支", "工作树", "目录", "停止", "暂停", "加载", "模型", "fix login")):
            continue
        frame = element.get("frame") or {}
        rows.append(f"{role}:{text}@{frame.get('x')},{frame.get('y')} en={element.get('enabled')}")
    return " | ".join(rows[:24])


def project_row(ui: CatalogDrive, name: str) -> dict[str, Any] | None:
    """Menu row in the composer popover. Skip the sidebar project button on the left."""
    roles = {"AXButton", "AXMenuItem", "AXCell", "AXRow", "AXMenuButton"}
    found: list[dict[str, Any]] = []
    for element in ui.elements:
        if element.get("role") not in roles or not element.get("element_token"):
            continue
        if frame_x(element) <= 240:
            continue
        if any(text == name for text in element_names(element)):
            found.append(element)
    if not found:
        return None
    return max(found, key=frame_x)


def choose_project(ui: CatalogDrive, repo: Path) -> None:
    name = repo.name
    trigger = None
    deadline = time.time() + 8
    while time.time() < deadline:
        refresh(ui)
        trigger = project_trigger(ui, name)
        if trigger is not None:
            break
        time.sleep(0.3)
    if trigger is None:
        raise GateFailure(f"project button missing {button_dump(ui)}")
    if project_shows(trigger, name):
        return
    ui.pixel_click(trigger)
    deadline = time.time() + 8
    row = None
    while time.time() < deadline and row is None:
        time.sleep(0.3)
        refresh(ui)
        row = project_row(ui, name)
    if row is None:
        raise GateFailure(f"project row missing {name} {button_dump(ui)}")
    ui.pixel_click(row)
    deadline = time.time() + 10
    while time.time() < deadline:
        refresh(ui)
        trigger = project_trigger(ui, name)
        if trigger is not None and project_shows(trigger, name):
            return
        time.sleep(0.3)
    raise GateFailure(f"project {name} was not selected {button_dump(ui)}")


def clear_project(ui: CatalogDrive) -> None:
    trigger = None
    deadline = time.time() + 8
    while time.time() < deadline:
        refresh(ui)
        trigger = project_trigger(ui)
        if trigger is not None:
            break
        time.sleep(0.3)
    if trigger is None:
        raise GateFailure(f"project button missing {button_dump(ui)}")
    if project_cleared(trigger):
        return
    ui.pixel_click(trigger)
    click_exact(ui, "不使用工作目录", None, 8)
    deadline = time.time() + 8
    while time.time() < deadline:
        refresh(ui)
        trigger = project_trigger(ui)
        if trigger is not None and project_cleared(trigger):
            return
        time.sleep(0.3)
    raise GateFailure(f"project was not cleared {button_dump(ui)}")


def ensure_home(ui: CatalogDrive) -> None:
    refresh(ui)
    if find_exact(ui.elements, "日常办公", "AXRadioButton") is not None:
        return
    stopped = find_exact(ui.elements, "停止", "AXButton")
    if stopped is not None:
        ui.pixel_click(stopped)
        time.sleep(0.4)
    click_exact(ui, "新建任务", "AXButton", 12)
    wait_exact(ui, "日常办公", "AXRadioButton", 20)


def select_code(ui: CatalogDrive) -> None:
    click_exact(ui, "代码开发", "AXRadioButton", 12)
    wait_exact(ui, "当前目录", "AXRadioButton", 8)


def select_placement(ui: CatalogDrive, name: str) -> None:
    click_exact(ui, name, "AXRadioButton", 8)


def open_branch_menu(ui: CatalogDrive) -> None:
    click_exact(ui, "分支", "AXButton", 8)
    time.sleep(0.3)


def shown_branches(ui: CatalogDrive) -> list[str]:
    refresh(ui)
    found = []
    for text in texts_of(ui.elements):
        if text.startswith("code/") or text == "release":
            found.append(text)
    return found


def frame_center(element: dict[str, Any]) -> tuple[float, float]:
    frame = element.get("frame") or {}
    try:
        x = float(frame.get("x") or 0)
        y = float(frame.get("y") or 0)
        w = float(frame.get("w") or 0)
        h = float(frame.get("h") or 0)
    except (TypeError, ValueError):
        return 0.0, 0.0
    return x + w / 2, y + h / 2


def nearest_named(ui: CatalogDrive, names: set[str], origin: tuple[float, float] | None) -> dict[str, Any] | None:
    best: dict[str, Any] | None = None
    best_dist: float | None = None
    for element in ui.elements:
        if element.get("role") not in {"AXButton", "AXPopUpButton"} or not element.get("element_token"):
            continue
        if not any(text in names or text.endswith("发送") for text in element_names(element)):
            continue
        if "发送" not in names and not any(text in names for text in element_names(element)):
            continue
        if origin is None:
            return element
        point = frame_center(element)
        dist = (point[0] - origin[0]) ** 2 + (point[1] - origin[1]) ** 2
        if best_dist is None or dist < best_dist:
            best = element
            best_dist = dist
    return best


def send(ui: CatalogDrive) -> None:
    """Press the live 发送 next to the composer. The page fallback does not place a worktree."""
    deadline = time.time() + 12
    saw_disabled = False
    paused = False
    while time.time() < deadline:
        refresh(ui)
        if find_exact(ui.elements, "语音输入", "AXButton") is not None:
            time.sleep(0.35)
            continue
        try:
            origin = frame_center(composer(ui))
        except GateFailure:
            origin = None
        best = nearest_named(ui, {"发送"}, origin)
        if best is not None:
            if str(best.get("enabled")).lower() in {"false", "0"}:
                saw_disabled = True
                time.sleep(0.35)
                continue
            ui.pixel_click(best)
            return
        # A restored turn shows 暂停 instead of 发送 until it is stopped.
        if not paused:
            pause = nearest_named(ui, {"暂停", "停止"}, origin)
            if pause is not None:
                ui.pixel_click(pause)
                paused = True
                deadline = time.time() + 12
        time.sleep(0.35)
    if saw_disabled:
        raise GateFailure(f"send disabled {control_dump(ui)}")
    raise GateFailure(f"missing send {control_dump(ui)}")


def wait_live_composer(ui: CatalogDrive) -> None:
    """The homepage fallback has 语音输入 and its send never calls placeCodeSend."""
    deadline = time.time() + 15
    while time.time() < deadline:
        refresh(ui)
        if find_exact(ui.elements, "语音输入", "AXButton") is None and project_trigger(ui) is not None:
            return
        time.sleep(0.3)
    raise GateFailure(f"live composer missing {control_dump(ui)}")


def prepare_code_send(ui: CatalogDrive, placement: str, log: Log) -> None:
    ensure_home(ui)
    select_code(ui)
    wait_live_composer(ui)
    log.line("composer_surface=live")
    select_placement(ui, placement)


def wait_text_path(ui: CatalogDrive, path: Path, timeout: float) -> bool:
    deadline = time.time() + timeout
    while time.time() < deadline:
        refresh(ui)
        if path_visible(texts_of(ui.elements), path):
            return True
        time.sleep(0.3)
    return False


def wait_text(ui: CatalogDrive, text: str, timeout: float) -> bool:
    deadline = time.time() + timeout
    while time.time() < deadline:
        refresh(ui)
        if text in texts_of(ui.elements):
            return True
        time.sleep(0.3)
    return False


def new_dirs(before: set[str], parent: Path) -> list[Path]:
    return [parent / name for name in sorted(parent_names(parent) - before) if (parent / name).is_dir()]


def write_launch_failure(evidence: Path, launcher_log: Path, exc: BaseException) -> None:
    """The launcher process never opened a window. A login shell is not this case."""
    text = launcher_log.read_text(encoding="utf-8", errors="replace") if launcher_log.is_file() else ""
    (evidence / "cua-launch-failure.log").write_text(
        f"{type(exc).__name__}: {exc}\n{text}",
        encoding="utf-8",
    )


def launch(evidence: Path, port: int, git_log: Path, write_receipt: Path, name: str) -> tuple[subprocess.Popen[str], int, CatalogDrive, Path]:
    launcher_log = evidence / f"launcher-{name}.log"
    before = {pid for pid, _command in towork_processes()}
    saved = {
        key: os.environ.get(key)
        for key in ("STUDIO_RENDERER_PORT", "A3S_ACP_AGENT_BIN", "CODE_MODE_GIT_LOG", "CODE_MODE_WRITE_RESULT", "PATH")
    }
    os.environ["STUDIO_RENDERER_PORT"] = str(port)
    os.environ["A3S_ACP_AGENT_BIN"] = str(AGENT)
    os.environ["CODE_MODE_GIT_LOG"] = str(git_log)
    os.environ["CODE_MODE_WRITE_RESULT"] = str(write_receipt)
    os.environ["PATH"] = str(wrapper_dir(evidence / f"wrap-{name}")) + os.pathsep + os.environ.get("PATH", "")
    os.environ.pop("A3S_GUI_ALLOW_MOUSE", None)
    proc: subprocess.Popen[str] | None = None
    try:
        proc = launch_fork(launcher_log, SLUG)
        pid = wait_for_fork(proc, before, launcher_log)
    except Exception as exc:
        exited = proc is None or proc.poll() is not None
        if proc is not None and proc.poll() is None:
            try:
                os.killpg(proc.pid, signal.SIGTERM)
            except (ProcessLookupError, PermissionError):
                pass
        if exited:
            write_launch_failure(evidence, launcher_log, exc)
        raise
    finally:
        for key, value in saved.items():
            if value is None:
                os.environ.pop(key, None)
            else:
                os.environ[key] = value
    try:
        command = next((item for found, item in process_rows() if found == pid), "")
        if "/toyou-fork2/" in command or "towork-cua" in command:
            raise GateFailure("refusing the look window or a cua-marked fork")
        ui = CatalogDrive(pid, 0)
        window_id = wait_for_window(ui, pid)
        ui.window_id = window_id
        size = ui._window_size()
        if size is None or size[0] < 1000 or size[1] < 700:
            ui.set_window_size(1440, 920)
        trace_pointer(ui)
        return proc, pid, ui, launcher_log
    except Exception:
        stop_launch(proc, pid)
        raise


def trace_pointer(ui: CatalogDrive) -> None:
    """Record pointer changes. Background actions must leave it where it was."""
    original = ui.guard.call
    last = {"point": pointer()}

    def wrapped(invoke, name, arguments):
        before = pointer()
        if before != last["point"]:
            ui.guard.actions.append(
                f"POINTER_DRIFT {last['point'][0]},{last['point'][1]}->{before[0]},{before[1]} before {name}"
            )
        result = original(invoke, name, arguments)
        after = pointer()
        if after != before:
            ui.guard.actions.append(f"POINTER_MOVE {name} {before[0]},{before[1]}->{after[0]},{after[1]}")
        last["point"] = after
        return result

    ui.guard.call = wrapped


def assert_signed_in(launcher_log: Path, port: int, log: Log) -> None:
    deadline = time.time() + 25
    page = ""
    web = ""
    while time.time() < deadline:
        page, web = page_urls(launcher_log)
        if page:
            break
        time.sleep(0.4)
    url = page or web
    log.line(f"renderer_url={url or 'missing'}")
    if not url or "/windows/main/index.html" not in url or f":{port}/" not in url:
        if url and "/windows/main/index.html" not in url:
            log.line("homepage=login_shell")
        raise GateFailure(f"signed-in renderer missing: {url or 'no page-load'}")


def record_mouse(log: Log, ui: CatalogDrive, before: tuple[int, int]) -> str | None:
    after = pointer()
    same = before == after and ui.guard.non_background == 0
    log.line("A3S_GUI_ALLOW_MOUSE=unset")
    log.line(f"pointer_before={before[0]},{before[1]}")
    log.line(f"pointer_after={after[0]},{after[1]}")
    log.line(f"pointer_same={str(before == after).lower()}")
    for action in ui.guard.actions:
        log.line(action)
    log.line(f"non_background={ui.guard.non_background}")
    if same:
        return None
    return "mouse delivery or pointer moved"


def run_home(ui: CatalogDrive, log: Log, run: int) -> None:
    log.line(f"home_run={run}")
    # A restored task has no mode radios. 「新建任务」 returns to the homepage.
    deadline = time.time() + 20
    while time.time() < deadline:
        refresh(ui)
        if find_exact(ui.elements, "日常办公", "AXRadioButton") is not None:
            break
        time.sleep(0.35)
    else:
        ensure_home(ui)
    refresh(ui)
    if ui.degraded_reason == "web_labels_absent":
        log.line("homepage=web_labels_absent")
        raise GateFailure("web labels absent")
    chat = find_exact(ui.elements, "日常办公", "AXRadioButton")
    code = find_exact(ui.elements, "代码开发", "AXRadioButton")
    if chat is None or code is None:
        log.line("homepage=web_labels_absent")
        raise GateFailure("mode radios missing")
    log.line("radio_daily=" + json.dumps(brief(chat), ensure_ascii=False))
    log.line("radio_code=" + json.dumps(brief(code), ensure_ascii=False))
    if not radio_on(chat) or radio_on(code):
        raise GateFailure("日常办公 is not the checked mode")
    if workspace_group(ui.elements):
        raise GateFailure("工作区 is showing before 代码开发")
    log.line("workspace_group_before=false")
    log.line("daily_checked=true")
    click_exact(ui, "代码开发", "AXRadioButton")
    current = wait_exact(ui, "当前目录", "AXRadioButton", 8)
    worktree = wait_exact(ui, "新工作树", "AXRadioButton", 8)
    if not current.get("element_token") or not worktree.get("element_token"):
        raise GateFailure("placement radios have no token")
    log.line("placement_names=当前目录,新工作树")
    log.line(f"home_run_pass={run}")


def run_placement(ui: CatalogDrive, log: Log, run: int, repo: Path, runtime: dict[str, Any], git_log: Path) -> None:
    log.line(f"placement_run={run}")
    log.line(f"fixture={repo}")
    log.line("model_call=false")
    log.line("model_oracle=false")
    name = repo.name
    parent = repo.parent
    if not runtime.get("apiBaseUrl"):
        raise GateFailure("signed-in runtime is missing")
    log.line("runtime=present")
    prepare_code_send(ui, "新工作树", log)
    clear_project(ui)
    set_composer(ui, TOPIC_ASCII)
    typed = composer_value(ui)
    if TOPIC_ASCII not in typed:
        raise GateFailure(f"composer missing topic: {typed[:160]}")
    before_names = parent_names(parent)
    before_git = len(git_lines(git_log))
    send(ui)
    seen: list[str] = []
    interesting = ("选择", "模型", "工作树", "无法", "分支", "工作区", "加载", "不可访问")
    deadline = time.time() + 10
    found = False
    while time.time() < deadline:
        refresh(ui)
        texts = texts_of(ui.elements)
        if NEED_PROJECT in texts:
            found = True
            break
        for text in texts:
            if any(piece in text for piece in interesting) and text not in seen:
                seen.append(text)
        time.sleep(0.25)
    if not found:
        raise GateFailure(
            f"need-project toast missing seen={' | '.join(seen[:12])} controls={control_dump(ui)}"
        )
    refresh(ui)
    draft = composer_value(ui)
    if TOPIC_ASCII not in draft:
        raise GateFailure(f"draft changed: {draft}")
    if new_dirs(before_names, parent) or len(worktree_adds(git_lines(git_log)[before_git:])) != 0:
        raise GateFailure("need-project created a worktree")
    log.line("need_project=true")
    log.line("draft_kept=true")

    choose_project(ui, repo)
    select_placement(ui, "当前目录")
    set_composer(ui, TOPIC_ASCII)
    before_names = parent_names(parent)
    before_git = len(git_lines(git_log))
    send(ui)
    if not wait_text_path(ui, repo, 25):
        raise GateFailure(f"checkout session cwd missing {path_debug(ui, repo)}")
    if new_dirs(before_names, parent) or worktree_adds(git_lines(git_log)[before_git:]):
        raise GateFailure("checkout created a sibling")
    log.line(f"checkout_cwd={repo}")
    log.line("checkout_sibling=false")

    prepare_code_send(ui, "新工作树", log)
    choose_project(ui, repo)
    set_composer(ui, TOPIC_ASCII)
    before_names = parent_names(parent)
    before_git = len(git_lines(git_log))
    send(ui)
    deadline = time.time() + 25
    created: list[str] = []
    while time.time() < deadline:
        created = worktree_adds(git_lines(git_log)[before_git:])
        if created:
            break
        time.sleep(0.4)
    flagged = [line for line in created if has_dash_b(line)]
    if not flagged:
        raise GateFailure("new worktree invocation missing")
    branch = branch_after_flag(flagged[-1])
    sibling = Path(flagged[-1].split()[flagged[-1].split().index("-b") + 2])
    if not sibling.is_dir() or sibling.name != f"{repo.name}-{branch.replace('/', '-')}":
        raise GateFailure(f"sibling mismatch {sibling} {branch}")
    if not wait_text_path(ui, sibling, 20):
        raise GateFailure(f"worktree cwd missing {path_debug(ui, sibling)}")
    log.line(f"worktree_cwd={sibling}")
    log.line(f"worktree_branch={branch}")

    prepare_code_send(ui, "新工作树", log)
    choose_project(ui, repo)
    open_branch_menu(ui)
    click_exact(ui, "release", None, 8)
    branches_before = git_branches(repo)
    before_names = parent_names(parent)
    before_git = len(git_lines(git_log))
    set_composer(ui, TOPIC_ASCII)
    send(ui)
    deadline = time.time() + 25
    added: list[str] = []
    while time.time() < deadline:
        added = worktree_adds(git_lines(git_log)[before_git:])
        if added:
            break
        time.sleep(0.4)
    parsed = [existing_checkout(line) for line in added]
    parsed = [item for item in parsed if item is not None]
    if not parsed or any(has_dash_b(line) for line in added):
        raise GateFailure(f"release invocation used -b: {added}")
    dest, checked = parsed[-1]
    if checked != "release" or "release" not in Path(dest).name or not Path(dest).is_dir():
        raise GateFailure(f"release checkout mismatch {dest} {checked}")
    if head_name(Path(dest)) != "release":
        raise GateFailure("release is not the worktree head")
    if git_branches(repo) != branches_before:
        raise GateFailure("release send created a branch")
    if not wait_text_path(ui, Path(dest), 20):
        raise GateFailure(f"release cwd missing {path_debug(ui, Path(dest))}")
    log.line(f"release_cwd={dest}")
    log.line("release_dash_b=false")
    log.line("release_branch_created=false")

    prepare_code_send(ui, "新工作树", log)
    choose_project(ui, repo)
    set_composer(ui, TOPIC_ASCII)
    open_branch_menu(ui)
    generated = ""
    deadline = time.time() + 8
    while time.time() < deadline and not generated:
        refresh(ui)
        generated = next((text for text in texts_of(ui.elements) if ASCII_BRANCH.match(text)), "")
        if not generated:
            time.sleep(0.3)
    if not generated:
        raise GateFailure("generated branch option missing")
    click_exact(ui, generated, None, 8)
    time.sleep(0.3)
    before_plan = branch_button_value(ui)
    open_branch_menu(ui)
    click_exact(ui, "新建分支", None, 8)
    if not wait_text(ui, "名称留空就用自动名称", 8):
        raise GateFailure("create dialog missing")
    refresh(ui)
    field = name_field(ui)
    ui.pixel_click(field)
    ui.chord(["cmd", "a"], field)
    ui.type_text("release", field)
    click_exact(ui, "创建", "AXButton", 8)
    if not wait_text(ui, BRANCH_EXISTS, 8):
        raise GateFailure("existing-branch error missing")
    if not wait_text(ui, "名称留空就用自动名称", 2):
        raise GateFailure("create dialog closed")
    click_exact(ui, "取消", "AXButton", 8)
    time.sleep(0.4)
    after_plan = branch_button_value(ui)
    if after_plan != before_plan:
        raise GateFailure(f"stored plan changed {before_plan} -> {after_plan}")
    log.line("branch_exists=true")
    log.line("stored_plan_unchanged=true")

    set_composer(ui, TOPIC_ASCII)
    open_branch_menu(ui)
    click_exact(ui, "新建分支", None, 8)
    if not wait_text(ui, "名称留空就用自动名称", 8):
        raise GateFailure("blank dialog missing")
    refresh(ui)
    field = name_field(ui)
    ui.pixel_click(field)
    ui.chord(["cmd", "a"], field)
    ui.press_background("delete", field)
    click_exact(ui, "创建", "AXButton", 8)
    if not wait_text(ui, "code/fix-login-", 8):
        raise GateFailure("ascii generated name missing")
    refresh(ui)
    if not any(ASCII_BRANCH.match(text) for text in texts_of(ui.elements)):
        raise GateFailure("ascii branch pattern missing")
    log.line("blank_ascii=true")

    set_composer(ui, TOPIC_CJK)
    open_branch_menu(ui)
    click_exact(ui, "新建分支", None, 8)
    if not wait_text(ui, "名称留空就用自动名称", 8):
        raise GateFailure("cjk dialog missing")
    refresh(ui)
    field = name_field(ui)
    ui.pixel_click(field)
    ui.chord(["cmd", "a"], field)
    ui.press_background("delete", field)
    click_exact(ui, "创建", "AXButton", 8)
    if not wait_text(ui, "code/task-", 8):
        raise GateFailure("cjk generated name missing")
    refresh(ui)
    if not any(CJK_BRANCH.match(text) for text in texts_of(ui.elements)):
        raise GateFailure("cjk branch pattern missing")
    log.line("blank_cjk=true")
    log.line(f"placement_run_pass={run}")


def run_changes(ui: CatalogDrive, log: Log, repo: Path, runtime: dict[str, Any], git_log: Path, receipt: Path) -> None:
    log.line("changes_run=1")
    log.line(f"fixture={repo}")
    log.line("model_call=false")
    log.line("model_oracle=false")
    if not runtime.get("apiBaseUrl"):
        raise GateFailure("signed-in runtime is missing")
    log.line("runtime=present")
    prepare_code_send(ui, "当前目录", log)
    choose_project(ui, repo)
    set_composer(ui, "看一下目录")
    send(ui)
    if not wait_text(ui, EMPTY_TURN, 30):
        raise GateFailure("empty turn missing")
    log.line(f"empty_turn={EMPTY_TURN}")
    set_composer(ui, WRITE_MARKER)
    send(ui)
    if not wait_text(ui, NOTE_NAME, 30) or not wait_text(ui, NOTE_LINE, 10):
        raise GateFailure("written diff missing")
    refresh(ui)
    buttons = [
        element
        for element in ui.elements
        if element.get("role") == "AXButton" and NOTE_NAME in norm(element.get("label")) and element.get("element_token")
    ]
    if len(buttons) != 1:
        raise GateFailure(f"file buttons={len(buttons)}")
    ui.pixel_click(buttons[0])
    time.sleep(0.4)
    refresh(ui)
    if NOTE_LINE not in texts_of(ui.elements) or EMPTY_TURN in texts_of(ui.elements):
        raise GateFailure("one diff was not shown")
    written = receipt.read_text(encoding="utf-8").strip() if receipt.is_file() else ""
    disk = Path(written) if written else repo / NOTE_NAME
    if not disk.is_file() or NOTE_LINE not in disk.read_text(encoding="utf-8"):
        raise GateFailure("tool write file missing")
    log.line(f"tool_write={disk}")
    log.line("diff_count=1")

    before_git = git_lines(git_log)
    click_exact(ui, "创建拉取请求", "AXButton", 12)
    click_exact(ui, "取消", "AXButton", 8)
    time.sleep(0.8)
    refresh(ui)
    if any("push" in line.split() for line in git_lines(git_log)[len(before_git) :]):
        raise GateFailure("cancel ran git push")
    if find_exact(ui.elements, "创建拉取请求", "AXButton") is None:
        raise GateFailure("pull request button disappeared after cancel")
    if find_exact(ui.elements, "拉取请求", "AXLink") is not None:
        raise GateFailure("cancel created a pull request link")
    pushing = False
    for pid, command in process_rows():
        if "git" in command and " push " in f" {command} ":
            pushing = True
    if pushing:
        raise GateFailure("git push still running")
    log.line("cancel_push=false")

    gh = subprocess.run(["gh", "auth", "status"], capture_output=True, text=True)
    origin = subprocess.run(["/usr/bin/git", "-C", str(repo), "remote", "get-url", "origin"], capture_output=True, text=True)
    if gh.returncode != 0 or origin.returncode != 0:
        reason = "gh" if gh.returncode != 0 else "origin"
        if gh.returncode != 0 and origin.returncode != 0:
            reason = "gh,origin"
        log.line(f"pull_request_confirm=skip reason={reason}")
    else:
        click_exact(ui, "创建拉取请求", "AXButton", 8)
        click_exact(ui, "确认", "AXButton", 8)
        if not wait_text(ui, "拉取请求", 40):
            raise GateFailure("pull request link missing")
        log.line("pull_request_confirm=created")
    log.line("changes_run_pass=1")


def runtime_of(pid: int) -> dict[str, Any]:
    deadline = time.time() + 20
    while time.time() < deadline:
        runtime = runtime_for(SLUG, pid)
        if runtime and runtime.get("token") and runtime.get("apiBaseUrl"):
            return runtime
        time.sleep(0.4)
    raise GateFailure("runtime file missing")


def guard_fork2(log: Log) -> None:
    rows = [(pid, command) for pid, command in towork_processes() if "/toyou-fork2/" in command]
    log.line("look_window_pids=" + (",".join(str(pid) for pid, _command in rows) or "absent"))


def run_pair(evidence: Path, port: int, home: Log, place: Log, run: int) -> None:
    root = fixture_root(f"a3s-code-mode-{run}-")
    repo = make_fixture(root)
    git_log = evidence / f"git-{run}.log"
    receipt = evidence / f"write-{run}.txt"
    proc = None
    pid = 0
    ui = None
    before = None
    error: BaseException | None = None
    mouse: str | None = None
    try:
        store_fixture_project(repo, port)
        proc, pid, ui, launcher_log = launch(evidence, port, git_log, receipt, f"run{run}")
        before = settle_pointer()
        home.line(f"fork_pid={pid}")
        place.line(f"fork_pid={pid}")
        assert_signed_in(launcher_log, port, home)
        run_home(ui, home, run)
        runtime = runtime_of(pid)
        run_placement(ui, place, run, repo, runtime, git_log)
    except BaseException as exc:
        error = exc
    finally:
        if ui is not None and before is not None:
            home_mouse = record_mouse(home, ui, before)
            place_mouse = record_mouse(place, ui, before)
            mouse = home_mouse or place_mouse
            ui.close()
        if proc is not None:
            stop_launch(proc, pid)
        remove_fixture(repo, place)
        if root.exists():
            shutil.rmtree(root, ignore_errors=True)
    if error is not None:
        raise error
    if mouse is not None:
        raise GateFailure(mouse)


def run_change_gate(evidence: Path, port: int, changes: Log) -> None:
    root = fixture_root("a3s-code-mode-changes-")
    repo = make_fixture(root)
    git_log = evidence / "git-changes.log"
    receipt = evidence / "write-changes.txt"
    proc = None
    pid = 0
    ui = None
    before = None
    error: BaseException | None = None
    mouse: str | None = None
    try:
        store_fixture_project(repo, port)
        proc, pid, ui, launcher_log = launch(evidence, port, git_log, receipt, "changes")
        before = settle_pointer()
        changes.line(f"fork_pid={pid}")
        assert_signed_in(launcher_log, port, changes)
        runtime = runtime_of(pid)
        run_changes(ui, changes, repo, runtime, git_log, receipt)
    except BaseException as exc:
        error = exc
    finally:
        if ui is not None and before is not None:
            mouse = record_mouse(changes, ui, before)
            ui.close()
        if proc is not None:
            stop_launch(proc, pid)
        remove_fixture(repo, changes)
        if root.exists():
            shutil.rmtree(root, ignore_errors=True)
    if error is not None:
        raise error
    if mouse is not None:
        raise GateFailure(mouse)


def self_check() -> None:
    line = "worktree add /tmp/repo-release release"
    flagged = "worktree add -b code/fix-login-ab12 /tmp/repo-code-fix-login-ab12"
    if has_dash_b(line) or not has_dash_b(flagged):
        raise SystemExit("dash-b parser failed")
    if existing_checkout(line) != ("/tmp/repo-release", "release"):
        raise SystemExit("existing parser failed")
    if existing_checkout(flagged) is not None:
        raise SystemExit("existing parser accepted -b")
    if branch_after_flag(flagged) != "code/fix-login-ab12":
        raise SystemExit("branch parser failed")
    if not ASCII_BRANCH.match("code/fix-login-ab12") or not CJK_BRANCH.match("code/task-ab12"):
        raise SystemExit("branch pattern failed")
    if ASCII_BRANCH.match("code/task-ab12"):
        raise SystemExit("ascii pattern matched a task branch")
    with tempfile.TemporaryDirectory() as tmp:
        db = Path(tmp) / "localstorage.sqlite3"
        repo = Path(tmp) / "repo-name"
        repo.mkdir()
        setup = sqlite3.connect(db)
        setup.cursor().execute(
            "CREATE TABLE ItemTable (key TEXT UNIQUE ON CONFLICT REPLACE, value BLOB NOT NULL ON CONFLICT FAIL)"
        )
        setup.cursor().execute(
            "INSERT INTO ItemTable(key, value) VALUES(?, ?)",
            (
                "cs_cache_persist",
                json.dumps({"ui.agent.last_used_session_id": "session-old"}).encode("utf-16-le"),
            ),
        )
        setup.cursor().execute(
            "INSERT INTO ItemTable(key, value) VALUES(?, ?)",
            ("workbench.code-session.v1", "{}".encode("utf-16-le")),
        )
        setup.commit()
        setup.close()
        write_project_row(db, repo)
        connection = sqlite3.connect(db)
        cursor = connection.cursor()
        stored = cursor.execute("SELECT value FROM ItemTable WHERE key='a3s.workbench.projects'").fetchone()
        pending = cursor.execute(
            "SELECT value FROM ItemTable WHERE key='workbench.pending_project:v1'"
        ).fetchone()
        session = cursor.execute(
            "SELECT value FROM ItemTable WHERE key='workbench.code-session.v1'"
        ).fetchone()
        cache = json.loads(
            cursor.execute("SELECT value FROM ItemTable WHERE key='cs_cache_persist'").fetchone()[0].decode("utf-16-le")
        )
        connection.close()
        if session is not None or cache.get("ui.agent.last_used_session_id") is not None:
            raise SystemExit("saved task was not cleared")
        parsed = json.loads(stored[0].decode("utf-16-le"))
        if parsed[0]["path"] != str(repo) or parsed[0]["name"] != repo.name or parsed[0]["type"] != "user":
            raise SystemExit("project row was not stored")
        if pending is not None:
            raise SystemExit("pending project was stored")
    sample = Path("/tmp/a3s-code-mode-1-abcd/a3s-code-mode-1-abcd")
    if not path_visible([str(sample)], sample):
        raise SystemExit("full path not visible")
    if not path_visible(["/tmp/a3s-code-mode-1-abcd/a3s"], sample):
        raise SystemExit("truncated cwd was rejected")
    if path_visible(["/tmp/other"], sample):
        raise SystemExit("unrelated path matched")
    print("self_check=pass")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--evidence", type=Path)
    parser.add_argument("--port", type=int, default=PORT_DEFAULT)
    parser.add_argument("--only", choices=["all", "home", "placement", "changes", "self-check"], default="all")
    args = parser.parse_args()
    if args.only == "self-check":
        self_check()
        return
    if args.evidence is None:
        raise SystemExit("--evidence is required")
    refuse_mouse()
    if os.environ.get("A3S_GUI_ALLOW_MOUSE"):
        raise SystemExit("A3S_GUI_ALLOW_MOUSE is set")
    args.evidence.mkdir(parents=True, exist_ok=True)
    stale_launch_failure = args.evidence / "cua-launch-failure.log"
    if stale_launch_failure.exists():
        stale_launch_failure.unlink()
    home = Log(args.evidence / "gate-home.log")
    place = Log(args.evidence / "gate-placement.log")
    changes = Log(args.evidence / "gate-changes.log")
    try:
        guard_fork2(home)
        if args.only in {"all", "home", "placement"}:
            runs = (1, 2)
            for number in runs:
                if args.only == "home":
                    root = Path(tempfile.mkdtemp(prefix="a3s-code-mode-home-"))
                    git_log = args.evidence / f"git-home-{number}.log"
                    receipt = args.evidence / f"write-home-{number}.txt"
                    proc, pid, ui, launcher_log = launch(args.evidence, args.port, git_log, receipt, f"home{number}")
                    before = settle_pointer()
                    mouse = None
                    try:
                        home.line(f"fork_pid={pid}")
                        assert_signed_in(launcher_log, args.port, home)
                        run_home(ui, home, number)
                    finally:
                        mouse = record_mouse(home, ui, before)
                        ui.close()
                        stop_launch(proc, pid)
                        shutil.rmtree(root, ignore_errors=True)
                    if mouse is not None:
                        raise GateFailure(mouse)
                elif args.only == "placement":
                    run_pair_placement_only(args.evidence, args.port, place, number)
                else:
                    run_pair(args.evidence, args.port, home, place, number)
        if args.only in {"all", "changes"}:
            run_change_gate(args.evidence, args.port, changes)
        guard_fork2(home)
        home.line("look_window_after=see_pids")
    finally:
        home.close()
        place.close()
        changes.close()


def run_pair_placement_only(evidence: Path, port: int, place: Log, run: int) -> None:
    root = fixture_root(f"a3s-code-mode-{run}-")
    repo = make_fixture(root)
    git_log = evidence / f"git-{run}.log"
    receipt = evidence / f"write-{run}.txt"
    store_fixture_project(repo, port)
    proc, pid, ui, launcher_log = launch(evidence, port, git_log, receipt, f"place{run}")
    before = settle_pointer()
    mouse = None
    try:
        place.line(f"fork_pid={pid}")
        assert_signed_in(launcher_log, port, place)
        run_placement(ui, place, run, repo, runtime_of(pid), git_log)
    finally:
        mouse = record_mouse(place, ui, before)
        ui.close()
        stop_launch(proc, pid)
        remove_fixture(repo, place)
        shutil.rmtree(root, ignore_errors=True)
    if mouse is not None:
        raise GateFailure(mouse)


if __name__ == "__main__":
    try:
        main()
    except Exception:
        traceback.print_exc()
        raise
