#!/usr/bin/env python3
"""Resize the sole Aster logo.png master and assemble platform containers.

The installed Tauri CLI produces every size from the same approved artwork.
Light and dark names are compatibility exports of that same logo, not inverses.
The macOS tray uses its background-free template silhouette.

Outputs, all in `apps/web/public/tenant/aster/`:

  icons/{n}x{n}.png   the ladder, also the Linux hicolor set
  icon.png            1024 px, the master raster
  icon.ico            multi-resolution Windows container
  icon.icns           multi-resolution macOS container, per-size frames
  logo/mark/tray      the same artwork at each consumer's required size
  favicon.png        browser tab icon
  icon-assets.sha256 source and export digests for dependency-free verification

`tauri.conf.json` bundles the icon containers; `just studio-desktop` installs the
ladder into the hicolor theme so a dock can find it by name.

Usage:
    python3 scripts/app-icon-assets.py            # render and verify
    python3 scripts/app-icon-assets.py --check    # verify committed files
"""

from __future__ import annotations

import hashlib
import shutil
import struct
import subprocess
import sys
import tempfile
import zlib
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent
TENANT_DIR = REPO / "apps" / "web" / "public" / "tenant" / "aster"
ICON_DIR = TENANT_DIR / "icons"
TAURI_CLI = REPO / "apps" / "studio" / "node_modules" / ".bin" / "tauri"

LADDER = (16, 24, 32, 40, 48, 64, 80, 128, 256, 512, 1024)
# Windows asks for the container to carry the small frames; Explorer and the
# taskbar pick the frame that matches their own scale factor.
ICO_SIZES = [16, 24, 32, 48, 64, 128, 256]
# (icl4 tag, pixel size). The `ic07`+ family takes PNG payloads, and the `ic1x`
# tags are the @2x entries macOS reads on Retina displays.
ICNS_FRAMES = [
    ("icp4", 16),
    ("icp5", 32),
    ("icp6", 64),
    ("ic07", 128),
    ("ic08", 256),
    ("ic09", 512),
    ("ic10", 1024),
    ("ic11", 32),
    ("ic12", 64),
    ("ic13", 256),
    ("ic14", 512),
]
MARK_SIZES = {
    "logo-light.png": 512,
    "logo-dark.png": 512,
    "mark-light.png": 256,
    "mark-dark.png": 256,
    "tray.png": 32,
    "favicon.png": 32,
}


def png_size(data: bytes) -> tuple[int, int]:
    if data[:8] != b"\x89PNG\r\n\x1a\n":
        raise ValueError("not a PNG")
    width, height = struct.unpack(">II", data[16:24])
    return width, height


def png_pixels(data: bytes) -> tuple[int, int, bytes]:
    """Decode an 8-bit RGBA, non-interlaced PNG into raw RGBA bytes.

    Only the layout Tauri writes is supported, which keeps this dependency-free
    while still letting the verifier look at real alpha values instead of image
    dimensions alone.
    """
    width, height = png_size(data)
    depth, colour, _, _, interlace = data[24:29]
    if (depth, colour, interlace) != (8, 6, 0):
        raise ValueError(f"unsupported PNG layout {depth}/{colour}/{interlace}")
    offset = 8
    payload = bytearray()
    while offset < len(data):
        length = struct.unpack(">I", data[offset : offset + 4])[0]
        kind = data[offset + 4 : offset + 8]
        chunk = data[offset + 8 : offset + 8 + length]
        if kind == b"IDAT":
            payload += chunk
        elif kind == b"IEND":
            break
        offset += 12 + length
    raw = zlib.decompress(bytes(payload))
    stride = width * 4
    out = bytearray(stride * height)
    previous = bytearray(stride)
    position = 0
    for row in range(height):
        filter_type = raw[position]
        position += 1
        line = bytearray(raw[position : position + stride])
        position += stride
        if filter_type == 1:
            for index in range(4, stride):
                line[index] = (line[index] + line[index - 4]) & 0xFF
        elif filter_type == 2:
            for index in range(stride):
                line[index] = (line[index] + previous[index]) & 0xFF
        elif filter_type == 3:
            for index in range(stride):
                left = line[index - 4] if index >= 4 else 0
                line[index] = (line[index] + ((left + previous[index]) >> 1)) & 0xFF
        elif filter_type == 4:
            for index in range(stride):
                left = line[index - 4] if index >= 4 else 0
                up = previous[index]
                up_left = previous[index - 4] if index >= 4 else 0
                estimate = left + up - up_left
                pa, pb, pc = (
                    abs(estimate - left),
                    abs(estimate - up),
                    abs(estimate - up_left),
                )
                predictor = left if pa <= pb and pa <= pc else (up if pb <= pc else up_left)
                line[index] = (line[index] + predictor) & 0xFF
        elif filter_type != 0:
            raise ValueError(f"unknown PNG filter {filter_type}")
        out[row * stride : (row + 1) * stride] = line
        previous = line
    return width, height, bytes(out)


