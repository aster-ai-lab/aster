#!/usr/bin/env python3
"""F11 visual E2E where agent-desktop itself launches, drives and restarts the app.

Unlike f11-desktop-e2e.py (which starts the app with a Python subprocess),
every lifecycle action here is an agent-desktop command:

    agent-desktop launch <bundle id> --env ... --activate
    agent-desktop snapshot / find / click / toggle / get
    agent-desktop close-app <bundle id> --force
    agent-desktop launch ... (relaunch)

A Launch-Services-launchable wrapper bundle is installed at
~/Applications/AsterVisualE2E.app. Its Info.plist sets
LSArchitecturePriority = arm64 so NSWorkspace starts it natively (avoids the
Rosetta + arch -arm64 re-exec that made the launch identities disagree).

The disk ACL snapshot is read with Python only as evidence, never as a test
action. Requires `just build-studio` from this checkout first (AGENTS.md).

Status 2026-11-12 (OUTCOME round 24): fully working. Requires the
AsterVisualE2E wrapper app to have Accessibility granted in System Settings,
and the agent-desktop host to have been respawned from that wrapper's
Launch Services context (where the setuid /bin/ps is executable). Two
independent runs passed with --instance f11-live-01 --port 4325.
"""
import argparse
import json
from pathlib import Path
import plistlib
import subprocess
import sys
import time
import uuid

REPO = Path(__file__).resolve().parents[5]
HOME = Path.home()
APP_DIR = HOME / "Applications" / "AsterVisualE2E.app"
BUNDLE_ID = "dev.aster.studio.visual-e2e.launcher"
SWITCH = "运行任务时保持系统唤醒"
KEY = "app.power.prevent_sleep_when_busy"
SUPPORT = HOME / "Library/Application Support/dev.aster.studio/studio/aster/instances"


def ax(*args, timeout=90):
    out = subprocess.run(["agent-desktop", *args], capture_output=True, text=True, timeout=timeout)
    try:
        return json.loads(out.stdout)
    except json.JSONDecodeError:
        raise RuntimeError(f"agent-desktop {' '.join(args)}: {out.stdout[:250]} {out.stderr[:150]}")


def install_wrapper():
    macos = APP_DIR / "Contents/MacOS"
    macos.mkdir(parents=True, exist_ok=True)
    plist = {
        "CFBundleName": "AsterVisualE2E",
        "CFBundleDisplayName": "AsterVisualE2E",
        "CFBundleIdentifier": BUNDLE_ID,
        "CFBundleExecutable": "AsterVisualE2E",
        "CFBundlePackageType": "APPL",
        "CFBundleVersion": "1",
        "LSArchitecturePriority": ["arm64"],
    }
    (APP_DIR / "Contents/Info.plist").write_bytes(plistlib.dumps(plist))
    launcher = macos / "AsterVisualE2E"
    launcher.write_text(
        "#!/bin/sh\n"
        "# Started natively by NSWorkspace (LSArchitecturePriority=arm64).\n"
        "export NODE=/opt/homebrew/bin/node\n"
        "export PATH=\"/opt/homebrew/bin:$HOME/.cargo/bin:$HOME/.bun/bin:/usr/bin:/bin:/usr/sbin:/sbin\"\n"
        f"cd {REPO} || exit 1\n"
        "export ASTER_STUDIO_INSTANCE=\"${ASTER_STUDIO_INSTANCE:-visual-e2e-01}\"\n"
        "export STUDIO_RENDERER_PORT=\"${STUDIO_RENDERER_PORT:-4325}\"\n"
        "exec /opt/homebrew/bin/just studio >> /tmp/aster-visual-e2e/launcher.log 2>&1\n"
    )
    launcher.chmod(0o755)
    subprocess.run(
        ["/System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/"
         "Support/lsregister", "-f", str(APP_DIR)], check=True
    )


def launch(instance, port):
    return ax(
        "launch", BUNDLE_ID,
        "--env", f"ASTER_STUDIO_INSTANCE={instance}",
        "--env", f"STUDIO_RENDERER_PORT={port}",
        "--activate", "--timeout", "90000",
    )


def wait_for_window(timeout=180):
    deadline = time.time() + timeout
    while time.time() < deadline:
        result = ax("snapshot", "--app", "Aster", "--skeleton", "-i", "--compact")
        if result.get("ok"):
            return
        time.sleep(3)
    raise TimeoutError("Aster window never appeared via agent-desktop")


