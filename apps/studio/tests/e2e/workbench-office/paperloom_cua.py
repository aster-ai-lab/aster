#!/usr/bin/env python3
"""Open one owned ToWork fork and read the 论文精读 surface in the background.

A3S_GUI_ALLOW_MOUSE must be unset. Clicks use delivery_mode background.
The command does not move the pointer, call set_frontmost, or stop a ToWork
that is already running. A second run can reuse the pid printed as fork_pid.
"""

from __future__ import annotations

import os
import subprocess
import sys
import time
import traceback
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "task-catalog"))
from cua_guard import refuse_mouse  # noqa: E402
from window import (  # noqa: E402
    CatalogDrive,
    alive,
    find_exact,
    label_of,
    launch_fork,
    owned_fork,
    pointer,
    towork_processes,
    wait_for_fork,
    wait_for_window,
)

SCRATCH = Path("/var/folders/3d/0pn3wn612t7cs79b5ml89x440000gn/T/grok-goal-1ded7aefdc6e/implementer")
TEXT_LAYER = "Page 1"
LOCAL_TEXT = "本机文本层"
PROCESS = "处理论文"


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


class ForkFrontDrive(CatalogDrive):
    """Skip the accessibility call when the owned fork is not the front app.

    A background window snapshot warps the operator pointer. The drive records
    the block and does not raise the window.
    """

    def call(self, name: str, arguments: dict) -> dict:
        if "toyou-fork2" not in frontmost_bundle():
            raise RuntimeError("fork_not_front")
        return super().call(name, arguments)


def mark_pointer(name: str) -> None:
    point = pointer()
    print(f"pointer_{name}={point[0]},{point[1]}", flush=True)


def visible(elements: list[dict], text: str) -> bool:
    needle = " ".join(text.split())
    return any(needle in label_of(element) or needle in " ".join(str(element.get("value") or "").split()) for element in elements)


def click_named(ui: CatalogDrive, text: str, role: str | None = "AXButton") -> bool:
    ui.refresh()
    found = find_exact(ui.elements, text, role)
    if found is None:
        return False
    ui.pixel_click(found)
    time.sleep(0.6)
    return True


FIXTURE_ROOT = "toyou-fork2/compile-fixture"
FIXTURE_PDF = (
    "/Users/roylin/Library/Application Support/dev.a3s.studio/"
    "toyou-fork2/compile-fixture/paper.pdf"
)


def composer_text(field: dict) -> str:
    value = " ".join(str(field.get("value") or "").split())
    if not value or value.startswith("描述任务"):
        return ""
    return value


def composer_field(ui: CatalogDrive) -> dict | None:
    return next(
        (
            element
            for element in ui.elements
            if element.get("role") == "AXTextArea"
            and element.get("element_token")
            and any(hint in label_of(element) for hint in ("描述任务", "今天想做点什么"))
        ),
        None,
    )


def file_trigger(elements: list[dict]) -> dict | None:
    matches = [
        element
        for element in elements
        if label_of(element) == "工作区文件" and element.get("role") in ("AXButton", "AXPopUpButton") and element.get("element_token")
    ]
    return matches[-1] if matches else None


def tray_project(elements: list[dict]) -> dict | None:
    """The composer project control sits on the same row as 工作区文件, to its left."""
    files = file_trigger(elements)
    if files is None:
        return None
    anchor = files.get("frame") or {}
    ax = float(anchor.get("x", 0))
    ay = float(anchor.get("y", 0))
    matches = [
        element
        for element in elements
        if element.get("role") in ("AXButton", "AXPopUpButton")
        and element.get("element_token")
        and "项目文件夹" in label_of(element)
        and abs(float((element.get("frame") or {}).get("y", 0)) - ay) < 24
        and float((element.get("frame") or {}).get("x", 0)) < ax
    ]
    return matches[-1] if matches else None


def project_bound(elements: list[dict]) -> bool:
    """The composer tray names the fixture project. The sidebar button alone does not."""
    tray = tray_project(elements)
    if tray is None:
        return False
    blob = f"{label_of(tray)} {tray.get('value') or ''}"
    return "compile-fixture" in blob


def fixture_root_visible(elements: list[dict]) -> bool:
    for element in elements:
        blob = f"{label_of(element)} {element.get('value') or ''}"
        if FIXTURE_ROOT in blob:
            return True
    return False


