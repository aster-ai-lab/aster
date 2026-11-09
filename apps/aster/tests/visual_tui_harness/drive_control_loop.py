#!/usr/bin/env python3
"""Visual companion for aster-core control-loop DeepSeek E2E (R1–R3) in Aster TUI.

Maps the session/Run cybernetic analogues onto the product pager with
agent-desktop window ops + Terminal history + workspace file oracles:

- V-R1: Run-inner write sensor (loop_out.txt exact token)
- V-R2: /goal controller — status → set → pause → resume (visible copy)
- V-R3: quit + --continue reopen; token bytes + goal status survive

Companion to crates/code/core/tests/test_control_loop_deepseek_e2e.rs.
No Mock LLM. Pins DeepSeek via ASTER_CONFIG + --model.
"""
from __future__ import annotations

import argparse
import json
import subprocess
import sys
import time
import uuid
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from drive_shop_multiturn import (  # noqa: E402
    answer_dialogs,
    capture,
    find_wid,
    keystroke_raw,
    list_aster_terminal_windows,
    paste_submit,
    settled,
    sh,
    terminal_history,
    wait_pred,
)


def clear_composer() -> None:
    """Hard-clear the pager composer before the next paste_submit."""
    keystroke_raw(
        """  key code 53
  delay 0.08
  key code 53
  delay 0.08
  keystroke "a" using {command down}
  delay 0.08
  key code 51
  delay 0.12
"""
    )


def submit_line(text: str) -> None:
    clear_composer()
    time.sleep(0.2)
    paste_submit(text)
    time.sleep(0.4)


def cancel_turn() -> None:
    keystroke_raw(
        """  keystroke "c" using {control down}
  delay 0.4
"""
    )


def wait_idle(timeout: float, label: str) -> bool:
    """Wait until the pager is not Responding/Thinking; cancel once if stuck."""
    started = time.time()
    deadline = started + timeout
    cancelled = False
    while time.time() < deadline:
        hist = terminal_history()
        if answer_dialogs(hist):
            time.sleep(1.0)
            continue
        if settled(hist):
            print(f"OK {label}", flush=True)
            return True
        if not cancelled and time.time() - started > 25:
            cancel_turn()
            cancelled = True
        time.sleep(1.5)
    print(f"FAIL {label}", flush=True)
    return False


def ensure_terminal_app() -> None:
    """agent-desktop binds the localized name 终端 on this host."""
    probe = sh(["agent-desktop", "list-windows", "--app", "终端"], check=False)
    if probe.returncode == 0 and (probe.stdout or "").strip():
        try:
            payload = json.loads(probe.stdout)
            if payload.get("ok"):
                return
        except json.JSONDecodeError:
            pass
    subprocess.run(["open", "-a", "Terminal"], check=False)
    deadline = time.time() + 30
    while time.time() < deadline:
        probe = sh(["agent-desktop", "list-windows", "--app", "终端"], check=False)
        if probe.returncode == 0 and '"ok":true' in (probe.stdout or "").replace(" ", ""):
            return
        if probe.returncode == 0:
            try:
                if json.loads(probe.stdout).get("ok"):
                    return
            except json.JSONDecodeError:
                pass
        time.sleep(0.5)
    raise SystemExit("Terminal/终端 did not become visible to agent-desktop")


def hist_has_goal_empty(h: str) -> bool:
    return (
        "No goal is set." in h
        or "当前未设置目标" in h
        or "未设置目标" in h
        or "没有目标" in h
    )


def hist_has_paused(h: str) -> bool:
    return "Goal paused" in h or "目标已暂停" in h


def hist_has_goal_progress(h: str) -> bool:
    tokens = (
        "Goal complete",
        "目标已完成",
        "目标尚未完成",
        "需要你的输入后才能继续",
        "Your input is needed",
        "已达到执行上限",
        "Budget",
        "Goal slice",
        "目标切片",
        "已完成",
        "Completed",
        "Goal paused",
        "目标已暂停",
    )
    return any(t in h for t in tokens)


