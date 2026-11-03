"""Capture version selection by the frozen mise binary using an owned local plugin.

Run on the native platform represented by contracts/mise-cli-reference.json.
The linked plugin performs no downloads and echoes ASDF_INSTALL_VERSION; mise
does all selection. Each case gets fresh state, with paranoid mode enabled.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import subprocess
import tempfile


ROOT = Path(__file__).resolve().parents[1]
CONTRACTS = ROOT / "specs/031-cli-mise-replacement/contracts"
VERSIONS = [
    ("0.8.0", False), ("0.8.9", False), ("0.81.0", False),
    ("1.2", False), ("1.2.3", False), ("1.2.9", False), ("1.20.0", False),
    ("2.78.9", False), ("2.79.0", False), ("3.2.1", False), ("4.0.0-rc.1", True),
]
REQUESTS = [
    ("latest", True), ("1.2", True), ("prefix:1.2", True), ("0.8", True),
    ("prefix:0.8", True), ("v1.2", True), ("sub-1:latest", True),
    ("sub-0.0.1:2.79.0", True), ("4.0.0-rc.1", True), ("prefix:4", False),
]
TIMEOUT_SECONDS = 25


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--mise", type=Path, required=True)
    parser.add_argument("--output", type=Path, default=CONTRACTS / "mise-version-reference.json")
    args = parser.parse_args()
    if os.name != "posix":
        parser.error("this reference fixture requires a native POSIX shell")
    reference = json.loads((CONTRACTS / "mise-cli-reference.json").read_text(encoding="utf-8"))
    binary = args.mise.resolve()
    with binary.open("rb") as binary_file:
        actual_digest = hashlib.file_digest(binary_file, "sha256").hexdigest()
    if actual_digest != reference["binary_sha256"]:
        parser.error("mise binary does not match the frozen reference digest")
    results = []
    with tempfile.TemporaryDirectory(prefix="aster-mise-version-reference-") as temporary:
        base = Path(temporary)
        source = base / "owned plugin"
        (source / "bin").mkdir(parents=True)
        hooks = {
            "list-all": "#!/bin/sh\nset -eu\nprintf 'queried\\n' > \"$VERSION_ORACLE_MARKER\"\n"
            + "cat <<'VERSIONS'\n" + " ".join(version for version, _ in VERSIONS) + "\nVERSIONS\n",
            "install": "#!/bin/sh\nset -eu\nmkdir -p \"$ASDF_INSTALL_PATH/bin\"\n"
            + "printf '#!/bin/sh\\nprintf \"%%s\\\\n\" \"%s\"\\n' \"$ASDF_INSTALL_VERSION\" > \"$ASDF_INSTALL_PATH/bin/fixture\"\n"
            + "chmod 755 \"$ASDF_INSTALL_PATH/bin/fixture\"\n",
        }
        for name, text in hooks.items():
            path = source / "bin" / name
            path.write_text(text, encoding="utf-8", newline="\n")
            path.chmod(0o755)
        for index, (request, should_succeed) in enumerate(REQUESTS):
            case = base / f"case-{index}"
            for name in ["home", "config", "data", "cache", "system", "project"]:
                (case / name).mkdir(parents=True)
            marker = case / "versions-queried"
            environment = {
                "PATH": os.pathsep.join([str(Path("/usr/bin")), str(Path("/bin"))]),
                "HOME": str(case / "home"), "MISE_CONFIG_DIR": str(case / "config"),
                "MISE_DATA_DIR": str(case / "data"), "MISE_CACHE_DIR": str(case / "cache"),
                "MISE_SYSTEM_CONFIG_DIR": str(case / "system"),
                "MISE_GLOBAL_CONFIG_FILE": str(case / "config" / "global.toml"),
                "MISE_EXPERIMENTAL": "1", "MISE_PARANOID": "1", "NO_COLOR": "1",
                "VERSION_ORACLE_MARKER": str(marker),
            }
            common = dict(env=environment, cwd=case / "project", stdin=subprocess.DEVNULL,
                          capture_output=True, encoding="utf-8", timeout=TIMEOUT_SECONDS)
            linked = subprocess.run([binary, "plugins", "link", "fixture", source], **common)
            if linked.returncode != 0:
                raise RuntimeError(f"reference plugin link failed: {linked.stderr}")
            command = ["exec", f"fixture@{request}", "--", "fixture"]
            completed = subprocess.run([binary, *command], **common)
            if not marker.is_file():
                raise RuntimeError(f"reference did not query the fixture for {request}: {completed.stderr}")
            if (completed.returncode == 0) != should_succeed:
                raise RuntimeError(f"unexpected reference outcome for {request}: {completed.stderr}")
            if should_succeed and completed.stdout.strip() not in {version for version, _ in VERSIONS}:
                raise RuntimeError(f"reference executable did not report an input version: {completed.stdout!r}")
            results.append({"request": request, "command": ["mise", *command],
                            "exit": completed.returncode, "stdout": completed.stdout,
                            "stderr": completed.stderr})
    artifact = {
        "tag": reference["tag"], "release_commit": reference["release_commit"],
        "binary_sha256": actual_digest, "host": {"os": platform.system(), "arch": platform.machine()},
        "fixture": "owned local plugin linked with the reference CLI; fresh state for each case",
        "timeout_seconds": TIMEOUT_SECONDS, "paranoid": True,
        "hook_sha256": {name: hashlib.sha256(text.encode()).hexdigest() for name, text in hooks.items()},
        "versions": [{"version": version, "prerelease": prerelease} for version, prerelease in VERSIONS],
        "cases": results,
    }
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(artifact, ensure_ascii=False, indent=2) + "\n", encoding="utf-8", newline="\n")
    print(f"captured {len(results)} reference executions in {args.output}")


if __name__ == "__main__":
    main()
