#!/usr/bin/env python3
"""Stage and verify one Studio Linux generation without installing user software."""

import argparse
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import shutil
import stat
import struct
import sys
import tempfile

HERE = Path(__file__).resolve().parent
APP_ID = "dev.a3s.studio"
FCITX4_SHA = "307bddeabbec383eb27c4ea222d9f71d8d9011d7942f4010368e5bc3ca7440a1"
FCITX5_SHA = "92d7a5e3ed9eac8c8173983723013816f4706dddc3f019b4bb5c76c48cf4dd92"
R4_BINARIES = {
    "kylin-im-backend": "34371bbf987e4620871845371734e8cdc11154bba4048b5688cd3acf4ad7f230",
    "immodules/fcitx4/im-fcitx.so": "73503354d8594dd03feb4cb27ad7a5e7f17a455eda58e2ea2e3138e8ed2d0734",
    "immodules/fcitx4/libfcitx-gclient.so.1": "7db5cc21ed9c62933e4785b221773027b64d091e40b2d92a9a7d4b268fe091d3",
    "immodules/fcitx4/libfcitx-utils.so.0": "10718ef3263b1b2b386e1fda073578526bbad3466acee01c9ef5ff69b2519cf0",
    "immodules/fcitx5/im-fcitx5.so": "1d73f6d8d82abd06d2e8974842b5cf67baf7e1967f833e59391b99b5ef5a8346",
}
TRAY_LIBS = ("libayatana-appindicator3.so.1", "libayatana-indicator3.so.7", "libayatana-ido3-0.4.so.0",
             "libdbusmenu-gtk3.so.4", "libdbusmenu-glib.so.4")
TRAY_LICENSES = ("libayatana-appindicator3-1", "libayatana-indicator3-7", "libayatana-ido3-0.4-0",
                 "libdbusmenu-gtk3-4", "libdbusmenu-glib4")
BRAND_FILES = ("INFO.md", "logo-light.png", "logo-dark.png", "mark-light.png", "mark-dark.png", "icon.png", "tray.png")
IM_FILES = [
    "kylin-im-backend",
    *["immodules/fcitx4/" + name for name in (
        "im-fcitx.so", "libfcitx-gclient.so.1", "libfcitx-utils.so.0", "immodules.cache",
        "query-path", "module-id", "COPYING", "fcitx-4.2.9.9.tar.xz", "fcitx4_legacy_portal.patch")],
    *["immodules/fcitx5/" + name for name in (
        "im-fcitx5.so", "immodules.cache", "query-path", "module-id", "LGPL-2.1-or-later.txt",
        "fcitx5-gtk-5.1.7.tar.gz")],
]


def regular(path):
    if not stat.S_ISREG(path.lstat().st_mode) or path.stat().st_size == 0:
        raise ValueError(f"missing or non-regular delivery input: {path}")
    return path


def digest(path):
    regular(path)
    result = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            result.update(chunk)
    return result.hexdigest()


def elf(path):
    regular(path)
    with path.open("rb") as stream:
        data = stream.read(20)
    if len(data) != 20 or data[:6] != b"\x7fELF\x02\x01" or struct.unpack_from("<H", data, 18)[0] != 62:
        raise ValueError(f"not a Linux x86_64 ELF: {path}")


def slug(value):
    if not value or len(value) > 32 or not value[0].isascii() or not value[0].islower():
        raise ValueError("invalid tenant slug")
    if any(c not in "abcdefghijklmnopqrstuvwxyz0123456789-" for c in value):
        raise ValueError("invalid tenant slug")
    return value


def files(root):
    result = {}
    for path in sorted(root.rglob("*")):
        if path.is_symlink():
            raise ValueError(f"symlink in delivery: {path}")
        if path.is_file():
            if len(result) >= 10000:
                raise ValueError("too many delivery files")
            result[path.relative_to(root).as_posix()] = digest(path)
    return result


def load(path):
    regular(path)
    if path.stat().st_size > 2 * 1024 * 1024:
        raise ValueError(f"oversize delivery receipt: {path}")
    return json.loads(path.read_text())


def write_json(path, value):
    path.write_text(json.dumps(value, sort_keys=True, indent=2) + "\n")


def inspect_ime(root):
    for name in IM_FILES:
        regular(root / name)
    for name in R4_BINARIES:
        elf(root / name)
    for name, expected in (("immodules/fcitx4/fcitx-4.2.9.9.tar.xz", FCITX4_SHA),
                           ("immodules/fcitx5/fcitx5-gtk-5.1.7.tar.gz", FCITX5_SHA)):
        if digest(root / name) != expected:
            raise ValueError(f"wrong corresponding source: {name}")
    if digest(root / "immodules/fcitx4/fcitx4_legacy_portal.patch") != digest(HERE / "fcitx4_legacy_portal.patch"):
        raise ValueError("unqualified fcitx4 portal patch")
    return {name: digest(root / name) for name in IM_FILES}


def record_ime(root, origin="built from the Studio recipes"):
    write_json(root / "ime-assets.json", {"schema": 1, "origin": origin,
               "files": inspect_ime(root)})


