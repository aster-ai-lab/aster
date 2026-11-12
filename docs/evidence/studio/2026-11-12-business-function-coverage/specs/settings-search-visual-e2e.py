#!/usr/bin/env python3
"""F10 sampling: settings search entry and its writable search control.

Single-launch, headless-semantic-only agent-desktop visual E2E. Kept short on
purpose: this session's host degrades on long runs and on process restarts.
No physical keyboard, no close-app/relaunch.

Output:
  SETTINGS_SEARCH_L3: PASS  - search entry opens a writable control
  SETTINGS_SEARCH_L3: FAIL  - real finding, reported as-is
"""
import argparse
import importlib.util
import sys
import time
from pathlib import Path

HERE = Path(__file__).resolve().parent
SETTINGS = "设置"
SEARCH_ENTRY = "搜索设置..."


def load_helpers():
    spec = importlib.util.spec_from_file_location("ad_launch", HERE / "agent-desktop-launch-e2e.py")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def find_one(h, name, role=None):
    data = h.ax("find", "--app", "Aster", "--name", name, "--exact").get("data", {})
    return next((m for m in data.get("matches", [])
                 if (role is None or m.get("role") == role) and m.get("ref_id")), None)


def wait_for(h, name, role=None, attempts=6):
    for _ in range(attempts):
        match = find_one(h, name, role)
        if match:
            return match
        time.sleep(1)
    return None


def search_controls(h):
    """Return writable controls visible after the search entry is clicked."""
    result = []
    for role in ("combobox", "textfield", "searchfield"):
        data = h.ax("find", "--app", "Aster", "--role", role).get("data", {})
        for match in data.get("matches", []):
            if match.get("ref_id"):
                result.append((role, match))
    return result


def check(label, condition, detail=""):
    print(f"  {'OK ' if condition else 'FAIL'}  {label}{(' - ' + detail) if detail else ''}", flush=True)
    if not condition:
        raise AssertionError(label)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--instance", default="f11-live-01")
    ap.add_argument("--port", type=int, default=4325)
    args = ap.parse_args()
    h = load_helpers()
    launched = False
    try:
        print("[1] install wrapper + agent-desktop launch", flush=True)
        h.install_wrapper()
        h.launch(args.instance, args.port)
        launched = True
        h.wait_for_window()
        h.dismiss_onboarding()

        print("[2] open Settings", flush=True)
        settings = wait_for(h, SETTINGS, "button")
        check("Settings button exists", settings is not None)
        clicked = h.ax("click", settings["ref_id"])
        check("Settings click delivered", clicked.get("ok") is True)
        time.sleep(2)

        print("[3] settings search entry", flush=True)
        entry = wait_for(h, SEARCH_ENTRY, "button")
        check("search entry button exists", entry is not None)
        check("search entry is a button, not a writable field",
              entry.get("role") == "button")

        print("[4] click entry and find the writable search control", flush=True)
        opened = h.ax("click", entry["ref_id"])
        check("search entry click delivered", opened.get("ok") is True)
        time.sleep(2)

        controls = []
        for _ in range(6):
            controls = search_controls(h)
            if controls:
                break
            time.sleep(1)
        print("    writable controls:",
              [(role, m.get("name"), m.get("role")) for role, m in controls], flush=True)
        check("a writable search control appeared", len(controls) > 0)

        print("[5] set-value + read back", flush=True)
        # The entry button itself becomes a writable combobox after the click;
        # do not pick the first writable control on the page (that is 更多选项).
        target = next((m for _, m in controls if SEARCH_ENTRY in str(m.get("name"))), None)
        check("search entry became a writable combobox", target is not None)
        print(f"    target = {target.get('name')!r} role={target.get('role')}", flush=True)
        written = h.ax("set-value", target["ref_id"], "网络")
        check("set-value delivered", written.get("ok") is True,
              written.get("error", {}).get("code", ""))
        time.sleep(1)
        value = h.ax("get", target["ref_id"], "--property", "value")
        check("value readable", value.get("ok") is True)
        read_back = value.get("data", {}).get("value")
        print(f"    read back = {read_back!r}", flush=True)
        check("typed value round-trips", str(read_back) == "网络", f"got {read_back!r}")

        print("[6] close-app", flush=True)
        h.ax("close-app", h.BUNDLE_ID, "--force")
        launched = False

        print("\nSETTINGS_SEARCH_L3: PASS", flush=True)
        print("  settings search entry opens a writable, readable control", flush=True)
        return 0
    except Exception as error:
        print(f"\nSETTINGS_SEARCH_L3: FAIL - {error}", file=sys.stderr, flush=True)
        return 1
    finally:
        if launched:
            try:
                h.ax("close-app", h.BUNDLE_ID, "--force")
            except Exception:
                pass


if __name__ == "__main__":
    sys.exit(main())
