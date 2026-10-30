"""Keep the Aster app icon shippable: sources current, rasters consistent, identity shared.

All exports come from logo.png through the existing icon scripts. These checks
detect source and export drift without a rasteriser or a node install.
"""

from __future__ import annotations

import base64
import importlib.util
import json
import shutil
import subprocess
import sys
import tempfile
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
    def test_svgs_match_the_generator(self) -> None:
        result = run("app-icon.py", "--check")
        self.assertEqual(result.returncode, 0, result.stderr)

    def test_website_svg_embeds_the_only_master(self) -> None:
        svg = ET.parse(TENANT / "logo.svg").getroot()
        self.assertEqual(svg.attrib["viewBox"], "0 0 1024 1024")
        image = svg.find("{http://www.w3.org/2000/svg}image")
        self.assertIsNotNone(image)
        prefix, payload = image.attrib["href"].split(",", 1)
        self.assertEqual(prefix, "data:image/png;base64")
        self.assertEqual(base64.b64decode(payload), (TENANT / "icon.png").read_bytes())

    def test_retired_drawings_are_removed(self) -> None:
        for name in ("app-icon.svg", "app-icon-small.svg", "app-icon-micro.svg"):
            self.assertFalse((TENANT / name).exists(), name)


class IconRasters(unittest.TestCase):
    def test_rasters_match_the_sources(self) -> None:
        result = run("app-icon-assets.py", "--check")
        self.assertEqual(result.returncode, 0, result.stderr)

    def test_generation_receipt_detects_a_changed_tray_export(self) -> None:
        spec = importlib.util.spec_from_file_location("app_icon_assets", SCRIPTS / "app-icon-assets.py")
        assets = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(assets)
        with tempfile.TemporaryDirectory() as scratch:
            assets.TENANT_DIR = Path(scratch) / "aster"
            assets.ICON_DIR = assets.TENANT_DIR / "icons"
            shutil.copytree(TENANT, assets.TENANT_DIR)
            self.assertEqual(assets.verify(), [])
            tray = assets.TENANT_DIR / "tray.png"
            tray.write_bytes(tray.read_bytes() + b"changed")
            self.assertTrue(any("generation receipt" in error for error in assets.verify()))

    def test_ladder_covers_the_scales_a_dock_asks_for(self) -> None:
        # 1x through 8x for the sizes panels, launchers, taskbars and Finder
        # request; the container formats index into the same frames.
        present = {
            int(path.stem.split("x")[0])
            for path in (TENANT / "icons").glob("[0-9]*x[0-9]*.png")
        }
        self.assertLessEqual({16, 24, 32, 48, 64, 128, 256, 512, 1024}, present)

    def test_brand_exports_share_the_same_artwork(self) -> None:
        self.assertEqual((TENANT / "icon.png").read_bytes(),
                         (TENANT / "icons/1024x1024.png").read_bytes())
        for name in ("logo-light.png", "logo-dark.png"):
            self.assertEqual((TENANT / name).read_bytes(),
                             (TENANT / "icons/512x512.png").read_bytes(), name)
        for name in ("mark-light.png", "mark-dark.png"):
            self.assertEqual((TENANT / name).read_bytes(),
                             (TENANT / "icons/256x256.png").read_bytes(), name)
        self.assertEqual((TENANT / "favicon.png").read_bytes(),
                         (TENANT / "icons/32x32.png").read_bytes())


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