def dismiss_onboarding():
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
        detail = ax("snapshot", "--root", dialogs[0], "-i", "--compact").get("data", {}).get("tree", {})
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
        if not pick:
            return
        ax("click", pick)
        time.sleep(2)


def ref(name, role=None):
    data = ax("find", "--app", "Aster", "--name", name, "--exact").get("data", {})
    for match in data.get("matches", []):
        if (role is None or match.get("role") == role) and match.get("ref_id"):
            return match.get("ref_id"), match
    return None, None


def open_settings():
    r, _ = ref("设置", "button")
    if not r:
        raise RuntimeError("settings button not found")
    ax("click", r)
    time.sleep(2)
    if ref(SWITCH, "switch")[0] is None:
        raise RuntimeError("target switch not visible")


def switch_value():
    # Read via `get --property value`; the find payload's value field is not
    # always populated right after a relaunch or a toggle.
    for _ in range(6):
        r, _ = ref(SWITCH, "switch")
        if r:
            result = ax("get", r, "--property", "value")
            if result.get("ok"):
                return str(result.get("data", {}).get("value"))
        time.sleep(1)
    raise RuntimeError("switch value not readable via agent-desktop")


def acl_path(instance):
    root = SUPPORT / instance / "preferences" / "local"
    files = sorted(root.glob("*.acl")) if root.exists() else []
    return files[0] if files else None


def check(condition, message):
    if not condition:
        raise AssertionError(message)
    print(f"  OK  {message}", flush=True)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--instance", default="visual-e2e-01")
    ap.add_argument("--port", type=int, default=4325)
    args = ap.parse_args()
    token = uuid.uuid4().hex[:6]
    launched = False
    try:
        print("[0] install the agent-desktop-launchable wrapper", flush=True)
        install_wrapper()

        print("[1] agent-desktop launch", flush=True)
        result = launch(args.instance, args.port)
        launched = True
        print(f"    launch result: {result.get('ok')} {result.get('error', {}).get('code', '')}", flush=True)
        wait_for_window()
        dismiss_onboarding()

        print("[2] agent-desktop drives the settings UI", flush=True)
        open_settings()
        before = switch_value()
        print(f"    initial switch = {before}", flush=True)
        r, _ = ref(SWITCH, "switch")
        ax("toggle", r)
        time.sleep(2)
        after = switch_value()
        check(after != before, f"switch toggled to {after}")

        print("[3] disk evidence (Rust PreferenceStore ACL snapshot)", flush=True)
        acl = acl_path(args.instance)
        check(acl is not None, "ACL snapshot exists")
        text = acl.read_text()
        expected = "true" if after == "1" else "false"
        check(KEY in text and f"value = {expected}" in text, f"ACL has {KEY} = {expected}")
        snapshot = text

        print("[4] agent-desktop close-app (real process exit)", flush=True)
        ax("close-app", BUNDLE_ID, "--force")
        time.sleep(4)

        print("[5] agent-desktop launch again", flush=True)
        launch(args.instance, args.port)
        wait_for_window()
        dismiss_onboarding()
        open_settings()
        check(switch_value() == after, "switch restored after agent-desktop relaunch")
        check(acl_path(args.instance).read_text() == snapshot, "ACL snapshot unchanged")

        print("[6] restore", flush=True)
        r = None
        for _ in range(6):
            r, _ = ref(SWITCH, "switch")
            if r:
                break
            time.sleep(1)
        if not r:
            raise RuntimeError("switch control not found for restore")
        ax("toggle", r)
        time.sleep(2)
        check(switch_value() == before, "switch restored to original")

        print(f"\nAGENT_DESKTOP_LAUNCH_L3: PASS (token {token})", flush=True)
        print("  launch / drive / quit / relaunch all performed by agent-desktop", flush=True)
        return 0
    except Exception as error:
        print(f"\nAGENT_DESKTOP_LAUNCH_L3: BLOCKED - {error}", file=sys.stderr, flush=True)
        return 1
    finally:
        if launched:
            try:
                ax("close-app", BUNDLE_ID, "--force")
            except Exception:
                pass


if __name__ == "__main__":
    sys.exit(main())
