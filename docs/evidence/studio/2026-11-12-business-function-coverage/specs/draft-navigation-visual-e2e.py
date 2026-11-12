#!/usr/bin/env python3
"""F01 L3 sampling: composer draft survives cross-page navigation.

Fully agent-desktop visual E2E: the app is launched by `agent-desktop launch`,
every interaction is an agent-desktop command, and the process is quit with
`agent-desktop close-app`. Reuses the wrapper/launch helpers from
agent-desktop-launch-e2e.py (the same bundle that has Accessibility granted).

Companion to draft-restart-visual-e2e.py, which observed that a new-task
composer draft does NOT survive a process exit (NOT_PERSISTED). This case
checks the different, in-session guarantee: switching away to Settings and
back must not lose the text.

Output:
  DRAFT_NAV_L3: PASS   - draft preserved across navigation
  DRAFT_NAV_L3: FAIL   - draft lost (a real product finding, reported as-is)
"""
import argparse
import importlib.util
from pathlib import Path
import sys
import time
import uuid

HERE = Path(__file__).resolve().parent
COMPOSER = "描述任务，输入 / 使用指令或技能，@ 选择工作区文件"


def load_helpers():
    spec = importlib.util.spec_from_file_location("ad_launch", HERE / "agent-desktop-launch-e2e.py")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def composer_ref(h):
    data = h.ax("find", "--app", "Aster", "--name", COMPOSER).get("data", {})
    return next((m["ref_id"] for m in data.get("matches", [])
                 if m.get("role") == "textfield" and m.get("ref_id")), None)


def composer_value(h):
    ref = composer_ref(h)
    if not ref:
        return ""
    result = h.ax("get", ref, "--property", "value")
    return str(result.get("data", {}).get("value", "")) if result.get("ok") else ""


def visible_text_count(h, token):
    """Confirm the typed text really reached the live UI, not just the AX
    value attribute of a contenteditable node."""
    result = h.ax("find", "--app", "Aster", "--name", token)
    return int(result.get("data", {}).get("total_matches", 0) or 0)


def type_draft(h, token):
    ref = composer_ref(h)
    if not ref:
        raise RuntimeError("composer textfield not found")
    h.ax("focus", ref)
    time.sleep(1)
    ref2 = composer_ref(h)  # focus invalidates snapshot refs
    if not ref2:
        raise RuntimeError("composer textfield not found after focus")
    result = h.ax("type", ref2, token, "--headed")
    if not result.get("ok"):
        raise RuntimeError(f"physical typing failed: {result.get('error', {}).get('code')}")
    time.sleep(2)


def clear_draft(h):
    ref = composer_ref(h)
    if not ref:
        return
    result = h.ax("clear", ref)
    if not result.get("ok"):
        # Rich text editors may refuse AXSetValue; fall back to physical keys.
        h.ax("focus", ref)
        time.sleep(1)
        ref2 = composer_ref(h)
        if ref2:
            h.ax("press", "cmd+a", "--headed")
            h.ax("press", "delete", "--headed")
    time.sleep(1)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--instance", default="f11-live-01")
    ap.add_argument("--port", type=int, default=4325)
    args = ap.parse_args()
    h = load_helpers()
    token = f"ASTERNAV{uuid.uuid4().hex[:6].upper()}"
    launched = False
    try:
        print("[1] install wrapper + agent-desktop launch", flush=True)
        h.install_wrapper()
        result = h.launch(args.instance, args.port)
        launched = True
        print(f"    launch ok={result.get('ok')}", flush=True)
        h.wait_for_window()
        h.dismiss_onboarding()

        print(f"[2] type draft {token} (focus -> re-find -> type --headed)", flush=True)
        type_draft(h, token)
        before = composer_value(h)
        visible_before = visible_text_count(h, token)
        print(f"    value = {before!r}  ui_text_matches = {visible_before}", flush=True)
        if token not in before:
            raise RuntimeError("draft not entered into composer")
        if visible_before < 1:
            raise RuntimeError("typed text is not visible in the live UI; input did not reach the editor")

        print("[3] navigate away to Settings", flush=True)
        settings_ref, _ = h.ref("设置", "button")
        if not settings_ref:
            raise RuntimeError("settings button not found")
        h.ax("click", settings_ref)
        time.sleep(2)

        print("[4] navigate back to Work and re-read the composer", flush=True)
        work_ref, _ = h.ref("工作", "button")
        if not work_ref:
            raise RuntimeError("work button not found")
        h.ax("click", work_ref)
        time.sleep(2)
        after = composer_value(h)
        visible_after = visible_text_count(h, token)
        print(f"    value = {after!r}  ui_text_matches = {visible_after}", flush=True)

        print("[5] restore (clear the draft)", flush=True)
        clear_draft(h)
        restored = composer_value(h)
        print(f"    value after clear = {restored!r}", flush=True)

        if token in after and visible_after > 0:
            print("\nDRAFT_NAV_L3: PASS", flush=True)
            print("  composer draft survived a real cross-page round trip", flush=True)
            print("  (contrast: draft-restart-visual-e2e.py observed NOT_PERSISTED", flush=True)
            print("   across a process exit)", flush=True)
            return 0
        print("\nDRAFT_NAV_L3: FAIL", flush=True)
        print("  composer draft was lost on navigation; reported as a finding", flush=True)
        return 1
    except Exception as error:
        print(f"\nDRAFT_NAV_L3: ERROR - {error}", file=sys.stderr, flush=True)
        return 1
    finally:
        if launched:
            try:
                h.ax("close-app", h.BUNDLE_ID, "--force")
            except Exception:
                pass


if __name__ == "__main__":
    sys.exit(main())
