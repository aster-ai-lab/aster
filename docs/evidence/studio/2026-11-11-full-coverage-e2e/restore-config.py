#!/usr/bin/env python3
"""Restore the composer permission-mode label left by an earlier test.

The functional suite's planning-mode case toggled "先规划" and its cleanup did
not stick. This managed process starts a fresh window, clears every overlay,
re-selects the 请求批准 menu item (which resets the planning marker) and
verifies the label before shutting down.
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
PORT = 4323


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


def find(w, **kw):
    a = ["find", "--app", APP, "--window-id", w, "--first", "--timeout-ms", "6000"]
    for k, v in kw.items():
        a += ["--" + k.replace("_", "-"), str(v)]
    return (cli(*a).get("data") or {}).get("match")


def click(w, r):
    if not r:
        return False
    return bool(cli("click", r, "--timeout-ms", "2500", timeout=15).get("ok"))


def wait(ms=350):
    cli("wait", str(ms), timeout=10)


def labels(w):
    return [str(x.get("name")) for x in snap(w)
            if x.get("name") and "授权模式:" in str(x.get("name"))]


def scrub(w):
    """Close model picker, task scope picker, dialogs and restore the sidebar."""
    for _ in range(5):
        acted = False
        for label in ("取消", "关闭模型选择", "关闭", "返回应用", "工作"):
            m = find(w, role="button", name=label, exact="true")
            if m:
                click(w, m.get("ref_id"))
                wait(200)
                acted = True
        m = find(w, role="switch", name="展开侧边栏", exact="true")
        if m:
            click(w, m.get("ref_id"))
            wait(200)
            acted = True
        if not acted:
            break
    return len(snap(w))


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
    log = open("/tmp/restore-config-studio.log", "w", encoding="utf-8")
    proc = subprocess.Popen(["just", "studio", "aster"], cwd=ROOT, env=env,
                            stdout=log, stderr=subprocess.STDOUT)
    report = {"launcher_pid": proc.pid, "port": PORT}
    try:
        w = wait_window(proc)
        report["window"] = w
        if not w:
            report["status"] = "NO_WINDOW"
            return report
        for label in ("继续", "稍后设置", "关闭"):
            m = find(w, role="button", name=label, exact="true")
            if m:
                click(w, m.get("ref_id"))
                wait(300)
                break
        report["nodes_after_scrub"] = scrub(w)
        report["labels_before"] = labels(w)

        if any("先规划" in x for x in report["labels_before"]):
            # Strategy 1: re-select the primary 请求批准 menu item.
            opener = find(w, role="combobox", text="授权模式")
            if opener:
                click(w, opener.get("ref_id"))
                wait(450)
                primary = (find(w, role="menuitem", text="请求批准 读取无需确认")
                           or find(w, role="menuitem", text="读取无需确认"))
                report["primary_found"] = bool(primary)
                if primary:
                    click(w, primary.get("ref_id"))
                    wait(600)
            report["labels_after_primary"] = labels(w)

            # Strategy 2: toggle the planning menu item itself.
            if any("先规划" in x for x in report["labels_after_primary"]):
                scrub(w)
                opener = find(w, role="combobox", text="授权模式")
                if opener:
                    click(w, opener.get("ref_id"))
                    wait(450)
                    toggle = find(w, role="menuitem", name="规划模式", exact="true")
                    report["toggle_found"] = bool(toggle)
                    if toggle:
                        click(w, toggle.get("ref_id"))
                        wait(600)
            report["labels_after_toggle"] = labels(w)

        scrub(w)
        report["labels_final"] = labels(w)
        report["status"] = ("RESTORED" if report["labels_final"]
                            and not any("先规划" in x for x in report["labels_final"])
                            else "UNRESOLVED")
        return report
    finally:
        (HERE / "restore-config-report.json").write_text(
            json.dumps(report, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
        try:
            proc.terminate()
            proc.wait(timeout=15)
        except Exception:
            proc.kill()
        log.close()


if __name__ == "__main__":
    print(json.dumps(main(), ensure_ascii=False, indent=2), flush=True)
