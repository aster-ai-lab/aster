#!/usr/bin/env python3
"""Capture multi-tool removal from the frozen mise binary in isolated local stores."""
from __future__ import annotations

import argparse
import hashlib
import json
import os
import platform
import subprocess
import tempfile
from pathlib import Path

CONTRACTS = Path(__file__).resolve().parents[1] / "specs/031-cli-mise-replacement/contracts"
TIMEOUT_SECONDS = 25
INSTALLATIONS = {"fixture": ["1.0.0", "2.0.0"], "other": ["1.0.0"]}
CASES = [
    ("multiple_exact", ["fixture@1.0.0", "other@1.0.0"], 0),
    ("later_ambiguous", ["other", "fixture"], 1),
    ("global_all", ["--all"], 0),
    ("global_preview", ["--all", "--dry-run-code"], 1),
    ("duplicate_requests", ["fixture@1.0.0", "fixture@1.0.0"], 0),
]


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--mise", required=True, type=Path)
    parser.add_argument("--output", required=True, type=Path)
    parser.add_argument("--case", choices=[case[0] for case in CASES], action="append")
    args = parser.parse_args()
    if os.name != "posix":
        parser.error("the native reference fixtures require a POSIX shell")
    if args.output.exists():
        parser.error("refusing to overwrite an existing capture")
    binary = args.mise.resolve()
    reference = json.loads((CONTRACTS / "mise-cli-reference.json").read_text(encoding="utf-8"))
    digest = hashlib.sha256(binary.read_bytes()).hexdigest()
    if digest != reference["binary_sha256"]:
        parser.error("mise does not match the frozen binary digest")
    report = {
        "tag": reference["tag"], "release_commit": reference["release_commit"],
        "binary_sha256": digest, "host": {"os": platform.system(), "arch": platform.machine()},
        "timeout_seconds": TIMEOUT_SECONDS, "installations": INSTALLATIONS,
        "requested_cases": args.case or [case[0] for case in CASES], "status": "running", "cases": [],
    }
    args.output.parent.mkdir(parents=True, exist_ok=True)

    def record() -> None:
        args.output.write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n", encoding="utf-8", newline="\n")

    record()
    with tempfile.TemporaryDirectory(prefix="aster mise multi uninstall ") as raw:
        base = Path(raw)
        for index, (name, command_args, expected_exit) in enumerate(CASES):
            if name not in report["requested_cases"]:
                continue
            root = base / str(index)
            for directory in ["home", "config", "data", "cache", "system", "project"]:
                (root / directory).mkdir(parents=True)
            environment = {
                "PATH": "/usr/bin:/bin", "HOME": str(root / "home"),
                "MISE_CONFIG_DIR": str(root / "config"), "MISE_DATA_DIR": str(root / "data"),
                "MISE_CACHE_DIR": str(root / "cache"), "MISE_SYSTEM_CONFIG_DIR": str(root / "system"),
                "MISE_GLOBAL_CONFIG_FILE": str(root / "config/global.toml"),
                "MISE_EXPERIMENTAL": "1", "MISE_PARANOID": "1", "MISE_YES": "1", "NO_COLOR": "1",
            }
            case = {"name": name, "arguments": command_args, "commands": [], "status": "setup", "hooks": {}}
            report["cases"].append(case)

            def invoke(command: list[str | Path]) -> subprocess.CompletedProcess[str]:
                row = {"argv": [str(value) for value in command], "status": "running"}
                case["commands"].append(row)
                record()
                try:
                    result = subprocess.run(command, cwd=root / "project", env=environment, stdin=subprocess.DEVNULL,
                                            capture_output=True, encoding="utf-8", timeout=TIMEOUT_SECONDS)
                except subprocess.TimeoutExpired as error:
                    row.update(status="timeout", stdout_hex=(error.stdout or b"").hex(), stderr_hex=(error.stderr or b"").hex())
                    case["status"] = report["status"] = "timeout"
                    record()
                    raise
                row.update(status="returned", exit=result.returncode, stdout=result.stdout, stderr=result.stderr)
                record()
                return result

            for tool, versions in INSTALLATIONS.items():
                plugin = root / f"owned {tool} plugin"
                (plugin / "bin").mkdir(parents=True)
                hooks = {
                    "list-all": "#!/bin/sh\nset -eu\nprintf '%s\\n' '1.0.0 2.0.0'\n",
                    "install": "#!/bin/sh\nset -eu\nmkdir -p \"$ASDF_INSTALL_PATH/bin\"\n"
                    + "printf '#!/bin/sh\\nprintf \"%%s\\\\n\" \"%s\"\\n' \"$ASDF_INSTALL_VERSION\" > \"$ASDF_INSTALL_PATH/bin/" + tool + "\"\n"
                    + "chmod 755 \"$ASDF_INSTALL_PATH/bin/" + tool + "\"\n",
                }
                case["hooks"][tool] = {hook: hashlib.sha256(text.encode()).hexdigest() for hook, text in hooks.items()}
                for hook, text in hooks.items():
                    path = plugin / "bin" / hook
                    path.write_text(text, encoding="utf-8", newline="\n")
                    path.chmod(0o755)
                linked = invoke([binary, "plugins", "link", tool, plugin])
                assert linked.returncode == 0, linked.stderr
                installed = invoke([binary, "install", *[f"{tool}@{version}" for version in versions]])
                assert installed.returncode == 0, installed.stderr
                assert all((root / "data/installs" / tool / version / "bin" / tool).is_file() for version in versions)
            removed = invoke([binary, "uninstall", *command_args])
            case.update(exit=removed.returncode, retained={})
            for tool, versions in INSTALLATIONS.items():
                retained = []
                for version in versions:
                    installation = root / "data/installs" / tool / version
                    if installation.is_dir() and not installation.is_symlink():
                        retained.append(version)
                        executed = invoke([installation / "bin" / tool])
                        assert executed.returncode == 0 and executed.stdout == version + "\n", executed
                case["retained"][tool] = retained
            record()
            assert removed.returncode == expected_exit, case
            case["status"] = "passed"
            record()
    report["status"] = "passed"
    record()
    assert json.loads(args.output.read_text(encoding="utf-8")) == report
    print(f"captured {len(report['cases'])} multi-tool uninstall cases in {args.output}")


if __name__ == "__main__":
    main()
