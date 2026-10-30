#!/usr/bin/env python3
"""Build the Aster application-icon sources from one parameterised mark.

A desktop icon is the one brand surface that gets scaled down to 16 px, and a
single drawing cannot serve both ends of that range: a 22 px orbit stroke and a
40 px companion star read as grey aliasing in a tray, while a 40 px stroke reads
as a toy at 1024 px. The mark is therefore generated as a three-step ladder:

  * `app-icon.svg`       — 128 px and above. Full detail: rim light, gloss,
    companion star, three-stop gradients.
  * `app-icon-small.svg` — 48–80 px. Heavier orbit, larger star, no companion.
  * `app-icon-micro.svg` — 16–40 px. A flat orbit and larger star, without
    gloss, bevels, companion star or orbital gradients.

Each file is written into `apps/web/public/tenant/aster/`, the tenant directory
that owns Aster branding: `tauri.conf.json` bundles `icon.png`, `icon.icns` and
`icon.ico` from beside them, and `scripts/app-icon-assets.py` rasterises the
size ladder and the Linux hicolor set from them.

Geometry follows the macOS app-icon grid: an 824 px plate centred in a 1024 px
canvas. The plate is a superellipse (squircle), not a rounded rectangle, because
a circular corner at that radius reads boxier than the mask the platform applies.

Usage:
    python3 scripts/app-icon.py            # rewrite the three SVGs
    python3 scripts/app-icon.py --check    # fail when a file is stale
"""

from __future__ import annotations

import math
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent
TENANT_DIR = REPO / "apps" / "web" / "public" / "tenant" / "aster"

CANVAS = 1024
# macOS app-icon grid: an 824 px plate centred in a 1024 px canvas, which leaves
# the optical margin the platform reserves around every icon.
PLATE_INSET = 100
PLATE_SIZE = CANVAS - 2 * PLATE_INSET
# Continuous corners; 5 is close to the exponent the platform mask approximates.
PLATE_EXPONENT = 5.0

# The mark: a four-point star with a diamond core, a tilted silver orbit and one
# companion star. Geometry is in canvas pixels and shared by every variant.
STAR_PATH = (
    "M512 177C557 379 589 430 756 507C589 552 554 616 512 850"
    "C470 616 435 552 268 507C435 430 467 379 512 177Z"
)
STAR_CX = 512.0
STAR_CY = 513.0
CORE_PATH = "m512 440 48 74-48 93-48-93 48-74Z"
COMPANION_PATH = "m760 190 13 33 33 13-33 13-13 33-13-33-33-13 33-13 13-33Z"

ORBIT_CX = 512
ORBIT_CY = 522
ORBIT_RX = 366
ORBIT_RY = 138
ORBIT_ROTATION = -26


def squircle_path(size: float, exponent: float, steps: int = 144) -> str:
    """Return a closed superellipse path of `size` px centred on the canvas.

    `steps` keeps the chord deviation under a fifth of a pixel at 1024 px, so
    the polygon is indistinguishable from the analytic curve at any raster size
    and needs no Bézier fitting.
    """
    half = size / 2.0
    power = 2.0 / exponent
    points = []
    for step in range(steps):
        angle = 2.0 * math.pi * step / steps
        cos, sin = math.cos(angle), math.sin(angle)
        x = math.copysign(abs(cos) ** power, cos) * half
        y = math.copysign(abs(sin) ** power, sin) * half
        points.append(f"{x + CANVAS / 2:.1f} {y + CANVAS / 2:.1f}")
    return "M" + "L".join(points) + "Z"


def scaled_about_canvas(factor: float, cx: float, cy: float) -> str:
    return f"translate({cx} {cy}) scale({factor}) translate(-{cx} -{cy})"


