#!/usr/bin/env python3
"""Capture unused-version cleanup from the pinned native mise executable."""
from __future__ import annotations

import argparse
import hashlib
import json
import os
import platform
import shutil
import subprocess
import tempfile
from pathlib import Path

from cli_mise_multi_uninstall_reference import CONTRACTS, TIMEOUT_SECONDS

VERSIONS = ["1.0.0", "1.5.0", "2.0.0"]
# name, declarations, operation, flags, exit, retained versions
CASES = [
    ("unreferenced_current_is_unused", [], "none", ["--yes"], 0, []),
    ("global_exact", [("global", "1.0.0")], "none", ["--yes"], 0, ["1.0.0"]),
    ("two_tracked_projects", [("a", "1.0.0"), ("b", "2.0.0")], "none", ["--yes"], 0, ["1.0.0", "2.0.0"]),
    ("prefix_keeps_selected_version", [("a", "1")], "none", ["--yes"], 0, ["1.5.0"]),
    ("ordered_versions", [("a", ["2.0.0", "1.0.0"])], "none", ["--yes"], 0, ["1.0.0", "2.0.0"]),
    ("changed_tracked_declaration", [("a", "1.0.0")], "change", ["--yes"], 0, ["2.0.0"]),
    ("deleted_tracked_declaration", [("a", "1.0.0")], "delete", ["--yes"], 0, []),
    ("preview_preserves_payloads", [("a", "1.0.0")], "none", ["--dry-run"], 0, VERSIONS),
    ("preview_code_preserves_payloads", [("a", "1.0.0")], "none", ["--dry-run-code"], 1, VERSIONS),
    ("confirmation_is_required", [], "none", [], 1, VERSIONS),
    ("configs_only_preserves_payloads", [("a", "1.0.0")], "delete", ["--configs", "--yes"], 0, VERSIONS),
    ("running_version_is_retained", [], "running", ["--yes"], 0, ["1.0.0"]),
]


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--mise", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    if os.name != "posix":
        parser.error("the frozen fixture requires a POSIX shell")
    if args.output.exists():
        parser.error("refusing to overwrite a capture")
    binary = args.mise.resolve()
    frozen = json.loads((CONTRACTS / "mise-cli-reference.json").read_text(encoding="utf-8"))
    digest = hashlib.sha256(binary.read_bytes()).hexdigest()
    if digest != frozen["binary_sha256"]:
        parser.error("mise does not match the frozen binary digest")
    sleep = next((path for path in [Path("/usr/bin/sleep"), Path("/bin/sleep")] if path.is_file()), None)
    if sleep is None:
        parser.error("the running-version fixture needs the native sleep executable")
    report = {
        "tag": frozen["tag"], "release_commit": frozen["release_commit"],
        "binary_sha256": digest, "timeout_seconds": TIMEOUT_SECONDS,
        "host": {"os": platform.system(), "architecture": platform.machine()},
        "installations": {"fixture": VERSIONS}, "status": "running", "cases": [],
    }
    args.output.parent.mkdir(parents=True, exist_ok=True)

    def save() -> None:
        args.output.write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")

    save()
    with tempfile.TemporaryDirectory(prefix="aster prune 中文 ") as raw:
        for index, (name, declarations, operation, flags, expected_exit, expected_versions) in enumerate(CASES):
            root = Path(raw) / str(index)
            work = root / "work"
            for part in ["work", "home", "config", "data", "cache", "state", "system"]:
                (root / part).mkdir(parents=True)
            env = {
                "PATH": os.pathsep.join([str(binary.parent), "/usr/bin", "/bin"]),
                "HOME": str(root / "home"), "MISE_CONFIG_DIR": str(root / "config"),
                "MISE_DATA_DIR": str(root / "data"), "MISE_CACHE_DIR": str(root / "cache"),
                "MISE_STATE_DIR": str(root / "state"), "XDG_STATE_HOME": str(root / "state"),
                "MISE_SYSTEM_CONFIG_DIR": str(root / "system"),
                "MISE_GLOBAL_CONFIG_FILE": str(root / "config/global.toml"),
                "MISE_EXPERIMENTAL": "1", "MISE_PARANOID": "1", "NO_COLOR": "1",
            }
            case = {
                "name": name, "declarations": declarations, "operation": operation,
                "flags": flags, "expected_exit": expected_exit,
                "expected_retained": expected_versions, "commands": [], "status": "running",
            }
            report["cases"].append(case)

            def invoke(argv: list[str | Path], cwd: Path = work) -> subprocess.CompletedProcess[str]:
                command = {"argv": [str(value) for value in argv], "cwd": str(cwd), "status": "running"}
                case["commands"].append(command)
                save()
                try:
                    result = subprocess.run(argv, cwd=cwd, env=env, stdin=subprocess.DEVNULL,
                                            capture_output=True, encoding="utf-8", timeout=TIMEOUT_SECONDS)
                except subprocess.TimeoutExpired as error:
                    command.update(status="timeout", stdout_hex=(error.stdout or b"").hex(),
                                   stderr_hex=(error.stderr or b"").hex())
                    report["status"] = case["status"] = "timeout"
                    save()
                    raise
                command.update(status="returned", exit=result.returncode, stdout=result.stdout, stderr=result.stderr)
                save()
                return result

            plugin = root / "plugin fixture/bin"
            plugin.mkdir(parents=True)
            hooks = {
                "list-all": "#!/bin/sh\nprintf '%s\\n' '1.0.0 1.5.0 2.0.0'\n",
                "install": "#!/bin/sh\nset -eu\nmkdir -p \"$ASDF_INSTALL_PATH/bin\"\n"
                "printf '#!/bin/sh\\nprintf \"%%s\\\\n\" \"%s\"\\n' \"$ASDF_INSTALL_VERSION\" > \"$ASDF_INSTALL_PATH/bin/fixture\"\n"
                "chmod 755 \"$ASDF_INSTALL_PATH/bin/fixture\"\n"
                "cp \"$ASDF_PLUGIN_PATH/native-sleep\" \"$ASDF_INSTALL_PATH/bin/sleep\"\n",
            }
            native_sleep = plugin.parent / "native-sleep"
            shutil.copyfile(sleep, native_sleep)
            native_sleep.chmod(0o755)
            if platform.system() == "Darwin":
                result = invoke(["/usr/bin/codesign", "--force", "--sign", "-", native_sleep])
                assert result.returncode == 0, result.stderr
            case["hooks"] = {key: hashlib.sha256(value.encode()).hexdigest() for key, value in hooks.items()}
            case["native_sleep_sha256"] = hashlib.sha256(sleep.read_bytes()).hexdigest()
            case["fixture_sleep_sha256"] = hashlib.sha256(native_sleep.read_bytes()).hexdigest()
            for key, value in hooks.items():
                path = plugin / key
                path.write_bytes(value.encode("utf-8"))
                path.chmod(0o755)
            for argv in [[binary, "plugins", "link", "fixture", plugin.parent],
                         [binary, "install", *["fixture@" + version for version in VERSIONS]]]:
                result = invoke(argv)
                assert result.returncode == 0, result.stderr

            configs = []
            for location, request in declarations:
                path = (root / "config/global.toml" if location == "global"
                        else root / ("project " + location + " 中文") / "mise.toml")
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_text("[tools]\nfixture = " + json.dumps(request) + "\n", encoding="utf-8")
                configs.append(path)
                result = invoke([binary, "trust", path], path.parent)
                assert result.returncode == 0, result.stderr
                result = invoke([binary, "exec", "--", "fixture"], path.parent)
                assert result.returncode == 0, result.stderr
            if operation == "change":
                configs[0].write_text('[tools]\nfixture = "2.0.0"\n', encoding="utf-8")
                result = invoke([binary, "trust", configs[0]])
                assert result.returncode == 0, result.stderr
            elif operation == "delete":
                configs[0].unlink()
            before = {str(path): path.read_bytes().hex() for path in configs if path.is_file()}
            held = None
            try:
                if operation == "running":
                    executable = root / "data/installs/fixture/1.0.0/bin/sleep"
                    held = subprocess.Popen([executable, "300"], stdin=subprocess.DEVNULL,
                                            stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, env=env)
                    case["held_pid"] = held.pid
                    assert held.poll() is None
                result = invoke([binary, "prune", *flags])
                case["exit"] = result.returncode
                retained = []
                for version in VERSIONS:
                    path = root / "data/installs/fixture" / version / "bin/fixture"
                    if path.is_file():
                        executed = invoke([path])
                        assert executed.returncode == 0 and executed.stdout.strip() == version
                        retained.append(version)
                case["retained"] = retained
                case["configs_unchanged"] = all(path.read_bytes().hex() == before[str(path)] for path in configs if path.is_file())
                if held is not None:
                    case["held_alive_after_prune"] = held.poll() is None
                    case["held_exit_after_prune"] = held.returncode
                    assert case["held_alive_after_prune"]
                assert case["configs_unchanged"]
                assert result.returncode == expected_exit, result.stderr
                assert retained == expected_versions, (name, retained, result.stderr)
                case["status"] = "passed"
            except BaseException:
                case["status"] = report["status"] = "failed"
                save()
                raise
            finally:
                if held is not None:
                    held.terminate()
                    held.wait(timeout=TIMEOUT_SECONDS)
                    case["held_reaped"] = True
                save()
    report["status"] = "passed"
    save()
    print(f"{len(report['cases'])} frozen prune cases passed")


if __name__ == "__main__":
    main()
