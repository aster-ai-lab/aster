"""Sample the ACP process during the immediate-grep stall and clean it up."""

import os
from pathlib import Path
import signal
import subprocess
import tempfile
import time

ROOT = Path(__file__).resolve().parents[4]
EVIDENCE = ROOT / "docs/evidence/code/2026-11-04-grep-e2e"
PROMPT = (
    "Use the search tool exactly once with mode='grep', "
    "query='aster-convergence-plan\\.html|native-client-architecture\\.md|target-goal-issues|target-issue-search|\\.kimi-tmp', "
    "path='.', include='*.{md,html,toml,yml,yaml,json,acl,justfile}', "
    "output_mode='content', limit=40. Do not use any other tool, do not run commands, "
    "and do not delegate. Return the tool's truncation status and a one-sentence summary."
)


def main():
    aster = ROOT / "target/debug/aster"
    acp = ROOT / "target/debug/aster-acp"
    sandbox = subprocess.check_output(
        ["python3", "scripts/aster_sandbox.py", "cache", "--target-dir", "target"],
        cwd=ROOT,
        text=True,
    ).strip()
    with tempfile.TemporaryDirectory(prefix="aster-grep-debug-") as temporary:
        home = Path(temporary) / "aster-home"
        home.mkdir()
        config = Path(temporary) / "config.acl"
        config.write_text(
            'import_accounts = ["cursor"]\ndefault_model = "cursor/auto"\n'
            "workspace_retrieval { enabled = true\nlocal_cpu {}\n}\n",
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
            ASTER_LOG_DIR=str(EVIDENCE / "debug-logs"),
            TERM="xterm-256color",
            RUST_LOG="info,aster_core::workspace=debug,aster_core::tools=debug",
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
            "3",
        ]
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
            time.sleep(90)
            processes = subprocess.check_output(
                ["ps", "-axo", "pid,ppid,command"], text=True
            ).splitlines()
            child = next(
                (
                    line.split(None, 2)[0]
                    for line in processes
                    if str(process.pid) in line and "target/debug/aster-acp" in line
                ),
                None,
            )
            if child:
                sample = subprocess.run(
                    ["sample", child, "1"],
                    capture_output=True,
                    text=True,
                    timeout=15,
                )
                (EVIDENCE / "acp-sample.txt").write_text(
                    sample.stdout + sample.stderr, encoding="utf-8"
                )
            else:
                (EVIDENCE / "acp-sample.txt").write_text(
                    "ACP process not found\n", encoding="utf-8"
                )
        finally:
            if process.poll() is None:
                os.killpg(process.pid, signal.SIGKILL)
            output, _ = process.communicate()
            (EVIDENCE / "debug-output.log").write_text(output, encoding="utf-8")


if __name__ == "__main__":
    main()
