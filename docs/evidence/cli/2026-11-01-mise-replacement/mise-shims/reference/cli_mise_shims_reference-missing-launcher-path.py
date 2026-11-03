#!/usr/bin/env python3
"""Freeze real shim dispatch and reshim behavior from the pinned mise binary."""
from __future__ import annotations

import argparse
import hashlib
import json
import os
import platform
import subprocess
import tempfile
from pathlib import Path

from cli_mise_batch_install_reference import CONTRACTS, TIMEOUT_SECONDS


def executable(path: Path, text: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(text, encoding="utf-8", newline="\n")
    path.chmod(0o755)


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--mise", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    if os.name != "posix":
        parser.error("the native fixtures require a POSIX shell")
    if args.output.exists():
        parser.error("refusing to replace an existing capture")
    binary = args.mise.resolve()
    frozen = json.loads((CONTRACTS / "mise-cli-reference.json").read_text(encoding="utf-8"))
    digest = hashlib.sha256(binary.read_bytes()).hexdigest()
    if digest != frozen["binary_sha256"]:
        parser.error("mise does not match the frozen binary digest")
    report = {"tag": frozen["tag"], "release_commit": frozen["release_commit"],
              "binary_sha256": digest, "host": {"os": platform.system(), "arch": platform.machine()},
              "timeout_seconds": TIMEOUT_SECONDS, "status": "running", "commands": [], "cases": []}
    args.output.parent.mkdir(parents=True, exist_ok=True)

    def record() -> None:
        args.output.write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")

    record()
    with tempfile.TemporaryDirectory(prefix="aster shim mise 中文 ") as raw:
        root = Path(raw)
        for name in ["home", "config", "data", "cache", "system", "one", "two", "missing", "empty"]:
            (root / name).mkdir()
        env = {"PATH": "/usr/bin:/bin", "HOME": str(root / "home"),
               "MISE_CONFIG_DIR": str(root / "config"), "MISE_DATA_DIR": str(root / "data"),
               "MISE_CACHE_DIR": str(root / "cache"), "MISE_SYSTEM_CONFIG_DIR": str(root / "system"),
               "MISE_GLOBAL_CONFIG_FILE": str(root / "config/global.toml"), "MISE_EXPERIMENTAL": "1",
               "MISE_PARANOID": "1", "MISE_YES": "1", "NO_COLOR": "1"}

        def invoke(argv: list[str | Path], cwd: Path, success: bool = True) -> subprocess.CompletedProcess[str]:
            row = {"argv": [str(a) for a in argv], "cwd": str(cwd), "status": "running"}
            report["commands"].append(row)
            record()
            try:
                result = subprocess.run(argv, cwd=cwd, env=env, stdin=subprocess.DEVNULL,
                                        capture_output=True, encoding="utf-8", timeout=TIMEOUT_SECONDS)
            except subprocess.TimeoutExpired as error:
                row.update(status="timeout", stdout_hex=(error.stdout or b"").hex(),
                           stderr_hex=(error.stderr or b"").hex())
                report["status"] = "timeout"
                record()
                raise
            row.update(status="returned", exit=result.returncode, stdout=result.stdout, stderr=result.stderr)
            record()
            assert (result.returncode == 0) == success, row
            return result

        plugin = root / "plugin"
        hooks = {
            "list-all": "#!/bin/sh\nprintf '%s\\n' '1.0.0 2.0.0 3.0.0'\n",
            "install": "#!/bin/sh\nset -eu\nmkdir -p \"$ASDF_INSTALL_PATH/bin\"\n"
            + "printf '#!/bin/sh\\nprintf \"%%s\\\\n\" \"%s\"\\nprintf \"%%s\\\\n\" \"$@\"\\n' \"$ASDF_INSTALL_VERSION\" > \"$ASDF_INSTALL_PATH/bin/fixture\"\n"
            + "chmod 755 \"$ASDF_INSTALL_PATH/bin/fixture\"\n"
            + "if [ \"$ASDF_INSTALL_VERSION\" = '2.0.0' ]; then\n"
            + "printf '#!/bin/sh\\nprintf \"%%s\\\\n\" \"extra:2.0.0\"\\n' > \"$ASDF_INSTALL_PATH/bin/fixture-extra\"\n"
            + "chmod 755 \"$ASDF_INSTALL_PATH/bin/fixture-extra\"\nfi\n",
        }
        report["hook_sha256"] = {name: hashlib.sha256(text.encode()).hexdigest() for name, text in hooks.items()}
        for name, text in hooks.items():
            executable(plugin / "bin" / name, text)
        invoke([binary, "plugins", "link", "fixture", plugin], root / "empty")
        invoke([binary, "install", "fixture@1.0.0", "fixture@2.0.0"], root / "empty")
        for name, versions in [("one", ["1.0.0", "2.0.0"]), ("two", ["2.0.0", "1.0.0"]), ("missing", ["3.0.0"])]:
            config = root / name / "mise.toml"
            config.write_text('[tools]\nfixture = ' + json.dumps(versions) + '\n', encoding="utf-8")
            invoke([binary, "trust", config], root / name)
        shim = root / "data/shims/fixture"
        extra = root / "data/shims/fixture-extra"
        tail = ["space $value", "quote'\"", "中文"]
        for name, command, cwd, success in [
            ("project-forward", [shim, *tail], "one", True),
            ("project-reverse", [shim], "two", True),
            ("secondary-command", [extra], "one", True),
            ("missing-selected", [shim], "missing", True),
            ("unconfigured", [shim], "empty", False),
        ]:
            result = invoke(command, root / cwd, success)
            report["cases"].append({"name": name, "exit": result.returncode, "stdout": result.stdout})
            record()
        shim.unlink()
        invoke([binary, "reshim"], root / "one")
        result = invoke([shim], root / "two")
        report["cases"].append({"name": "repair-missing-shim", "exit": result.returncode, "stdout": result.stdout})
    report["status"] = "passed"
    record()
    assert json.loads(args.output.read_text(encoding="utf-8")) == report
    print(f"captured {len(report['cases'])} native shim cases")


if __name__ == "__main__":
    main()
