#!/usr/bin/env python3
"""Subset the packaged Noto Sans Hans layout font in place."""

import sys
from pathlib import Path

from fontTools.subset import Options, Subsetter
from fontTools.ttLib import TTFont

RANGES = (
    (0x0000, 0x024F),
    (0x2000, 0x206F),
    (0x2070, 0x209F),
    (0x20A0, 0x20CF),
    (0x2100, 0x214F),
    (0x2190, 0x21FF),
    (0x2200, 0x22FF),
    (0x2460, 0x24FF),
    (0x2500, 0x257F),
    (0x25A0, 0x25FF),
    (0x2600, 0x26FF),
    (0x3000, 0x303F),
    (0x3040, 0x30FF),
    (0xFF00, 0xFFEF),
)


def gb2312_codepoints() -> set[int]:
    points: set[int] = set()
    for row in range(0xA1, 0xF8):
        for col in range(0xA1, 0xFF):
            try:
                text = bytes((row, col)).decode("gb2312")
            except UnicodeDecodeError:
                continue
            if len(text) == 1:
                points.add(ord(text))
    return points


def subset(font_path: Path, unicodes: set[int]) -> None:
    font = TTFont(font_path)
    options = Options()
    options.layout_features = ["*"]
    options.name_IDs = ["*"]
    options.notdef_outline = True
    options.recommended_glyphs = True
    options.glyph_names = True
    subsetter = Subsetter(options=options)
    subsetter.populate(unicodes=unicodes)
    subsetter.subset(font)
    temporary = font_path.with_name(font_path.name + ".tmp")
    font.save(temporary)
    font.close()
    temporary.replace(font_path)
    print(f"{font_path.name} {font_path.stat().st_size}")


def main() -> int:
    if len(sys.argv) != 2:
        print("usage: subset-layout-font.py DIST", file=sys.stderr)
        return 2
    root = Path(sys.argv[1])
    fonts = sorted(root.rglob("noto-sans-hans-regular*.otf"))
    if not fonts:
        print("layout font was not emitted", file=sys.stderr)
        return 1
    unicodes = gb2312_codepoints()
    for start, end in RANGES:
        unicodes.update(range(start, end + 1))
    for font_path in fonts:
        subset(font_path, unicodes)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
