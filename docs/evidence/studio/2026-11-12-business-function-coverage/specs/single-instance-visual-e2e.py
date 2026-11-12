#!/usr/bin/env python3
"""F14 sampling: a second agent-desktop launch must not create a second app.

Fully agent-desktop visual E2E (wrapper launched by `agent-desktop launch`,
all observations are agent-desktop commands). Reuses helpers from
agent-desktop-launch-e2e.py.

Product contract (apps/studio/src-tauri/src/single_instance.rs): one process
per tenant; a second launch asks the running one to show its window and exits
before starting an authority or writing the runtime file
(lib.rs: Claim::Busy -> process::exit(0)).

Output:
  SINGLE_INSTANCE_L3: PASS  - second launch added no Aster main window/process
  SINGLE_INSTANCE_L3: FAIL  - a second independent app appeared (real finding)
"""
import argparse
import importlib.util
from pathlib import Path
import sys
import time

HERE = Path(__file__).resolve().parent


def load_helpers():
    spec = importlib.util.spec_from_file_location("ad_launch", HERE / "agent-desktop-launch-e2e.py")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def visible_aster_windows(h):
    result = h.ax("list-windows")
    windows = result.get("data", [])
    if not isinstance(windows, list):
        raise RuntimeError(f"list-windows failed: {result.get('error', {}).get('code')}")
    return [w.get("id") for w in windows
            if w.get("app_name") == "Aster" and w.get("visible")]


def aster_main_apps(h):
    result = h.ax("list-apps")
    apps = result.get("data", {}).get("apps", [])
    if not result.get("ok"):
        raise RuntimeError(f"list-apps failed: {result.get('error', {}).get('code')}")
    return [(a.get("pid"), a.get("bundle_id")) for a in apps
            if a.get("name") == "Aster" and a.get("presentation") == "foreground"]


def launch_again(h, instance, port):
    """Second launch of the same bundle. Deliberately WITHOUT --no-attach:
    forcing a fresh instance would replace the running app at the NSWorkspace
    layer and say nothing about the product's flock-based single-instance
    contract (single_instance.rs Claim::Busy -> exit(0))."""
    return h.ax(
        "launch", h.BUNDLE_ID,
        "--env", f"ASTER_STUDIO_INSTANCE={instance}",
        "--env", f"STUDIO_RENDERER_PORT={port}",
        "--activate", "--timeout", "90000",
    )


def wait_for_settings(h, attempts=8):
    for _ in range(attempts):
        ref, _ = h.ref("设置", "button")
        if ref:
            return ref
        time.sleep(2)
    return None


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--instance", default="f11-live-01")
    ap.add_argument("--port", type=int, default=4325)
    args = ap.parse_args()
    h = load_helpers()
    launched = False
    try:
        print("[1] install wrapper + first agent-desktop launch", flush=True)
        h.install_wrapper()
        first = h.launch(args.instance, args.port)
        launched = True
        print(f"    first launch ok={first.get('ok')}", flush=True)
        h.wait_for_window()
        h.dismiss_onboarding()
        time.sleep(2)
        windows_before = visible_aster_windows(h)
        apps_before = aster_main_apps(h)
        print(f"    visible Aster windows = {len(windows_before)} {windows_before}", flush=True)
        print(f"    foreground Aster apps = {len(apps_before)} {apps_before}", flush=True)
        if not windows_before or not apps_before:
            raise RuntimeError("first launch produced no Aster main window/process")

        print("[2] second agent-desktop launch (same bundle, no --no-attach)", flush=True)
        second = launch_again(h, args.instance, args.port)
        print(f"    second launch ok={second.get('ok')} code={second.get('error', {}).get('code', '')}", flush=True)
        time.sleep(8)
        windows_after = visible_aster_windows(h)
        apps_after = aster_main_apps(h)
        print(f"    visible Aster windows = {len(windows_after)} {windows_after}", flush=True)
        print(f"    foreground Aster apps = {len(apps_after)} {apps_after}", flush=True)

        print("[3] verify the original window is still usable", flush=True)
        settings_ref = wait_for_settings(h)
        print(f"    settings button found = {bool(settings_ref)}", flush=True)

        print("[4] close-app", flush=True)
        h.ax("close-app", h.BUNDLE_ID, "--force")
        launched = False

        same_pid = (len(apps_before) == 1 and len(apps_after) == 1
                    and apps_before[0][0] == apps_after[0][0])
        same_window = (len(windows_before) == 1 and len(windows_after) == 1
                       and windows_before[0] == windows_after[0])
        no_extra = len(windows_after) <= 1 and len(apps_after) <= 1
        print(f"    same_pid={same_pid} same_window={same_window} no_extra={no_extra}", flush=True)

        if same_pid and same_window and bool(settings_ref):
            print("\nSINGLE_INSTANCE_L3: PASS", flush=True)
            print("  second launch reused the same Aster process and window;", flush=True)
            print("  the original window stayed operable via agent-desktop", flush=True)
            return 0
        print("\nSINGLE_INSTANCE_L3: FAIL", flush=True)
        print("  second launch did not demonstrably reuse the first instance;", flush=True)
        print("  reported as an observation (NSWorkspace behaviour may differ", flush=True)
        print("  from the product's flock-based Claim::Busy path)", flush=True)
        return 1
    except Exception as error:
        print(f"\nSINGLE_INSTANCE_L3: ERROR - {error}", file=sys.stderr, flush=True)
        return 1
    finally:
        if launched:
            try:
                h.ax("close-app", h.BUNDLE_ID, "--force")
            except Exception:
                pass


if __name__ == "__main__":
    sys.exit(main())
