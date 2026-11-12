#!/usr/bin/env python3
"""agent-desktop visual E2E: does the composer draft survive a real process exit?

Fits the user's criterion (app is a real Tauri window, every test action goes
through agent-desktop). Note: `just studio` starts the app because the
instance's hand-built Aster.app cannot be launched by Launch Services; see
OUTCOME.md "口径补充".

The composer is a rich-text editor, so semantic AX input is refused. The
working sequence is: focus -> re-find the ref (focus invalidates it) ->
agent-desktop type --headed (physical keyboard, no coordinates or mouse).

Output is an OBSERVATION, not a forced pass:
  DRAFT_RESTART: RESTORED       -> F16 L3 sampling point holds
  DRAFT_RESTART: NOT_PERSISTED  -> the product does not keep drafts across a
                                   process restart; NOT counted as coverage.

Usage: python3 draft-restart-visual-e2e.py [--port PORT] [--instance NAME]
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
COMPOSER = "描述任务，输入 / 使用指令或技能，@ 选择工作区文件"


def ax(*args, timeout=40):
    out = subprocess.run(["agent-desktop", *args], capture_output=True, text=True, timeout=timeout)
    try:
        return json.loads(out.stdout)
    except json.JSONDecodeError:
        raise RuntimeError(f"agent-desktop {' '.join(args)}: {out.stdout[:250]} {out.stderr[:150]}")


def composer_ref():
    data = ax("find", "--app", "Aster", "--name", COMPOSER).get("data", {})
    return next((m["ref_id"] for m in data.get("matches", []) if m.get("role") == "textfield" and m.get("ref_id")), None)


def read_draft():
    ref = composer_ref()
    if not ref:
        raise RuntimeError("composer textfield not found")
    return ax("get", ref, "--property", "value").get("data", {}).get("value", "")


def type_draft(token):
    ref = composer_ref()
    if not ref:
        raise RuntimeError("composer textfield not found")
    ax("focus", ref)
    time.sleep(1)
    ref2 = composer_ref()  # focus invalidates the previous snapshot refs
    if not ref2:
        raise RuntimeError("composer textfield not found after focus")
    result = ax("type", ref2, token, "--headed")
    if not result.get("ok"):
        raise RuntimeError(f"physical typing failed: {result.get('error', {}).get('code')}")
    time.sleep(2)
    return read_draft()


def windows():
    return [w for w in ax("list-windows").get("data", []) if w.get("visible") and w.get("app_name") == "Aster"]


def port_busy(port):
    out = subprocess.run(["lsof", "-nP", f"-iTCP:{port}", "-sTCP:LISTEN"], capture_output=True, text=True)
    return "LISTEN" in out.stdout


class Studio:
    def __init__(self, instance, port, tag):
        self.instance, self.port, self.tag = instance, port, tag
        self.proc = None
        self.log = open(f"/tmp/draft-{tag}.log", "w")

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
                raise RuntimeError(f"studio exited early ({self.proc.returncode}), see /tmp/draft-{self.tag}.log")
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


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--port", type=int, default=4325)
    ap.add_argument("--instance", default="f11-live-01")
    args = ap.parse_args()
    token = f"ASTERDRAFT{int(time.time()) % 1000000}"
    studio = None
    try:
        print(f"[1] start {args.instance} on {args.port}", flush=True)
        studio = Studio(args.instance, args.port, "run1")
        studio.start()

        print(f"[2] type draft {token} via agent-desktop physical keyboard", flush=True)
        before = type_draft(token)
        print(f"    composer value = {before!r}", flush=True)
        if token not in before:
            raise RuntimeError("draft was not entered into the composer")
        print("  OK  draft visible in the real window", flush=True)

        print("[3] kill the real process", flush=True)
        studio.stop()
        if windows() or port_busy(args.port):
            raise RuntimeError("process did not fully exit")

        print("[4] relaunch the same instance", flush=True)
        studio = Studio(args.instance, args.port, "run2")
        studio.start()

        print("[5] read the composer back in the real window", flush=True)
        after = read_draft()
        print(f"    composer value = {after!r}", flush=True)

        if token in after:
            print("\nDRAFT_RESTART: RESTORED", flush=True)
            print("  F16 L3 sampling point holds: draft survived a real process exit.", flush=True)
        else:
            print("\nDRAFT_RESTART: NOT_PERSISTED", flush=True)
            print("  Observation: the product does not keep a new-task composer draft", flush=True)
            print("  across a real process exit. Reported as a finding, NOT counted", flush=True)
            print("  as F16 coverage; the expectation was not weakened to force a pass.", flush=True)
        return 0
    except Exception as error:
        print(f"\nDRAFT_RESTART: ERROR - {error}", file=sys.stderr, flush=True)
        return 1
    finally:
        if studio:
            try:
                studio.stop()
            except Exception:
                pass


if __name__ == "__main__":
    sys.exit(main())
