#!/usr/bin/env python3
"""Run real-model turns in the native Aster window and capture each render phase.

Physical input (--headed) is user-authorised for this run. All typing and
sending happens inside the Aster window; observations use headless snapshots.
"""
from __future__ import annotations

import json
import subprocess
import sys
import time
from pathlib import Path

APP = "Aster"
WINDOW = sys.argv[1] if len(sys.argv) > 1 else ""
HERE = Path(__file__).resolve().parent
SCREENS = HERE / "screens"
if not SCREENS.exists():
    SCREENS.mkdir()

TASKS = [
    {
        "id": "T1-basic-reply",
        "prompt": "只回复 OK 两个字母，不要调用任何工具，不要修改或读取任何文件。",
        "covers": ["R06", "B02", "S1", "S5", "R13", "R26"],
        "wait": 60,
    },
]


def cli(*args: str) -> dict:
    try:
        run = subprocess.run(["agent-desktop", *args], capture_output=True, text=True, timeout=60)
        return json.loads(run.stdout)
    except Exception as error:
        return {"ok": False, "error": {"code": "HARNESS", "message": str(error)}}


def snapshot() -> dict:
    return cli("snapshot", "--app", APP, "--window-id", WINDOW, "-i", "--compact", "--timeout-ms", "15000")


def nodes(payload: dict) -> list[dict]:
    out: list[dict] = []

    def walk(node):
        if not isinstance(node, dict):
            return
        out.append(node)
        for child in node.get("children") or []:
            walk(child)

    walk((payload.get("data") or {}).get("tree"))
    return out


def named(payload: dict) -> list[str]:
    return [str(n.get("name")) for n in nodes(payload) if n.get("name")]


def find_node(payload: dict, role: str, token: str) -> dict | None:
    return next(
        (n for n in nodes(payload) if n.get("role") == role and token.lower() in str(n.get("name") or "").lower() and n.get("ref_id")),
        None,
    )


def shot(name: str) -> bool:
    return cli("screenshot", "--window-id", WINDOW, str(SCREENS / f"{name}.png")).get("ok") is True


def switch_model(target: str) -> dict:
    trigger = find_node(snapshot(), "combobox", "deepseek-flash") or find_node(snapshot(), "combobox", "model")
    if not trigger:
        return {"ok": False, "reason": "model trigger not found"}
    opened = cli("click", trigger["ref_id"])
    # Model rows are statictext inside a dialog, not interactive AX nodes; -i hides them.
    payload = cli("snapshot", "--app", APP, "--window-id", WINDOW, "--timeout-ms", "12000")
    option = (
        find_node(payload, "statictext", target)
        or find_node(payload, "button", target)
        or find_node(payload, "combobox", target)
    )
    if not option:
        names = [x for x in named(payload) if any(k in x.lower() for k in ("k3", "kimi", "pro", "flash"))]
        return {"ok": False, "reason": "option not found", "opened": opened.get("ok"), "similar": names[:12]}
    clicked = cli("click", option["ref_id"])
    after = snapshot()
    return {
        "ok": bool(clicked.get("ok")),
        "option": option.get("name"),
        "model_combobox": [x for x in named(after) if any(k in x.lower() for k in ("k3", "kimi", "flash"))][-4:],
    }


def run_task(task: dict) -> dict:
    result: dict = {"id": task["id"], "prompt": task["prompt"], "covers": task["covers"], "phases": []}
    before = snapshot()
    result["before_refs"] = (before.get("data") or {}).get("ref_count")

    comp = find_node(before, "textfield", "描述任务")
    if not comp:
        result["result"] = "BLOCKED: composer not found"
        return result

    typed = cli("type", "--headed", comp["ref_id"], task["prompt"])
    after_type = snapshot()
    comp_after = find_node(after_type, "textfield", "描述任务")
    value = str((comp_after or {}).get("value") or "")
    result["type"] = {"reported_ok": typed.get("ok"), "error": (typed.get("error") or {}).get("code"), "value_set": task["prompt"][:8] in value}
    shot(f"{task['id']}-00-composer")
    if not result["type"]["value_set"]:
        result["result"] = "BLOCKED: composer value not set"
        return result

    send = find_node(snapshot(), "button", "发送")
    sent = cli("click", "--headed", send["ref_id"]) if send else {"ok": False}
    if not sent.get("ok"):
        sent = cli("press", "return", "--headed")
    result["send"] = {"ok": sent.get("ok"), "error": (sent.get("error") or {}).get("code")}

    for i in range(max(1, task["wait"] // 5)):
        time.sleep(5)
        snap = snapshot()
        nm = named(snap)
        phase = {
            "t": (i + 1) * 5,
            "refs": (snap.get("data") or {}).get("ref_count"),
            "markers": [x for x in nm if any(k in x for k in ("正在", "连接", "生成", "停止", "Stop", "已处理", "工具调用", "发生", "错误"))][:10],
            "tail": nm[-14:],
        }
        result["phases"].append(phase)
        if i in (0, 2, 5):
            shot(f"{task['id']}-{i+1:02d}-t{(i+1)*5:03d}s")
    shot(f"{task['id']}-final")
    result["result"] = "OBSERVED"
    return result


if __name__ == "__main__":
    report = {"windowId": WINDOW, "authorised": "--headed physical input", "model_switch": switch_model("K3.1 Pro"), "tasks": []}
    for task in TASKS:
        entry = run_task(task)
        report["tasks"].append(entry)
        print(entry["id"], entry.get("result"), json.dumps(entry.get("type"), ensure_ascii=False), json.dumps(entry.get("send"), ensure_ascii=False))
    (HERE / "live-tasks.json").write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n")
    print("model_switch:", json.dumps(report["model_switch"], ensure_ascii=False))