def open_exact_pdf(ui: CatalogDrive) -> bool:
    """Type the owned fixture path. The field accepts only an absolute path inside the shown root."""
    field = next(
        (
            element
            for element in ui.elements
            if element.get("role") in ("AXTextField", "AXTextArea", "AXComboBox")
            and "工作区 PDF 路径" in label_of(element)
            and element.get("element_token")
        ),
        None,
    )
    if field is None:
        return False
    ui.chord(["cmd", "a"], field)
    time.sleep(0.2)
    ui.press_background("delete", field)
    time.sleep(0.2)
    ui.type_text(FIXTURE_PDF, field)
    time.sleep(0.3)
    return click_named(ui, "打开 PDF")


def open_reader(ui: CatalogDrive) -> bool:
    """Reach the shipped reader from the workbench. A sign-in wall stops here.

    The file list is opened only after the chooser shows the owned fixture root.
    Rows from any other workspace stay unclicked.
    """
    ui.refresh()
    mark_pointer("open_start")
    # The previous drive can leave the process dialog open. Close it before
    # looking for the composer, and do not treat that dialog as a new pass.
    if (
        find_exact(ui.elements, PROCESS, "AXButton") is None
        and find_exact(ui.elements, "添加 1 项", "AXButton") is None
        and find_exact(ui.elements, "取消", "AXButton") is not None
    ):
        click_named(ui, "取消")
        ui.refresh()
    if (
        find_exact(ui.elements, PROCESS, "AXButton") is None
        and visible(ui.elements, LOCAL_TEXT)
        and find_exact(ui.elements, "关闭", "AXButton") is not None
    ):
        click_named(ui, "关闭")
        ui.refresh()
    if find_exact(ui.elements, PROCESS, "AXButton") is not None:
        return True
    if find_exact(ui.elements, "登录", "AXButton") is not None and find_exact(ui.elements, "论文精读", "AXButton") is None:
        print("blocked=sign_in_required", flush=True)
        return False
    # The owned fork can be left on another rail. The composer exists on 工作.
    # A stale lazy chunk leaves the work page on the plugin error text. Leave
    # that route so the next visit loads the chunk that is on the dev server.
    if composer_field(ui) is None and visible(ui.elements, "似乎出现了一些问题"):
        other = find_exact(ui.elements, "知识", "AXButton")
        if other is not None:
            ui.pixel_click(other)
            time.sleep(0.8)
            ui.refresh()
            print("rail=leave_error", flush=True)
    if composer_field(ui) is None:
        rail = find_exact(ui.elements, "工作", "AXButton")
        if rail is not None:
            ui.pixel_click(rail)
            time.sleep(1.2)
            ui.refresh()
            print("rail=work", flush=True)
    if find_exact(ui.elements, "取消", "AXButton") is not None and find_exact(ui.elements, "添加 1 项", "AXButton") is None:
        click_named(ui, "取消")
        ui.refresh()
    if not project_bound(ui.elements):
        trigger = tray_project(ui.elements)
        if trigger is not None:
            trigger_x = float((trigger.get("frame") or {}).get("x", 0))
            ui.pixel_click(trigger)
            time.sleep(0.6)
            ui.refresh()
            search = next(
                (
                    element
                    for element in ui.elements
                    if element.get("role") in ("AXTextField", "AXSearchField", "AXComboBox")
                    and "搜索工作目录" in label_of(element)
                    and element.get("element_token")
                ),
                None,
            )
            if search is not None:
                ui.type_text("compile-fixture", search)
                time.sleep(0.5)
                ui.refresh()
            options = [
                element
                for element in ui.elements
                if element.get("element_token")
                and element.get("role") in ("AXButton", "AXPopUpButton", "AXMenuItem")
                and label_of(element) == "compile-fixture"
                and float((element.get("frame") or {}).get("x", 0)) > trigger_x - 40
            ]
            empty_menu = any(label_of(element) == "暂无工作目录" for element in ui.elements)
            if options:
                ui.pixel_click(options[0])
                time.sleep(0.6)
                ui.refresh()
            else:
                ui.pixel_click(trigger)
                time.sleep(0.3)
                ui.refresh()
                if empty_menu:
                    print("project_menu=empty", flush=True)
    print(f"project_bound={str(project_bound(ui.elements)).lower()}", flush=True)
    mark_pointer("project")
    field = composer_field(ui)
    if field is None:
        print("blocked=composer_missing", flush=True)
        return False
    ui.pixel_click(field)
    time.sleep(0.2)
    ui.refresh()
    field = composer_field(ui) or field
    if composer_text(field):
        ui.set_shown_value(field, "")
        time.sleep(0.3)
        ui.refresh()
        field = composer_field(ui) or field
    if composer_text(field):
        print("blocked=composer_not_cleared", flush=True)
        return False
    ui.type_text("@论文精读", field)
    deadline = time.time() + 8
    accepted = False
    while time.time() < deadline and not accepted:
        time.sleep(0.4)
        ui.refresh()
        field = composer_field(ui) or field
        frame = field.get("frame") or {}
        field_y = float(frame.get("y", 0))
        field_x = float(frame.get("x", 0))
        suggestions = [
            element
            for element in ui.elements
            if label_of(element).startswith("论文精读")
            and element.get("element_token")
            and element.get("role") not in ("AXWindow", "AXTextArea", "AXStaticText", "AXHeading")
            and abs(float((element.get("frame") or {}).get("y", 0)) - field_y) < 320
            and float((element.get("frame") or {}).get("x", 0)) >= field_x - 40
        ]
        if suggestions:
            ui.pixel_click(suggestions[0])
            print("suggestion=clicked", flush=True)
            mark_pointer("suggestion")
            accepted = True
            break
        # The panel footer (智能体, Tab/↩︎) is visible before a row exists.
        # Return at that moment does not accept the mention.
    if not accepted:
        print("blocked=mention_not_accepted", flush=True)
        for element in ui.elements:
            label = label_of(element)
            if "论文精读" in label or "没有已安装" in label:
                frame = element.get("frame") or {}
                print(
                    f"mention-hit role={element.get('role')} y={frame.get('y')} token={bool(element.get('element_token'))} label={label[:80]}",
                    flush=True,
                )
        return False
    click_named(ui, "发送")
    mark_pointer("send")
    time.sleep(0.8)
    ui.refresh()
    if not fixture_root_visible(ui.elements) and find_exact(ui.elements, "选择工作区中的 PDF") is None:
        deadline = time.time() + 4
        while time.time() < deadline and not fixture_root_visible(ui.elements):
            time.sleep(0.4)
            ui.refresh()
            if find_exact(ui.elements, PROCESS, "AXButton") is not None:
                return True
    print(f"fixture_root={str(fixture_root_visible(ui.elements)).lower()}", flush=True)
    if not fixture_root_visible(ui.elements):
        if find_exact(ui.elements, "取消", "AXButton") is not None:
            click_named(ui, "取消")
        print("blocked=fixture_root", flush=True)
        return False
    files = [
        element
        for element in ui.elements
        if label_of(element) == "工作区文件" and element.get("role") in ("AXPopUpButton", "AXButton") and element.get("element_token")
    ]
    if files:
        ui.pixel_click(files[-1])
        time.sleep(0.6)
    ui.refresh()
    paper = find_exact(ui.elements, "paper.pdf")
    if paper is None:
        if files:
            ui.refresh()
            opened = [
                element
                for element in ui.elements
                if label_of(element) == "工作区文件"
                and element.get("role") in ("AXPopUpButton", "AXButton")
                and element.get("element_token")
            ]
            if opened and any(label_of(element) == "搜索文件" for element in ui.elements):
                ui.pixel_click(opened[-1])
                time.sleep(0.3)
                ui.refresh()
        if not open_exact_pdf(ui):
            if find_exact(ui.elements, "取消", "AXButton") is not None:
                click_named(ui, "取消")
            print("blocked=paper_pdf", flush=True)
            return False
    else:
        ui.pixel_click(paper)
        time.sleep(0.4)
        click_named(ui, "添加 1 项")
        mark_pointer("added")
    deadline = time.time() + 12
    while time.time() < deadline:
        ui.refresh()
        text_ready = visible(ui.elements, TEXT_LAYER)
        process_ready = find_exact(ui.elements, PROCESS, "AXButton") is not None
        if text_ready and (process_ready or visible(ui.elements, LOCAL_TEXT)):
            return True
        time.sleep(0.4)
    return False


