#!/usr/bin/env python3
"""Freeze project removal of an installation whose executable was deleted."""
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


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--mise", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    if os.name != "posix" or args.output.exists():
        parser.error("requires a POSIX host and a new capture path")
    binary = args.mise.resolve()
    frozen = json.loads((CONTRACTS / "mise-cli-reference.json").read_text(encoding="utf-8"))
    digest = hashlib.sha256(binary.read_bytes()).hexdigest()
    if digest != frozen["binary_sha256"]:
        parser.error("mise does not match the frozen binary digest")
    report = {"tag": frozen["tag"], "release_commit": frozen["release_commit"],
              "binary_sha256": digest, "timeout_seconds": TIMEOUT_SECONDS,
              "host": {"os": platform.system(), "architecture": platform.machine()},
              "status": "running", "commands": []}
    args.output.parent.mkdir(parents=True, exist_ok=True)

    def save() -> None:
        args.output.write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")

    save()
    with tempfile.TemporaryDirectory(prefix="aster damaged project 中文 ") as raw:
        root = Path(raw)
        for name in ["home", "config", "data", "cache", "system", "project", "plugin/bin"]:
            (root / name).mkdir(parents=True)
        env = {"PATH": os.pathsep.join([str(binary.parent), "/usr/bin", "/bin"]),
               "HOME": str(root / "home"), "MISE_CONFIG_DIR": str(root / "config"),
               "MISE_DATA_DIR": str(root / "data"), "MISE_CACHE_DIR": str(root / "cache"),
               "MISE_SYSTEM_CONFIG_DIR": str(root / "system"),
               "MISE_GLOBAL_CONFIG_FILE": str(root / "config/global.toml"),
               "MISE_EXPERIMENTAL": "1", "MISE_PARANOID": "1", "NO_COLOR": "1"}

        def run(argv: list[str | Path]) -> subprocess.CompletedProcess[str]:
            row = {"argv": [str(value) for value in argv], "status": "running"}
            report["commands"].append(row)
            save()
            result = subprocess.run(argv, cwd=root / "project", env=env, stdin=subprocess.DEVNULL,
                                    capture_output=True, encoding="utf-8", timeout=TIMEOUT_SECONDS)
            row.update(status="returned", exit=result.returncode, stdout=result.stdout, stderr=result.stderr)
            save()
            return result

        hooks = {"list-all": "#!/bin/sh\nprintf '%s\\n' '1.0.0 2.0.0'\n",
                 "install": "#!/bin/sh\nset -eu\nmkdir -p \"$ASDF_INSTALL_PATH/bin\"\n"
                 + "printf '#!/bin/sh\\nprintf \"%%s\\\\n\" \"%s\"\\n' \"$ASDF_INSTALL_VERSION\" > \"$ASDF_INSTALL_PATH/bin/fixture\"\n"
                 + "chmod 755 \"$ASDF_INSTALL_PATH/bin/fixture\"\n"}
        report["hook_sha256"] = {key: hashlib.sha256(value.encode()).hexdigest() for key, value in hooks.items()}
        for name, text in hooks.items():
            path = root / "plugin/bin" / name
            path.write_text(text, encoding="utf-8", newline="\n")
            path.chmod(0o755)
        assert run([binary, "plugins", "link", "fixture", root / "plugin"]).returncode == 0
        assert run([binary, "install", "fixture@1.0.0", "fixture@2.0.0"]).returncode == 0
        config = root / "project/mise.toml"
        config.write_text('[tools]\nfixture = "1.0.0"\n', encoding="utf-8")
        assert run([binary, "trust", config]).returncode == 0
        before = config.read_bytes()
        selected = root / "data/installs/fixture/1.0.0"
        (selected / "bin/fixture").unlink()
        report["before"] = {"selected_directory_exists": selected.is_dir(),
                            "selected_executable_exists": (selected / "bin/fixture").exists()}
        result = run([binary, "uninstall", "--project", "--yes"])
        report["exit"] = result.returncode
        report["retained_versions"] = [version for version in ["1.0.0", "2.0.0"]
                                       if (root / "data/installs/fixture" / version).exists()]
        retained = run([root / "data/installs/fixture/2.0.0/bin/fixture"])
        report["retained_stdout"] = retained.stdout
        report["declaration_unchanged"] = config.read_bytes() == before
        save()
        assert result.returncode == 0 and not selected.exists()
        assert retained.returncode == 0 and retained.stdout == "2.0.0\n"
        assert report["declaration_unchanged"]
    report["status"] = "passed"
    save()
    print("captured damaged project uninstall")


if __name__ == "__main__":
    main()
