#!/usr/bin/env python3
"""Try every headless input route for the connector search field.

F12 is the last unverified case: set-value and type both returned
ACTION_FAILED and get --property value stayed empty. This probes focus+type,
click+type, clipboard+paste and press, recording the readable value after each.
No --headed is used anywhere.
"""
from __future__ import annotations

import json
import os
import subprocess
import time
from pathlib import Path

ROOT = Path("/Users/roylin/code/aster")
HERE = Path(__file__).resolve().parent
APP = "Aster"
PORT = 4325
FIELD = "搜索连接器或关键词"


def cli(*args, timeout=45):
    try:
        r = subprocess.run(["agent-desktop", *args], capture_output=True,
                           text=True, timeout=timeout)
        return json.loads(r.stdout)
    except Exception as e:
        return {"ok": False, "error": {"code": "HARNESS", "message": str(e)}}


def flat(p):
    out = []

    def walk(n):
        if not isinstance(n, dict):
            return
        out.append(n)
        for c in n.get("children") or []:
            walk(c)
    walk((p.get("data") or {}).get("tree"))
    return out


def snap(w):
    a = ["snapshot", "--app", APP, "--window-id", w, "-i", "--compact",
         "--timeout-ms", "15000"]
    p = cli(*a)
    if not p.get("ok") or not (p.get("data") or {}).get("tree"):
        p = cli(*a)
    return flat(p)


def ref(w, role, name):
    p = cli("find", "--app", APP, "--window-id", w, "--role", role,
            "--name", name, "--exact", "--first", "--timeout-ms", "6000")
    return ((p.get("data") or {}).get("match") or {}).get("ref_id")


def click(w, r):
    return cli("click", r, "--timeout-ms", "2500", timeout=15) if r else {"ok": False}


def wait(ms=400):
    cli("wait", str(ms), timeout=10)


def brief(p):
    e = p.get("error") or {}
    return {"ok": p.get("ok"), "code": e.get("code"),
            "details": {k: v for k, v in e.items() if k != "code"}}


def value(w):
    r = ref(w, "textfield", FIELD)
    if not r:
        return {"ref": None}
    p = cli("get", r, "--property", "value", timeout=20)
    return {"ref": r, "get_ok": p.get("ok"),
            "value": (p.get("data") or {}).get("value"),
            "error": (p.get("error") or {}).get("code")}


def wait_window(proc, limit=150):
    for _ in range(limit):
        if proc.poll() is not None:
            return None
        d = cli("list-windows")
        for w in (d.get("data") or []):
            if (w.get("app_name") in ("Aster", "KStudio") and w.get("visible")
                    and w.get("accessible")):
                if len(snap(w["id"])) >= 40:
                    return w["id"]
        time.sleep(2)
    return None


def main():
    env = dict(os.environ)
    env.update({"STUDIO_RENDERER_DEV": "0", "STUDIO_RENDERER_PORT": str(PORT),
                "STUDIO_RENDERER_DIST": ".studio/dist/e2e-latest",
                "ASTER_TENANT_SLUG": "aster"})
    log = open("/tmp/probe-input2-studio.log", "w", encoding="utf-8")
    proc = subprocess.Popen(["just", "studio", "aster"], cwd=ROOT, env=env,
                            stdout=log, stderr=subprocess.STDOUT)
    report = {"port": PORT, "attempts": []}
    try:
        w = wait_window(proc)
        report["window"] = w
        if not w:
            report["status"] = "NO_WINDOW"
            return report
        for _ in range(4):
            if any(x.get("role") == "dialog" for x in snap(w)):
                for label in ("继续", "稍后设置", "关闭"):
                    r = ref(w, "button", label)
                    if r:
                        click(w, r)
                        wait()
                        break
            wait(250)
        click(w, ref(w, "button", "连接器"))
        wait(900)
        report["baseline_value"] = value(w)

        attempts = []

        # A: focus then type
        r = ref(w, "textfield", FIELD)
        a = cli("focus", r, "--timeout-ms", "3000", timeout=20) if r else {}
        b = cli("type", r, "abcA", "--timeout-ms", "8000", timeout=25) if r else {}
        wait(700)
        attempts.append({"name": "focus+type", "focus": brief(a), "type": brief(b),
                         "after": value(w)})

        # B: click then type
        r = ref(w, "textfield", FIELD)
        a = click(w, r)
        b = cli("type", r, "abcB", "--timeout-ms", "8000", timeout=25) if r else {}
        wait(700)
        attempts.append({"name": "click+type", "click": brief(a), "type": brief(b),
                         "after": value(w)})

        # C: clipboard + headless press (keys go to the app process)
        r = ref(w, "textfield", FIELD)
        if r:
            click(w, r)
        cs = cli("clipboard-set", "abcC", timeout=20)
        p = cli("press", "cmd+v", "--app", APP, "--window-id", w, timeout=20)
        wait(700)
        attempts.append({"name": "clipboard+press cmd+v", "clipboard": brief(cs),
                         "press": brief(p), "after": value(w)})

        # D: single character press
        r = ref(w, "textfield", FIELD)
        if r:
            click(w, r)
        p = cli("press", "d", "--app", APP, "--window-id", w, timeout=20)
        wait(700)
        attempts.append({"name": "press single char", "press": brief(p),
                         "after": value(w)})

        # E: set-value after focus (fresh ref)
        r = ref(w, "textfield", FIELD)
        a = cli("focus", r, "--timeout-ms", "3000", timeout=20) if r else {}
        b = cli("set-value", r, "abcE", "--timeout-ms", "3000", timeout=20) if r else {}
        wait(700)
        attempts.append({"name": "focus+set-value", "focus": brief(a),
                         "set_value": brief(b), "after": value(w)})

        report["attempts"] = attempts
        report["success"] = [a["name"] for a in attempts
                             if a["after"].get("value")]
        report["status"] = "COMPLETED"
        return report
    finally:
        # Restore clipboard so the probe leaves no trace.
        cli("clipboard-clear", timeout=15)
        (HERE / "input-probe2.json").write_text(
            json.dumps(report, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
        try:
            proc.terminate()
            proc.wait(timeout=15)
        except Exception:
            proc.kill()
        log.close()


if __name__ == "__main__":
    print(json.dumps(main(), ensure_ascii=False, indent=2), flush=True)
