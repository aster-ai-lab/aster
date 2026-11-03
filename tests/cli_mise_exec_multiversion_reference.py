#!/usr/bin/env python3
"""Capture ordered temporary tool versions from the frozen native mise binary."""
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

CASES = [
    ("managed-forward", "fixture", ["1.0.0", "2.0.0"], "fixture"),
    ("managed-reverse", "fixture", ["2.0.0", "1.0.0"], "fixture"),
    ("managed-fallback", "fixture", ["1.0.0", "2.0.0"], "fixture-extra"),
    ("native-forward", "node", ["path:./one", "path:./two"], "node"),
    ("native-reverse", "node", ["path:./two", "path:./one"], "node"),
    ("native-fallback", "node", ["path:./one", "path:./two"], "node-extra"),
    ("native-duplicate", "node", ["path:./one", "path:./one", "path:./two"], "node"),
    ("system-first", "node", ["system", "path:./two"], "node"),
    ("system-last", "node", ["path:./one", "system"], "node"),
]


def executable(path: Path, text: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(text, encoding="utf-8", newline="\n")
    path.chmod(0o755)


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--mise", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
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
        "tag": frozen["tag"], "release_commit": frozen["release_commit"], "binary_sha256": digest,
        "host": {"os": platform.system(), "arch": platform.machine()}, "timeout_seconds": TIMEOUT_SECONDS,
        "requested_cases": args.case or [case[0] for case in CASES], "status": "running", "cases": [],
    }
    args.output.parent.mkdir(parents=True, exist_ok=True)

    def record() -> None:
        args.output.write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n", encoding="utf-8", newline="\n")

    record()
    with tempfile.TemporaryDirectory(prefix="aster 临时多版本 mise ") as raw:
        for index, (name, tool, requests, command) in enumerate(CASES):
            if name not in report["requested_cases"]:
                continue
            root = Path(raw) / str(index)
            for directory in ["home", "config", "data", "cache", "system", "project", "system-bin"]:
                (root / directory).mkdir(parents=True)
            cwd = root / "project"
            for version in ["one", "two"]:
                executable(cwd / version / "bin/node", f"#!/bin/sh\nprintf '%s\\n' '{version}'\n")
            executable(cwd / "two/bin/node-extra", "#!/bin/sh\nprintf '%s\\n' 'extra:two'\n")
            executable(root / "system-bin/node", "#!/bin/sh\nprintf '%s\\n' 'system'\n")
            environment = {
                "PATH": os.pathsep.join([str(root / "system-bin"), "/usr/bin", "/bin"]), "HOME": str(root / "home"),
                "MISE_CONFIG_DIR": str(root / "config"), "MISE_DATA_DIR": str(root / "data"),
                "MISE_CACHE_DIR": str(root / "cache"), "MISE_SYSTEM_CONFIG_DIR": str(root / "system"),
                "MISE_GLOBAL_CONFIG_FILE": str(root / "config/global.toml"),
                "MISE_EXPERIMENTAL": "1", "MISE_PARANOID": "1", "MISE_YES": "1", "NO_COLOR": "1",
            }
            case = {"name": name, "tool": tool, "requests": requests, "command": command,
                    "commands": [], "status": "setup"}
            report["cases"].append(case)

            def invoke(arguments: list[str | Path]) -> subprocess.CompletedProcess[str]:
                row = {"argv": [str(value) for value in arguments], "status": "running"}
                case["commands"].append(row)
                record()
                try:
                    result = subprocess.run(arguments, cwd=cwd, env=environment, stdin=subprocess.DEVNULL,
                                            capture_output=True, encoding="utf-8", timeout=TIMEOUT_SECONDS)
                except subprocess.TimeoutExpired as error:
                    row.update(status="timeout", stdout_hex=(error.stdout or b"").hex(), stderr_hex=(error.stderr or b"").hex())
                    report["status"] = case["status"] = "timeout"
                    record()
                    raise
                row.update(status="returned", exit=result.returncode, stdout=result.stdout, stderr=result.stderr)
                record()
                assert result.returncode == 0, result.stderr
                return result

            if tool == "fixture":
                plugin = root / "plugin"
                hooks = {
                    "list-all": "#!/bin/sh\nprintf '%s\\n' '1.0.0 2.0.0 3.0.0'\n",
                    "install": "#!/bin/sh\nset -eu\nmkdir -p \"$ASDF_INSTALL_PATH/bin\"\n"
                    + "printf '#!/bin/sh\\nprintf \"%%s\\\\n\" \"%s\"\\n' \"$ASDF_INSTALL_VERSION\" > \"$ASDF_INSTALL_PATH/bin/fixture\"\n"
                    + "chmod 755 \"$ASDF_INSTALL_PATH/bin/fixture\"\n"
                    + "if [ \"$ASDF_INSTALL_VERSION\" = '2.0.0' ]; then\n"
                    + "printf '#!/bin/sh\\nprintf \"%%s\\\\n\" \"extra:%s\"\\n' \"$ASDF_INSTALL_VERSION\" > \"$ASDF_INSTALL_PATH/bin/fixture-extra\"\n"
                    + "chmod 755 \"$ASDF_INSTALL_PATH/bin/fixture-extra\"\nfi\n",
                }
                case["hooks"] = {hook: hashlib.sha256(text.encode()).hexdigest() for hook, text in hooks.items()}
                for hook, text in hooks.items():
                    executable(plugin / "bin" / hook, text)
                invoke([binary, "plugins", "link", tool, plugin])
                invoke([binary, "install", "fixture@1.0.0", "fixture@2.0.0"])
            config = cwd / "mise.toml"
            before = '[tools]\n' + tool + ' = "path:./overridden missing directory"\n[env]\nKEPT = "literal value"\n'
            config.write_text(before, encoding="utf-8", newline="\n")
            invoke([binary, "trust", config])
            arguments = [binary, "exec", *[f"{tool}@{request}" for request in requests], "--", "/bin/sh", "-c",
                         f'{command}; printf "%s\\n" "$KEPT" "$PATH"']
            result = invoke(arguments)
            output, kept, path = result.stdout.splitlines()
            assert kept == "literal value"
            locations = {"one": cwd / "one/bin", "two": cwd / "two/bin", "system": root / "system-bin"}
            if tool == "fixture":
                locations.update({version: root / "data/installs/fixture" / version / "bin" for version in ["1.0.0", "2.0.0"]})
            order = [label for entry in path.split(os.pathsep) for label, directory in locations.items()
                     if Path(entry).resolve() == directory.resolve()]
            assert config.read_text(encoding="utf-8") == before
            case.update(status="passed", execution={"exit": result.returncode, "stdout": output + "\n", "path_order": order},
                        config_unchanged=True)
            record()
    report["status"] = "passed"
    record()
    assert json.loads(args.output.read_text(encoding="utf-8")) == report
    print(f"captured {len(report['cases'])} temporary multi-version cases in {args.output}")


if __name__ == "__main__":
    main()
