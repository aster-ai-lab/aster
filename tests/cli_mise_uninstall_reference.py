#!/usr/bin/env python3
"""Capture uninstall behavior from the fixed native mise binary and an owned plugin."""
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
VERSIONS = ["1.2", "1.2.3", "1.2.9", "1.20.0"]
TIMEOUT_SECONDS = 25
CASES = [
    ("exact", "1.2", [], 0),
    ("bare_ambiguous", "1", [], 1),
    ("name_ambiguous", None, [], 1),
    # Keep the capture ID; the pinned binary resolves an explicit prefix to one version.
    ("prefix_ambiguous", "prefix:1.2", [], 0),
    ("all_prefix", "prefix:1.2", ["--all"], 0),
    ("all_named", None, ["--all"], 0),
    ("dry_run", "1.2.3", ["--dry-run"], 0),
    ("dry_run_code", "1.2.3", ["--dry-run-code"], 1),
]


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--mise", required=True, type=Path)
    parser.add_argument("--output", type=Path, default=CONTRACTS / "mise-uninstall-reference.json")
    parser.add_argument("--case", choices=[case[0] for case in CASES], action="append")
    args = parser.parse_args()
    if os.name != "posix":
        parser.error("the owned reference plugin requires a native POSIX shell")
    if args.output.exists():
        parser.error("refusing to overwrite an existing reference capture")
    binary = args.mise.resolve()
    reference = json.loads((CONTRACTS / "mise-cli-reference.json").read_text(encoding="utf-8"))
    with binary.open("rb") as source:
        digest = hashlib.file_digest(source, "sha256").hexdigest()
    if digest != reference["binary_sha256"]:
        parser.error("mise does not match the frozen binary digest")
    hooks = {
        "list-all": "#!/bin/sh\nset -eu\nprintf '%s\\n' '" + " ".join(VERSIONS) + "'\n",
        "install": "#!/bin/sh\nset -eu\nmkdir -p \"$ASDF_INSTALL_PATH/bin\"\n"
        + "printf '#!/bin/sh\\nprintf \"%%s\\\\n\" \"%s\"\\n' \"$ASDF_INSTALL_VERSION\" > \"$ASDF_INSTALL_PATH/bin/fixture\"\n"
        + "chmod 755 \"$ASDF_INSTALL_PATH/bin/fixture\"\n",
    }
    result = {"tag": reference["tag"], "release_commit": reference["release_commit"],
              "binary_sha256": digest, "host": {"os": platform.system(), "arch": platform.machine()},
              "timeout_seconds": TIMEOUT_SECONDS, "paranoid": True, "versions": VERSIONS,
              "hook_sha256": {name: hashlib.sha256(text.encode()).hexdigest() for name, text in hooks.items()},
              "status": "running", "cases": []}
    result["requested_cases"] = args.case or [case[0] for case in CASES]
    args.output.parent.mkdir(parents=True, exist_ok=True)

    def record() -> None:
        args.output.write_text(json.dumps(result, ensure_ascii=False, indent=2) + "\n", encoding="utf-8", newline="\n")

    record()
    with tempfile.TemporaryDirectory(prefix="aster mise uninstall ") as raw:
        base = Path(raw)
        plugin = base / "owned plugin"
        (plugin / "bin").mkdir(parents=True)
        for name, text in hooks.items():
            path = plugin / "bin" / name
            path.write_text(text, encoding="utf-8", newline="\n")
            path.chmod(0o755)
        for index, (name, request, flags, expected_exit) in enumerate(CASES):
            if name not in result["requested_cases"]:
                continue
            root = base / str(index)
            for directory in ["home", "config", "data", "cache", "system", "project"]:
                (root / directory).mkdir(parents=True)
            environment = {"PATH": "/usr/bin:/bin", "HOME": str(root / "home"),
                           "MISE_CONFIG_DIR": str(root / "config"), "MISE_DATA_DIR": str(root / "data"),
                           "MISE_CACHE_DIR": str(root / "cache"), "MISE_SYSTEM_CONFIG_DIR": str(root / "system"),
                           "MISE_GLOBAL_CONFIG_FILE": str(root / "config/global.toml"),
                           "MISE_EXPERIMENTAL": "1", "MISE_PARANOID": "1", "MISE_YES": "1", "NO_COLOR": "1"}
            case = {"name": name, "request": request, "flags": flags, "commands": [], "status": "setup"}
            result["cases"].append(case)

            def invoke(command: list[str | Path]) -> subprocess.CompletedProcess[str]:
                row = {"argv": [str(value) for value in command], "status": "running"}
                case["commands"].append(row)
                record()
                try:
                    completed = subprocess.run(command, cwd=root / "project", env=environment,
                                               stdin=subprocess.DEVNULL, capture_output=True,
                                               encoding="utf-8", timeout=TIMEOUT_SECONDS)
                except subprocess.TimeoutExpired as error:
                    row.update(status="timeout", stdout_hex=(error.stdout or b"").hex(), stderr_hex=(error.stderr or b"").hex())
                    case["status"] = "timeout"
                    result["status"] = "timeout"
                    record()
                    raise
                row.update(status="returned", exit=completed.returncode, stdout=completed.stdout, stderr=completed.stderr)
                record()
                return completed

            linked = invoke([binary, "plugins", "link", "fixture", plugin])
            assert linked.returncode == 0, linked.stderr
            installed = invoke([binary, "install", *[f"fixture@{version}" for version in VERSIONS]])
            assert installed.returncode == 0, installed.stderr
            installs = root / "data/installs/fixture"
            assert all((installs / version / "bin/fixture").is_file() for version in VERSIONS)
            tool = "fixture" if request is None else f"fixture@{request}"
            removed = invoke([binary, "uninstall", *flags, tool])
            # mise recreates compatibility symlinks after removal; they are not installed payload directories.
            case.update(exit=removed.returncode, stdout=removed.stdout, stderr=removed.stderr,
                        retained_versions=[version for version in VERSIONS if (installs / version).is_dir() and not (installs / version).is_symlink()],
                        retained_entry_kinds={version: {"is_symlink":(installs / version).is_symlink(),"link_target":str((installs / version).readlink()) if (installs / version).is_symlink() else None} for version in VERSIONS if (installs / version).exists()})
            record()
            assert removed.returncode == expected_exit, case
            for version in case["retained_versions"]:
                executed = invoke([installs / version / "bin/fixture"])
                assert executed.returncode == 0 and executed.stdout == version + "\n", executed
            case["status"] = "passed"
            record()
    result["status"] = "passed"
    record()
    assert json.loads(args.output.read_text(encoding="utf-8")) == result
    print(f"captured {len(result['cases'])} uninstall cases in {args.output}")


if __name__ == "__main__":
    main()
