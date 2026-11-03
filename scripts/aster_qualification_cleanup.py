#!/usr/bin/env python3
"""Preview or remove old frozen qualification candidates."""

import argparse
import json
import shutil
import subprocess
from pathlib import Path

PREFIXES = (
    "fallback-state-",
    "deloop-changing-feed-",
    "deloop-independent-records-",
    "glm-tui-",
    "tui-",
    "rust-multifile-real-diagnostic-",
    "native-multiturn-diagnostic-",
    "rust-std-multiturn-diagnostic-",
)
EXCLUDED_PREFIXES = ("tui-current-",)


def retention_plan(root: Path, protected: set[str]) -> tuple[list[Path], list[Path]]:
    directories = [
        path for path in root.iterdir() if path.is_dir() and not path.is_symlink()
    ]
    keep = [
        path
        for path in directories
        if any(path.name.startswith(prefix) for prefix in EXCLUDED_PREFIXES)
    ]
    delete = []
    for prefix in PREFIXES:
        candidates = sorted(
            (
                path
                for path in directories
                if path.name.startswith(prefix)
                and not any(path.name.startswith(item) for item in EXCLUDED_PREFIXES)
            ),
            key=lambda path: (path.stat().st_mtime_ns, path.name),
            reverse=True,
        )
        keep.extend(candidates[:2])
        keep.extend(path for path in candidates[2:] if path.name in protected)
        retained = set(keep)
        delete.extend(path for path in candidates if path not in retained)
    return keep, delete


def active_commands() -> str:
    if shutil.which("pgrep") is None:
        raise RuntimeError("active-process checking requires pgrep")
    result = subprocess.run(
        ["pgrep", "-fl", "."], capture_output=True, text=True, check=False
    )
    if result.returncode not in (0, 1):
        raise RuntimeError("could not inspect active processes")
    return result.stdout


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=Path("target/qualification"))
    parser.add_argument("--keep", action="append", default=[], metavar="DIRECTORY")
    parser.add_argument("--execute", action="store_true")
    args = parser.parse_args()

    root = args.root.resolve()
    keep, delete = retention_plan(root, set(args.keep))
    commands = active_commands()
    active = [path for path in delete if path.name in commands]
    if args.execute and active:
        raise SystemExit("refusing to remove candidates still referenced by processes")

    payload = {
        "root": str(root),
        "keep": [path.name for path in keep],
        "delete": [path.name for path in delete],
        "active": [path.name for path in active],
        "executed": args.execute,
    }
    if args.execute:
        for path in delete:
            shutil.rmtree(path)
    print(json.dumps(payload, indent=2))


if __name__ == "__main__":
    main()
