#!/usr/bin/env python3
"""Capture nearest-project removal from the pinned native mise executable."""
from __future__ import annotations

import argparse
import hashlib
import json
import os
import platform
import subprocess
import tempfile
from pathlib import Path

from cli_mise_multi_uninstall_reference import CONTRACTS, TIMEOUT_SECONDS

INSTALLATIONS = {"fixture": ["1.0.0", "1.5.0", "2.0.0"], "other": ["9.0.0"]}
CASES = [
    ("nearest_only", {"fixture": "1.0.0"}, ["--yes"], 0),
    ("ordered_versions", {"fixture": ["1.0.0", "2.0.0"]}, ["--yes"], 0),
    ("prefix_selection", {"fixture": "1"}, ["--yes"], 0),
    ("external_requests", {"fixture": "path:../external", "other": "system"}, ["--yes"], 0),
    ("empty_nearest", {}, ["--yes"], 0),
    ("no_project", None, ["--yes"], 1),
    ("confirmation_required", {"fixture": "1.0.0"}, [], 1),
    ("preview_code", {"fixture": ["1.0.0", "2.0.0"]}, ["--dry-run-code"], 1),
]


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--mise", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    if os.name != "posix":
        parser.error("the frozen fixtures require a POSIX shell")
    if args.output.exists():
        parser.error("refusing to overwrite a capture")
    binary = args.mise.resolve()
    frozen = json.loads((CONTRACTS / "mise-cli-reference.json").read_text(encoding="utf-8"))
    digest = hashlib.sha256(binary.read_bytes()).hexdigest()
    if digest != frozen["binary_sha256"]:
        parser.error("mise does not match the frozen binary digest")
    report = {"tag": frozen["tag"], "release_commit": frozen["release_commit"],
              "binary_sha256": digest, "timeout_seconds": TIMEOUT_SECONDS,
              "host": {"os": platform.system(), "architecture": platform.machine()},
              "installations": INSTALLATIONS, "status": "running", "cases": []}
    args.output.parent.mkdir(parents=True, exist_ok=True)

    def save() -> None:
        args.output.write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")

    save()
    with tempfile.TemporaryDirectory(prefix="aster project uninstall 中文 ") as raw:
        for index, (name, requests, flags, expected_exit) in enumerate(CASES):
            root = Path(raw) / str(index)
            cwd = root / "parent/project/nested"
            for path in [cwd, *[root / p for p in ["home", "config", "data", "cache", "system"]]]:
                path.mkdir(parents=True, exist_ok=True)
            env = {"PATH": os.pathsep.join([str(binary.parent), "/usr/bin", "/bin"]),
                   "HOME": str(root / "home"), "MISE_CONFIG_DIR": str(root / "config"),
                   "MISE_DATA_DIR": str(root / "data"), "MISE_CACHE_DIR": str(root / "cache"),
                   "MISE_SYSTEM_CONFIG_DIR": str(root / "system"),
                   "MISE_GLOBAL_CONFIG_FILE": str(root / "config/global.toml"),
                   "MISE_EXPERIMENTAL": "1", "MISE_PARANOID": "1", "NO_COLOR": "1"}
            case = {"name": name, "requests": requests, "flags": flags,
                    "commands": [], "status": "setup", "hooks": {}}
            report["cases"].append(case)

            def invoke(argv: list[str | Path]) -> subprocess.CompletedProcess[str]:
                row = {"argv": [str(value) for value in argv], "cwd": str(cwd), "status": "running"}
                case["commands"].append(row)
                save()
                try:
                    result = subprocess.run(argv, cwd=cwd, env=env, stdin=subprocess.DEVNULL,
                                            capture_output=True, encoding="utf-8", timeout=TIMEOUT_SECONDS)
                except subprocess.TimeoutExpired as error:
                    row.update(status="timeout", stdout_hex=(error.stdout or b"").hex(),
                               stderr_hex=(error.stderr or b"").hex())
                    report["status"] = case["status"] = "timeout"
                    save()
                    raise
                row.update(status="returned", exit=result.returncode, stdout=result.stdout, stderr=result.stderr)
                save()
                return result

            for tool, versions in INSTALLATIONS.items():
                directory = root / ("plugin " + tool) / "bin"
                directory.mkdir(parents=True)
                hooks = {"list-all": "#!/bin/sh\nprintf '%s\\n' '1.0.0 1.5.0 1.9.0 2.0.0 9.0.0'\n",
                         "install": "#!/bin/sh\nset -eu\nmkdir -p \"$ASDF_INSTALL_PATH/bin\"\n"
                         + "printf '#!/bin/sh\\nprintf \"%%s\\\\n\" \"%s\"\\n' \"$ASDF_INSTALL_VERSION\" > \"$ASDF_INSTALL_PATH/bin/"
                         + tool + "\"\nchmod 755 \"$ASDF_INSTALL_PATH/bin/" + tool + "\"\n"}
                case["hooks"][tool] = {key: hashlib.sha256(value.encode()).hexdigest() for key, value in hooks.items()}
                for key, value in hooks.items():
                    path = directory / key
                    path.write_text(value, encoding="utf-8", newline="\n")
                    path.chmod(0o755)
                result = invoke([binary, "plugins", "link", tool, directory.parent])
                assert result.returncode == 0, result.stderr
                result = invoke([binary, "install", *[tool + "@" + version for version in versions]])
                assert result.returncode == 0, result.stderr

            declarations = {root / "config/global.toml": {"other": "9.0.0"}}
            if requests is not None:
                declarations[root / "parent/mise.toml"] = {"fixture": "2.0.0"}
                declarations[root / "parent/project/mise.toml"] = requests
            for path, values in declarations.items():
                path.write_text("[tools]\n" + "".join(key + " = " + json.dumps(value) + "\n"
                                                        for key, value in values.items()), encoding="utf-8")
                result = invoke([binary, "trust", path])
                assert result.returncode == 0, result.stderr
            before = {str(path): path.read_bytes() for path in declarations}
            result = invoke([binary, "uninstall", "--project", *flags])
            case.update(exit=result.returncode, retained={})
            for tool, versions in INSTALLATIONS.items():
                remaining = []
                for version in versions:
                    path = root / "data/installs" / tool / version / "bin" / tool
                    if path.is_file():
                        executed = invoke([path])
                        assert executed.returncode == 0 and executed.stdout == version + "\n", executed
                        remaining.append(version)
                case["retained"][tool] = remaining
            case["declarations_unchanged"] = all(path.read_bytes() == before[str(path)] for path in declarations)
            save()
            assert result.returncode == expected_exit and case["declarations_unchanged"], case
            case["status"] = "passed"
            save()
    report["status"] = "passed"
    save()
    assert json.loads(args.output.read_text(encoding="utf-8")) == report
    print(f"captured {len(report['cases'])} nearest-project uninstall cases")


if __name__ == "__main__":
    main()
