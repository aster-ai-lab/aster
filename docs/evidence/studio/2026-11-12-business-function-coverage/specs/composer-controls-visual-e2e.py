#!/usr/bin/env python3
"""F02 control subset: composer AX surface and the More-options attachment dialog.

Fully agent-desktop visual E2E (launch/drive/quit), but deliberately limited
to headless semantic actions: this session's window cannot become frontmost,
so `type --headed` fails focus_verification and Chinese/slash-command input
assertions are NOT part of this case. See OUTCOME round 30 for that limit.

Output:
  COMPOSER_CONTROLS_L3: PASS  - all semantic control assertions hold
  COMPOSER_CONTROLS_L3: FAIL  - a control assertion failed (recorded as finding)
"""
import argparse
import importlib.util
import sys
import time
from pathlib import Path

HERE = Path(__file__).resolve().parent
COMPOSER = "描述任务，输入 / 使用指令或技能，@ 选择工作区文件"


def load_helpers():
    spec = importlib.util.spec_from_file_location("ad_launch", HERE / "agent-desktop-launch-e2e.py")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def find(h, name, role=None):
    data = h.ax("find", "--app", "Aster", "--name", name, "--exact").get("data", {})
    matches = [m for m in data.get("matches", []) if (role is None or m.get("role") == role) and m.get("ref_id")]
    return matches


def one(h, name, role=None):
    matches = find(h, name, role)
    return matches[0] if matches else None


def wait_for(h, name, role, attempts=8):
    """The More-options dialog mounts asynchronously; poll instead of a fixed
    sleep. This waits for the control to exist, it does not relax the assertion."""
    for _ in range(attempts):
        match = one(h, name, role)
        if match:
            return match
        time.sleep(1)
    return None


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
        time.sleep(2)

        print("[2] composer and its semantic controls", flush=True)
        composer = one(h, COMPOSER, "textfield")
        check("composer textfield exists", composer is not None)
        value_before = h.ax("get", composer["ref_id"], "--property", "value").get("data", {}).get("value", "")
        more = one(h, "更多选项", "combobox")
        check("More-options combobox exists", more is not None)
        permission = one(h, "授权模式: 请求批准", "combobox")
        check("permission-mode combobox exists", permission is not None)
        optimize = one(h, "优化提示词", "combobox")
        check("prompt-optimize combobox exists", optimize is not None)
        sidebar = one(h, "收起侧边栏", "switch")
        check("sidebar collapse switch exists", sidebar is not None)
        check("sidebar switch is checked", "checked" in (sidebar.get("states") or []))

        print("[3] open More options and inspect the attachment dialog", flush=True)
        attachment = wait_for(h, "上传附件", "combobox", attempts=2)
        if attachment is None:
            clicked = h.ax("click", more["ref_id"])
            check("More-options click delivered", clicked.get("ok") is True)
            attachment = wait_for(h, "上传附件", "combobox")
        check("attachment item exists in dialog", attachment is not None)
        check("attachment item lives under the dialog",
              any("dialog:更多选项" in str(p) for p in (attachment.get("path") or [])))

        print("[4] dismiss dialog; composer draft must be unchanged", flush=True)
        h.ax("press", "escape")
        time.sleep(1)
        composer2 = one(h, COMPOSER, "textfield")
        check("composer still present", composer2 is not None)
        value_after = h.ax("get", composer2["ref_id"], "--property", "value").get("data", {}).get("value", "")
        check("draft unchanged by attachment panel", value_after == value_before,
              f"before={value_before!r} after={value_after!r}")

        print("[5] close-app", flush=True)
        h.ax("close-app", h.BUNDLE_ID, "--force")
        launched = False

        print("\nCOMPOSER_CONTROLS_L3: PASS", flush=True)
        print("  (F02 control subset only; typed input / slash commands / queue", flush=True)
        print("   are NOT asserted - see OUTCOME for the frontmost limitation)", flush=True)
        return 0
    except Exception as error:
        print(f"\nCOMPOSER_CONTROLS_L3: FAIL - {error}", file=sys.stderr, flush=True)
        return 1
    finally:
        if launched:
            try:
                h.ax("close-app", h.BUNDLE_ID, "--force")
            except Exception:
                pass


if __name__ == "__main__":
    sys.exit(main())
