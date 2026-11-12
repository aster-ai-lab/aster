#!/usr/bin/env python3
"""F13 L3 first-run case: real component install + onboarding on a real webview.

Drives a brand-new isolated Aster instance through agent-desktop so the
first-run sequence genuinely installs runtime components and writes their
bytes into the native profile. Then the process is killed and relaunched to
check that onboarding does not repeat.

IMPORTANT: PLAN.md section 4 expects THREE runtime components. The current
product ships COMPONENTS = 3 specs but only the required `aster-document` is
installed during first run (the other two are optional and the full
/settings/dependencies page is hidden for the Aster tenant). This case
therefore reports PARTIAL_PASS and does NOT claim F13 is strictly covered.

Usage: python3 f13-first-run-e2e.py [--port PORT]
"""
import argparse
import json
import os
from pathlib import Path
import re
import signal
import subprocess
import sys
import time

REPO = Path(__file__).resolve().parents[5]
SUPPORT = Path.home() / "Library/Application Support/dev.aster.studio/studio/aster/instances"


def ax(*args, timeout=40):
    out = subprocess.run(["agent-desktop", *args], capture_output=True, text=True, timeout=timeout)
    try:
        return json.loads(out.stdout)
    except json.JSONDecodeError:
        raise RuntimeError(f"agent-desktop {' '.join(args)}: {out.stdout[:300]} {out.stderr[:200]}")


def tree():
    return ax("snapshot", "--app", "Aster", "-i", "--compact").get("data", {}).get("tree", {})


def dialog_names():
    result = []
    dialogs = []

    def find_dialogs(node):
        if isinstance(node, dict):
            if node.get("role") == "dialog":
                dialogs.append(node)
            for child in node.get("children") or []:
                find_dialogs(child)

    find_dialogs(tree())
    for dialog in dialogs:
        result.append(dialog.get("name", ""))

        def collect(node):
            if isinstance(node, dict):
                if node.get("name"):
                    result.append(node["name"])
                for child in node.get("children") or []:
                    collect(child)

        collect(dialog)
    return result, dialogs


def button_ref(name, dialog_root=None):
    data = ax("snapshot", "--root", dialog_root, "-i", "--compact") if dialog_root else ax(
        "find", "--app", "Aster", "--name", name, "--exact"
    )
    matches = []

    def collect(node):
        if isinstance(node, dict):
            if node.get("role") == "button" and node.get("name") == name:
                matches.append(node.get("ref_id"))
            for child in node.get("children") or []:
                collect(child)

    collect(data.get("data", {}).get("tree", data.get("data", {})))
    if not matches and not dialog_root:
        for m in data.get("data", {}).get("matches", []):
            if m.get("role") == "button":
                matches.append(m.get("ref_id"))
    return matches[0] if matches else None


def windows():
    return [w for w in ax("list-windows").get("data", []) if w.get("visible") and w.get("app_name") == "Aster"]


def port_busy(port):
    out = subprocess.run(["lsof", "-nP", f"-iTCP:{port}", "-sTCP:LISTEN"], capture_output=True, text=True)
    return "LISTEN" in out.stdout


class Studio:
    def __init__(self, instance, port, tag):
        self.instance, self.port, self.tag = instance, port, tag
        self.proc = None
        self.log = open(f"/tmp/f13-{tag}.log", "w")

    def start(self):
        env = {**os.environ, "ASTER_STUDIO_INSTANCE": self.instance, "STUDIO_RENDERER_PORT": str(self.port)}
        self.proc = subprocess.Popen(
            ["just", "studio"], cwd=REPO, env=env, stdout=self.log, stderr=subprocess.STDOUT,
            start_new_session=True,
        )
        deadline = time.time() + 180
        while time.time() < deadline:
            if windows() and port_busy(self.port):
                time.sleep(3)
                return
            if self.proc.poll() is not None:
                raise RuntimeError(f"studio exited early ({self.proc.returncode}), see /tmp/f13-{self.tag}.log")
            time.sleep(2)
        raise TimeoutError("studio window did not appear")

    def stop(self):
        if self.proc and self.proc.poll() is None:
            os.killpg(os.getpgid(self.proc.pid), signal.SIGTERM)
            try:
                self.proc.wait(timeout=20)
            except subprocess.TimeoutExpired:
                os.killpg(os.getpgid(self.proc.pid), signal.SIGKILL)
        deadline = time.time() + 30
        while time.time() < deadline:
            if not windows() and not port_busy(self.port):
                return
            time.sleep(1)
        raise RuntimeError("studio did not fully exit")


