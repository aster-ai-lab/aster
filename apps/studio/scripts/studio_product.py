#!/usr/bin/env python3
"""Closed release identities for the shared Studio host."""

import argparse
import json
from html.parser import HTMLParser
from pathlib import Path


def identity(tenant):
    if tenant == "aster":
        return {"product": "AsterDream", "identifier": "dev.a3s.studio.aster", "feature": "tenant-aster"}
    if tenant == "toyou":
        return {"product": "ToWork", "identifier": "dev.a3s.studio", "feature": "tenant-toyou"}
    raise ValueError(f"unsupported product tenant: {tenant}")


def validate_brand(tenant_root, tenant):
    identity(tenant)
    for name in ("INFO.md", "logo-light.png", "mark-light.png", "icon.png"):
        asset = Path(tenant_root) / tenant / name
        if asset.is_symlink() or not asset.is_file() or asset.stat().st_size == 0:
            raise ValueError(f"missing or invalid product brand asset: {asset}")


def validate_renderer(renderer, tenant):
    product = identity(tenant)
    index = Path(renderer) / "windows/main/index.html"
    if index.is_symlink() or not index.is_file() or index.stat().st_size > 1024 * 1024:
        raise ValueError("invalid renderer identity input")

    class HtmlIdentity(HTMLParser):
        def __init__(self):
            super().__init__()
            self.roots = []

        def handle_starttag(self, tag, attrs):
            if tag == "html":
                self.roots.append(attrs)

    with index.open("rb") as stream:
        document = stream.read(1024 * 1024 + 1)
    if len(document) > 1024 * 1024:
        raise ValueError("invalid renderer identity input")
    parser = HtmlIdentity()
    parser.feed(document.decode("utf-8"))
    expected = {"data-tenant": tenant, "data-a3s-product": product["product"]}
    if len(parser.roots) != 1 or any(
        [value for name, value in parser.roots[0] if name == key] != [value]
        for key, value in expected.items()
    ):
        raise ValueError("renderer identity does not match selected product tenant")


def tauri_config(tenant, tenant_root):
    product = identity(tenant)
    icons = [Path(tenant_root).resolve() / tenant / name for name in ("icon.icns", "icon.png", "icon.ico")]
    for icon in icons:
        if icon.is_symlink() or not icon.is_file() or icon.stat().st_size == 0:
            raise ValueError(f"missing product icon: {icon}")
    return {"productName": product["product"], "identifier": product["identifier"],
            "bundle": {"icon": [str(icon) for icon in icons]}}


def desktop_entry(tenant):
    product = identity(tenant)
    return ("[Desktop Entry]\nType=Application\nName=" + product["product"] +
            "\nComment=" + product["product"] + " desktop workspace\nExec=kylin-launch %U\nIcon=" +
            product["identifier"] + "\nTerminal=false\nCategories=Development;Utility;\nStartupWMClass=Studio\n")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("tenant")
    parser.add_argument("field", choices=("product", "identifier", "feature", "tauri-config", "desktop"))
    parser.add_argument("--tenant-root", type=Path,
                        default=Path(__file__).resolve().parents[2] / "web/public/tenant")
    parser.add_argument("--frontend-dist")
    args = parser.parse_args()
    product = identity(args.tenant)
    if args.field == "tauri-config":
        config = tauri_config(args.tenant, args.tenant_root)
        if args.frontend_dist is not None:
            config["build"] = {"frontendDist": args.frontend_dist}
        print(json.dumps(config))
    elif args.field == "desktop":
        print(desktop_entry(args.tenant), end="")
    else:
        print(product[args.field])


if __name__ == "__main__":
    try:
        main()
    except (OSError, ValueError) as error:
        raise SystemExit(f"studio-product: {error}")
