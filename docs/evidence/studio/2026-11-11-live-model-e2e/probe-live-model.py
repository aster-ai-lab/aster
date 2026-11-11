#!/usr/bin/env python3
"""Decisive probe: can a real-model turn be started from the native Aster window
using headless semantic input only (no --headed, no focus, no coordinates)?"""
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


def composer(payload: dict) -> dict | None:
    for node in nodes(payload):
        if node.get("role") == "textfield" and "描述任务" in str(node.get("name") or ""):
            return node
    return None


def names(payload: dict) -> list[str]:
    return [str(n.get("name")) for n in nodes(payload) if n.get("name")]


def main() -> None:
    report: dict = {"windowId": WINDOW, "prompt": PROMPT, "delivery": "headless-semantic", "steps": []}
    initial = snapshot()
    comp = composer(initial)
    report["initial"] = {
        "ok": initial.get("ok"),
        "ref_count": (initial.get("data") or {}).get("ref_count"),
        "composer": comp and {"ref": comp.get("ref_id"), "value": comp.get("value")},
    }
    if not comp:
        report["result"] = "BLOCKED: composer not found"
        (HERE / "probe.json").write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n")
        print(report["result"])
        return

    ref = comp["ref_id"]
    for method, args in (("set-value", ["set-value", ref, PROMPT]), ("type", ["type", ref, PROMPT])):
        result = cli(*args)
        report["steps"].append({"method": method, "ok": result.get("ok"), "error": (result.get("error") or {}).get("code")})
        after = snapshot()
        current = composer(after)
        value = current and current.get("value")
        report["steps"][-1]["value_after"] = value
        if value and PROMPT[:10] in str(value):
            report["input_method"] = method
            report["input_ref"] = current.get("ref_id")
            break
        ref = (current or comp).get("ref_id")
        report["steps"][-1]["next_ref"] = ref

    if not report.get("input_method"):
        report["result"] = "BLOCKED: headless semantic input cannot set the composer value"
        (HERE / "probe.json").write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n")
        print(report["result"])
        return

    # Send without focusing: prefer the send button, fall back to a key combo scoped to the app.
    sent = False
    for node in nodes(snapshot()):
        if node.get("role") == "button" and node.get("name") == "发送" and node.get("ref_id"):
            res = cli("click", node["ref_id"])
            sent = bool(res.get("ok"))
            report["send"] = {"method": "click 发送", "ok": sent, "error": (res.get("error") or {}).get("code")}
            break
    if not sent:
        res = cli("press", "return", "--app", APP, "--window-id", WINDOW)
        report["send"] = {"method": "press return --app", "ok": res.get("ok"), "error": (res.get("error") or {}).get("code")}
        sent = bool(res.get("ok"))

    # Observe a real turn for up to 120s.
    observations = []
    for i in range(24):
        time.sleep(5)
        snap = snapshot()
        nm = names(snap)
        observations.append({
            "t": (i + 1) * 5,
            "ref_count": (snap.get("data") or {}).get("ref_count"),
            "has_stop": any("停止" in x or "Stop" in x for x in nm),
            "has_working": any("正在" in x or "连接" in x or "生成" in x for x in nm),
            "ok_text": any(x.strip() == "OK" for x in nm),
            "sample": nm[-12:],
        })
        if observations[-1]["ok_text"] or (i >= 3 and not observations[-1]["has_working"] and observations[-1]["ref_count"]):
            break
    report["observations"] = observations
    report["result"] = "LIVE_RESPONSE_OBSERVED" if any(o["ok_text"] for o in observations) else "NO_CONFIRMED_MODEL_RESPONSE"
    cli("screenshot", "--window-id", WINDOW, str(HERE / "live-model-probe.png"))
    (HERE / "probe.json").write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n")
    print(report["result"], json.dumps(report.get("send"), ensure_ascii=False))


if __name__ == "__main__":
    main()