def check(condition, message):
    if not condition:
        raise AssertionError(message)
    print(f"  OK  {message}", flush=True)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--port", type=int, default=4326)
    ap.add_argument("--instance", default=f"f13-e2e-{int(time.time()) % 100000}")
    args = ap.parse_args()
    instance, port = args.instance, args.port
    profile = SUPPORT / instance
    registry = profile / "runtime-plugins/state/registry.json"
    studio = None

    try:
        print(f"[1] fresh instance {instance} on {port}", flush=True)
        studio = Studio(instance, port, "run1")
        studio.start()

        print("[2] first-run component install (check -> install -> ready)", flush=True)
        seen = set()
        names, dialogs = [], []
        deadline = time.time() + 120
        while time.time() < deadline:
            names, dialogs = dialog_names()
            for n in names:
                seen.add(n)
            if any("组件已就绪" in n for n in names):
                break
            time.sleep(2)
        print("    observed states:", " | ".join(sorted(seen))[:500], flush=True)
        check(any("正在准备组件" in n for n in seen), "component check phase observed")
        check(any("正在安装" in n or "正在连接插件源" in n for n in seen), "install phase observed")
        check(any("组件已就绪" in n for n in names), "component-ready dialog reached")
        summary = next((n for n in names if "运行组件" in n and "就绪" in n), "")
        check(summary != "", f"component summary present: {summary}")

        print("[3] real install artifacts on disk", flush=True)
        deadline = time.time() + 30
        while time.time() < deadline and not registry.exists():
            time.sleep(1)
        check(registry.exists(), f"registry.json created: {registry}")
        data = json.loads(registry.read_text())
        routes = data.get("routes", [])
        check(len(routes) >= 1, f"{len(routes)} installed runtime component route(s)")
        route = routes[0]
        check("manifestSha256" in route and "packageSha256" in route, "install carries manifest + package sha256")
        payloads = list((profile / "runtime-plugins/data").rglob("*")) if (profile / "runtime-plugins/data").exists() else []
        real_files = [p for p in payloads if p.is_file() and p.stat().st_size > 0]
        check(len(real_files) > 3, f"{len(real_files)} non-empty real payload files on disk")

        print("[4] summary matches disk version", flush=True)
        version = route.get("version", "")
        ui_version = re.search(r"v(\d+\.\d+\.\d+)", summary)
        check(ui_version is not None and version.startswith(ui_version.group(1)),
              f"UI {ui_version.group(1) if ui_version else '?'} == disk {version.split('+')[0]}")
        disk_count = len(routes)

        print("[5] exit component dialog -> permissions dialog", flush=True)
        root = dialogs[0].get("ref_id")
        r = button_ref("继续", root)
        check(r is not None, "continue button found")
        ax("click", r)
        time.sleep(2)
        names2, dialogs2 = dialog_names()
        print("    dialog texts:", " | ".join(names2[:8]), flush=True)
        check(any("三项权限" in n for n in names2), "three-permissions dialog is shown")

        print("[6] skip permissions -> main page usable", flush=True)
        r = button_ref("稍后设置", dialogs2[0].get("ref_id"))
        check(r is not None, "later button found")
        ax("click", r)
        time.sleep(2)
        names3, _ = dialog_names()
        check(not names3, "no onboarding dialog remains")
        home = ax("find", "--app", "Aster", "--name", "新建任务").get("data", {}).get("matches", [])
        check(len(home) > 0, "main page is usable (new-task control present)")

        print("[7] real process exit + relaunch", flush=True)
        studio.stop()
        check(not windows() and not port_busy(port), "window gone and port released")
        check(registry.exists(), "install artifacts survive process exit")
        studio = Studio(instance, port, "run2")
        studio.start()
        names4, _ = dialog_names()
        check(not any("组件已就绪" in n or "三项权限" in n for n in names4),
              "onboarding does not repeat after restart")
        home2 = ax("find", "--app", "Aster", "--name", "新建任务").get("data", {}).get("matches", [])
        check(len(home2) > 0, "main page usable after restart")

        print("\nF13_L3: PARTIAL_PASS", flush=True)
        print(f"  real first-run install verified end to end; installed components = {disk_count}", flush=True)
        print("  NOT strictly covered: PLAN.md expects 3 runtime components, current product installs "
              "only the required aster-document (2 optional ones are deferred and the full "
              "/settings/dependencies page is hidden for the Aster tenant)", flush=True)
        return 0
    except Exception as error:
        print(f"\nF13_L3: FAIL - {error}", file=sys.stderr, flush=True)
        return 1
    finally:
        if studio:
            try:
                studio.stop()
            except Exception:
                pass


if __name__ == "__main__":
    sys.exit(main())
