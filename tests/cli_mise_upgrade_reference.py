#!/usr/bin/env python3
"""Capture upgrade behavior from the unchanged pinned native mise binary."""

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

from cli_mise_multi_uninstall_reference import CONTRACTS, TIMEOUT_SECONDS

VERSIONS = ["1.0.0", "1.5.0", "2.0.0"]
CASES = [
    ("exact_stays_pinned", "1.0.0", []),
    ("prefix_stays_in_range", "1", []),
    ("latest_advances", "latest", []),
    ("bump_preserves_precision", "1", ["--bump"]),
    ("bump_pin_writes_resolved", "1", ["--bump", "--pin"]),
    ("bump_named_latest", "1", ["fixture@latest", "--bump"]),
    ("bump_named_prefix", "1", ["fixture@prefix:1", "--bump"]),
    ("keep_previous", "latest", ["--no-prune"]),
    ("prune_previous_now", "latest", ["--prune"]),
    ("preview", "latest", ["--dry-run"]),
    ("preview_code", "latest", ["--dry-run-code"]),
    ("exclude", "latest", ["--exclude", "fixture"]),
]


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--mise", required=True, type=Path)
    parser.add_argument("--output", required=True, type=Path)
    args = parser.parse_args()
    if os.name != "posix":
        parser.error("these native reference fixtures need a POSIX shell")
    if args.output.exists():
        parser.error("refusing to overwrite a capture")
    binary = args.mise.resolve(strict=True)
    frozen = json.loads((CONTRACTS / "mise-cli-reference.json").read_text(encoding="utf-8"))
    digest = hashlib.sha256(binary.read_bytes()).hexdigest()
    if digest != frozen["binary_sha256"]:
        parser.error("mise does not match the frozen binary digest")
    report = {
        "tag": frozen["tag"], "release_commit": frozen["release_commit"],
        "binary_sha256": digest, "timeout_seconds": TIMEOUT_SECONDS,
        "host": {"os": platform.system(), "architecture": platform.machine()},
        "available_versions": VERSIONS, "initial_versions": ["1.0.0"],
        "status": "running", "cases": [],
    }
    args.output.parent.mkdir(parents=True, exist_ok=True)

    def save() -> None:
        args.output.write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n", encoding="utf-8", newline="\n")

    save()
    with tempfile.TemporaryDirectory(prefix="aster upgrade 中文 ") as raw:
        for index, (name, request, flags) in enumerate(CASES):
            root = Path(raw) / str(index)
            for part in ["project", "home", "config", "data", "cache", "state", "system"]:
                (root / part).mkdir(parents=True)
            environment = {
                "PATH": "/usr/bin:/bin", "HOME": str(root / "home"),
                "MISE_CONFIG_DIR": str(root / "config"), "MISE_DATA_DIR": str(root / "data"),
                "MISE_CACHE_DIR": str(root / "cache"), "MISE_STATE_DIR": str(root / "state"),
                "MISE_SYSTEM_CONFIG_DIR": str(root / "system"),
                "MISE_GLOBAL_CONFIG_FILE": str(root / "config/global.toml"),
                "MISE_EXPERIMENTAL": "1", "MISE_PARANOID": "1", "MISE_YES": "1", "NO_COLOR": "1",
            }
            case = {"name": name, "request": request, "flags": flags, "commands": [], "status": "setup"}
            report["cases"].append(case)

            def invoke(command: list[str | Path]) -> subprocess.CompletedProcess[str]:
                row = {"argv": [str(value) for value in command], "status": "running"}
                case["commands"].append(row)
                save()
                try:
                    result = subprocess.run(command, cwd=root / "project", env=environment,
                                            stdin=subprocess.DEVNULL, capture_output=True, encoding="utf-8", timeout=TIMEOUT_SECONDS)
                except subprocess.TimeoutExpired as error:
                    row.update(status="timeout", stdout_hex=(error.stdout or b"").hex(), stderr_hex=(error.stderr or b"").hex())
                    case["status"] = report["status"] = "timeout"
                    save()
                    raise
                row.update(status="returned", exit=result.returncode, stdout=result.stdout, stderr=result.stderr)
                save()
                return result

            plugin = root / "owned plugin/bin"
            plugin.mkdir(parents=True)
            hooks = {
                "list-all": "#!/bin/sh\nprintf '%s\\n' '1.0.0 1.5.0 2.0.0'\n",
                "install": "#!/bin/sh\nset -eu\nmkdir -p \"$ASDF_INSTALL_PATH/bin\"\n"
                "printf '#!/bin/sh\\nprintf \"%%s\\\\n\" \"%s\"\\n' \"$ASDF_INSTALL_VERSION\" > \"$ASDF_INSTALL_PATH/bin/fixture\"\n"
                "chmod 755 \"$ASDF_INSTALL_PATH/bin/fixture\"\n",
            }
            case["hooks"] = {name: hashlib.sha256(value.encode()).hexdigest() for name, value in hooks.items()}
            for hook, value in hooks.items():
                path = plugin / hook
                path.write_text(value, encoding="utf-8", newline="\n")
                path.chmod(0o755)
            for command in [[binary, "plugins", "link", "fixture", plugin.parent], [binary, "install", "fixture@1.0.0"]]:
                result = invoke(command)
                assert result.returncode == 0, result.stderr
            config = root / "project/mise.toml"
            config.write_text('[tools]\nfixture = ' + json.dumps(request) + '\n', encoding="utf-8", newline="\n")
            result = invoke([binary, "trust", config])
            assert result.returncode == 0, result.stderr
            before = config.read_bytes()
            result = invoke([binary, "upgrade", *flags])
            case.update(exit=result.returncode, config_before=before.decode(), config_after=config.read_text(encoding="utf-8"),
                        config_unchanged=before == config.read_bytes(), pin=tomllib.loads(config.read_text(encoding="utf-8"))["tools"]["fixture"])
            assert result.returncode == (1 if name == "preview_code" else 0), result.stderr
            retained = []
            for version in VERSIONS:
                path = root / "data/installs/fixture" / version / "bin/fixture"
                if path.is_file():
                    executed = invoke([path])
                    assert executed.returncode == 0 and executed.stdout.strip() == version
                    retained.append(version)
            case["retained"] = retained
            case["status"] = "passed"
            save()
    report["status"] = "passed"
    save()


if __name__ == "__main__":
    main()
