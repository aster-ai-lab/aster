#!/usr/bin/env python3
"""Capture multi-version declarations and executable selection from native mise."""
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

from cli_mise_batch_install_reference import CONTRACTS, TIMEOUT_SECONDS

CASES = [
    ("use_forward", ["use", "--pin", "fixture@1.0.0", "fixture@2.0.0"], None, "fixture"),
    ("use_reverse", ["use", "--pin", "fixture@2.0.0", "fixture@1.0.0"], None, "fixture"),
    ("install_configured", ["install", "fixture"], ["1.0.0", "2.0.0"], "fixture"),
    ("use_duplicate", ["use", "--pin", "fixture@1.0.0", "fixture@1.0.0"], None, "fixture"),
    ("executable_fallback", ["use", "--pin", "fixture@1.0.0", "fixture@2.0.0"], None, "fixture-extra"),
]


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--mise", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--case", choices=[case[0] for case in CASES], action="append")
    args = parser.parse_args()
    if os.name != "posix":
        parser.error("the reference fixtures require a POSIX shell")
    if args.output.exists():
        parser.error("refusing to overwrite a capture")
    binary = args.mise.resolve()
    frozen = json.loads((CONTRACTS / "mise-cli-reference.json").read_text(encoding="utf-8"))
    digest = hashlib.sha256(binary.read_bytes()).hexdigest()
    if digest != frozen["binary_sha256"]:
        parser.error("mise does not match the frozen binary digest")
    report = {
        "tag": frozen["tag"], "release_commit": frozen["release_commit"], "binary_sha256": digest,
        "host": {"os": platform.system(), "arch": platform.machine()}, "timeout_seconds": TIMEOUT_SECONDS,
        "requested_cases": args.case or [case[0] for case in CASES], "status": "running", "cases": [],
    }
    args.output.parent.mkdir(parents=True, exist_ok=True)

    def record() -> None:
        args.output.write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n", encoding="utf-8", newline="\n")

    record()
    with tempfile.TemporaryDirectory(prefix="aster 多版本 mise ") as raw:
        for index, (name, arguments, configured, executable) in enumerate(CASES):
            if name not in report["requested_cases"]:
                continue
            root = Path(raw) / str(index)
            for directory in ["home", "config", "data", "cache", "system", "project", "plugin/bin"]:
                (root / directory).mkdir(parents=True)
            environment = {
                "PATH": "/usr/bin:/bin", "HOME": str(root / "home"),
                "MISE_CONFIG_DIR": str(root / "config"), "MISE_DATA_DIR": str(root / "data"),
                "MISE_CACHE_DIR": str(root / "cache"), "MISE_SYSTEM_CONFIG_DIR": str(root / "system"),
                "MISE_GLOBAL_CONFIG_FILE": str(root / "config/global.toml"),
                "MISE_EXPERIMENTAL": "1", "MISE_PARANOID": "1", "MISE_YES": "1", "NO_COLOR": "1",
            }
            case = {"name": name, "arguments": arguments, "configured": configured, "commands": [], "status": "setup"}
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
                    report["status"] = case["status"] = "timeout"
                    record()
                    raise
                row.update(status="returned", exit=result.returncode, stdout=result.stdout, stderr=result.stderr)
                record()
                return result

            hooks = {
                "list-all": "#!/bin/sh\nset -eu\nprintf '%s\\n' '1.0.0 2.0.0 3.0.0'\n",
                "install": "#!/bin/sh\nset -eu\nmkdir -p \"$ASDF_INSTALL_PATH/bin\"\n"
                + "printf '#!/bin/sh\\nprintf \"%%s\\\\n\" \"%s\"\\n' \"$ASDF_INSTALL_VERSION\" > \"$ASDF_INSTALL_PATH/bin/fixture\"\n"
                + "chmod 755 \"$ASDF_INSTALL_PATH/bin/fixture\"\n"
                + "if [ \"$ASDF_INSTALL_VERSION\" = '2.0.0' ]; then\n"
                + "printf '#!/bin/sh\\nprintf \"%%s\\\\n\" \"extra:%s\"\\n' \"$ASDF_INSTALL_VERSION\" > \"$ASDF_INSTALL_PATH/bin/fixture-extra\"\n"
                + "chmod 755 \"$ASDF_INSTALL_PATH/bin/fixture-extra\"\nfi\n",
            }
            case["hooks"] = {hook: hashlib.sha256(body.encode()).hexdigest() for hook, body in hooks.items()}
            for hook, body in hooks.items():
                path = root / "plugin/bin" / hook
                path.write_text(body, encoding="utf-8", newline="\n")
                path.chmod(0o755)
            linked = invoke([binary, "plugins", "link", "fixture", root / "plugin"])
            assert linked.returncode == 0, linked.stderr
            config = root / "project/mise.toml"
            before = '[env]\nKEPT = "literal value"\n'
            if configured is not None:
                before += "[tools]\nfixture = " + json.dumps(configured) + "\n"
            config.write_text(before, encoding="utf-8", newline="\n")
            trusted = invoke([binary, "trust", config])
            assert trusted.returncode == 0, trusted.stderr
            installed = invoke([binary, *arguments])
            case.update(exit=installed.returncode, config_unchanged=config.read_text(encoding="utf-8") == before,
                        pins=tomllib.loads(config.read_text(encoding="utf-8")).get("tools", {}))
            assert installed.returncode == 0, installed.stderr
            if arguments[0] == "install":
                assert case["config_unchanged"], case
            assert tomllib.loads(config.read_text(encoding="utf-8"))["env"] == {"KEPT": "literal value"}
            installations = root / "data/installs/fixture"
            case["installed"] = sorted(p.name for p in installations.iterdir() if p.is_dir() and not p.is_symlink())
            which = invoke([binary, "which", executable])
            assert which.returncode == 0, which.stderr
            selected = Path(which.stdout.strip()).resolve().relative_to(installations.resolve())
            assert selected.parts[0] in case["installed"], selected
            result = invoke([binary, "exec", "--", executable])
            assert result.returncode == 0, result.stderr
            case["execution"] = {"command": executable, "selected_relative_path": str(selected),
                                 "exit": result.returncode, "stdout": result.stdout}
            case["status"] = "passed"
            record()
    report["status"] = "passed"
    record()
    assert json.loads(args.output.read_text(encoding="utf-8")) == report
    print(f"captured {len(report['cases'])} multi-version cases in {args.output}")


if __name__ == "__main__":
    main()
