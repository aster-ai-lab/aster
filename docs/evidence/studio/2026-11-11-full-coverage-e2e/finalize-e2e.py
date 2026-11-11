#!/usr/bin/env python3
"""One managed process: launch Studio, restore state, run functional tests,
re-check two open questions, then stop the window.

The earlier re-run failed because `just studio` was a separate background task
that exited mid-test, so every AX snapshot returned size=0. Here the launcher
is a child of THIS process and stays alive until all checks finish.

Outputs: finalize-report.json (+ the test script's own functional-results.json).
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
PORT = 4322


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


def wait(ms=400):
    cli("wait", str(ms), timeout=10)


def names(nodes):
    return [str(x.get("name")) for x in nodes if x.get("name")]


def wait_window(proc, limit=150):
    for _ in range(limit):
        if proc.poll() is not None:
            return None
        d = cli("list-windows")
        for w in (d.get("data") or []):
            if (w.get("app_name") in ("Aster", "KStudio") and w.get("visible")
                    and w.get("accessible")):
                nodes = snap(w["id"])
                # A real main window, not a stale/shutdown shell.
                if len(nodes) >= 8:
                    return w["id"]
        time.sleep(2)
    return None


def dismiss_modal(w):
    for label in ("继续", "稍后设置", "关闭"):
        m = find(w, role="button", name=label, exact="true")
        if m:
            click(w, m.get("ref_id"))
            wait(350)
            break


def scrub(w):
    for _ in range(3):
        acted = False
        for label in ("取消", "关闭模型选择", "关闭", "返回应用", "工作"):
            m = find(w, role="button", name=label, exact="true")
            if m:
                click(w, m.get("ref_id"))
                wait(180)
                acted = True
        m = find(w, role="switch", name="展开侧边栏", exact="true")
        if m:
            click(w, m.get("ref_id"))
            wait(180)
            acted = True
        if not acted:
            break


def permission_labels(w):
    return [n for n in names(snap(w)) if "授权模式:" in n]


def restore_planning(w):
    """Remove the '· 先规划' marker a previous test left behind."""
    scrub(w)
    before = permission_labels(w)
    if not any("先规划" in x for x in before):
        return {"status": "clean", "labels": before[:2]}
    opener = find(w, role="combobox", text="授权模式")
    if opener:
        click(w, opener.get("ref_id"))
        wait(400)
        toggle = find(w, role="menuitem", name="规划模式", exact="true")
        if toggle:
            click(w, toggle.get("ref_id"))
            wait(500)
    after = permission_labels(w)
    ok = not any("先规划" in x for x in after)
    return {"status": "restored" if ok else "FAILED", "before": before[:2],
            "after": after[:2]}


def probe_f09(w):
    """Search modal close: is the dialog really gone from the AX tree?"""
    scrub(w)
    click(w, (find(w, role="button", name="搜索任务", exact="true") or {}).get("ref_id"))
    wait(450)
    opened = snap(w)
    dialogs_open = [x for x in opened if x.get("role") == "dialog"]
    click(w, (find(w, role="button", name="关闭", exact="true") or {}).get("ref_id"))
    wait(600)
    closed = snap(w)
    dialogs_after = [x for x in closed if x.get("role") == "dialog"]
    return {
        "open_dialog_count": len(dialogs_open),
        "after_dialog_count": len(dialogs_after),
        "after_dialog_names": [x.get("name") for x in dialogs_after][:4],
        "after_dialog_child_counts": [len(x.get("children") or []) for x in dialogs_after][:4],
        "after_nodes": len(closed),
        "has_关闭": any(x.get("name") == "关闭" for x in closed),
        "has_composer": any("描述任务" in str(x.get("name")) for x in closed),
        "verdict": ("a11y_residual_empty_dialog" if dialogs_after and
                    all(not (x.get("children") or []) for x in dialogs_after) and
                    any("描述任务" in str(x.get("name")) for x in closed)
                    else "functional_close_ok" if not dialogs_after
                    else "needs_manual_review"),
    }


def probe_f18(w):
    """Returning from Settings/About: what actually happens?"""
    scrub(w)
    click(w, (find(w, role="button", name="设置", exact="true") or {}).get("ref_id"))
    wait(650)
    click(w, (find(w, role="button", name="关于", exact="true") or {}).get("ref_id"))
    wait(700)
    sub = snap(w)
    sub_names = names(sub)
    back_names = [str(x.get("name")) for x in sub if x.get("role") == "button"
                  and any(k in str(x.get("name")) for k in ("返回", "工作", "关闭"))]
    target = (find(w, role="button", name="返回应用", exact="true")
              or find(w, role="button", name="工作", exact="true"))
    clicked = click(w, (target or {}).get("ref_id")) if target else False
    wait(900)
    back = snap(w)
    return {
        "subpage_nodes": len(sub),
        "subpage_has_关于_related": any("版本" in n or "更新" in n for n in sub_names),
        "back_button_candidates": back_names[:8],
        "clicked_back": clicked,
        "after_nodes": len(back),
        "after_has_composer": any("描述任务" in str(x.get("name")) for x in back),
        "after_has_开机自动启动": any(x.get("name") == "开机自动启动" for x in back),
        "after_names_tail": [str(x.get("name")) for x in back if x.get("name")][-10:],
    }


def main():
    env = dict(os.environ)
    env.update({
        "STUDIO_RENDERER_DEV": "0",
        "STUDIO_RENDERER_PORT": str(PORT),
        "STUDIO_RENDERER_DIST": ".studio/dist/e2e-latest",
        "ASTER_TENANT_SLUG": "aster",
    })
    log = open("/tmp/finalize-studio.log", "w", encoding="utf-8")
    proc = subprocess.Popen(["just", "studio", "aster"], cwd=ROOT, env=env,
                            stdout=log, stderr=subprocess.STDOUT)
    report = {"launcher_pid": proc.pid, "port": PORT}
    try:
        w = wait_window(proc)
        report["window"] = w
        if not w:
            report["status"] = "NO_WINDOW"
            return report
        dismiss_modal(w)
        scrub(w)
        report["baseline_nodes"] = len(snap(w))

        report["planning_restore_before_tests"] = restore_planning(w)

        # Run the (bug-fixed) functional suite in-process-child, window stays up.
        run = subprocess.run(
            [sys.executable, str(HERE / "functional-tests.py"), w],
            capture_output=True, text=True, timeout=900,
        )
        (HERE / "functional-final.log").write_text(run.stdout + run.stderr, encoding="utf-8")
        report["functional_exit"] = run.returncode
        result_path = HERE / "functional-results.json"
        if result_path.exists():
            results = json.loads(result_path.read_text(encoding="utf-8"))
            report["functional_summary"] = {k: v for k, v in results.items()
                                            if k != "results"}
            report["functional_cases"] = [
                {"id": r["id"], "status": r["status"], "actual": r["actual"][:160]}
                for r in results.get("results", [])
            ]

        report["f09_probe"] = probe_f09(w)
        report["f18_probe"] = probe_f18(w)

        # Final cleanup: never leave the user's config modified.
        report["planning_restore_after_tests"] = restore_planning(w)
        scrub(w)
        report["final_nodes"] = len(snap(w))
        report["status"] = "COMPLETED"
        return report
    finally:
        (HERE / "finalize-report.json").write_text(
            json.dumps(report, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
        try:
            proc.terminate()
            proc.wait(timeout=15)
        except Exception:
            proc.kill()
        log.close()


if __name__ == "__main__":
    out = main()
    print(json.dumps({k: v for k, v in out.items() if k != "functional_cases"},
                     ensure_ascii=False, indent=2), flush=True)
