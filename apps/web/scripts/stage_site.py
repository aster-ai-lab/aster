#!/usr/bin/env python3
"""Stage only the checked public files in the explicit site allowlist."""

import argparse
import hashlib
import json
from pathlib import Path
import shutil
import sys
import tempfile

from check_site import PUBLIC_ROOT, check_site
from site_contract import DOCS_FILES, HOME_FILES, SITE_FILES


def stage_site(public_root, destination):
    source, destination = Path(public_root).absolute(), Path(destination).absolute()
    check_site(source)
    if destination.exists() or destination.is_symlink():
        raise ValueError("stage destination already exists")
    if destination.resolve().is_relative_to(source.resolve()):
        raise ValueError("stage destination must be outside public sources")
    destination.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix="aster-site-", dir=destination.parent) as temporary:
        out = Path(temporary) / "site"
        out.mkdir()
        for name in SITE_FILES + HOME_FILES + DOCS_FILES:
            target = out / name
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(source / name, target)
        check_site(out)
        files = {path.relative_to(out).as_posix(): hashlib.sha256(path.read_bytes()).hexdigest()
                 for path in sorted(out.rglob("*")) if path.is_file()}
        out.rename(destination)
    return {"status": "staged", "files": files, "private_sources_required": False}


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("destination", type=Path)
    parser.add_argument("--public-root", type=Path, default=PUBLIC_ROOT)
    args = parser.parse_args()
    try:
        print(json.dumps(stage_site(args.public_root, args.destination), sort_keys=True))
    except (ValueError, OSError, UnicodeError) as error:
        print(f"site stage failed: {error}", file=sys.stderr)
        raise SystemExit(1)