def build_svg(detail: str) -> str:
    if detail not in {"full", "small", "micro"}:
        raise SystemExit(f"unknown detail level: {detail}")
    plate = squircle_path(PLATE_SIZE, PLATE_EXPONENT)
    if detail == "micro":
        body = f"""<svg xmlns="http://www.w3.org/2000/svg" width="1024" height="1024" viewBox="0 0 1024 1024" role="img">
  <title>Aster</title>
  <desc>A bevelled silver four-point star with an obsidian diamond core and a wrapped metallic orbit, on a softly lit graphite squircle plate.</desc>
  <defs>
    <path id="plate" d="{plate}"/>
    <linearGradient id="plate" x1="340" y1="90" x2="700" y2="960" gradientUnits="userSpaceOnUse">
      <stop stop-color="#3A3A3A"/><stop offset=".5" stop-color="#171717"/><stop offset="1" stop-color="#050505"/>
    </linearGradient>
    <linearGradient id="star" x1="440" y1="170" x2="600" y2="840" gradientUnits="userSpaceOnUse">
      <stop stop-color="#FFFFFF"/><stop offset=".48" stop-color="#F5F5F5"/><stop offset="1" stop-color="#D4D4D4"/>
    </linearGradient>
    <radialGradient id="halo"><stop stop-color="#FFFFFF" stop-opacity=".18"/><stop offset="1" stop-color="#FFFFFF" stop-opacity="0"/></radialGradient>
  </defs>
  <use href="#plate" fill="url(#plate)"/>


  <g transform="rotate({ORBIT_ROTATION} {ORBIT_CX} {ORBIT_CY})">
    <ellipse cx="{ORBIT_CX}" cy="{ORBIT_CY}" rx="{ORBIT_RX}" ry="{ORBIT_RY}" fill="none"
      stroke="#E5E5E5" stroke-opacity=".42" stroke-width="36"/>
  </g>

  <g transform="{scaled_about_canvas(1.16, STAR_CX, STAR_CY)}">
    <path d="{STAR_PATH}" fill="url(#star)"/>
    <g transform="{scaled_about_canvas(1.22, 512, STAR_CY)}">
      <path d="{CORE_PATH}" fill="#171717"/>
    </g>
  </g>
  <use href="#plate" fill="none" stroke="#FFFFFF" stroke-opacity=".22" stroke-width="6"/>
</svg>
"""
    else:
        small = detail == "small"
        plate_top = "#404040" if small else "#3A3A3A"
        plate_bottom = "#080808" if small else "#050505"
        star_mid = "#F0F0F0" if small else "#E8E8E8"
        star_dark = "#C8C8C8" if small else "#A3A3A3"
        halo_opacity = ".22" if small else ".12"
        star_scale = 1.10 if small else 1.05
        core_scale = 1.10 if small else 1.08
        orbit_stroke = 30 if small else 22
        companion_definition = "" if small else """    <linearGradient id="companion" x1="742" y1="190" x2="778" y2="280" gradientUnits="userSpaceOnUse">
      <stop stop-color="#FFFFFF"/><stop offset="1" stop-color="#BFBFBF"/>
    </linearGradient>
"""
        companion = "" if small else f"""    <path d="{COMPANION_PATH}" fill="#000000" fill-opacity=".24" transform="translate(0 5)"/>
  <path d="{COMPANION_PATH}" fill="url(#companion)"/>
"""
        body = f"""<svg xmlns="http://www.w3.org/2000/svg" width="1024" height="1024" viewBox="0 0 1024 1024" role="img">
  <title>Aster</title>
  <desc>A bevelled silver four-point star with an obsidian diamond core and a wrapped metallic orbit, on a softly lit graphite squircle plate.</desc>
  <defs>
    <path id="plate" d="{plate}"/>
    <linearGradient id="plate" x1="340" y1="90" x2="700" y2="960" gradientUnits="userSpaceOnUse">
      <stop stop-color="{plate_top}"/><stop offset=".5" stop-color="#171717"/><stop offset="1" stop-color="{plate_bottom}"/>
    </linearGradient>
    <linearGradient id="gloss" x1="512" y1="100" x2="512" y2="600" gradientUnits="userSpaceOnUse">
      <stop stop-color="#FFFFFF" stop-opacity=".14"/><stop offset="1" stop-color="#FFFFFF" stop-opacity="0"/>
    </linearGradient>
    <radialGradient id="plate-light" cx="260" cy="170" r="720" gradientUnits="userSpaceOnUse">
      <stop stop-color="#FFFFFF" stop-opacity=".16"/><stop offset=".48" stop-color="#FFFFFF" stop-opacity=".045"/><stop offset="1" stop-color="#FFFFFF" stop-opacity="0"/>
    </radialGradient>
    <radialGradient id="vignette" cx="512" cy="460" r="590" gradientUnits="userSpaceOnUse">
      <stop offset=".46" stop-color="#000000" stop-opacity="0"/><stop offset="1" stop-color="#000000" stop-opacity=".34"/>
    </radialGradient>
    <linearGradient id="rim" x1="210" y1="120" x2="820" y2="920" gradientUnits="userSpaceOnUse">
      <stop stop-color="#FFFFFF" stop-opacity=".68"/><stop offset=".42" stop-color="#FFFFFF" stop-opacity=".2"/><stop offset="1" stop-color="#FFFFFF" stop-opacity=".07"/>
    </linearGradient>
    <linearGradient id="star-sheen" x1="332" y1="280" x2="684" y2="732" gradientUnits="userSpaceOnUse">
      <stop stop-color="#FFFFFF" stop-opacity=".34"/><stop offset=".38" stop-color="#FFFFFF" stop-opacity=".035"/><stop offset=".72" stop-color="#000000" stop-opacity=".035"/><stop offset="1" stop-color="#000000" stop-opacity=".16"/>
    </linearGradient>
    <linearGradient id="star-edge" x1="390" y1="190" x2="630" y2="840" gradientUnits="userSpaceOnUse">
      <stop stop-color="#FFFFFF" stop-opacity=".95"/><stop offset=".52" stop-color="#FFFFFF" stop-opacity=".3"/><stop offset="1" stop-color="#FFFFFF" stop-opacity=".42"/>
    </linearGradient>
    <linearGradient id="core" x1="478" y1="440" x2="548" y2="610" gradientUnits="userSpaceOnUse">
      <stop stop-color="#343434"/><stop offset=".44" stop-color="#151515"/><stop offset="1" stop-color="#000000"/>
    </linearGradient>
    <linearGradient id="star" x1="440" y1="170" x2="600" y2="840" gradientUnits="userSpaceOnUse">
      <stop stop-color="#FFFFFF"/><stop offset=".48" stop-color="{star_mid}"/><stop offset="1" stop-color="{star_dark}"/>
    </linearGradient>
    <linearGradient id="orbit" x1="170" y1="700" x2="860" y2="330" gradientUnits="userSpaceOnUse">
      <stop stop-color="#666666"/><stop offset=".26" stop-color="#D4D4D4"/><stop offset=".5" stop-color="#FFFFFF"/><stop offset=".78" stop-color="#8C8C8C"/><stop offset="1" stop-color="#E5E5E5"/>
    </linearGradient>
    <linearGradient id="orbit-front" x1="170" y1="700" x2="860" y2="330" gradientUnits="userSpaceOnUse">
      <stop stop-color="#8A8A8A"/><stop offset=".36" stop-color="#F5F5F5"/><stop offset=".6" stop-color="#FFFFFF"/><stop offset="1" stop-color="#A3A3A3"/>
    </linearGradient>
{companion_definition}    <radialGradient id="halo"><stop stop-color="#FFFFFF" stop-opacity="{halo_opacity}"/><stop offset="1" stop-color="#FFFFFF" stop-opacity="0"/></radialGradient>
    <radialGradient id="lift"><stop stop-color="#000000" stop-opacity=".34"/><stop offset="1" stop-color="#000000" stop-opacity="0"/></radialGradient>
  </defs>
  <use href="#plate" fill="url(#plate)"/>
  <use href="#plate" fill="url(#gloss)"/>
  <use href="#plate" fill="url(#plate-light)"/>
  <use href="#plate" fill="url(#vignette)"/>
  <circle cx="512" cy="500" r="410" fill="url(#halo)"/>
  <g transform="rotate({ORBIT_ROTATION} {ORBIT_CX} {ORBIT_CY})">
    <ellipse cx="{ORBIT_CX}" cy="{ORBIT_CY}" rx="{ORBIT_RX}" ry="{ORBIT_RY}" fill="none" stroke="url(#orbit)" stroke-width="{orbit_stroke}"/>
  </g>
{companion}  <ellipse cx="512" cy="548" rx="250" ry="250" fill="url(#lift)"/>
  <g transform="{scaled_about_canvas(star_scale, STAR_CX, STAR_CY)}">
        <path d="{STAR_PATH}" fill="#000000" fill-opacity=".22" transform="translate(0 7)"/>
    <path d="{STAR_PATH}" fill="url(#star)"/>
    <path d="{STAR_PATH}" fill="url(#star-sheen)"/>
    <path d="{STAR_PATH}" fill="none" stroke="url(#star-edge)" stroke-width="3"/>

    <g transform="{scaled_about_canvas(core_scale, 512, STAR_CY)}">
      <path d="{CORE_PATH}" fill="url(#core)" stroke="#FFFFFF" stroke-opacity=".14" stroke-width="2"/>
    </g>
  </g>
  <g transform="rotate({ORBIT_ROTATION} {ORBIT_CX} {ORBIT_CY})">
    <path d="M{ORBIT_CX - ORBIT_RX} {ORBIT_CY}A{ORBIT_RX} {ORBIT_RY} 0 0 0 {ORBIT_CX + ORBIT_RX} {ORBIT_CY}" fill="none" stroke="#000000" stroke-opacity=".2"
      stroke-width="{orbit_stroke + 8}" transform="translate(0 5)"/>
    <path d="M{ORBIT_CX - ORBIT_RX} {ORBIT_CY}A{ORBIT_RX} {ORBIT_RY} 0 0 0 {ORBIT_CX + ORBIT_RX} {ORBIT_CY}" fill="none" stroke="url(#orbit-front)" stroke-width="{orbit_stroke}"/>
  </g>
  <use href="#plate" fill="none" stroke="url(#rim)" stroke-opacity="1" stroke-width="5"/>
</svg>
"""
    return "\n".join(line.rstrip() for line in body.splitlines()) + "\n"


TARGETS = {
    "app-icon.svg": build_svg("full"),
    "app-icon-small.svg": build_svg("small"),
    "app-icon-micro.svg": build_svg("micro"),
}


def main(argv: list[str]) -> int:
    check = "--check" in argv
    stale = []
    for name, body in TARGETS.items():
        path = TENANT_DIR / name
        if check:
            current = path.read_text(encoding="utf-8") if path.is_file() else ""
            if current != body:
                stale.append(name)
            continue
        path.write_text(body, encoding="utf-8")
        print(f"wrote {path.relative_to(REPO)}")
    if stale:
        print(
            "stale app-icon sources (run `python3 scripts/app-icon.py`): "
            + ", ".join(stale),
            file=sys.stderr,
        )
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
