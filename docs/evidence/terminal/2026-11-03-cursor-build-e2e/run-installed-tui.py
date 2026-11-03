"""Direct PTY acceptance of the installed release pair, using only Python stdlib."""

import fcntl
import json
import os
from pathlib import Path
import pty
import re
import select
import struct
import subprocess
import tempfile
import termios
import time
import uuid


ROOT = Path.cwd()
PAIR = ROOT / "target/cursor-build-e2e-20261103/installed"
EVIDENCE = ROOT / "docs/evidence/terminal/2026-11-03-cursor-build-e2e"
READ_PROMPT = "Use read to read input.txt. In your final answer print its single line followed by a space and completed. Do not write files, run commands or delegate."
PONG_PROMPT = "Reply with the one word obtained by joining p and ong. Do not use tools."
ESCAPES = re.compile(rb"\x1b\][^\x07]*(?:\x07|\x1b\\)|\x1b\[[0-?]*[ -/]*[@-~]")


def main():
    results = []
    for model in ["auto", "composer-2.5"]:
        result = {"model": "cursor/" + model, "passed": False}
        with tempfile.TemporaryDirectory(prefix="aster-cursor-pty-") as temporary:
            root = Path(temporary)
            workspace = root / "workspace 空格"
            workspace.mkdir()
            home = root / "aster-home"
            home.mkdir()
            marker = "cursor-tui-" + str(uuid.uuid4())
            data = (marker + "\n").encode()
            (workspace / "input.txt").write_bytes(data)
            config = root / "config.acl"
            config.write_text(f'import_accounts = ["cursor"]\ndefault_model = "cursor/{model}"\n', encoding="utf-8")
            env = {key: os.environ[key] for key in ["HOME", "PATH", "TMPDIR", "LANG", "LC_ALL"] if key in os.environ}
            env.update(ASTER_HOME=str(home), ASTER_CONFIG=str(config), ASTER_DISABLE_GROK_ACCOUNT="1",
                       ASTER_ACP_AGENT_BIN=str(PAIR / "aster-acp"), TERM="xterm-256color")
            master, slave = pty.openpty()
            fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack("HHHH", 40, 120, 0, 0))

            def terminal_session():
                os.setsid()
                fcntl.ioctl(slave, termios.TIOCSCTTY, 0)

            process = subprocess.Popen([str(PAIR / "aster"), "--cwd", str(workspace)], cwd=workspace,
                                       env=env, stdin=slave, stdout=slave, stderr=slave,
                                       preexec_fn=terminal_session)
            os.close(slave)
            try:
                wait_for(master, process, "cursor ·", 45, trust=True)
                os.write(master, b"\x15" + READ_PROMPT.encode() + b"\r")
                text = wait_for(master, process, marker + " completed", 240)
                assert "eyJ" not in text
                assert (workspace / "input.txt").read_bytes() == data
                assert not (workspace / "output.txt").exists()
                os.write(master, b"\x15" + PONG_PROMPT.encode() + b"\r")
                text = wait_for(master, process, "pong", 90)
                assert "eyJ" not in text
                result.update(passed=True, displayed_file_line=marker + " completed", pong=True)
            except (AssertionError, OSError, TimeoutError) as error:
                result["error"] = str(error)
            finally:
                os.close(master)
                if process.poll() is None:
                    try:
                        process.terminate()
                        try:
                            process.wait(timeout=10)
                        except subprocess.TimeoutExpired:
                            process.kill()
                            process.wait(timeout=10)
                    except (OSError, subprocess.TimeoutExpired) as error:
                        result["cleanup_error"] = str(error)
                        result["passed"] = False
        results.append(result)
        print(json.dumps(result), flush=True)
        (EVIDENCE / "installed-tui-results.json").write_text(json.dumps({"read_prompt": READ_PROMPT,
            "pong_prompt": PONG_PROMPT, "timeouts_seconds": {"ready": 45, "read": 240, "pong": 90},
            "cases": results}, indent=2) + "\n", encoding="utf-8")
    return 0 if all(case["passed"] for case in results) else 1


def wait_for(master, process, needle, timeout, trust=False):
    deadline = time.monotonic() + timeout
    raw = b""
    replied = 0
    trusted = False
    while time.monotonic() < deadline:
        if process.poll() is not None:
            raise AssertionError(f"TUI exited before {needle!r}")
        if not select.select([master], [], [], 0.05)[0]:
            continue
        raw += os.read(master, 65536)
        assert len(raw) <= 8 * 1024 * 1024, "terminal output exceeded its bound"
        queries = raw.count(b"\x1b[6n")
        if queries > replied:
            os.write(master, b"\x1b[12;1R" * (queries - replied))
            replied = queries
        text = ESCAPES.sub(b"", raw).decode("utf-8", errors="replace")
        if trust and not trusted and "Do you trust the contents of this directory" in text:
            os.write(master, b"y")
            trusted = True
        if needle in text:
            return text
    text = ESCAPES.sub(b"", raw).decode("utf-8", errors="replace")
    tail = "[suppressed]" if "eyJ" in text else text[-3500:]
    raise TimeoutError(f"{timeout}s waiting for {needle!r}: {tail}")


if __name__ == "__main__":
    raise SystemExit(main())
