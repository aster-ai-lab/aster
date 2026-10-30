#!/usr/bin/env python3
"""Export the sole Aster logo.png master as a self-contained website SVG."""

from __future__ import annotations

import base64
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent
TENANT_DIR = REPO / "apps" / "web" / "public" / "tenant" / "aster"
LEGACY_SOURCES = ("app-icon.svg", "app-icon-small.svg", "app-icon-micro.svg", "tray-template.png")


def main(argv: list[str]) -> int:
    image = base64.b64encode((TENANT_DIR / "icon.png").read_bytes()).decode("ascii")
    svg = f'''<svg xmlns="http://www.w3.org/2000/svg" width="1024" height="1024" viewBox="0 0 1024 1024" role="img">
  <title>Aster</title>
  <desc>Silver star and orbit on a textured obsidian tile.</desc>
  <image width="1024" height="1024" href="data:image/png;base64,{image}"/>
</svg>
'''
    path = TENANT_DIR / "logo.svg"
    if "--check" in argv:
        if not path.is_file() or path.read_text(encoding="utf-8") != svg:
            print("stale logo.svg; run `just app-icons`", file=sys.stderr)
            return 1
        if any((TENANT_DIR / name).exists() for name in LEGACY_SOURCES):
            print("retired icon drawings remain; run `just app-icons`", file=sys.stderr)
            return 1
        return 0
    path.write_text(svg, encoding="utf-8")
    for name in LEGACY_SOURCES:
        (TENANT_DIR / name).unlink(missing_ok=True)
    print(f"wrote {path.relative_to(REPO)} from logo.png")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
