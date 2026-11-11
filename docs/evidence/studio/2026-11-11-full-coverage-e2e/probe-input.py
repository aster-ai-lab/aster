#!/usr/bin/env python3
"""Find an observable way to verify text entry in the connector search field.

F12 stayed unverified because find() reports value=None for that textfield and
set-value returns failure. This probes every readable AX property before and
after an input attempt, plus visible-text changes, on a stable managed window.
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
PORT = 4325  # already-initialized origin: no first-run permission modal
FIELD = "搜索连接器或关键词"
QUERY = "zzzz-no-such-connector"


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


def click(w, r):
    if not r:
        return False
    return bool(cli("click", r, "--timeout-ms", "2500", timeout=15).get("ok"))


def find_ref(w, role, name):
    p = cli("find", "--app", APP, "--window-id", w, "--role", role,
            "--name", name, "--exact", "--first", "--timeout-ms", "6000")
    m = (p.get("data") or {}).get("match")
    return m.get("ref_id") if m else None


def wait(ms=400):
    cli("wait", str(ms), timeout=10)


def field_nodes(w):
    return [x for x in snap(w) if x.get("role") == "textfield"
            and FIELD in str(x.get("name") or "")]


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
    log = open("/tmp/probe-input-studio.log", "w", encoding="utf-8")
    proc = subprocess.Popen(["just", "studio", "aster"], cwd=ROOT, env=env,
                            stdout=log, stderr=subprocess.STDOUT)
    report = {"port": PORT}
    try:
        w = wait_window(proc)
        report["window"] = w
        if not w:
            report["status"] = "NO_WINDOW"
            return report
        for _ in range(4):
            if any(x.get("role") == "dialog" for x in snap(w)):
                for label in ("继续", "稍后设置", "关闭"):
                    r = find_ref(w, "button", label)
                    if r:
                        click(w, r)
                        wait()
                        break
            wait(300)
        click(w, find_ref(w, "button", "连接器"))
        wait(800)
        report["connector_nodes"] = len(snap(w))
        report["field_before"] = field_nodes(w)[:1]

        r = find_ref(w, "textfield", FIELD)
        report["field_ref"] = r
        if not r:
            report["status"] = "FIELD_NOT_FOUND"
            return report

        set_result = cli("set-value", r, QUERY, "--timeout-ms", "3000", timeout=20)
        report["set_value"] = {
            "ok": set_result.get("ok"),
            "error": (set_result.get("error") or {}).get("code"),
            "data": {k: v for k, v in (set_result.get("data") or {}).items()
                     if k in ("action", "disposition", "value")},
        }
        wait(900)
        report["field_after"] = field_nodes(w)[:1]
        report["nodes_after"] = len(snap(w))

        # Probe every readable property on the live ref.
        props = {}
        for prop in ("value", "name", "description", "text", "states", "role"):
            p = cli("get", r, "--property", prop, timeout=20)
            props[prop] = {"ok": p.get("ok"),
                           "data": (p.get("data") or {}),
                           "error": (p.get("error") or {}).get("code")}
        report["get_properties"] = props

        nm = [str(x.get("name")) for x in snap(w) if x.get("name")]
        report["visible_markers"] = {
            "has_query_text": any(QUERY[:8] in x for x in nm),
            "has_清除": any("清除" in x for x in nm),
            "has_暂无": any("暂无" in x for x in nm),
            "has_没有": any("没有" in x for x in nm),
            "names_tail": nm[-14:],
        }
        report["status"] = "COMPLETED"
        return report
    finally:
        (HERE / "input-probe.json").write_text(
            json.dumps(report, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
        try:
            proc.terminate()
            proc.wait(timeout=15)
        except Exception:
            proc.kill()
        log.close()


if __name__ == "__main__":
    print(json.dumps(main(), ensure_ascii=False, indent=2), flush=True)
