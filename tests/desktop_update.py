#!/usr/bin/env python3
"""Release-feed regressions: missing assets must never become an advertised update."""

import copy
import importlib.util
import io
import json
from pathlib import Path
import shutil
import tempfile
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[1]
SPEC = importlib.util.spec_from_file_location("prepare_update", ROOT / "apps/work/scripts/prepare-update.py")
UPDATE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(UPDATE)


class DesktopUpdateTest(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.archive = Path(self.temp.name) / "AsterDream.app.tar.gz"
        fixtures = ROOT / "apps/work/src-tauri/tests/fixtures/updater"
        shutil.copyfile(fixtures / self.archive.name, self.archive)
        shutil.copyfile(fixtures / (self.archive.name + ".sig"), str(self.archive) + ".sig")
        self.config = json.loads(UPDATE.CONFIG.read_text())
        self.config["version"] = "0.1.2"
        self.release = {
            "tag_name": "aster-v0.1.2", "draft": False, "published_at": "2026-10-20T00:00:00Z",
            "assets": [{"name": self.archive.name, "size": self.archive.stat().st_size,
                        "digest": f"sha256:{UPDATE.sha256(self.archive)}",
                        "browser_download_url": "https://github.com/aster-ai-lab/aster/releases/download/aster-v0.1.2/AsterDream.app.tar.gz"}],
        }

    def test_missing_compatibility_tag_or_archive_is_not_advertised(self):
        for change in [{"tag_name": "v0.1.2"}, {"assets": []}, {"draft": True}]:
            release = {**self.release, **change}
            with self.subTest(change=change), self.assertRaises(ValueError):
                UPDATE.manifest_for(self.archive, "修复更新", self.config, release)

    def test_mismatched_uploaded_bytes_and_old_client_key_are_rejected(self):
        release = copy.deepcopy(self.release)
        release["assets"][0]["digest"] = "sha256:" + "0" * 64
        with self.assertRaises(ValueError):
            UPDATE.manifest_for(self.archive, "修复更新", self.config, release)
        self.config["plugins"]["updater"]["pubkey"] = "a different product key"
        with self.assertRaises(ValueError):
            UPDATE.manifest_for(self.archive, "修复更新", self.config, self.release)

    def test_verified_upload_produces_old_client_compatible_url_and_signature(self):
        manifest = UPDATE.manifest_for(self.archive, "修复更新", self.config, self.release)
        platform = manifest["platforms"]["darwin-aarch64"]
        self.assertEqual(manifest["version"], "0.1.2")
        self.assertEqual(platform["url"], self.release["assets"][0]["browser_download_url"])
        self.assertEqual(platform["signature"], Path(str(self.archive) + ".sig").read_text().strip())

    def test_release_gate_requires_macos_before_accessing_the_release(self):
        with patch.object(UPDATE.sys, "platform", "linux"):
            with self.assertRaisesRegex(SystemExit, "macOS"):
                UPDATE.main()

    def test_public_download_requires_exact_bytes_and_size(self):
        content = self.archive.read_bytes()
        changed = content[:-1] + bytes([content[-1] ^ 1])
        for downloaded in [content, content[:-1], changed, content + b"extra"]:
            with self.subTest(bytes=len(downloaded)), patch.object(
                UPDATE.urllib.request, "urlopen", return_value=io.BytesIO(downloaded)
            ):
                if downloaded == content:
                    UPDATE.verify_download("https://example.invalid/update", self.archive)
                else:
                    with self.assertRaises(ValueError):
                        UPDATE.verify_download("https://example.invalid/update", self.archive)

    def test_zero_exit_or_wrong_version_cannot_replace_native_upgrade_verification(self):
        notes = Path(self.temp.name) / "notes.txt"
        notes.write_text("修复更新")
        config = Path(self.temp.name) / "config.json"
        config.write_text(json.dumps(self.config))
        output = Path(self.temp.name) / "latest.json"
        argv = ["prepare-update.py", str(self.archive), "--notes", str(notes),
                "--old-app", str(Path(self.temp.name) / "old.app"), "--output", str(output)]
        for version in [None, "0.1.1", "0.1.2"]:
            def native_check(*args, **kwargs):
                if version is not None:
                    Path(kwargs["env"]["ASTER_UPDATE_VERIFIED"]).write_text(version)

            with self.subTest(version=version), patch.object(UPDATE, "CONFIG", config), \
                    patch.object(UPDATE.sys, "platform", "darwin"), patch.object(UPDATE.sys, "argv", argv), \
                    patch.object(UPDATE.urllib.request, "urlopen", return_value=io.BytesIO(json.dumps(self.release).encode())), \
                    patch.object(UPDATE, "verify_download"), patch.object(UPDATE.subprocess, "run", side_effect=native_check):
                if version == "0.1.2":
                    UPDATE.main()
                    self.assertEqual(json.loads(output.read_text())["version"], version)
                else:
                    with self.assertRaisesRegex(ValueError, "did not verify"):
                        UPDATE.main()
                    self.assertFalse(output.exists())


if __name__ == "__main__":
    unittest.main()
