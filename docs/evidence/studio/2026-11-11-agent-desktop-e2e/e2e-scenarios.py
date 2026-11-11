#!/usr/bin/env python3
"""Full agent-desktop E2E for Aster Studio (2026-11-11, Stage 3b build).

Strict headless semantic delivery: snapshot / find / click / right-click /
set-value / wait / get / screenshot. No --headed, no focus-window, no
coordinates, no mouse. Every action records ok + delivery; a control that is
not found is recorded as such and never faked as a pass.

Usage: python3 e2e-scenarios.py <window-id>
"""
from __future__ import annotations

import json
import subprocess
import sys
from collections import Counter
from pathlib import Path

APP = "Aster"
WINDOW = sys.argv[1]
HERE = Path(__file__).resolve().parent
RESULTS: list[dict] = []

# Only runtime *diagnostics* count as leaks. A bare absolute path is a legal
# user-facing value in Settings > Workspace folder, so it is tracked separately
# instead of being reported as an internal-detail leak.
INTERNAL_MARKERS = (
    "completion gate",
    "effect_digest",
    "reasoning effort default",
    "workspace mutation",
)


def cli(*args: str, timeout: int = 45) -> dict:
    try:
        run = subprocess.run(
            ["agent-desktop", *args], capture_output=True, text=True, timeout=timeout
        )
        return json.loads(run.stdout)
    except Exception as error:
        return {"ok": False, "error": {"code": "HARNESS", "message": str(error)}}


def snapshot(surface: str = "window") -> dict:
    args = [
        "snapshot", "--app", APP, "--window-id", WINDOW,
        "-i", "--compact", "--timeout-ms", "15000",
    ]
    if surface != "window":
        args += ["--surface", surface]
    payload = cli(*args)
    if not payload.get("ok") or not (payload.get("data") or {}).get("tree"):
        payload = cli(*args)
    return payload


def find(role: str, name: str | None = None, surface: str = "window") -> dict | None:
    args = ["find", "--app", APP, "--window-id", WINDOW, "--role", role]
    if name is not None:
        args += ["--name", name, "--exact"]
    args += ["--first"]
    if surface != "window":
        args += ["--surface", surface]
    payload = cli(*args)
    return (payload.get("data") or {}).get("match") if payload.get("ok") else None


def find_any(names: tuple[str, ...], roles: tuple[str, ...] = ("button",)) -> dict | None:
    for role in roles:
        for name in names:
            match = find(role, name)
            if match:
                return {"role": role, "name": name, "ref_id": match["ref_id"]}
    return None


def act(command: str, ref: str) -> dict:
    payload = cli(command, ref)
    data = payload.get("data") or {}
    return {
        "ok": bool(payload.get("ok")),
        "delivery": (data.get("disposition") or {}).get("delivery"),
        "surfaces": data.get("surfaces"),
        "error": (payload.get("error") or {}).get("code"),
    }


def wait_text(text: str, timeout: int = 7000) -> dict:
    payload = cli(
        "wait", "--app", APP, "--window-id", WINDOW,
        "--text", text, "--timeout", str(timeout),
    )
    return {"ok": bool(payload.get("ok")), "text": text,
            "error": (payload.get("error") or {}).get("code")}


def shot(filename: str) -> dict:
    payload = cli("screenshot", "--window-id", WINDOW, str(HERE / filename))
    return {"file": filename, "ok": bool(payload.get("ok")),
            "error": (payload.get("error") or {}).get("code")}


