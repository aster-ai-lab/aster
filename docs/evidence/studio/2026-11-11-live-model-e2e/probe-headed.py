#!/usr/bin/env python3
"""Start one real-model turn via --headed physical input (user-authorised).

Records the foreground app before and after, keeps all input inside the Aster
window, and observes the live turn with headless snapshots.
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
PROMPT = "E2E 探针：只回复 OK 两个字母，不要调用任何工具，不要修改或读取任何文件。"


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


def foreground() -> str:
    payload = cli("snapshot", "--surface", "focused", "-i", "--compact", "--timeout-ms", "8000")
    return str((payload.get("data") or {}).get("app") or "unknown")


def main() -> None:
    report: dict = {"windowId": WINDOW, "prompt": PROMPT, "authorised": "--headed physical input", "steps": []}
    report["foreground_before"] = foreground()

    initial = snapshot()
    comp = next(
        (n for n in nodes(initial) if n.get("role") == "textfield" and "描述任务" in str(n.get("name") or "")),
        None,
    )
    if not comp:
        report["result"] = "BLOCKED: composer not found"
        (HERE / "probe-headed.json").write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n")
        print(report["result"])
        return

    ref = comp["ref_id"]
    typed = cli("type", "--headed", ref, PROMPT)
    report["steps"].append({"action": "type --headed", "ok": typed.get("ok"), "error": (typed.get("error") or {}).get("code")})
    after = snapshot()
    comp_after = next(
        (n for n in nodes(after) if n.get("role") == "textfield" and "描述任务" in str(n.get("name") or "")),
        None,
    )
    report["composer_value_after_type"] = comp_after and comp_after.get("value")
    if not comp_after or PROMPT[:8] not in str(comp_after.get("value") or ""):
        report["result"] = "BLOCKED: headed type did not set composer value"
        (HERE / "probe-headed.json").write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n")
        print(report["result"])
        return

    cli("screenshot", "--window-id", WINDOW, str(HERE / "live-00-before-send.png"))
    send = next((n for n in nodes(snapshot()) if n.get("role") == "button" and n.get("name") == "发送"), None)
    if send and send.get("ref_id"):
        result = cli("click", "--headed", send["ref_id"])
        report["steps"].append({"action": "click 发送 --headed", "ok": result.get("ok"), "error": (result.get("error") or {}).get("code")})
    if not report["steps"][-1]["ok"]:
        result = cli("press", "return", "--headed")
        report["steps"].append({"action": "press return --headed", "ok": result.get("ok"), "error": (result.get("error") or {}).get("code")})

    observations = []
    for i in range(18):
        time.sleep(5)
        snap = snapshot()
        nm = [str(n.get("name")) for n in nodes(snap) if n.get("name")]
        obs = {
            "t": (i + 1) * 5,
            "ref_count": (snap.get("data") or {}).get("ref_count"),
            "ok_text": any(x.strip() == "OK" for x in nm),
            "live_markers": [x for x in nm if any(k in x for k in ("正在", "连接", "生成", "停止", "Stop", "已处理"))][:6],
            "tail": nm[-10:],
        }
        observations.append(obs)
        if i in (0, 3):
            cli("screenshot", "--window-id", WINDOW, str(HERE / f"live-{i+1:02d}-streaming.png"))
        if obs["ok_text"]:
            break
    report["observations"] = observations
    report["result"] = "LIVE_MODEL_RESPONSE_OBSERVED" if any(o["ok_text"] for o in observations) else "NO_CONFIRMED_RESPONSE"
    cli("screenshot", "--window-id", WINDOW, str(HERE / "live-final.png"))
    report["foreground_after"] = foreground()
    (HERE / "probe-headed.json").write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n")
    print(report["result"], "steps=", json.dumps(report["steps"], ensure_ascii=False))


if __name__ == "__main__":
    main()
