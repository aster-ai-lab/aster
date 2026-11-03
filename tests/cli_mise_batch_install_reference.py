#!/usr/bin/env python3
"""Capture batch install/use behavior from the frozen native mise binary."""
from __future__ import annotations

import argparse
import hashlib
import json
import os
import platform
import subprocess
import tempfile
import tomllib
from pathlib import Path

CONTRACTS = Path(__file__).resolve().parents[1] / "specs/031-cli-mise-replacement/contracts"
TIMEOUT_SECONDS = 25
TOOLS = ["fixture", "other"]
CASES = [
    ("install_explicit", ["install", "fixture@1.0.0", "other@1.0.0"], 0),
    ("install_configured", ["install", "fixture", "other"], 0),
    ("install_versions", ["install", "fixture@1.0.0", "fixture@3.0.0"], 0),
    ("install_duplicate", ["install", "fixture@1.0.0", "fixture@1.0.0"], 0),
    ("use_explicit", ["use", "--pin", "fixture@1.0.0", "other@1.0.0"], 0),
    ("use_installed", ["use", "--pin", "fixture", "other"], 0),
    ("use_late_failure", ["use", "--pin", "fixture@1.0.0", "broken@1.0.0"], 1),
]


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--mise", required=True, type=Path)
    parser.add_argument("--output", required=True, type=Path)
    parser.add_argument("--case", choices=[case[0] for case in CASES], action="append")
    args = parser.parse_args()
    if os.name != "posix":
        parser.error("the native fixtures require a POSIX shell")
    if args.output.exists():
        parser.error("refusing to overwrite a capture")
    binary = args.mise.resolve()
    frozen = json.loads((CONTRACTS / "mise-cli-reference.json").read_text(encoding="utf-8"))
    digest = hashlib.sha256(binary.read_bytes()).hexdigest()
    if digest != frozen["binary_sha256"]:
        parser.error("mise does not match the frozen binary digest")
    report = {
        "tag": frozen["tag"], "release_commit": frozen["release_commit"],
        "binary_sha256": digest, "host": {"os": platform.system(), "arch": platform.machine()},
        "timeout_seconds": TIMEOUT_SECONDS, "initial_versions": ["2.0.0"],
        "available_versions": ["1.0.0", "2.0.0", "3.0.0"],
        "requested_cases": args.case or [case[0] for case in CASES], "status": "running", "cases": [],
    }
    args.output.parent.mkdir(parents=True, exist_ok=True)

    def record() -> None:
        args.output.write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n", encoding="utf-8", newline="\n")

    record()
    with tempfile.TemporaryDirectory(prefix="aster mise batch 安装 ") as raw:
        for index, (name, arguments, expected_exit) in enumerate(CASES):
            if name not in report["requested_cases"]:
                continue
            root = Path(raw) / str(index)
            for directory in ["home", "config", "data", "cache", "system", "project"]:
                (root / directory).mkdir(parents=True)
            environment = {
                "PATH": "/usr/bin:/bin", "HOME": str(root / "home"),
                "MISE_CONFIG_DIR": str(root / "config"), "MISE_DATA_DIR": str(root / "data"),
                "MISE_CACHE_DIR": str(root / "cache"), "MISE_SYSTEM_CONFIG_DIR": str(root / "system"),
                "MISE_GLOBAL_CONFIG_FILE": str(root / "config/global.toml"),
                "MISE_EXPERIMENTAL": "1", "MISE_PARANOID": "1", "MISE_YES": "1", "NO_COLOR": "1",
            }
            case = {"name": name, "arguments": arguments, "commands": [], "hooks": {}, "status": "setup"}
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

            for tool in [*TOOLS, "broken"]:
                plugin = root / f"owned {tool} plugin"
                (plugin / "bin").mkdir(parents=True)
                install = "#!/bin/sh\nset -eu\nexit 23\n" if tool == "broken" else (
                    "#!/bin/sh\nset -eu\nmkdir -p \"$ASDF_INSTALL_PATH/bin\"\n"
                    + "printf '#!/bin/sh\\nprintf \"%%s\\\\n\" \"%s\"\\n' \"$ASDF_INSTALL_VERSION\" > \"$ASDF_INSTALL_PATH/bin/" + tool + "\"\n"
                    + "chmod 755 \"$ASDF_INSTALL_PATH/bin/" + tool + "\"\n"
                )
                hooks = {"list-all": "#!/bin/sh\nset -eu\nprintf '%s\\n' '1.0.0 2.0.0 3.0.0'\n", "install": install}
                case["hooks"][tool] = {hook: hashlib.sha256(text.encode()).hexdigest() for hook, text in hooks.items()}
                for hook, text in hooks.items():
                    path = plugin / "bin" / hook
                    path.write_text(text, encoding="utf-8", newline="\n")
                    path.chmod(0o755)
                linked = invoke([binary, "plugins", "link", tool, plugin])
                assert linked.returncode == 0, linked.stderr
                if tool != "broken":
                    installed = invoke([binary, "install", f"{tool}@2.0.0"])
                    assert installed.returncode == 0, installed.stderr
            config = root / "project/mise.toml"
            config.write_text('[tools]\nfixture = "1.0.0"\nother = "1.0.0"\n[env]\nKEPT = "literal value"\n', encoding="utf-8", newline="\n")
            before = config.read_bytes()
            trusted = invoke([binary, "trust", config])
            assert trusted.returncode == 0, trusted.stderr
            result = invoke([binary, *arguments])
            case.update(exit=result.returncode, config_unchanged=config.read_bytes() == before,
                        pins=tomllib.loads(config.read_text(encoding="utf-8"))["tools"], installed={}, executions={})
            for tool in TOOLS:
                installations = root / "data/installs" / tool
                versions = sorted(p.name for p in installations.iterdir() if p.is_dir() and not p.is_symlink())
                case["installed"][tool] = versions
                case["executions"][tool] = {}
                for version in versions:
                    execution = invoke([installations / version / "bin" / tool])
                    assert execution.returncode == 0 and execution.stdout == version + "\n", execution
                    case["executions"][tool][version] = execution.stdout
            record()
            assert result.returncode == expected_exit, case
            if arguments[0] == "install" or result.returncode != 0:
                assert case["config_unchanged"], case
            case["status"] = "passed"
            record()
    report["status"] = "passed"
    record()
    assert json.loads(args.output.read_text(encoding="utf-8")) == report
    print(f"captured {len(report['cases'])} batch install/use cases in {args.output}")


if __name__ == "__main__":
    main()
