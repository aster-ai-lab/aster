#!/usr/bin/env python3
"""Diagnose why a freshly launched window has open popovers (247 vs 58 nodes).

Managed launcher: Studio stays a child of this process for the whole probe.
Each candidate close action is applied and the AX tree is measured, so we can
see exactly which one removes the model picker / task-scope popover.
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
PORT = 4324


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


def state(w, tag):
    n = snap(w)
    overlay_roles = ("dialog", "popover", "menu", "sheet", "alert")
    overlays = [{"role": x.get("role"), "name": x.get("name"),
                 "children": len(x.get("children") or [])}
                for x in n if x.get("role") in overlay_roles]
    nm = [str(x.get("name")) for x in n if x.get("name")]
    return {
        "tag": tag, "nodes": len(n), "overlays": overlays[:8],
        "has_选择模型": any("选择模型" in x for x in nm),
        "has_当前目录": any("在所选项目文件夹里直接修改" in x for x in nm),
        "has_composer": any("描述任务" in x for x in nm),
        "buttons_tail": [str(x.get("name")) for x in n if x.get("role") == "button"][-12:],
    }


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
    log = open("/tmp/diagnose-studio.log", "w", encoding="utf-8")
    proc = subprocess.Popen(["just", "studio", "aster"], cwd=ROOT, env=env,
                            stdout=log, stderr=subprocess.STDOUT)
    report = {"launcher_pid": proc.pid, "port": PORT, "steps": []}
    try:
        w = wait_window(proc)
        report["window"] = w
        if not w:
            report["status"] = "NO_WINDOW"
            return report
        report["steps"].append(state(w, "initial"))

        for label in ("继续", "稍后设置", "关闭"):
            m = find(w, role="button", name=label, exact="true")
            if m:
                click(w, m.get("ref_id"))
                wait(300)
                report["steps"].append(state(w, f"dismiss-modal:{label}"))
                break

        # One action at a time, measuring the effect of each.
        for label in ("关闭模型选择", "关闭", "取消", "工作", "返回应用"):
            m = find(w, role="button", name=label, exact="true")
            if not m:
                report["steps"].append({"tag": f"skip:{label}", "reason": "not found"})
                continue
            click(w, m.get("ref_id"))
            wait(400)
            report["steps"].append(state(w, f"click:{label}"))

        m = find(w, role="switch", name="展开侧边栏", exact="true")
        if m:
            click(w, m.get("ref_id"))
            wait(400)
            report["steps"].append(state(w, "click:展开侧边栏"))

        # Click a neutral surface control (model picker toggle) to see if
        # popovers are toggles that need a second click.
        cur = state(w, "before-toggle-probe")
        report["clean_achieved"] = cur["nodes"] <= 80 and cur["has_composer"]
        report["final"] = cur
        report["status"] = "COMPLETED"
        return report
    finally:
        (HERE / "overlay-diagnosis.json").write_text(
            json.dumps(report, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
        try:
            proc.terminate()
            proc.wait(timeout=15)
        except Exception:
            proc.kill()
        log.close()


if __name__ == "__main__":
    out = main()
    print(json.dumps({k: v for k, v in out.items() if k != "steps"},
                     ensure_ascii=False, indent=2), flush=True)
    for s in out.get("steps", []):
        print(json.dumps({"tag": s.get("tag"), "nodes": s.get("nodes"),
                          "overlays": s.get("overlays"),
                          "选择模型": s.get("has_选择模型"),
                          "当前目录": s.get("has_当前目录"),
                          "composer": s.get("has_composer")},
                         ensure_ascii=False), flush=True)