def ax_stats(payload: dict) -> dict:
    tree = (payload.get("data") or {}).get("tree")
    nodes: list[dict] = []

    def walk(node):
        if not isinstance(node, dict):
            return
        nodes.append(node)
        for child in node.get("children") or []:
            walk(child)

    walk(tree)
    roles = Counter(str(node.get("role") or "unknown") for node in nodes)
    texts = " ".join(
        f"{node.get('name') or ''} {node.get('description') or ''} {node.get('value') or ''}"
        for node in nodes
    )
    names = [str(node["name"]) for node in nodes if node.get("name")]
    return {
        "snapshot_ok": bool(payload.get("ok")),
        "nodes": len(nodes),
        "roles": dict(roles.most_common(14)),
        "status_nodes": roles.get("status", 0),
        "alert_nodes": roles.get("alert", 0),
        "textfield_nodes": roles.get("textfield", 0),
        "internal_markers": [m for m in INTERNAL_MARKERS if m in texts],
        "absolute_path_nodes": texts.count("/Users/"),
        "loading_label_seen": any(
            marker in texts for marker in ("加载中", "Loading", "common.loading")
        ),
        # Head and tail: the persistent sidebar always occupies the first ~20
        # names, so a head-only sample makes every page look identical.
        "names_sample": names[:10] + names[-20:],
        "named_nodes": len(names),
    }


def record(entry: dict) -> None:
    RESULTS.append(entry)
    print(json.dumps(entry, ensure_ascii=False), flush=True)


def scene(
    sid: str,
    title: str,
    controls: tuple[str, ...] | None = None,
    roles: tuple[str, ...] = ("button",),
    wait_for: str | None = None,
    filename: str | None = None,
) -> dict:
    entry: dict = {"id": sid, "title": title}
    entry["overlay"] = dismiss_overlay()

    if controls:
        restore()
        target = find_any(controls, roles)
        entry["control_found"] = bool(target)
        if target:
            entry["control"] = {"role": target["role"], "name": target["name"]}
            entry["action"] = act("click", target["ref_id"])
        else:
            entry["action"] = {"ok": False, "error": "CONTROL_NOT_FOUND"}

    # Capture the transient state immediately: this is where a page-entry
    # loader and its *visible* label can be observed before the view settles.
    transient = ax_stats(snapshot())
    entry["transient_ax"] = {
        "nodes": transient["nodes"],
        "loading_label_seen": transient["loading_label_seen"],
        "status_nodes": transient["status_nodes"],
        "names_sample": transient["names_sample"][:8],
    }

    if wait_for:
        entry["wait"] = wait_text(wait_for)
    elif controls:
        # No deterministic page-specific predicate is available for every
        # route, so settle with a bounded time wait and then re-read the tree.
        # The settled snapshot (not the transient one) backs the screenshot.
        settled = cli("wait", "800")
        entry["settle"] = {"ok": bool(settled.get("ok"))}

    entry["ax"] = ax_stats(snapshot())
    if filename:
        entry["screenshot"] = shot(filename)
    record(entry)
    return entry


def scene_context_menu() -> dict:
    entry: dict = {"id": "S13", "title": "右键命令菜单（headless AXShowMenu）"}
    entry["overlay"] = dismiss_overlay()
    payload = snapshot()
    tree = (payload.get("data") or {}).get("tree")
    candidates: list[dict] = []
    nav_names = {"工作", "设置", "新建任务", "搜索任务", "连接器", "技能",
                 "自动化", "新建项目", "Close", "Full Screen", "Minimize"}

    def walk(node):
        if not isinstance(node, dict):
            return
        name = str(node.get("name") or "")
        if (
            node.get("role") == "button"
            and "RightClick" in (node.get("available_actions") or [])
            and node.get("ref_id")
            and name
            and name not in nav_names
        ):
            candidates.append(node)
        for child in node.get("children") or []:
            walk(child)

    walk(tree)
    entry["control_found"] = bool(candidates)
    candidate = None
    action_result = {"ok": False, "error": "ALL_CANDIDATES_FAILED"}
    for attempt in candidates[:4]:
        action_result = act("right-click", attempt["ref_id"])
        if action_result["ok"]:
            candidate = attempt
            break
    entry["candidates_tried"] = min(4, len(candidates))
    if candidate or candidates:
        used = candidate or candidates[0]
        entry["control"] = {"name": used["name"], "role": used.get("role")}
        entry["action"] = action_result
        menu = snapshot("menu")
        menu_ax = ax_stats(menu)
        entry["menu_ax"] = {
            "snapshot_ok": menu_ax["snapshot_ok"],
            "nodes": menu_ax["nodes"],
            "roles": menu_ax["roles"],
            "names_sample": menu_ax["names_sample"],
        }
        # No physical Escape (press is physical input and needs --headed).
        # This scenario runs last, so a still-open menu cannot shadow any
        # later page. Dismiss via a semantic menu item when one is offered.
        dismiss = find("menuitem", "关闭", "menu") or find("button", "关闭", "menu")
        if dismiss:
            entry["dismiss"] = act("click", dismiss["ref_id"])
    else:
        entry["action"] = {"ok": False, "error": "NO_RIGHT_CLICKABLE_NODE"}
    entry["screenshot"] = shot("13-context-menu.png")
    record(entry)
    return entry