def drive(fork_pid: int | None) -> int:
    refuse_mouse()
    print("A3S_GUI_ALLOW_MOUSE=unset", flush=True)
    print("mouse_grab=0", flush=True)
    users = [(pid, command) for pid, command in towork_processes() if not owned_fork(command)]
    print("user_pids=" + (",".join(str(pid) for pid, _ in users) or "absent"), flush=True)
    operator = frontmost_bundle()
    print(f"frontmost_operator={operator}", flush=True)
    code = 1
    ui: CatalogDrive | None = None
    before_pointer: tuple[int, int] | None = None
    before_front = ""
    try:
        if fork_pid is None:
            launcher_log = SCRATCH / "paperloom-fork-launcher.log"
            launcher_log.parent.mkdir(parents=True, exist_ok=True)
            before_ids = {pid for pid, _ in towork_processes()}
            os.environ.pop("A3S_GUI_ALLOW_MOUSE", None)
            os.environ["STUDIO_RENDERER_PORT"] = "4541"
            proc = launch_fork(launcher_log, "toyou")
            pid = wait_for_fork(proc, before_ids, launcher_log)
        else:
            command = ""
            for found, row in towork_processes():
                if found == fork_pid:
                    command = row
            if not owned_fork(command):
                raise RuntimeError(f"refusing pid {fork_pid}; it is not an owned fork")
            pid = fork_pid
        print(f"fork_pid={pid}", flush=True)
        give_back_focus(operator)
        time.sleep(0.3)
        # The mention shelf is empty until the signed-in catalog arrives. Wait here,
        # after focus is settled, so a background snapshot is not taken first.
        time.sleep(8)
        before_pointer = pointer()
        quiet_until = time.time() + 2
        while time.time() < quiet_until:
            time.sleep(0.2)
            current = pointer()
            if current == before_pointer:
                break
            before_pointer = current
        before_front = frontmost_bundle()
        print(f"pointer_before={before_pointer[0]},{before_pointer[1]}", flush=True)
        print(f"frontmost_before={before_front}", flush=True)
        ui = ForkFrontDrive(pid, 0)
        window = wait_for_window(ui, pid)
        ui.window_id = window
        print(f"window={window}", flush=True)
        ui.refresh()
        print(f"ax_count={len(ui.elements)}", flush=True)
        if ui.degraded_reason:
            print(f"blocked={ui.degraded_reason}", flush=True)
        text_layer = False
        local_text = False
        if not open_reader(ui):
            print("text_before_dialog=false", flush=True)
            print("text_layer=false", flush=True)
            print("local_text=false", flush=True)
        else:
            ui.refresh()
            text_layer = visible(ui.elements, TEXT_LAYER)
            print(f"text_before_dialog={str(text_layer).lower()}", flush=True)
            point = pointer()
            print(f"pointer_before_dialog={point[0]},{point[1]}", flush=True)
            if not (text_layer and visible(ui.elements, LOCAL_TEXT)):
                opened = find_exact(ui.elements, PROCESS, "AXButton")
                if opened is not None:
                    ui.pixel_click(opened)
                    time.sleep(0.6)
                    ui.refresh()
            text_layer = text_layer or visible(ui.elements, TEXT_LAYER)
            local_text = visible(ui.elements, LOCAL_TEXT)
            print(f"text_layer={str(text_layer).lower()}", flush=True)
            print(f"local_text={str(local_text).lower()}", flush=True)
            if text_layer:
                print("observed=Page 1", flush=True)
            if local_text:
                print("observed=本机文本层", flush=True)
            if text_layer and local_text:
                code = 0
        if code != 0:
            missing = []
            if not text_layer:
                missing.append("pdf_text_layer")
            if not local_text:
                missing.append("local_text_layer_copy")
            print("blocked=" + ",".join(missing), flush=True)
            labels = []
            for element in ui.elements:
                label = label_of(element) or " ".join(str(element.get("value") or "").split())
                if label:
                    labels.append(label)
            print("ax_labels=" + " | ".join(labels[:120]), flush=True)
    except Exception:
        print("blocked=" + traceback.format_exc().rstrip().splitlines()[-1], flush=True)
        print("error=" + traceback.format_exc().rstrip(), flush=True)
    finally:
        missed = ui.guard.non_background if ui is not None else 0
        if ui is not None:
            ui.close()
        give_back_focus(operator)
        time.sleep(0.2)
        after = pointer()
        after_front = frontmost_bundle()
        print(f"pointer_after={after[0]},{after[1]}", flush=True)
        if before_pointer is not None:
            print(f"pointer_same={str(before_pointer == after).lower()}", flush=True)
        print(f"frontmost_after={after_front}", flush=True)
        if before_front:
            print(f"frontmost_same={str(before_front == after_front).lower()}", flush=True)
        for pid, _command in users:
            print(f"user_pid={pid} alive={int(alive(pid))}", flush=True)
        print(f"non_background={missed}", flush=True)
    return code


if __name__ == "__main__":
    reuse = int(sys.argv[1]) if len(sys.argv) > 1 else None
    raise SystemExit(drive(reuse))