def import_r4(source, out, fcitx5_source):
    if out.exists():
        raise ValueError("IME destination must not exist")
    out.parent.mkdir(parents=True, exist_ok=True)
    for name, expected in R4_BINARIES.items():
        if digest(source / name) != expected:
            raise ValueError(f"not the qualified R4 artifact: {name}")
    with tempfile.TemporaryDirectory(dir=out.parent) as temporary:
        stage = Path(temporary) / "assets"
        stage.mkdir()
        for name in IM_FILES:
            src = fcitx5_source if name.endswith("fcitx5-gtk-5.1.7.tar.gz") else source / name
            dst = stage / name
            regular(src)
            dst.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(src, dst)
        record_ime(stage, "Work R4; exact binary hashes pinned in delivery.py")
        stage.rename(out)


def build_receipt(root, renderer, tenant, version, revision):
    for name in ("Studio", "a3s-code-acp", "a3s-office"):
        elf(root / name)
    regular(renderer / "windows/main/index.html")
    write_json(root / "studio-build.json", {"schema": 1, "product": APP_ID, "tenant": slug(tenant),
        "version": version, "revision": revision, "binaries": {
            name: digest(root / name) for name in ("Studio", "a3s-code-acp", "a3s-office")},
        "renderer": files(renderer)})


def cache_module():
    spec = importlib.util.spec_from_file_location("studio_immodule_cache", HERE / "immodule_cache.py")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def stage(build, renderer, tenant_root, im, destination):
    receipt = load(build / "studio-build.json")
    if receipt.get("schema") != 1 or receipt.get("product") != APP_ID:
        raise ValueError("not a Studio build receipt")
    tenant = slug(receipt["tenant"])
    if receipt["renderer"] != files(renderer):
        raise ValueError("renderer generation changed after build")
    im_receipt = load(im / "ime-assets.json")
    if im_receipt.get("schema") != 1 or set(im_receipt["files"]) != set(IM_FILES):
        raise ValueError("incomplete IME qualification receipt")
    if inspect_ime(im) != im_receipt["files"]:
        raise ValueError("IME generation changed")
    for name in ("Studio", "a3s-code-acp", "a3s-office"):
        elf(build / name)
        if receipt["binaries"].get(name) != digest(build / name):
            raise ValueError(f"binary generation changed: {name}")
    regular(tenant_root / tenant / "INFO.md")
    regular(build / "icon-128.png")
    with (build / "icon-128.png").open("rb") as stream:
        icon = stream.read(24)
    if icon[:16] != b"\x89PNG\r\n\x1a\n\x00\x00\x00\rIHDR" or struct.unpack(">II", icon[16:24]) != (128, 128):
        raise ValueError("desktop icon must be a 128x128 PNG")
    if destination.exists() and any(destination.iterdir()):
        raise ValueError("stage destination must be empty")
    destination.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(dir=destination.parent) as temporary:
        out = Path(temporary) / "files"
        out.mkdir()
        def copy(src, target):
            dst = out / target
            dst.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(regular(src), dst)
        copy(build / "Studio", "bin/Studio")
        copy(build / "a3s-code-acp", "bin/agent/a3s-code-acp")
        copy(build / "a3s-office", "bin/a3s-office")
        copy(im / "kylin-im-backend", "bin/kylin-im-backend")
        copy(HERE / "kylin-launch.sh", "bin/kylin-launch")
        copy(HERE / "delivery.py", "libexec/studio-kylin-delivery.py")
        copy(HERE / "dev.a3s.studio.desktop", "share/applications/dev.a3s.studio.desktop")
        copy(build / "icon-128.png", "share/icons/hicolor/128x128/apps/dev.a3s.studio.png")
        copy(build / "NotoSansHans-Regular.otf", "share/fonts/NotoSansHans-Regular.otf")
        copy(build / "font-LICENSE", "share/licenses/noto-sans/LICENSE")
        for name in TRAY_LIBS:
            elf(build / "native-libs" / name)
            copy(build / "native-libs" / name, "lib/" + name)
        for name in TRAY_LICENSES:
            copy(build / "native-licenses" / name / "copyright", f"share/licenses/native/{name}/copyright")
        shutil.copytree(renderer, out / "Resources/studio-frontend")
        for name in BRAND_FILES:
            copy(tenant_root / tenant / name, f"Resources/tenant/{tenant}/{name}")
        (out / "Resources/studio-tenant").write_text(tenant + "\n")
        copy(build / "studio-build.json", "Resources/studio-build.json")
        copy(im / "ime-assets.json", "share/licenses/studio/ime-assets.json")
        copy(HERE / "work-provenance.json", "share/licenses/studio/work-provenance.json")
        copy(HERE.parents[1] / "LICENSE", "share/licenses/studio/LICENSE")
        for name in ("kylin_im_backend.c", "build-immodule.sh", "build-fcitx4-immodule.sh", "immodule_cache.py"):
            copy(HERE / name, "share/licenses/studio/" + name)
        helper = cache_module()
        for version, binary in (("fcitx4", "im-fcitx.so"), ("fcitx5", "im-fcitx5.so")):
            src = im / "immodules" / version
            for name in ((binary, "libfcitx-gclient.so.1", "libfcitx-utils.so.0") if version == "fcitx4" else (binary,)):
                copy(src / name, f"lib/im/{version}/{name}")
            module_id = (src / "module-id").read_text().strip()
            text = helper.rewrite_immodule_cache((src / "immodules.cache").read_text(),
                (src / "query-path").read_text().strip(), f"/app/lib/im/{version}/{binary}", module_id)
            (out / f"lib/im/{version}/immodules.cache").write_text(text)
            (out / f"lib/im/{version}/module-id").write_text(module_id + "\n")
            for name in (("COPYING", "fcitx-4.2.9.9.tar.xz", "fcitx4_legacy_portal.patch") if version == "fcitx4" else
                         ("LGPL-2.1-or-later.txt", "fcitx5-gtk-5.1.7.tar.gz")):
                copy(src / name, f"share/licenses/{version}/{name}")
        write_json(out / "studio-generation.json", {"schema": 1, "product": APP_ID, "tenant": tenant,
            "version": receipt["version"], "revision": receipt["revision"], "files": files(out)})
        hashes = files(out)
        (out / "SHA256SUMS").write_text("".join(f"{value}  {name}\n" for name, value in hashes.items()))
        verify(out)
        out.rename(destination)


