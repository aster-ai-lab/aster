#!/usr/bin/env python3
"""Clean re-run: managed window, no blind scrubbing, then the functional suite.

Diagnosis (overlay-diagnosis.json) showed a fresh window is ALREADY clean
(64 nodes, zero overlays). The earlier 247-node baseline came from the
harness's own generic close-button clicks. So this runner only dismisses a
first-run permission dialog when one is actually present.
"""
from __future__ import annotations

import json
import os
import subprocess
import sys
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


def state(w):
    n = snap(w)
    nm = [str(x.get("name")) for x in n if x.get("name")]
    return {
        "nodes": len(n),
        "dialogs": [x.get("name") for x in n if x.get("role") == "dialog"][:4],
        "has_composer": any("描述任务" in x for x in nm),
        "has_model_picker": any("选择模型" in x for x in nm),
        "has_task_scope": any("在所选项目文件夹里直接修改" in x for x in nm),
        "permission_labels": [x for x in nm if "授权模式:" in x][:2],
    }


def wait_ready(w, limit=40, stable=3):
    """Require consecutive clean observations so a late first-run modal or a
    half-rendered webview cannot become a case's baseline."""
    streak = 0
    for _ in range(limit):
        n = snap(w)
        dialogs = [x for x in n if x.get("role") == "dialog"]
        if dialogs:
            for label in ("继续", "稍后设置", "关闭"):
                m = find(w, role="button", name=label, exact="true")
                if m:
                    click(w, m.get("ref_id"))
                    wait(350)
                    break
        clean = (not dialogs
                 and any("描述任务" in str(x.get("name")) for x in n)
                 and len(n) <= 90)
        streak = streak + 1 if clean else 0
        if streak >= stable:
            return True
        time.sleep(1)
    return False


def wait_window(proc, limit=150):
    for _ in range(limit):
        # `just studio` can exec/exit while the Tauri window stays alive; do
        # not treat a finished launcher as a failed launch. Only the actual
        # window list decides.
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
    log = open("/tmp/rerun-studio.log", "w", encoding="utf-8")
    proc = subprocess.Popen(["just", "studio", "aster"], cwd=ROOT, env=env,
                            stdout=log, stderr=subprocess.STDOUT)
    report = {"launcher_pid": proc.pid, "port": PORT}
    try:
        w = wait_window(proc)
        report["window"] = w
        if not w:
            report["status"] = "NO_WINDOW"
            return report

        report["ready"] = wait_ready(w)
        # Warm-up: the first few AX actions after a cold launch can be flaky
        # even when the baseline looks clean. Let the renderer settle and read
        # the tree a couple of times before the first case.
        for _ in range(3):
            snap(w)
            cli("wait", "500", timeout=10)
        report["baseline"] = state(w)

        run = subprocess.run(
            [sys.executable, str(HERE / "functional-tests.py"), w],
            capture_output=True, text=True, timeout=900,
        )
        (HERE / "functional-clean-run.log").write_text(run.stdout + run.stderr,
                                                       encoding="utf-8")
        report["functional_exit"] = run.returncode
        p = HERE / "functional-results.json"
        if p.exists():
            d = json.loads(p.read_text(encoding="utf-8"))
            report["functional_summary"] = {k: v for k, v in d.items() if k != "results"}
            report["cases"] = [{"id": r["id"], "status": r["status"],
                                "actual": r["actual"][:160]}
                               for r in d.get("results", [])]
        report["after_state"] = state(w)
        report["status"] = "COMPLETED"
        return report
    finally:
        (HERE / "clean-run-report.json").write_text(
            json.dumps(report, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
        try:
            proc.terminate()
            proc.wait(timeout=15)
        except Exception:
            proc.kill()
        log.close()


if __name__ == "__main__":
    out = main()
    print(json.dumps({k: v for k, v in out.items() if k != "cases"},
                     ensure_ascii=False, indent=2), flush=True)
    for c in out.get("cases", []):
        print(json.dumps(c, ensure_ascii=False), flush=True)
