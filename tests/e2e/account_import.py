"""Check first-run account choices with isolated host state and no model calls."""

import os
from pathlib import Path
import subprocess
import tempfile


root = Path(__file__).resolve().parents[2]
binaries = (Path(os.environ.get("CARGO_TARGET_DIR", root / "target")) / "debug").resolve()
with tempfile.TemporaryDirectory(prefix="aster-account-import-") as directory:
    env = {
        key: value
        for key, value in os.environ.items()
        if not key.startswith(("ASTER_", "GROK_"))
    }
    env.update({
        "HOME": directory,
        "GROK_HOME": str(Path(directory) / "grok"),
        "ASTER_HOME": str(Path(directory) / ".aster"),
        "ASTER_ACP_AGENT_BIN": str(binaries / "aster-acp"),
        "ASTER_LOG_FILE": "0",
        "TERM": "xterm-256color",
    })
    raise SystemExit(subprocess.call([
        "a3s-test", "run", "tests/e2e/account-import.acl",
        "--tui-executable", str(binaries / "aster"),
        "--tui-working-directory", directory,
        "--tui-columns", "120", "--tui-rows", "40", "--json",
    ], cwd=root, env=env))