def verify(root):
    receipt = load(root / "studio-generation.json")
    if receipt.get("schema") != 1 or receipt.get("product") != APP_ID:
        raise ValueError("not a Studio generation")
    tenant = slug(receipt["tenant"])
    required = {"bin/Studio", "bin/agent/a3s-code-acp", "bin/a3s-office", "bin/kylin-launch",
        "bin/kylin-im-backend", "libexec/studio-kylin-delivery.py", "Resources/studio-tenant",
        "Resources/studio-frontend/windows/main/index.html", f"Resources/tenant/{tenant}/INFO.md",
        "lib/im/fcitx4/im-fcitx.so", "lib/im/fcitx4/immodules.cache", "lib/im/fcitx4/module-id",
        "lib/im/fcitx5/im-fcitx5.so", "lib/im/fcitx5/immodules.cache", "lib/im/fcitx5/module-id"}
    required.update("lib/" + name for name in TRAY_LIBS)
    required.update({"lib/im/fcitx4/libfcitx-gclient.so.1", "lib/im/fcitx4/libfcitx-utils.so.0",
        "share/licenses/fcitx4/fcitx-4.2.9.9.tar.xz", "share/licenses/fcitx4/fcitx4_legacy_portal.patch",
        "share/licenses/fcitx5/fcitx5-gtk-5.1.7.tar.gz", "share/applications/dev.a3s.studio.desktop",
        "share/icons/hicolor/128x128/apps/dev.a3s.studio.png", "share/fonts/NotoSansHans-Regular.otf",
        "share/licenses/noto-sans/LICENSE"})
    required.update(f"share/licenses/native/{name}/copyright" for name in TRAY_LICENSES)
    required.update(f"Resources/tenant/{tenant}/{name}" for name in BRAND_FILES)
    if not required.issubset(receipt["files"]):
        raise ValueError("incomplete Studio generation")
    actual = files(root)
    actual.pop("SHA256SUMS", None)
    marker_hash = actual.pop("studio-generation.json")
    if actual != receipt["files"]:
        raise ValueError("Studio generation hash or membership mismatch")
    actual["studio-generation.json"] = marker_hash
    expected = "".join(f"{value}  {name}\n" for name, value in sorted(actual.items()))
    if (root / "SHA256SUMS").read_text() != expected:
        raise ValueError("Studio generation SHA256SUMS mismatch")
    if (root / "Resources/studio-tenant").read_text() != tenant + "\n":
        raise ValueError("tenant generation mismatch")
    for name in ("bin/Studio", "bin/agent/a3s-code-acp", "bin/a3s-office", "bin/kylin-im-backend", "bin/kylin-launch"):
        if not os.access(root / name, os.X_OK):
            raise ValueError(f"delivery executable is not executable: {name}")
    return receipt


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest="command", required=True)
    cmd = sub.add_parser("record-ime"); cmd.add_argument("root", type=Path)
    cmd = sub.add_parser("import-r4")
    for name in ("source", "out", "fcitx5_source"): cmd.add_argument(name, type=Path)
    cmd = sub.add_parser("record-build")
    for name in ("root", "renderer"): cmd.add_argument(name, type=Path)
    for name in ("tenant", "version", "revision"): cmd.add_argument(name)
    cmd = sub.add_parser("stage")
    for name in ("build", "renderer", "tenant_root", "im", "destination"): cmd.add_argument(name, type=Path)
    cmd = sub.add_parser("verify"); cmd.add_argument("root", type=Path)
    args = vars(parser.parse_args()); command = args.pop("command")
    {"record-ime": record_ime, "import-r4": import_r4, "record-build": build_receipt,
     "stage": stage, "verify": verify}[command](**args)


if __name__ == "__main__":
    try:
        main()
    except (ValueError, OSError, KeyError, TypeError) as error:
        print(f"studio-kylin: {error}", file=sys.stderr)
        sys.exit(1)
