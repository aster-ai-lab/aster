"""Hermetic package sealing checks; no application is installed."""

import importlib.util
import json
import pathlib
import plistlib
import tempfile
import unittest

SOURCE = pathlib.Path(__file__).resolve().parents[1] / "studio_delivery.py"
SPEC = importlib.util.spec_from_file_location("studio_delivery", SOURCE)
DELIVERY = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(DELIVERY)


class StudioDeliveryTest(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = pathlib.Path(self.temp.name)
        self.app = self.root / "ToWork.app"
        for name in ("MacOS/Studio", "Resources/agent/a3s-code-acp", "Resources/agent/a3s-office"):
            path = self.app / "Contents" / name
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes(name.encode())
        self.info = {"CFBundleExecutable": "Studio", "CFBundleIdentifier": "dev.a3s.studio",
                     "CFBundleShortVersionString": "2.1.4", "CFBundleName": "ToWork",
                     "CFBundleDisplayName": "ToWork"}
        self.write_info()
        self.renderer = self.root / "renderer"
        index = self.renderer / "windows/main/index.html"
        index.parent.mkdir(parents=True)
        index.write_text('<html data-tenant="toyou" data-a3s-product="ToWork">Studio</html>')
        tenant = self.root / "tenant/toyou/INFO.md"
        tenant.parent.mkdir(parents=True)
        tenant.write_text("tenant")
        aster = self.root / "tenant/aster/INFO.md"
        aster.parent.mkdir(parents=True)
        aster.write_text("aster")
        for slug in ("aster", "toyou"):
            for name in ("logo-light.png", "mark-light.png", "icon.png"):
                (self.root / "tenant" / slug / name).write_bytes(b"fixture PNG")

    def write_info(self):
        (self.app / "Contents/Info.plist").write_bytes(plistlib.dumps(self.info))

    def test_complete_generation_is_sealed_with_tenant_and_renderer(self):
        marker = DELIVERY.seal(self.app, self.renderer, "toyou", self.root / "tenant")
        data = json.loads(marker.read_text())
        self.assertEqual((data["product"], data["version"], data["tenant"]), ("a3s-studio", "2.1.4", "toyou"))
        self.assertIn("Contents/Resources/agent/a3s-code-acp", data["files"])
        self.assertIn("Contents/Resources/studio-frontend/windows/main/index.html", data["files"])
        self.assertNotIn("Contents/MacOS/Studio", data["files"])
        self.assertNotIn("Contents/Resources/studio-delivery.json", data["files"])

    def test_aster_package_uses_its_own_identity_and_brand(self):
        self.info.update(CFBundleIdentifier="dev.a3s.studio.aster", CFBundleName="AsterDream",
                         CFBundleDisplayName="AsterDream")
        self.write_info()
        (self.renderer / "windows/main/index.html").write_text('<html data-tenant="aster" data-a3s-product="AsterDream"></html>')
        marker = DELIVERY.seal(self.app, self.renderer, "aster", self.root / "tenant")
        data = json.loads(marker.read_text())
        self.assertEqual(data["tenant"], "aster")
        self.assertTrue((self.app / "Contents/Resources/tenant/aster/INFO.md").is_file())
        self.assertFalse((self.app / "Contents/Resources/tenant/toyou").exists())

    def test_toyou_bundle_cannot_be_sealed_as_aster(self):
        with self.assertRaisesRegex(ValueError, "identity"):
            DELIVERY.seal(self.app, self.renderer, "aster", self.root / "tenant")

    def test_known_bundle_cannot_be_sealed_under_unknown_product(self):
        with self.assertRaisesRegex(ValueError, "product tenant"):
            DELIVERY.seal(self.app, self.renderer, "a3s", self.root / "tenant")

    def test_each_native_required_brand_asset_refuses_sealing_when_missing(self):
        for name in ("logo-light.png", "mark-light.png", "icon.png"):
            with self.subTest(name=name):
                asset = self.root / "tenant/toyou" / name
                data = asset.read_bytes()
                asset.unlink()
                try:
                    with self.assertRaisesRegex(ValueError, "brand asset"):
                        DELIVERY.seal(self.app, self.renderer, "toyou", self.root / "tenant")
                finally:
                    asset.write_bytes(data)

    def test_foreign_renderer_cannot_be_sealed_under_selected_tenant(self):
        (self.renderer / "windows/main/index.html").write_text('<html data-tenant="aster" data-a3s-product="AsterDream"></html>')
        with self.assertRaisesRegex(ValueError, "renderer identity"):
            DELIVERY.seal(self.app, self.renderer, "toyou", self.root / "tenant")

    def test_same_bundle_identifier_does_not_accept_work(self):
        self.info["CFBundleExecutable"] = "aster-dream"
        self.write_info()
        with self.assertRaisesRegex(ValueError, "not a Studio"):
            DELIVERY.seal(self.app, self.renderer, "toyou", self.root / "tenant")

    def test_missing_companion_or_symlink_refuses_sealing(self):
        companion = self.app / "Contents/Resources/agent/a3s-office"
        companion.unlink()
        with self.assertRaisesRegex(ValueError, "missing Studio"):
            DELIVERY.seal(self.app, self.renderer, "toyou", self.root / "tenant")
        companion.symlink_to(self.app / "Contents/MacOS/Studio")
        with self.assertRaises(ValueError):
            DELIVERY.seal(self.app, self.renderer, "toyou", self.root / "tenant")

    def test_input_symlink_and_marker_symlink_cannot_copy_or_overwrite_external_bytes(self):
        outside = self.root / "outside"
        outside.write_text("private fixture")
        link = self.renderer / "external"
        link.symlink_to(outside)
        with self.assertRaisesRegex(ValueError, "invalid Studio resource"):
            DELIVERY.seal(self.app, self.renderer, "toyou", self.root / "tenant")
        link.unlink()
        marker = self.app / "Contents/Resources/studio-delivery.json"
        marker.symlink_to(outside)
        with self.assertRaisesRegex(ValueError, "symlink delivery marker"):
            DELIVERY.seal(self.app, self.renderer, "toyou", self.root / "tenant")
        self.assertEqual(outside.read_text(), "private fixture")


if __name__ == "__main__":
    unittest.main()