def alpha_at(width: int, pixels: bytes, x: int, y: int) -> int:
    return pixels[(y * width + x) * 4 + 3]


def write_ico(path: Path, frames: list[tuple[int, bytes]]) -> None:
    header = struct.pack("<HHH", 0, 1, len(frames))
    offset = 6 + 16 * len(frames)
    directory = bytearray()
    payload = bytearray()
    for size, data in frames:
        # A zero entry means 256: the field is a single byte.
        dimension = 0 if size >= 256 else size
        directory += struct.pack(
            "<BBBBHHII", dimension, dimension, 0, 0, 1, 32, len(data), offset
        )
        offset += len(data)
        payload += data
    path.write_bytes(header + bytes(directory) + bytes(payload))


def write_icns(path: Path, frames: list[tuple[str, bytes]]) -> None:
    body = bytearray()
    for tag, data in frames:
        body += tag.encode("ascii") + struct.pack(">I", len(data) + 8) + data
    path.write_bytes(b"icns" + struct.pack(">I", len(body) + 8) + bytes(body))


def verify() -> list[str]:
    problems: list[str] = []
    for size in LADDER:
        path = ICON_DIR / f"{size}x{size}.png"
        if not path.is_file():
            problems.append(f"missing {path.relative_to(REPO)}")
            continue
        data = path.read_bytes()
        width, height = png_size(data)
        if (width, height) != (size, size):
            problems.append(f"{path.name}: {width}x{height}, expected {size}")
            continue
        width, _, pixels = png_pixels(data)
        # Resampling the generated master leaves up to 2/255 alpha at a corner.
        if alpha_at(width, pixels, 1, 1) > 2:
            problems.append(f"{path.name}: corner is opaque, plate lost its margin")
        middle = alpha_at(width, pixels, size // 2, size // 2)
        if middle < 250:
            problems.append(f"{path.name}: centre alpha {middle}, expected at least 250")
    master = TENANT_DIR / "icon.png"
    if not master.is_file():
        problems.append("missing icon.png")
    elif png_size(master.read_bytes()) != (1024, 1024):
        problems.append("icon.png is not 1024x1024")
    ico = TENANT_DIR / "icon.ico"
    if not ico.is_file():
        problems.append("missing icon.ico")
    else:
        data = ico.read_bytes()
        count = struct.unpack("<H", data[4:6])[0]
        if count != len(ICO_SIZES):
            problems.append(f"icon.ico carries {count} frames, expected {len(ICO_SIZES)}")
    icns = TENANT_DIR / "icon.icns"
    if not icns.is_file():
        problems.append("missing icon.icns")
    elif icns.read_bytes()[:4] != b"icns":
        problems.append("icon.icns has no icns magic")
    for name, size in MARK_SIZES.items():
        path = TENANT_DIR / name
        if not path.is_file():
            problems.append(f"missing {name}")
            continue
        try:
            if png_size(path.read_bytes()) != (size, size):
                problems.append(f"{name} is not {size}x{size}")
        except ValueError as error:
            problems.append(f"{name}: {error}")
    tray = TENANT_DIR / "tray.png"
    if tray.is_file():
        width, height, pixels = png_pixels(tray.read_bytes())
        opaque = sum(alpha > 128 for alpha in pixels[3::4])
        solid = sum(alpha == 255 for alpha in pixels[3::4])
        if not width * height / 5 < opaque < width * height / 2:
            problems.append("tray.png must be a transparent glyph, not an app-icon tile")
        if solid < width * height / 8:
            problems.append("tray.png must have opaque strokes, not a faded raster mask")
    for name, size in MARK_SIZES.items():
        path = TENANT_DIR / name
        frame = ICON_DIR / f"{size}x{size}.png"
        if name != "tray.png" and size in LADDER and path.is_file() and frame.is_file():
            if path.read_bytes() != frame.read_bytes():
                problems.append(f"{name} differs from the shared {size}px icon")
    receipt = TENANT_DIR / "icon-assets.sha256"
    if not problems and (not receipt.is_file() or receipt.read_text() != asset_manifest()):
        problems.append("icon exports differ from their generation receipt; run `just app-icons`")
    config = (REPO / "apps" / "studio" / "src-tauri" / "tauri.conf.json").read_text()
    for name in ("icon.icns", "icon.png", "icon.ico"):
        if f'tenant/aster/{name}"' not in config:
            problems.append(f"tauri.conf.json does not bundle {name}")
    return problems


def render() -> None:
    if not TAURI_CLI.is_file():
        raise SystemExit(
            f"missing {TAURI_CLI.relative_to(REPO)}; run the package install in apps/studio"
        )
    ICON_DIR.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(dir=REPO / "target") as scratch:
        staging = Path(scratch)
        subprocess.run(
            [
                str(TAURI_CLI), "icon", str(TENANT_DIR / "logo.png"),
                "-o", str(staging), "-p", ",".join(map(str, LADDER)),
            ],
            cwd=REPO / "apps" / "studio",
            check=True,
            stdout=subprocess.DEVNULL,
        )
        for size in LADDER:
            rendered = staging / f"{size}x{size}.png"
            shutil.copyfile(rendered, ICON_DIR / f"{size}x{size}.png")
        shutil.copyfile(ICON_DIR / "1024x1024.png", TENANT_DIR / "icon.png")
        write_ico(
            TENANT_DIR / "icon.ico",
            [(size, (ICON_DIR / f"{size}x{size}.png").read_bytes()) for size in ICO_SIZES],
        )
        write_icns(
            TENANT_DIR / "icon.icns",
            [
                (tag, (ICON_DIR / f"{size}x{size}.png").read_bytes())
                for tag, size in ICNS_FRAMES
            ],
        )
        subprocess.run(
            [
                str(TAURI_CLI), "icon", str(TENANT_DIR / "tray-template.svg"),
                "-o", str(staging / "tray"), "-p", "32",
            ],
            cwd=REPO / "apps" / "studio",
            check=True,
            stdout=subprocess.DEVNULL,
        )
        for name, size in MARK_SIZES.items():
            directory = staging / "tray" if name == "tray.png" else staging
            shutil.copyfile(directory / f"{size}x{size}.png", TENANT_DIR / name)
    (TENANT_DIR / "icon-assets.sha256").write_text(asset_manifest())
    print(f"rendered {len(LADDER)} rasters into {ICON_DIR.relative_to(REPO)}")
    print("wrote icon.png, icon.ico, icon.icns, logos, tray and favicon from logo.png")


def asset_manifest() -> str:
    names = [
        "logo.png", "tray-template.svg", "icon.png", "icon.ico", "icon.icns", *MARK_SIZES,
        *(f"icons/{size}x{size}.png" for size in LADDER),
    ]
    return "".join(
        f"{hashlib.sha256((TENANT_DIR / name).read_bytes()).hexdigest()}  {name}\n"
        for name in names
    )


def main(argv: list[str]) -> int:
    check = "--check" in argv
    if not check:
        render()
    problems = verify()
    if problems:
        for problem in problems:
            print(f"icon: {problem}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