def hist_mentions_deepseek(h: str) -> bool:
    low = h.lower()
    return "deepseek" in low


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--bin-dir", type=Path, required=True)
    ap.add_argument("--work-dir", type=Path, required=True)
    ap.add_argument("--evidence", type=Path, required=True)
    ap.add_argument("--model", default="deepseek/deepseek-flash")
    ap.add_argument("--config", type=Path, default=None)
    args = ap.parse_args()

    ensure_terminal_app()

    bin_dir = args.bin_dir.resolve()
    ws = (args.work_dir / "ws").resolve()
    ev = args.evidence.resolve()
    ev.mkdir(parents=True, exist_ok=True)
    ws.mkdir(parents=True, exist_ok=True)
    for leftover in ("loop_out.txt", "goal_out.txt"):
        p = ws / leftover
        if p.exists():
            p.unlink()

    token = f"CL-VIS-{uuid.uuid4()}"
    (ev / "token.txt").write_text(token)

    config = args.config
    if config is None:
        repo_acl = Path(__file__).resolve().parents[4] / ".aster" / "config.acl"
        config = repo_acl if repo_acl.is_file() else None
    else:
        config = config.resolve()
        if not config.is_file():
            raise SystemExit(f"--config not a file: {config}")

    marker = str(ws.resolve())
    title_tag = f"ASTER-VIS-CL-{ws.name}-{int(time.time())}"
    launch = ev / "launch_aster.sh"
    config_line = f"export ASTER_CONFIG='{config}'" if config is not None else "# no ASTER_CONFIG"
    launch.write_text(
        "\n".join(
            [
                "#!/bin/bash",
                "set -euo pipefail",
                f"export PATH='{bin_dir}':\"$PATH\"",
                config_line,
                f"cd '{ws}'",
                f"printf '\\033]0;{title_tag}\\007'",
                "clear",
                f"exec aster --model '{args.model}' --no-plan --fullscreen",
                "",
            ]
        )
    )
    launch.chmod(0o755)

    before_ids = {w["id"] for w in list_aster_terminal_windows()}
    subprocess.run(["open", "-a", "Terminal", str(launch)], check=True)
    wid = find_wid(marker, title_tag=title_tag, before_ids=before_ids)
    (ev / "wid.txt").write_text(wid)
    (ev / "title_tag.txt").write_text(title_tag)
    sh(["agent-desktop", "focus-window", "--app", "终端", "--window-id", wid], check=False)
    time.sleep(2)

    # --- welcome / model pin ---
    assert wait_pred(
        lambda h: "Type a message" in h
        or "❯" in h
        or "Add a follow-up" in h
        or hist_mentions_deepseek(h),
        120,
        "welcome",
    ), "welcome"
    capture(ev, wid, "01-welcome")
    welcome_hist = terminal_history()
    deepseek_pin = hist_mentions_deepseek(welcome_hist)
    (ev / "01-welcome-deepseek.txt").write_text(f"deepseek_visible={deepseek_pin}\n")

    # --- V-R2 prelude: empty goal status (controller idle) ---
    submit_line("/goal status")
    assert wait_pred(
        lambda h: hist_has_goal_empty(h)
        or ("/goal" in h and ("status" in h.lower() or "目标" in h) and settled(h)),
        90,
        "goal-status-empty",
    ), "goal-status-empty"
    # Fail closed if the composer still holds an unsent slash line.
    stuck = terminal_history()
    if "→ /goal status" in stuck and "No goal is set." not in stuck and "当前未设置目标" not in stuck:
        clear_composer()
        submit_line("/goal status")
        assert wait_pred(lambda h: hist_has_goal_empty(h), 90, "goal-status-empty-retry"), (
            "goal-status-empty-retry"
        )
    capture(ev, wid, "02-goal-empty")
    assert hist_has_goal_empty(terminal_history()), "empty goal copy must be visible"

    # --- V-R1: Run-inner write sensor ---
    r1_prompt = (
        f"Create loop_out.txt containing exactly the token {token} "
        "using the write tool. Then stop. Do not invent verification."
    )
    submit_line(r1_prompt)
    assert wait_pred(
        lambda h: settled(h) and (ws / "loop_out.txt").is_file(),
        420,
        "r1-write",
    ), "r1-write"
    loop_bytes = (ws / "loop_out.txt").read_text()
    assert loop_bytes == token, f"R1 token mismatch: {loop_bytes!r}"
    capture(ev, wid, "03-r1-write")

    # --- V-R2: durable goal set → automatic slice → pause → resume ---
    goal_prompt = (
        f"/goal Create goal_out.txt containing exactly the token {token} "
        "using the write tool, then report the host-only release code that "
        "no file in this workspace contains"
    )
    submit_line(goal_prompt)
    assert wait_pred(
        lambda h: (ws / "goal_out.txt").is_file()
        or (hist_has_goal_progress(h) and settled(h)),
        420,
        "r2-goal-progress",
    ), "r2-goal-progress"
    capture(ev, wid, "04-r2-goal-active")
    # Slash control must not be queued as a follow-up while the slice is busy.
    assert wait_idle(180, "r2-idle-before-pause"), "r2-idle-before-pause"

    submit_line("/goal pause")
    assert wait_pred(
        lambda h: settled(h) and (hist_has_paused(h) or "paused" in h.lower() or "暂停" in h),
        120,
        "r2-goal-pause",
    ), "r2-goal-pause"
    capture(ev, wid, "05-r2-paused")
    paused_hist = terminal_history()

    submit_line("/goal resume")
    assert wait_pred(
        lambda h: settled(h)
        and (
            hist_has_goal_progress(h)
            or "resume" in h.lower()
            or "继续" in h
            or (ws / "goal_out.txt").is_file()
        ),
        420,
        "r2-goal-resume",
    ), "r2-goal-resume"
    capture(ev, wid, "06-r2-resumed")

    submit_line("/goal status")
    assert wait_pred(lambda h: settled(h) and not hist_has_goal_empty(h), 60, "r2-status"), "r2-status"
    capture(ev, wid, "07-r2-status")
    status_hist = terminal_history()

    # --- V-R3: reopen via --continue ---
    submit_line("/quit")
    time.sleep(2)

    continue_launch = ev / "relaunch_continue.sh"
    continue_launch.write_text(
        "\n".join(
            [
                "#!/bin/bash",
                "set -euo pipefail",
                f"export PATH='{bin_dir}':\"$PATH\"",
                config_line,
                f"cd '{ws}'",
                f"printf '\\033]0;{title_tag}-REOPEN\\007'",
                "clear",
                f"exec aster --model '{args.model}' --no-plan --fullscreen --continue",
                "",
            ]
        )
    )
    continue_launch.chmod(0o755)
    before_reopen = {w["id"] for w in list_aster_terminal_windows()}
    subprocess.run(["open", "-a", "Terminal", str(continue_launch)], check=True)
    wid2 = find_wid(marker, title_tag=f"{title_tag}-REOPEN", before_ids=before_reopen)
    (ev / "wid-reopen.txt").write_text(wid2)
    sh(["agent-desktop", "focus-window", "--app", "终端", "--window-id", wid2], check=False)
    time.sleep(2)

    assert wait_pred(
        lambda h: "Type a message" in h
        or "Add a follow-up" in h
        or "❯" in h
        or token in h
        or hist_has_goal_progress(h),
        120,
        "r3-reopen-welcome",
    ), "r3-reopen-welcome"
    assert (ws / "loop_out.txt").read_text() == token, "R3: loop_out.txt must survive reopen"
    capture(ev, wid2, "08-r3-reopen")

    submit_line("/goal status")
    assert wait_pred(lambda h: settled(h), 90, "r3-goal-status"), "r3-goal-status"
    capture(ev, wid2, "09-r3-goal-status")
    reopen_status = terminal_history()

    goal_out = ws / "goal_out.txt"
    summary = {
        "visual_control_loop": True,
        "model": args.model,
        "token": token,
        "deepseek_visible_on_welcome": deepseek_pin,
        "r1_loop_out_exact": (ws / "loop_out.txt").read_text() == token,
        "r2_paused_visible": hist_has_paused(paused_hist)
        or "暂停" in paused_hist
        or "paused" in paused_hist.lower(),
        "r2_status_bound": not hist_has_goal_empty(status_hist),
        "r3_loop_out_survived": (ws / "loop_out.txt").read_text() == token,
        "r3_goal_still_bound": not hist_has_goal_empty(reopen_status),
        "goal_out_written": goal_out.is_file(),
        "goal_out_exact": goal_out.read_text() == token if goal_out.is_file() else False,
    }
    summary["pass"] = bool(
        summary["r1_loop_out_exact"]
        and summary["r2_paused_visible"]
        and summary["r2_status_bound"]
        and summary["r3_loop_out_survived"]
        and summary["r3_goal_still_bound"]
    )
    (ev / "visual-control-loop-summary.json").write_text(json.dumps(summary, indent=2))
    (ev / "visual-control-loop-runs.txt").write_text(
        "\n".join(
            [
                f"token={token}",
                f"deepseek_pin={deepseek_pin}",
                f"r1={summary['r1_loop_out_exact']}",
                f"r2_paused={summary['r2_paused_visible']}",
                f"r2_bound={summary['r2_status_bound']}",
                f"r3_bytes={summary['r3_loop_out_survived']}",
                f"r3_goal={summary['r3_goal_still_bound']}",
                f"pass={summary['pass']}",
                "",
            ]
        )
    )
    print(json.dumps(summary, indent=2))
    return 0 if summary["pass"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
