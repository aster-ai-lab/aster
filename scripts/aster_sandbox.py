#!/usr/bin/env python3
"""Stage the shared macOS sandbox for CLI delivery and trusted development use."""

import argparse
import hashlib
from pathlib import Path
import platform
import shutil
import subprocess
import sys
import tempfile


ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "apps/studio/scripts"))
from sandbox_assets import stage as stage_shared, verify


def manifest_digest(root):
    with (root / "manifest.json").open("rb") as stream:
        data = stream.read(65537)
    if len(data) > 65536:
        raise ValueError("sandbox manifest exceeds its size bound")
    return hashlib.sha256(data).hexdigest()


def copy_verified(source, destination, expected_manifest):
    receipt = verify(source)
    if manifest_digest(source) != expected_manifest:
        raise ValueError("sandbox source changed before copying")
    if destination.is_symlink() or (destination.exists() and not destination.is_dir()):
        raise ValueError("sandbox destination must be a regular directory")
    if destination.is_dir():
        try:
            if verify(destination) == receipt and manifest_digest(destination) == expected_manifest:
                return destination
        except (OSError, ValueError, KeyError):
            pass
    destination.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix=".aster-sandbox-", dir=destination.parent) as temporary:
        staged = Path(temporary) / "sandbox"
        shutil.copytree(source, staged)
        if verify(staged) != receipt or manifest_digest(staged) != expected_manifest:
            raise ValueError("sandbox source changed while copying")
        # These are rebuildable artifacts, never a workspace or user-state directory.
        if destination.exists():
            shutil.rmtree(destination)
        staged.replace(destination)
    return destination


def stage(target, profile):
    stage_shared(target, profile)
    source = target / profile / "studio-resources/agent/sandbox"
    return copy_verified(source, target / profile / "sandbox", manifest_digest(source))


def development_cache(target, profile):
    source = target / profile / "sandbox"
    verify(source)
    identity = manifest_digest(source)
    cache = Path.home() / "Library/Caches/aster/sandbox" / identity
    return copy_verified(source, cache, identity)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("operation", choices=("stage", "cache", "verify", "members"))
    parser.add_argument("--target-dir", type=Path)
    parser.add_argument("--profile", choices=("debug", "release"), default="debug")
    parser.add_argument("--directory", type=Path)
    args = parser.parse_args()
    if args.operation in ("verify", "members"):
        if args.directory is None:
            parser.error(args.operation + " requires --directory")
        receipt = verify(args.directory)
        if args.operation == "members":
            for name in sorted([*receipt["files"], "manifest.json"]):
                print("sandbox/" + name)
        return
    if sys.platform != "darwin" or platform.machine() not in ("arm64", "aarch64"):
        raise ValueError("CLI sandbox staging requires macOS ARM64")
    if args.target_dir is None:
        import json
        metadata = subprocess.check_output(
            ["cargo", "metadata", "--locked", "--offline", "--no-deps", "--format-version", "1"],
            cwd=ROOT,
        )
        args.target_dir = Path(json.loads(metadata)["target_directory"])
    target = args.target_dir.resolve()
    if args.operation == "stage":
        stage(target, args.profile)
    else:
        print(development_cache(target, args.profile))


if __name__ == "__main__":
    try:
        main()
    except (OSError, ValueError, KeyError, subprocess.CalledProcessError) as error:
        print("sandbox staging failed: " + str(error), file=sys.stderr)
        raise SystemExit(1)
