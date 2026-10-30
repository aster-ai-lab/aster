"""Keep the Aster app icon shippable: sources current, rasters consistent, identity shared.

The icon is generated (`scripts/app-icon.py`, `scripts/app-icon-assets.py`) rather
than hand-drawn per size, so the committed files can drift from the generator and
still look plausible in a diff. These checks make the drift visible without a
rasteriser or a node install: `--check` reads what is on disk.
"""

from __future__ import annotations

import json
import runpy
import subprocess
import sys
import unittest
import xml.etree.ElementTree as ET
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SCRIPTS = ROOT / "scripts"
TENANT = ROOT / "apps" / "web" / "public" / "tenant" / "aster"
TAURI_ROOT = ROOT / "apps" / "studio" / "src-tauri"

sys.path.insert(0, str(ROOT / "apps" / "studio" / "scripts"))
from studio_product import desktop_entry, identity  # noqa: E402


def run(script: str, *arguments: str) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        [sys.executable, str(SCRIPTS / script), *arguments],
        capture_output=True,
        text=True,
        check=False,
    )


class IconSources(unittest.TestCase):
    def test_generator_uses_the_approved_graphite_palette(self) -> None:
        generated = runpy.run_path(str(SCRIPTS / "app-icon.py"))["TARGETS"]
        for name, source in generated.items():
            colors = [
                value
                for element in ET.fromstring(source).iter()
                for value in element.attrib.values()
                if value.startswith("#") and len(value) == 7
            ]
            self.assertTrue(colors, name)
            for color in colors:
                self.assertEqual(color[1:3], color[3:5], (name, color))
                self.assertEqual(color[3:5], color[5:7], (name, color))

    def test_svgs_match_the_generator(self) -> None:
        result = run("app-icon.py", "--check")
        self.assertEqual(result.returncode, 0, result.stderr)

    def test_every_source_is_a_1024_canvas(self) -> None:
        for name in ("app-icon.svg", "app-icon-small.svg", "app-icon-micro.svg"):
            text = (TENANT / name).read_text(encoding="utf-8")
            self.assertIn('viewBox="0 0 1024 1024"', text, name)
            # The plate keeps the platform's optical margin: no drawing reaches
            # the canvas edge, which is what stops an icon from looking clipped.
            self.assertNotIn('x="0" y="0" width="1024"', text, name)


class IconRasters(unittest.TestCase):
    def test_rasters_match_the_sources(self) -> None:
        result = run("app-icon-assets.py", "--check")
        self.assertEqual(result.returncode, 0, result.stderr)

    def test_ladder_covers_the_scales_a_dock_asks_for(self) -> None:
        # 1x through 8x for the sizes panels, launchers, taskbars and Finder
        # request; the container formats index into the same frames.
        present = {
            int(path.stem.split("x")[0])
            for path in (TENANT / "icons").glob("[0-9]*x[0-9]*.png")
        }
        self.assertLessEqual({16, 24, 32, 48, 64, 128, 256, 512, 1024}, present)

    def test_every_scale_is_drawn_not_resampled(self) -> None:
        # Each band has its own source drawing, so the smallest frame is not a
        # downscale of the master. Gloss, the orbit's multi-stop gradient and the
        # contact shadow are dropped below 48 px where they read as noise, and
        # the orbit is drawn heavier so it keeps a pixel of coverage at 16 px.
        micro = (TENANT / "app-icon-micro.svg").read_text(encoding="utf-8")
        master = (TENANT / "app-icon.svg").read_text(encoding="utf-8")
        for dropped in ("url(#gloss)", "url(#orbit)", "url(#lift)"):
            self.assertNotIn(dropped, micro)
            self.assertIn(dropped, master)
        self.assertIn('stroke-width="36"', micro)
        self.assertIn('stroke-width="22"', master)


class BundleIconList(unittest.TestCase):
    def test_bundled_icons_exist(self) -> None:
        config = json.loads((TAURI_ROOT / "tauri.conf.json").read_text(encoding="utf-8"))
        icons = config["bundle"]["icon"]
        missing = [entry for entry in icons if not (TAURI_ROOT / entry).is_file()]
        self.assertEqual(missing, [])

    def test_bundle_carries_a_size_ladder(self) -> None:
        # Debian and AppImage install every bundled PNG into the hicolor theme
        # at that PNG's own size; a single frame would be scaled for the rest.
        config = json.loads((TAURI_ROOT / "tauri.conf.json").read_text(encoding="utf-8"))
        ladder = [entry for entry in config["bundle"]["icon"] if "/icons/" in entry]
        self.assertIn("16x16.png", " ".join(ladder))
        self.assertIn("256x256.png", " ".join(ladder))


class DesktopEntryIdentity(unittest.TestCase):
    def test_entry_matches_the_claimed_window_class(self) -> None:
        # A dock only finds the entry — and the icon — when the entry name, the
        # icon name and `StartupWMClass` are the identifier the host claims. That
        # claim is two calls, because GDK rewrites the class field during
        # `gtk_init`; `tests/desktop_window_class.py` holds the claimed class
        # against this entry on a real display.
        identifier = identity("aster")["identifier"]
        entry = desktop_entry("aster")
        self.assertIn(f"StartupWMClass={identifier}", entry)
        self.assertIn(f"Icon={identifier}", entry)
        self.assertNotIn("StartupWMClass=Studio", entry)

    def test_host_claims_the_same_identifier(self) -> None:
        # Wayland takes the window's app id from the GTK application id, while
        # X11 takes WM_CLASS from the glib program name. The host sets both from
        # the bundle identifier, so the config has to publish it to GTK and the
        # identifier has to be the one the desktop entry is named after.
        config = json.loads((TAURI_ROOT / "tauri.conf.json").read_text(encoding="utf-8"))
        self.assertTrue(config["app"]["enableGTKAppId"])
        self.assertEqual(config["identifier"], identity("aster")["identifier"])

    def test_entry_names_one_main_category(self) -> None:
        categories = next(
            line.split("=", 1)[1]
            for line in desktop_entry("aster").splitlines()
            if line.startswith("Categories=")
        )
        main = {"AudioVideo", "Audio", "Video", "Development", "Education", "Game",
                "Graphics", "Network", "Office", "Science", "Settings", "System",
                "Utility"}
        self.assertEqual(len([item for item in categories.split(";") if item in main]), 1)


if __name__ == "__main__":
    unittest.main()
