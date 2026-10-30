#!/usr/bin/env python3
"""Build the Aster application-icon sources from one parameterised mark.

A desktop icon is the one brand surface that gets scaled down to 16 px, and a
single drawing cannot serve both ends of that range: a 22 px orbit stroke and a
40 px companion star read as grey aliasing in a tray, while a 40 px stroke reads
as a toy at 1024 px. The mark is therefore generated as a three-step ladder:

  * `app-icon.svg`       — 128 px and above. Full detail: rim light, gloss,
    companion star, three-stop gradients.
  * `app-icon-small.svg` — 48–80 px. Heavier orbit, larger star, no companion.
  * `app-icon-micro.svg` — 16–40 px. Plate and star only; the orbit and every
    gradient below the plate are dropped so the silhouette stays a star.

Each file is written into `apps/web/public/tenant/aster/`, the tenant directory
that owns Aster branding: `tauri.conf.json` bundles `icon.png`, `icon.icns` and
`icon.ico` from beside them, and `scripts/generate-app-icons.sh` rasterises the
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

BRAND_BLUE = "#005ABB"
BRAND_BLUE_LIGHT = "#2088E6"
BRAND_BLUE_DEEP = "#042C5C"

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
    micro = detail == "micro"
    small = detail == "small"

    plate = squircle_path(PLATE_SIZE, PLATE_EXPONENT)
    star_scale = 1.16 if micro else (1.10 if small else 1.05)
    core_scale = 1.22 if micro else (1.10 if small else 1.08)
    orbit_stroke = 36 if micro else (30 if small else 22)

    if micro:
        plate_top, plate_mid, plate_bottom = "#2489E3", BRAND_BLUE, "#0A3F7C"
        star_light, star_mid, star_dark = "#FFFFFF", "#F7FAFE", "#DCE7F5"
        halo_opacity = ".35"
    elif small:
        plate_top, plate_mid, plate_bottom = "#2C8BE4", BRAND_BLUE, "#0A4180"
        star_light, star_mid, star_dark = "#FFFFFF", "#F2F7FD", "#CBD9EA"
        halo_opacity = ".5"
    else:
        plate_top, plate_mid, plate_bottom = (
            BRAND_BLUE_LIGHT,
            BRAND_BLUE,
            BRAND_BLUE_DEEP,
        )
        star_light, star_mid, star_dark = "#FFFFFF", "#E1EAF6", "#A7B8CB"
        halo_opacity = ".42"

    # Detail that only survives above ~96 px; below that it is noise, not identity.
    gloss = (
        ""
        if micro
        else '<use href="#plate" fill="url(#gloss)"/>'
    )
    rim = (
        f'<use href="#plate" fill="none" stroke="#DCEAFF" '
        f'stroke-opacity="{".22" if micro else ".3"}" stroke-width="{6 if micro else 4}"/>'
    )
    halo = (
        ""
        if micro
        else f'<circle cx="512" cy="500" r="410" fill="url(#halo)"/>'
    )
    orbit = (
        f"""  <g transform="rotate({ORBIT_ROTATION} {ORBIT_CX} {ORBIT_CY})">
    <ellipse cx="{ORBIT_CX}" cy="{ORBIT_CY}" rx="{ORBIT_RX}" ry="{ORBIT_RY}" fill="none"
      stroke="#E4EEFB" stroke-opacity="{".42" if micro else "1"}" stroke-width="{orbit_stroke}"/>
  </g>
"""
        if micro
        else f"""  <g transform="rotate({ORBIT_ROTATION} {ORBIT_CX} {ORBIT_CY})">
    <ellipse cx="{ORBIT_CX}" cy="{ORBIT_CY}" rx="{ORBIT_RX}" ry="{ORBIT_RY}" fill="none" stroke="url(#orbit)" stroke-width="{orbit_stroke}"/>
  </g>
"""
    )
    companion = (
        f'<path d="{COMPANION_PATH}" fill="#EAF2FF"/>' if detail == "full" else ""
    )
    lift = (
        ""
        if micro
        else '<ellipse cx="512" cy="548" rx="250" ry="250" fill="url(#lift)"/>'
    )
    orbit_definition = (
        ""
        if micro
        else f"""    <linearGradient id="orbit" x1="170" y1="700" x2="860" y2="330" gradientUnits="userSpaceOnUse">
      <stop stop-color="#7EA2CE"/><stop offset=".42" stop-color="#F2F7FE"/><stop offset="1" stop-color="#AFC4DE"/>
    </linearGradient>
"""
    )
    lift_definition = (
        ""
        if micro
        else """    <radialGradient id="lift"><stop stop-color="#012A55" stop-opacity=".34"/><stop offset="1" stop-color="#012A55" stop-opacity="0"/></radialGradient>
"""
    )
    gloss_definition = (
        ""
        if micro
        else """    <linearGradient id="gloss" x1="512" y1="100" x2="512" y2="600" gradientUnits="userSpaceOnUse">
      <stop stop-color="#FFFFFF" stop-opacity=".18"/><stop offset="1" stop-color="#FFFFFF" stop-opacity="0"/>
    </linearGradient>
"""
    )

    body = f"""<svg xmlns="http://www.w3.org/2000/svg" width="{CANVAS}" height="{CANVAS}" viewBox="0 0 {CANVAS} {CANVAS}" role="img">
  <title>Aster</title>
  <desc>A silver four-point star with a blue diamond core on a tilted orbit, on a blue squircle plate.</desc>
  <defs>
    <path id="plate" d="{plate}"/>
    <linearGradient id="plate" x1="340" y1="90" x2="700" y2="960" gradientUnits="userSpaceOnUse">
      <stop stop-color="{plate_top}"/><stop offset=".5" stop-color="{plate_mid}"/><stop offset="1" stop-color="{plate_bottom}"/>
    </linearGradient>
{gloss_definition}    <linearGradient id="star" x1="440" y1="170" x2="600" y2="840" gradientUnits="userSpaceOnUse">
      <stop stop-color="{star_light}"/><stop offset=".48" stop-color="{star_mid}"/><stop offset="1" stop-color="{star_dark}"/>
    </linearGradient>
{orbit_definition}    <radialGradient id="halo"><stop stop-color="#66B0FF" stop-opacity="{halo_opacity}"/><stop offset="1" stop-color="#66B0FF" stop-opacity="0"/></radialGradient>
{lift_definition}  </defs>
  <use href="#plate" fill="url(#plate)"/>
{gloss}  {rim}
  {halo}
{orbit}  {companion}
  {lift}
  <g transform="{scaled_about_canvas(star_scale, STAR_CX, STAR_CY)}">
    <path d="{STAR_PATH}" fill="url(#star)"/>
    <g transform="{scaled_about_canvas(core_scale, 512, STAR_CY)}">
      <path d="{CORE_PATH}" fill="{BRAND_BLUE}"/>
    </g>
  </g>
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
