#!/usr/bin/env python3
"""Re-run the four state-coupled cases, one fresh window per case.

The remaining F03/F04/F12/F18 failures were traced to inter-case state
coupling, not product behaviour. This runner gives each of them its own
launcher/window/port so no earlier case can leave focus, a popover or a
persisted permission marker behind.

Result: isolated-results.json plus one isolated-<ID>.json per case.
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
CASES = list(filter(None, os.environ.get("ISOLATED_CASES",
                                        "F03,F04,F12,F18").split(",")))
BASE_PORT = int(os.environ.get("ISOLATED_BASE_PORT", "4331"))


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


def state(w):
    n = snap(w)
    nm = [str(x.get("name")) for x in n if x.get("name")]
    return {
        "nodes": len(n),
        "dialogs": [x.get("name") for x in n if x.get("role") == "dialog"][:3],
        "has_composer": any("描述任务" in x for x in nm),
        "has_model_picker": any("选择模型" in x for x in nm),
        "has_task_scope": any("在所选项目文件夹里直接修改" in x for x in nm),
        "permission_labels": [x for x in nm if "授权模式:" in x][:2],
        "menuitem_count": sum(1 for x in n if x.get("role") == "menuitem"),
    }


def find_button(w, name):
    p = cli("find", "--app", APP, "--window-id", w, "--role", "button",
            "--name", name, "--exact", "--first", "--timeout-ms", "6000")
    return ((p.get("data") or {}).get("match") or {}).get("ref_id")


def dismiss_permission_modal(w, rounds=6):
    """A new origin shows '使用 Aster 前需要三项权限' on first launch, and it
    can appear MID-CASE (F12's baseline was clean, then the AX tree became a
    12-node dialog). Keep clearing until no dialog remains."""
    for _ in range(rounds):
        n = snap(w)
        if not any(x.get("role") == "dialog" for x in n):
            return True
        acted = False
        for label in ("继续", "稍后设置", "关闭"):
            r = find_button(w, label)
            if r:
                cli("click", r, "--timeout-ms", "2500")
                cli("wait", "400")
                acted = True
                break
        if not acted:
            cli("wait", "400")
    return not any(x.get("role") == "dialog" for x in snap(w))


def wait_ready(w, limit=60, stable=4):
    """Require several consecutive clean observations: the permission modal
    can appear a few seconds after the window looks ready."""
    streak = 0
    for _ in range(limit):
        dismiss_permission_modal(w)
        n = snap(w)
        clean = (any("描述任务" in str(x.get("name")) for x in n)
                 and not any(x.get("role") == "dialog" for x in n)
                 and len(n) <= 90)
        streak = streak + 1 if clean else 0
        if streak >= stable:
            return True
        time.sleep(1)
    return False


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


def wait_no_window(limit=30):
    """Tauri is single-instance: a leftover window would be reused instead of
    a fresh one, which is exactly what broke the F18 isolated run (baseline
    112 nodes, no composer)."""
    for _ in range(limit):
        d = cli("list-windows")
        live = [w for w in (d.get("data") or [])
                if w.get("app_name") in ("Aster", "KStudio") and w.get("accessible")]
        if not live:
            return True
        time.sleep(1)
    return False


def run_case(cid, index):
    wait_no_window()
    time.sleep(2)
    port = BASE_PORT + index
    env = dict(os.environ)
    env.update({"STUDIO_RENDERER_DEV": "0", "STUDIO_RENDERER_PORT": str(port),
                "STUDIO_RENDERER_DIST": ".studio/dist/e2e-latest",
                "ASTER_TENANT_SLUG": "aster"})
    log = open(f"/tmp/isolated-{cid}-studio.log", "w", encoding="utf-8")
    proc = subprocess.Popen(["just", "studio", "aster"], cwd=ROOT, env=env,
                            stdout=log, stderr=subprocess.STDOUT)
    entry = {"case": cid, "port": port, "launcher_pid": proc.pid}
    try:
        w = wait_window(proc)
        entry["window"] = w
        if not w:
            entry["status"] = "NO_WINDOW"
            return entry
        entry["ready"] = wait_ready(w)
        entry["baseline"] = state(w)

        case_env = dict(os.environ)
        case_env["ONLY_CASES"] = cid
        case_env["FT_RESULTS_PATH"] = str(HERE / f"isolated-{cid}.json")
        run = subprocess.run(
            [sys.executable, str(HERE / "functional-tests.py"), w],
            env=case_env, capture_output=True, text=True, timeout=300,
        )
        entry["exit"] = run.returncode
        (HERE / f"isolated-{cid}.log").write_text(run.stdout + run.stderr,
                                                  encoding="utf-8")
        p = HERE / f"isolated-{cid}.json"
        if p.exists():
            d = json.loads(p.read_text(encoding="utf-8"))
            r = (d.get("results") or [{}])[0]
            entry["result"] = {"status": r.get("status"),
                               "expected": r.get("expected"),
                               "actual": r.get("actual", "")[:300],
                               "screenshot": r.get("screenshot")}
        entry["after"] = state(w)
        entry["status"] = "COMPLETED"
        return entry
    finally:
        try:
            proc.terminate()
            proc.wait(timeout=15)
        except Exception:
            proc.kill()
        log.close()


def main():
    out = {"cases": [], "summary": {}}
    for i, cid in enumerate(CASES):
        entry = run_case(cid, i)
        out["cases"].append(entry)
        print(json.dumps({"case": cid, "status": entry.get("status"),
                          "verdict": (entry.get("result") or {}).get("status"),
                          "baseline_nodes": (entry.get("baseline") or {}).get("nodes")},
                         ensure_ascii=False), flush=True)
        (HERE / "isolated-results.json").write_text(
            json.dumps(out, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    passed = sum(1 for c in out["cases"]
                 if (c.get("result") or {}).get("status") == "PASS")
    out["summary"] = {"isolated_cases": len(CASES), "passed": passed,
                      "failed": len(CASES) - passed}
    (HERE / "isolated-results.json").write_text(
        json.dumps(out, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    print(json.dumps(out["summary"], ensure_ascii=False), flush=True)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
