#!/usr/bin/env python3
"""Clear the persisted planFirst marker on the TEST origin.

Source: workbenchPermissionPostures.ts writes `workbench.task.permission:v1`
to localStorage; WorkbenchPermissionPicker's 规划模式 menuitem is a toggle
(onPlanFirstChange(!planFirst)), so re-clicking it clears the marker.
Re-selecting 请求批准 does NOT clear it (it only changes mode).

Managed launcher on the same port as the last test run (4325).
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


def wait(ms=400):
    cli("wait", str(ms), timeout=10)


def labels(w):
    return [str(x.get("name")) for x in snap(w)
            if x.get("name") and "授权模式:" in str(x.get("name"))]


def node_by(w, role, predicate):
    for x in snap(w):
        if x.get("role") == role and predicate(str(x.get("name") or "")):
            return x.get("ref_id")
    return None


def wait_window(proc, limit=150):
    for _ in range(limit):
        if proc.poll() is not None:
            return None
        d = cli("list-windows")
        for w in (d.get("data") or []):
            if (w.get("app_name") in ("Aster", "KStudio") and w.get("visible")
                    and w.get("accessible")):
                if len(snap(w["id"])) >= 8:
                    return w["id"]
        time.sleep(2)
    return None


def main():
    env = dict(os.environ)
    env.update({"STUDIO_RENDERER_DEV": "0", "STUDIO_RENDERER_PORT": str(PORT),
                "STUDIO_RENDERER_DIST": ".studio/dist/e2e-latest",
                "ASTER_TENANT_SLUG": "aster"})
    log = open("/tmp/clear-plan-studio.log", "w", encoding="utf-8")
    proc = subprocess.Popen(["just", "studio", "aster"], cwd=ROOT, env=env,
                            stdout=log, stderr=subprocess.STDOUT)
    report = {"port": PORT, "launcher_pid": proc.pid}
    try:
        w = wait_window(proc)
        report["window"] = w
        if not w:
            report["status"] = "NO_WINDOW"
            return report
        # First-run modal only if actually present.
        if any(x.get("role") == "dialog" for x in snap(w)):
            for label in ("继续", "稍后设置", "关闭"):
                r = node_by(w, "button", lambda n: n == label)
                if r:
                    click(w, r)
                    wait(350)
                    break
        report["before"] = labels(w)

        for attempt in range(3):
            if not any("先规划" in x for x in labels(w)):
                break
            # Open the permission menu via the chip itself (AX tree lookup,
            # not a text search that may hit a menu description).
            chip = node_by(w, "combobox", lambda n: n.startswith("授权模式:"))
            if not chip:
                report.setdefault("errors", []).append(f"attempt{attempt}: chip not found")
                break
            click(w, chip)
            wait(500)
            toggle = node_by(w, "menuitem", lambda n: n.strip() == "规划模式")
            report.setdefault("menu_items", []).append(
                [str(x.get("name")) for x in snap(w) if x.get("role") == "menuitem"][:6])
            if not toggle:
                report.setdefault("errors", []).append(f"attempt{attempt}: toggle not found")
                continue
            click(w, toggle)
            wait(600)
            report.setdefault("attempts", []).append({"attempt": attempt,
                                                      "after": labels(w)})
        report["after"] = labels(w)
        report["status"] = ("RESTORED" if report["after"]
                            and not any("先规划" in x for x in report["after"])
                            else "UNRESOLVED")
        return report
    finally:
        (HERE / "clear-plan-first-report.json").write_text(
            json.dumps(report, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
        try:
            proc.terminate()
            proc.wait(timeout=15)
        except Exception:
            proc.kill()
        log.close()


if __name__ == "__main__":
    print(json.dumps(main(), ensure_ascii=False, indent=2), flush=True)