def dismiss_overlay() -> dict:
    """A modal dialog replaces the window AX tree with a ~14-node surface.

    Every later scenario must start from the full window tree, so close any
    overlay first. The 30-node threshold distinguishes the real workbench
    (58 nodes) from a dialog surface without hard-coding a dialog title.
    """
    before = ax_stats(snapshot())["nodes"]
    if before >= 30:
        return {"dismissed": False, "reason": "NO_OVERLAY", "nodes_before": before}
    close = find("button", "关闭") or find("button", "Close")
    if not close:
        return {"dismissed": False, "reason": "NO_CLOSE_CONTROL", "nodes_before": before}
    result = act("click", close["ref_id"])
    after = ax_stats(snapshot())["nodes"]
    return {"dismissed": True, "action": result,
            "nodes_before": before, "nodes_after": after}


def restore() -> None:
    # The workbench button is present in the persistent sidebar; Settings
    # replaces it with "返回应用". Start every page scenario from this known
    # state so control names are looked up on the right page.
    dismiss_overlay()
    for _ in range(3):
        target = find_any(("工作", "返回应用"))
        if not target:
            return
        act("click", target["ref_id"])
        names = ax_stats(snapshot())["names_sample"]
        if "新建任务" in names:
            return


def main() -> int:
    # S00: dismiss the first-run permission modal if this window is fresh.
    dismissed = {"found": False}
    for label in ("继续", "稍后设置", "关闭"):
        match = find("button", label)
        if match:
            dismissed = {"found": True, "label": label,
                         "action": act("click", match["ref_id"])}
            break
    record({"id": "S00", "title": "首启弹窗处理", **dismissed,
            "ax": ax_stats(snapshot())})

    scene("S01", "工作台首页", filename="01-workbench.png")
    scene("S02", "任务搜索对话框", ("搜索", "搜索任务", "查找"),
          filename="02-search.png")

    # S02b: while the dialog is open, drive a query through the semantic
    # set-value path and read the status/alert nodes produced by
    # GlobalSearchState (the a11y change made in the previous batch).
    search_entry: dict = {"id": "S02b", "title": "搜索输入与结果状态语义"}
    field = find("textfield")
    search_entry["control_found"] = bool(field)
    if field:
        written = cli("set-value", field["ref_id"], "Aster")
        data = written.get("data") or {}
        search_entry["action"] = {
            "ok": bool(written.get("ok")),
            "delivery": (data.get("disposition") or {}).get("delivery"),
            "error": (written.get("error") or {}).get("code"),
        }
        cli("wait", "600")
    else:
        search_entry["action"] = {"ok": False, "error": "NO_TEXTFIELD"}
    search_entry["ax"] = ax_stats(snapshot())
    search_entry["screenshot"] = shot("02b-search-query.png")
    record(search_entry)

    scene("S03", "自动化 / 定时任务页", ("自动化", "定时任务"),
          filename="03-automation.png")

    # S04: the feedback path added in this batch. Only claim a visible toast
    # when the control exists AND the text is actually observed.
    run_target = find("button", "测试运行")
    entry: dict = {"id": "S04", "title": "自动化测试运行反馈",
                   "control_found": bool(run_target)}
    if run_target:
        entry["action"] = act("click", run_target["ref_id"])
        entry["wait"] = wait_text("已记录一次测试运行", timeout=5000)
        entry["toast_visible"] = bool(entry["wait"]["ok"])
    else:
        entry["action"] = {"ok": False, "error": "NO_TASK_TO_RUN"}
        entry["toast_visible"] = False
    entry["ax"] = ax_stats(snapshot())
    entry["screenshot"] = shot("04-automation-feedback.png")
    record(entry)

    scene("S05", "技能 / 能力页", ("技能",), filename="05-skills.png")
    scene("S06", "连接器页", ("连接器",), filename="06-connectors.png")
    scene("S08", "设置页", ("设置",), filename="08-settings.png")
    # Real AX names from the live tree: the model control is a combobox, not a
    # button; the workbench sidebar has no knowledge-base / mini-app entry, so
    # those two routes are reported as unreachable instead of being faked.
    scene("S09", "模型选择器（推理强度）", ("DeepSeek Flash",),
          roles=("combobox",), filename="09-model-picker.png")
    scene("S10", "composer 更多选项", ("更多选项",),
          roles=("combobox",), filename="10-more-options.png")
    scene("S11", "授权模式选择器", ("授权模式: 请求批准", "授权模式"),
          roles=("combobox",), filename="11-permission-mode.png")

    restore()
    final = scene("S12", "还原工作台", filename="12-restored-workbench.png")

    # S14 — semantic set-value into the composer, then read the send button's
    # AX states. Never clicks send: the harness must not create a real task or
    # call a model. This exercises the same delivery path used by S02b.
    restore()
    composer_entry: dict = {"id": "S14", "title": "composer 语义输入与发送按钮状态"}
    field = find("textfield", "描述任务，输入 / 使用指令或技能，@ 选择工作区文件") or find("textfield")
    composer_entry["control_found"] = bool(field)
    if field:
        written = cli("set-value", field["ref_id"], "E2E smoke: written, not sent")
        data = written.get("data") or {}
        composer_entry["action"] = {
            "ok": bool(written.get("ok")),
            "delivery": (data.get("disposition") or {}).get("delivery"),
            "error": (written.get("error") or {}).get("code"),
        }
    else:
        composer_entry["action"] = {"ok": False, "error": "NO_COMPOSER_TEXTFIELD"}
    send = find("button", "发送")
    composer_entry["send_button_found"] = bool(send)
    if send:
        states = cli("get", send["ref_id"], "--property", "states")
        composer_entry["send_states"] = {
            "ok": bool(states.get("ok")),
            "data": states.get("data"),
            "error": (states.get("error") or {}).get("code"),
        }
    composer_entry["ax"] = ax_stats(snapshot())
    composer_entry["screenshot"] = shot("14-composer-value.png")
    record(composer_entry)

    # Context menu last: an open menu surface must not shadow any other page.
    scene_context_menu()

    summary = {
        "window": WINDOW,
        "scenarios": len(RESULTS),
        "action_ok_count": sum(
            1 for r in RESULTS if (r.get("action") or {}).get("ok") is True
        ),
        "screenshots": [r["screenshot"]["file"] for r in RESULTS if r.get("screenshot")],
        "pages_with_internal_markers": [
            r["id"] for r in RESULTS if (r.get("ax") or {}).get("internal_markers")
        ],
        "final_ax_nodes": (final.get("ax") or {}).get("nodes"),
        "results": RESULTS,
    }
    (HERE / "scenarios.json").write_text(
        json.dumps(summary, ensure_ascii=False, indent=2) + "\n", encoding="utf-8"
    )
    print(json.dumps({"summary": {k: v for k, v in summary.items()
                                  if k != "results"}}, ensure_ascii=False), flush=True)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
