"""Run the real read-only repository-analysis task against the freshly built pair."""

import json
import os
from pathlib import Path
import shutil
import signal
import subprocess
import tempfile
import time

ROOT = Path(__file__).resolve().parents[4]
EVIDENCE = ROOT / "docs/evidence/code/2026-11-04-grep-e2e"
PROMPT = "分析下 aster 中存在哪些垃圾文件"
TIMEOUT_SECONDS = 300


def main():
    aster = ROOT / "target/debug/aster"
    acp = ROOT / "target/debug/aster-acp"
    assert aster.is_file() and acp.is_file(), "run just build first"
    sandbox = subprocess.check_output(
        ["python3", "scripts/aster_confine.py", "cache", "--target-dir", "target"],
        cwd=ROOT,
        text=True,
    ).strip()
    result = {
        "prompt": PROMPT,
        "timeout_seconds": TIMEOUT_SECONDS,
        "passed": False,
        "build_command": "CARGO_BUILD_JOBS=8 just build",
        "aster": str(aster),
        "acp": str(acp),
    }
    with tempfile.TemporaryDirectory(prefix="aster-grep-e2e-") as temporary:
        home = Path(temporary) / "aster-home"
        home.mkdir()
        config = Path(temporary) / "config.acl"
        config.write_text(
            'import_accounts = ["cursor"]\ndefault_model = "cursor/auto"\n',
            encoding="utf-8",
        )
        env = {
            key: os.environ[key]
            for key in ["HOME", "PATH", "TMPDIR", "LANG", "LC_ALL"]
            if key in os.environ
        }
        env.update(
            ASTER_HOME=str(home),
            ASTER_CONFIG=str(config),
            ASTER_DISABLE_GROK_ACCOUNT="1",
            ASTER_ACP_AGENT_BIN=str(acp),
            ASTER_SANDBOX_DIR=sandbox,
            TERM="xterm-256color",
        )
        command = [
            str(aster),
            "--single",
            PROMPT,
            "--no-plan",
            "--no-auto-update",
            "--permission-mode",
            "auto",
            "--max-turns",
            "12",
        ]
        result["command"] = command
        started = time.monotonic()
        process = subprocess.Popen(
            command,
            cwd=ROOT,
            env=env,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            text=True,
            start_new_session=True,
        )
        try:
            output, _ = process.communicate(timeout=TIMEOUT_SECONDS)
            result["exit_code"] = process.returncode
            result["passed"] = process.returncode == 0 and "Turn failed" not in output
        except subprocess.TimeoutExpired:
            os.killpg(process.pid, signal.SIGKILL)
            output, _ = process.communicate()
            result["error"] = f"task exceeded {TIMEOUT_SECONDS} seconds"
        result["elapsed_seconds"] = round(time.monotonic() - started, 3)
        (EVIDENCE / "task-output.log").write_text(output, encoding="utf-8")
        logs = home / "logs"
        if logs.is_dir():
            shutil.copytree(logs, EVIDENCE / "logs", dirs_exist_ok=True)
    (EVIDENCE / "result.json").write_text(
        json.dumps(result, ensure_ascii=False, indent=2) + "\n", encoding="utf-8"
    )
    print(json.dumps(result, ensure_ascii=False))
    return 0 if result["passed"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
