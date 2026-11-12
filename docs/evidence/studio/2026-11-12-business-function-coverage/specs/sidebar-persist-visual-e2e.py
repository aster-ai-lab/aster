#!/usr/bin/env python3
"""F08 sampling: sidebar collapse state survives a real process restart.

Fully agent-desktop visual E2E: launch / toggle / get / close-app / relaunch.
Reuses the F11 restart skeleton; no physical keyboard needed, so it avoids
this session's type --headed frontmost limitation.

Output:
  SIDEBAR_PERSIST_L3: PASS  - collapse state restored from the next process
  SIDEBAR_PERSIST_L3: FAIL  - real finding, reported as-is (assertion unchanged)
"""
import argparse
import importlib.util
import sys
import time
from pathlib import Path

HERE = Path(__file__).resolve().parent
# The switch is renamed with its state: "收起侧边栏" when expanded,
# "展开侧边栏" when collapsed. Its name, not its AX value, is the stable state.
EXPANDED = "收起侧边栏"
COLLAPSED = "展开侧边栏"


def load_helpers():
    spec = importlib.util.spec_from_file_location("ad_launch", HERE / "agent-desktop-launch-e2e.py")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def sidebar_switch(h):
    """Return (state_name, match). Never treat a missing control as a state."""
    for name in (EXPANDED, COLLAPSED):
        data = h.ax("find", "--app", "Aster", "--name", name, "--exact").get("data", {})
        match = next((m for m in data.get("matches", [])
                      if m.get("role") == "switch" and m.get("ref_id")), None)
        if match:
            return name, match
    return None, None


def wait_for_switch(h, attempts=10):
    for _ in range(attempts):
        name, match = sidebar_switch(h)
        if match:
            return name, match
        time.sleep(2)
    return None, None


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
    original = None
    try:
        print("[1] install wrapper + agent-desktop launch", flush=True)
        h.install_wrapper()
        h.launch(args.instance, args.port)
        launched = True
        h.wait_for_window()
        h.dismiss_onboarding()
        original, match = wait_for_switch(h)
        check("sidebar switch exists", match is not None)
        print(f"    initial state = {original!r}", flush=True)

        print("[2] toggle the sidebar switch", flush=True)
        toggled = h.ax("toggle", match["ref_id"])
        check("toggle delivered", toggled.get("ok") is True)
        time.sleep(1.5)
        changed, _ = wait_for_switch(h, attempts=4)
        print(f"    after toggle = {changed!r}", flush=True)
        check("state changed to the opposite label", changed is not None and changed != original,
              f"{original!r} -> {changed!r}")

        print("[3] close-app --force (real process exit)", flush=True)
        h.ax("close-app", h.BUNDLE_ID, "--force")
        launched = False
        time.sleep(3)

        print("[4] agent-desktop launch again and re-read", flush=True)
        h.launch(args.instance, args.port)
        launched = True
        h.wait_for_window()
        h.dismiss_onboarding()
        restored, match2 = wait_for_switch(h)
        check("sidebar switch present after relaunch", match2 is not None)
        print(f"    after relaunch = {restored!r}", flush=True)

        print("[5] restore original state", flush=True)
        if restored != original and match2:
            h.ax("toggle", match2["ref_id"])
            time.sleep(1.5)
        final, _ = wait_for_switch(h, attempts=4)
        print(f"    final = {final!r}", flush=True)

        print("[6] close-app", flush=True)
        h.ax("close-app", h.BUNDLE_ID, "--force")
        launched = False

        if restored == changed:
            print("\nSIDEBAR_PERSIST_L3: PASS", flush=True)
            print("  sidebar collapse state survived a real process restart", flush=True)
            print(f"  ({original!r} -> {changed!r} -> restart -> {restored!r})", flush=True)
            return 0
        print("\nSIDEBAR_PERSIST_L3: FAIL", flush=True)
        print(f"  expected {changed!r} after restart, got {restored!r}; reported as a finding", flush=True)
        return 1
    except Exception as error:
        print(f"\nSIDEBAR_PERSIST_L3: ERROR - {error}", file=sys.stderr, flush=True)
        return 1
    finally:
        if launched:
            try:
                h.ax("close-app", h.BUNDLE_ID, "--force")
            except Exception:
                pass


if __name__ == "__main__":
    sys.exit(main())
