#!/usr/bin/env python3
"""Seal Studio's unsigned resource generation before the final macOS code signature."""

import hashlib
import json
import pathlib
import plistlib
import re
import shutil
import sys

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
from studio_product import identity, validate_brand, validate_renderer


def seal(app, renderer, tenant, tenant_root):
    app, renderer, tenant_root = map(pathlib.Path, (app, renderer, tenant_root))
    if not re.fullmatch(r"[a-z][a-z0-9-]{0,31}", tenant):
        raise ValueError("invalid tenant slug")
    product = identity(tenant)
    with (app / "Contents/Info.plist").open("rb") as stream:
        info = plistlib.load(stream)
    expected = {"CFBundleExecutable": "Studio", "CFBundleIdentifier": product["identifier"],
                "CFBundleName": product["product"], "CFBundleDisplayName": product["product"]}
    if any(info.get(key) != value for key, value in expected.items()):
        raise ValueError("not a Studio product identity")
    validate_brand(tenant_root, tenant)
    validate_renderer(renderer, tenant)
    version = info["CFBundleShortVersionString"]
    resources = app / "Contents/Resources"
    marker = resources / "studio-delivery.json"
    if marker.is_symlink():
        raise ValueError("symlink delivery marker")
    for source in (renderer, tenant_root / tenant):
        for path in [source, *source.rglob("*")]:
            if path.is_symlink() or not (path.is_file() or path.is_dir()):
                raise ValueError(f"invalid Studio resource input: {path}")
    for name in ("studio-frontend", "tenant"):
        target = resources / name
        if target.exists():
            shutil.rmtree(target)
    shutil.copytree(renderer, resources / "studio-frontend")
    shutil.copytree(tenant_root / tenant, resources / "tenant" / tenant)
    required = ["Contents/MacOS/Studio", "Contents/Info.plist",
                "Contents/Resources/agent/a3s-code-acp", "Contents/Resources/agent/a3s-office",
                "Contents/Resources/studio-frontend/windows/main/index.html",
                f"Contents/Resources/tenant/{tenant}/INFO.md"]
    for name in required:
        path = app / name
        if path.is_symlink() or not path.is_file() or path.stat().st_size == 0:
            raise ValueError(f"missing Studio delivery input: {name}")
    files = {}
    # The final code signature seals the host. Hashing its signature here would be circular.
    for path in sorted([app / "Contents/Info.plist", *resources.rglob("*")]):
        if path == marker:
            continue
        if path.is_symlink():
            raise ValueError(f"symlink in Studio resources: {path}")
        if path.is_file():
            data = path.read_bytes()
            files[path.relative_to(app).as_posix()] = {"sha256": hashlib.sha256(data).hexdigest(), "size": len(data)}
    if len(files) > 4096:
        raise ValueError("too many Studio resources")
    marker.write_text(json.dumps({"schema": 1, "product": "a3s-studio", "version": version,
                                 "tenant": tenant, "files": files}, sort_keys=True, indent=2) + "\n")
    return marker


if __name__ == "__main__":
    if len(sys.argv) != 5:
        raise SystemExit("usage: studio_delivery.py APP RENDERER TENANT TENANT_ROOT")
    print(seal(*sys.argv[1:]))
