#!/usr/bin/env python3
"""F11 L3 end-to-end case: a real preference survives a real process restart.

Drives the genuine Tauri webview (not a Playwright Chromium page) through
agent-desktop, so the setting goes through usePreference -> Tauri IPC ->
PreferenceStore, and lands in the native profile's ACL snapshot. A real
process exit and relaunch then proves restoration from disk rather than a
localStorage illusion.

Usage:
    python3 f11-desktop-e2e.py [--instance NAME] [--port PORT]

Requires: just, agent-desktop, a built Studio (just build-studio), and the
renderer dev server that `just studio` starts itself.
"""
import argparse
import json
import os
from pathlib import Path
import signal
import subprocess
import sys
import time

REPO = Path(__file__).resolve().parents[5]
SUPPORT = Path.home() / "Library/Application Support/dev.aster.studio/studio/aster/instances"
SWITCH = "运行任务时保持系统唤醒"
KEY = "app.power.prevent_sleep_when_busy"


def ax(*args, timeout=40):
    out = subprocess.run(
        ["agent-desktop", *args], capture_output=True, text=True, timeout=timeout
    )
    try:
        return json.loads(out.stdout)
    except json.JSONDecodeError:
        raise RuntimeError(f"agent-desktop {' '.join(args)}: {out.stdout[:300]} {out.stderr[:200]}")


def ref(name, role=None, exact=True):
    cmd = ["find", "--app", "Aster", "--name", name]
    if exact:
        cmd.append("--exact")
    data = ax(*cmd).get("data", {})
    for m in data.get("matches", []):
        if role is None or m.get("role") == role:
            return m.get("ref_id"), m
    return None, None


def switch_value():
    _, m = ref(SWITCH, role="switch")
    if m is None:
        return None
    return str(m.get("value"))


def windows():
    return [w for w in ax("list-windows").get("data", []) if w.get("visible") and w.get("app_name") == "Aster"]


def port_busy(port):
    out = subprocess.run(["lsof", "-nP", f"-iTCP:{port}", "-sTCP:LISTEN"], capture_output=True, text=True)
    return "LISTEN" in out.stdout


def dismiss_onboarding():
    """Fresh instances show two dialogs; initialized ones show none."""
    for _ in range(3):
        tree = ax("snapshot", "--app", "Aster", "--skeleton", "-i", "--compact").get("data", {}).get("tree", {})
        dialogs = []

        def walk(node):
            if isinstance(node, dict):
                if node.get("role") == "dialog":
                    dialogs.append(node.get("ref_id"))
                for child in node.get("children") or []:
                    walk(child)

        walk(tree)
        if not dialogs:
            return
        root = dialogs[0]
        detail = ax("snapshot", "--root", root, "-i", "--compact").get("data", {}).get("tree", {})
        buttons = []

        def collect(node):
            if isinstance(node, dict):
                if node.get("role") == "button":
                    buttons.append((node.get("name"), node.get("ref_id")))
                for child in node.get("children") or []:
                    collect(child)

        collect(detail)
        pick = next((r for n, r in buttons if n == "稍后设置"), None) or next(
            (r for n, r in buttons if n == "继续"), None
        )
        if pick:
            ax("click", pick)
            time.sleep(2)
        else:
            return


class Studio:
    def __init__(self, instance, port, tag):
        self.instance, self.port, self.tag = instance, port, tag
        self.proc = None
        self.log = open(f"/tmp/f11-{tag}.log", "w")

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
                dismiss_onboarding()
                return
            if self.proc.poll() is not None:
                raise RuntimeError(f"studio exited early: {self.proc.returncode}, see /tmp/f11-{self.tag}.log")
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


def acl_path(instance):
    root = SUPPORT / instance / "preferences" / "local"
    files = sorted(root.glob("*.acl")) if root.exists() else []
    return files[0] if files else None


def open_settings():
    r, _ = ref("设置", role="button")
    if not r:
        raise RuntimeError("settings button not found")
    ax("click", r)
    time.sleep(2)
    if switch_value() is None:
        raise RuntimeError("target switch not visible in settings")


def check(condition, message):
    if not condition:
        raise AssertionError(message)
    print(f"  OK  {message}", flush=True)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--instance", default="f11-live-01")
    ap.add_argument("--port", type=int, default=4325)
    args = ap.parse_args()

    instance = args.instance
    studio = None
    original = None
    try:
        print(f"[1] start {instance} on {args.port}", flush=True)
        studio = Studio(instance, args.port, "run1")
        studio.start()

        open_settings()
        original = switch_value()
        print(f"    initial switch value = {original}", flush=True)
        target = "0" if original == "1" else "1"

        print("[2] toggle through the real webview (usePreference -> Tauri IPC)", flush=True)
        r, _ = ref(SWITCH, role="switch")
        ax("toggle", r)
        time.sleep(2)
        check(switch_value() == target, f"switch is now {target}")

        print("[3] verify the Rust-owned ACL snapshot on disk", flush=True)
        acl = acl_path(instance)
        check(acl is not None, f"ACL snapshot created: {acl}")
        text = acl.read_text()
        expected_literal = "true" if target == "1" else "false"
        check(KEY in text and f"value = {expected_literal}" in text, f"ACL contains {KEY} = {expected_literal}")
        snapshot = text

        print("[4] cross-page immediate effect (work -> settings)", flush=True)
        r, _ = ref("工作", role="button")
        check(r is not None, "work navigation found")
        ax("click", r)
        time.sleep(2)
        open_settings()
        check(switch_value() == target, "switch still reflects the new value after navigating away and back")

        print("[5] exit the real process", flush=True)
        studio.stop()
        check(not windows(), "Aster window is gone")
        check(not port_busy(args.port), f"port {args.port} released")
        check(acl_path(instance) is not None and acl_path(instance).read_text() == snapshot, "ACL snapshot survives process exit")

        print("[6] relaunch the same instance", flush=True)
        studio = Studio(instance, args.port, "run2")
        studio.start()
        open_settings()

        print("[7] read back after restart", flush=True)
        check(switch_value() == target, f"switch restored from disk as {target}")
        check(acl_path(instance).read_text() == snapshot, "ACL snapshot unchanged across restart")

        print("[8] restore original value", flush=True)
        r, _ = ref(SWITCH, role="switch")
        ax("toggle", r)
        time.sleep(2)
        check(switch_value() == original, f"switch back to {original}")

        print("\nF11_L3: PASS", flush=True)
        print("  evidence: real Tauri webview, real IPC, ACL disk snapshot, real process exit + relaunch", flush=True)
        return 0
    except Exception as error:
        print(f"\nF11_L3: FAIL - {error}", file=sys.stderr, flush=True)
        if original is not None:
            try:
                open_settings()
                if switch_value() != original:
                    r, _ = ref(SWITCH, role="switch")
                    if r:
                        ax("toggle", r)
            except Exception:
                pass
        return 1
    finally:
        if studio:
            try:
                studio.stop()
            except Exception:
                pass


if __name__ == "__main__":
    sys.exit(main())
