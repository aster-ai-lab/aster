#!/usr/bin/env python3
"""F01 P0' assertions: empty drafts are not persisted, tab switches preserve state.

Companion to draft-navigation-visual-e2e.py, which already has two independent
PASS results for the cross-page draft round trip. This case adds the two
assertions PLAN section 4 still requires before F01 can be counted, without
touching that already-passing case.

Fully agent-desktop visual E2E: launch / drive / quit all go through
agent-desktop. Tab controls are discovered dynamically by AX role; no control
name is guessed. If the environment cannot expose a second tab, the run reports
INCOMPLETE rather than PASS (fail-closed).

Prerequisite: an unrestricted host (see specs/recheck-host.sh). While the
system-level /bin/ps block is active this cannot be executed; it is delivered
as ready-to-run code, NOT as a passing test.

Output:
  F01_P0_PRIME: PASS       - both assertions verified in a real window
  F01_P0_PRIME: INCOMPLETE - a prerequisite control could not be observed
  F01_P0_PRIME: FAIL       - an assertion found a real product problem
"""
import importlib.util
import sys
import time
import uuid
from pathlib import Path

HERE = Path(__file__).resolve().parent

spec = importlib.util.spec_from_file_location("draft_nav", HERE / "draft-navigation-visual-e2e.py")
nav = importlib.util.module_from_spec(spec)
spec.loader.exec_module(nav)


def navigate_away_and_back(h):
    settings_ref, _ = h.ref("设置", "button")
    if not settings_ref:
        raise RuntimeError("settings button not found")
    h.ax("click", settings_ref)
    time.sleep(2)
    work_ref, _ = h.ref("工作", "button")
    if not work_ref:
        raise RuntimeError("work button not found")
    h.ax("click", work_ref)
    time.sleep(2)


def find_tabs(h):
    """Discover tab controls by AX role; never guess a control name."""
    data = h.ax("find", "--app", "Aster", "--role", "tab").get("data", {})
    return [m for m in data.get("matches", []) if m.get("ref_id") and m.get("name")]


def assertion_empty_draft_not_persisted(h):
    """An empty composer draft must not survive navigation as a real draft."""
    print("[A] empty draft is not persisted", flush=True)
    nav.clear_draft(h)
    before = nav.composer_value(h)
    print(f"    composer before = {before!r}", flush=True)
    navigate_away_and_back(h)
    after = nav.composer_value(h)
    print(f"    composer after  = {after!r}", flush=True)
    ok = before.strip() == "" and after.strip() == ""
    print(f"    result = {'PASS' if ok else 'FAIL'}", flush=True)
    # Scope note: this is the observable UI half of "not persisted". Disk-byte
    # evidence would require a second, separate case with a fixture.
    return ok


def assertion_tab_switch_preserves_state(h, token):
    """Switching tabs must not lose the draft or its visible state."""
    print("[B] tab switch preserves composer state", flush=True)
    tabs = find_tabs(h)
    print(f"    tabs discovered = {[t.get('name') for t in tabs][:6]}", flush=True)
    if len(tabs) < 2:
        print("    result = INCOMPLETE (no second tab observable)", flush=True)
        return None

    nav.type_draft(h, token)
    value_before = nav.composer_value(h)
    visible_before = nav.visible_text_count(h, token)
    if token not in value_before or visible_before < 1:
        print("    result = FAIL (draft never reached the live UI)", flush=True)
        return False

    other = next((t for t in tabs if token not in str(t.get("name"))), tabs[-1])
    h.ax("click", other["ref_id"])
    time.sleep(2)
    # Click the original tab again using a fresh lookup: refs are invalidated
    # by snapshot changes.
    tabs_after = find_tabs(h)
    original = next((t for t in tabs_after if t.get("name") == tabs[0].get("name")), None)
    if not original:
        print("    result = INCOMPLETE (original tab not found after switch)", flush=True)
        return None
    h.ax("click", original["ref_id"])
    time.sleep(2)

    value_after = nav.composer_value(h)
    visible_after = nav.visible_text_count(h, token)
    print(f"    value after = {value_after!r}  ui_matches = {visible_after}", flush=True)
    ok = token in value_after and visible_after > 0
    print(f"    result = {'PASS' if ok else 'FAIL'}", flush=True)
    return ok


def main():
    h = nav.load_helpers()
    token = f"ASTERP0{uuid.uuid4().hex[:6].upper()}"
    launched = False
    try:
        print("[1] install wrapper + agent-desktop launch", flush=True)
        h.install_wrapper()
        result = h.launch("f11-live-01", 4325)
        launched = True
        print(f"    launch ok={result.get('ok')}", flush=True)
        h.wait_for_window()
        h.dismiss_onboarding()

        empty_ok = assertion_empty_draft_not_persisted(h)
        tab_ok = assertion_tab_switch_preserves_state(h, token)

        print("[3] restore (clear the draft)", flush=True)
        nav.clear_draft(h)

        if empty_ok is False or tab_ok is False:
            print("\nF01_P0_PRIME: FAIL", flush=True)
            print("  an assertion observed a real product problem", flush=True)
            return 1
        if tab_ok is None:
            print("\nF01_P0_PRIME: INCOMPLETE", flush=True)
            print("  empty-draft assertion passed; tab-switch assertion needs an", flush=True)
            print("  observable second tab. NOT counted as a pass.", flush=True)
            return 2
        print("\nF01_P0_PRIME: PASS", flush=True)
        print("  both F01 P0' assertions verified in a real window", flush=True)
        return 0
    except Exception as error:
        print(f"\nF01_P0_PRIME: ERROR - {error}", file=sys.stderr, flush=True)
        return 1
    finally:
        if launched:
            try:
                h.ax("close-app", h.BUNDLE_ID, "--force")
            except Exception:
                pass


if __name__ == "__main__":
    sys.exit(main())
