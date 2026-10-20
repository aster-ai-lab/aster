"""Hermetic generation/layout/input-environment contracts; not a native GTK proof."""

import hashlib
import importlib.util
import json
import os
from pathlib import Path
import shutil
import struct
import subprocess
import tempfile
import unittest
from unittest.mock import patch

HERE = Path(__file__).resolve().parent
SPEC = importlib.util.spec_from_file_location("studio_kylin_delivery", HERE / "delivery.py")
delivery = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(delivery)
ELF = b"\x7fELF\x02\x01" + b"\x00" * 12 + struct.pack("<H", 62) + b"fixture"


class DeliveryTests(unittest.TestCase):
    def setUp(self):
        self.scope = tempfile.TemporaryDirectory()
        self.addCleanup(self.scope.cleanup)
        self.root = Path(self.scope.name)
        self.build, self.renderer, self.tenants, self.im = [self.root / name for name in (
            "build", "renderer", "tenants", "im")]
        self.out = self.root / "staged"
        for name in ("Studio", "a3s-code-acp", "a3s-office"):
            self.put(self.build / name, ELF, executable=True)
        for name in delivery.TRAY_LIBS:
            self.put(self.build / "native-libs" / name, ELF)
        for name in delivery.TRAY_LICENSES:
            self.put(self.build / f"native-licenses/{name}/copyright", b"test license")
        self.put(self.build / "NotoSansHans-Regular.otf", b"test font")
        self.put(self.build / "font-LICENSE", b"test font license")
        self.put(self.build / "icon-128.png", b"\x89PNG\r\n\x1a\n\x00\x00\x00\rIHDR" + struct.pack(">II", 128, 128))
        self.put(self.renderer / "windows/main/index.html", b"<p>Studio fixture</p>")
        for name in delivery.BRAND_FILES:
            self.put(self.tenants / "test" / name, b"test brand asset")
        self.put(self.tenants / "test/model-market.json", b"must not ship")
        for name in delivery.IM_FILES:
            data = ELF if name in delivery.R4_BINARIES else b"fixture input"
            self.put(self.im / name, data, executable=name in delivery.R4_BINARIES)
        for version, binary in (("fcitx4", "im-fcitx.so"), ("fcitx5", "im-fcitx5.so")):
            src = self.im / "immodules" / version
            self.put(src / "query-path", f"/build/{binary}\n".encode())
            self.put(src / "module-id", b"fcitx\n")
            self.put(src / "immodules.cache", f'"/build/{binary}"\n"fcitx" "Fcitx" "fcitx" "*" "en:zh"\n'.encode())
        shutil.copy2(HERE / "fcitx4_legacy_portal.patch", self.im / "immodules/fcitx4/fcitx4_legacy_portal.patch")
        # Archive bytes are an external build input; unit tests do not qualify real upstream source.
        self.sources = patch.multiple(delivery,
            FCITX4_SHA=delivery.digest(self.im / "immodules/fcitx4/fcitx-4.2.9.9.tar.xz"),
            FCITX5_SHA=delivery.digest(self.im / "immodules/fcitx5/fcitx5-gtk-5.1.7.tar.gz"))
        self.sources.start()
        self.addCleanup(self.sources.stop)
        delivery.record_ime(self.im, "synthetic test fixtures")
        delivery.build_receipt(self.build, self.renderer, "test", "2.1.3", "fixture-revision")

    @staticmethod
    def put(path, data, executable=False):
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(data)
        path.chmod(0o755 if executable else 0o644)

    def stage(self):
        delivery.stage(self.build, self.renderer, self.tenants, self.im, self.out)

    def test_complete_generation_has_studio_layout_and_independent_cache_paths(self):
        self.stage()
        result = delivery.verify(self.out)
        self.assertEqual(result["product"], "dev.a3s.studio")
        self.assertEqual((self.out / "bin/agent/a3s-code-acp").read_bytes(), ELF)
        self.assertEqual((self.out / "bin/a3s-office").read_bytes(), ELF)
        self.assertEqual((self.out / "Resources/studio-tenant").read_text(), "test\n")
        self.assertFalse((self.out / "Resources/tenant/test/model-market.json").exists())
        for version, module in (("fcitx4", "im-fcitx.so"), ("fcitx5", "im-fcitx5.so")):
            cache = (self.out / f"lib/im/{version}/immodules.cache").read_text()
            self.assertIn(f'"/app/lib/im/{version}/{module}"', cache)
            self.assertNotIn("/build/", cache)

    def test_each_companion_or_host_from_another_generation_is_refused(self):
        for name in ("Studio", "a3s-code-acp", "a3s-office"):
            with self.subTest(name=name):
                self.put(self.build / name, ELF + b"different generation", executable=True)
                with self.assertRaisesRegex(ValueError, "binary generation changed"):
                    self.stage()
                self.put(self.build / name, ELF, executable=True)
                self.assertFalse(self.out.exists())

    def test_renderer_changes_after_host_build_are_refused(self):
        self.put(self.renderer / "windows/main/index.html", b"new renderer")
        with self.assertRaisesRegex(ValueError, "renderer generation changed"):
            self.stage()

    def test_missing_input_method_asset_is_refused(self):
        (self.im / "immodules/fcitx4/libfcitx-gclient.so.1").unlink()
        with self.assertRaises(OSError):
            self.stage()

    def test_wrong_elf_and_modified_corresponding_source_are_refused(self):
        self.put(self.build / "Studio", b"Mach-O")
        with self.assertRaisesRegex(ValueError, "not a Linux x86_64 ELF"):
            self.stage()
        self.put(self.build / "Studio", ELF, executable=True)
        self.put(self.im / "immodules/fcitx4/fcitx-4.2.9.9.tar.xz", b"changed archive")
        with self.assertRaisesRegex(ValueError, "wrong corresponding source"):
            self.stage()

    def test_cache_registration_must_match_its_module_id(self):
        self.put(self.im / "immodules/fcitx4/module-id", b"wrong-id\n")
        delivery.record_ime(self.im)
        with self.assertRaisesRegex(ValueError, "module id does not match cache"):
            self.stage()

    def test_launch_verification_refuses_missing_tampered_extra_or_symlink_files(self):
        for mutation in ("missing", "tampered", "extra", "symlink", "not executable"):
            with self.subTest(mutation=mutation):
                self.stage()
                path = self.out / "bin/agent/a3s-code-acp"
                if mutation == "missing":
                    path.unlink()
                elif mutation == "tampered":
                    self.put(path, ELF + b"changed", executable=True)
                elif mutation == "extra":
                    self.put(self.out / "extra", b"unlisted")
                elif mutation == "not executable":
                    path.chmod(0o644)
                else:
                    path.unlink()
                    path.symlink_to(self.build / "a3s-code-acp")
                with self.assertRaises((ValueError, OSError)):
                    delivery.verify(self.out)
                shutil.rmtree(self.out)

    def test_foreign_generation_and_checksum_changes_are_refused(self):
        self.stage()
        marker = self.out / "studio-generation.json"
        data = json.loads(marker.read_text())
        data["product"] = "dev.a3s.AsterDream"
        marker.write_text(json.dumps(data))
        with self.assertRaisesRegex(ValueError, "not a Studio generation"):
            delivery.verify(self.out)

    def decision(self, owners, version=None, backend="x11"):
        probe = self.root / "probe"
        self.put(probe, f"#!/bin/sh\nprintf '%s\\n' {backend}\n".encode(), executable=True)
        env = {**os.environ, "KYLIN_IM_BUS_OWNERS": owners, "KYLIN_IM_BACKEND_PROBE": str(probe),
               "KYLIN_IM_MODULE_ROOT": str(self.im / "immodules"), "DISPLAY": ":12.0"}
        result = subprocess.run(["bash", str(HERE / "kylin-launch.sh"), "--print"],
            env=env, check=True, text=True, capture_output=True, timeout=3)
        return dict(line.split("=", 1) for line in result.stdout.splitlines())

    def test_native_daemon_identity_selects_only_its_own_module(self):
        for owner, version in (("org.fcitx.Fcitx5", "fcitx5"), ("org.fcitx.Fcitx-12", "fcitx4")):
            result = self.decision(owner)
            self.assertEqual(result["GTK_IM_MODULE"], "fcitx")
            self.assertEqual(result["GTK_IM_MODULE_FILE"], str(self.im / f"immodules/{version}/immodules.cache"))
        result = self.decision("org.freedesktop.IBus")
        self.assertEqual(result["GTK_IM_MODULE"], "ibus")
        (self.im / "immodules/fcitx4/module-id").unlink()
        result = self.decision("org.fcitx.Fcitx-12 org.freedesktop.IBus")
        self.assertEqual(result["GTK_IM_MODULE"], "")
        self.assertEqual(result["NOTE"], "missing-fcitx4-module")

    def test_wayland_does_not_force_an_x11_module(self):
        self.assertEqual(self.decision("org.fcitx.Fcitx5", backend="wayland")["GTK_IM_MODULE"], "")


if __name__ == "__main__":
    unittest.main()
