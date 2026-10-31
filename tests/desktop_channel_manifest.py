"""Hermetic contract tests for the desktop update channel manifest.

Drives scripts/desktop_release_manifest.py on temp fixtures: generation
digests the archive, regression is refused, and `check` pins the exact
schema the studio host updater parses.
"""

import json
import os
import subprocess
import sys
import tempfile
import textwrap
import unittest
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent
SCRIPT = REPO / "scripts" / "desktop_release_manifest.py"
sys.path.insert(0, str(REPO / "apps/studio/scripts/tests"))
import studio_delivery_test as studio_fixture


class ManifestContractTests(unittest.TestCase):
    def run_script(self, *args):
        return subprocess.run(
            [sys.executable, str(SCRIPT), *args],
            capture_output=True,
            text=True,
            timeout=30,
        )

    def make_archive(self, directory, name="Aster-darwin-aarch64-v0.2.0.zip", version="0.2.0"):
        path = Path(directory) / name
        fixture = studio_fixture.StudioDeliveryTest()
        try:
            fixture.setUp()
            fixture.app = fixture.app.rename(fixture.root / "Aster.app")
            fixture.info.update(CFBundleIdentifier="dev.aster.studio.aster", CFBundleName="Aster",
                                CFBundleDisplayName="Aster", CFBundleShortVersionString=version)
            fixture.write_info()
            (fixture.renderer / "windows/main/index.html").write_text('<html data-tenant="aster" data-aster-product="Aster"></html>')
            studio_fixture.DELIVERY.seal(fixture.app, fixture.renderer, "aster", fixture.root / "tenant")
            studio_fixture.DELIVERY.archive(fixture.app, path)
        finally:
            fixture.doCleanups()
        return path

    def generate(
        self,
        directory,
        version="0.2.0",
        existing=None,
        force=False,
        notes=None,
        notes_en=None,
        notes_zh_cn=None,
    ):
        archive = self.make_archive(directory, name=f"Aster-darwin-aarch64-v{version}.zip", version=version)
        out = Path(directory) / "desktop-latest.json"
        args = [
            "generate",
            "--zip",
            str(archive),
            "--asset",
            archive.name,
            "--version",
            version,
            "--out",
            str(out),
        ]
        if existing:
            args += ["--existing", str(existing)]
        if force:
            args += ["--force"]
        if notes is not None:
            notes_path = Path(directory) / "notes.md"
            notes_path.write_text(notes)
            args += ["--notes", str(notes_path)]
        if notes_en is not None:
            notes_en_path = Path(directory) / "notes-en.md"
            notes_en_path.write_text(notes_en)
            args += ["--notes-en", str(notes_en_path)]
        if notes_zh_cn is not None:
            notes_zh_path = Path(directory) / "notes-zh-cn.md"
            notes_zh_path.write_text(notes_zh_cn)
            args += ["--notes-zh-cn", str(notes_zh_path)]
        result = self.run_script(*args)
        return result, out

    def test_generate_digests_the_archive_into_the_manifest(self):
        with tempfile.TemporaryDirectory() as directory:
            import hashlib

            archive = self.make_archive(directory)
            result, out = self.generate(directory)
            self.assertEqual(result.returncode, 0, result.stderr)
            manifest = json.loads(out.read_text())
            self.assertEqual(manifest["product"], "aster")
            self.assertEqual(manifest["channel"], "stable")
            self.assertEqual(manifest["version"], "0.2.0")
            entry = manifest["platforms"]["darwin-aarch64"]
            self.assertEqual(
                entry["sha256"], hashlib.sha256(archive.read_bytes()).hexdigest()
            )
            self.assertEqual(entry["size"], archive.stat().st_size)
            self.assertEqual(
                entry["url"],
                "https://github.com/aster-ai-lab/aster/releases/download/desktop-stable/"
                + archive.name,
            )
            self.assertTrue(manifest["published_at"].startswith("20"))
            self.assertEqual(manifest["history"], [{"version": "0.2.0", "notes": ""}])

    def test_generate_builds_and_checks_bilingual_release_notes(self):
        with tempfile.TemporaryDirectory() as directory:
            result, out = self.generate(
                directory,
                notes_en="## New Features\n- Added a setting",
                notes_zh_cn="## 新功能\n- 新增一项设置",
            )
            self.assertEqual(result.returncode, 0, result.stderr)
            expected = (
                "<!--LANG:en-->\n## New Features\n- Added a setting\n"
                "<!--LANG:zh-CN-->\n## 新功能\n- 新增一项设置\n<!--LANG:END-->"
            )
            manifest = json.loads(out.read_text())
            self.assertEqual(manifest["notes"], expected)
            self.assertEqual(manifest["history"], [{"version": "0.2.0", "notes": expected}])
            checked = self.run_script("check", "--manifest", str(out))
            self.assertEqual(checked.returncode, 0, checked.stderr)

    def test_bilingual_inputs_are_paired_nonempty_and_unmarked(self):
        with tempfile.TemporaryDirectory() as directory:
            missing, _ = self.generate(directory, notes_en="English only")
            self.assertNotEqual(missing.returncode, 0)
            self.assertIn("provided together", missing.stderr)

            conflict, _ = self.generate(
                directory, notes="legacy", notes_en="English", notes_zh_cn="中文"
            )
            self.assertNotEqual(conflict.returncode, 0)
            self.assertIn("cannot be combined", conflict.stderr)

            marked, _ = self.generate(
                directory,
                notes_en="<!--LANG:en-->\nEnglish",
                notes_zh_cn="中文",
            )
            self.assertNotEqual(marked.returncode, 0)
            self.assertIn("LANG markers", marked.stderr)

    def test_generate_accumulates_replaces_and_migrates_release_notes(self):
        with tempfile.TemporaryDirectory() as directory:
            _, first = self.generate(directory, version="0.2.0", notes="old notes")
            legacy = json.loads(first.read_text())
            result, second = self.generate(
                directory, version="0.3.0", existing=first, notes="new notes"
            )
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertEqual(
                json.loads(second.read_text())["history"],
                [
                    {"version": "0.3.0", "notes": "new notes"},
                    {"version": "0.2.0", "notes": "old notes"},
                ],
            )

            replaced, replacement = self.generate(
                directory,
                version="0.3.0",
                existing=second,
                force=True,
                notes="replacement notes",
            )
            self.assertEqual(replaced.returncode, 0, replaced.stderr)
            self.assertEqual(
                json.loads(replacement.read_text())["history"],
                [
                    {"version": "0.3.0", "notes": "replacement notes"},
                    {"version": "0.2.0", "notes": "old notes"},
                ],
            )

            legacy.pop("history")
            first.write_text(json.dumps(legacy))
            migrated, migrated_out = self.generate(
                directory, version="0.3.0", existing=first, notes="new notes"
            )
            self.assertEqual(migrated.returncode, 0, migrated.stderr)
            self.assertEqual(
                json.loads(migrated_out.read_text())["history"],
                [
                    {"version": "0.3.0", "notes": "new notes"},
                    {"version": "0.2.0", "notes": "old notes"},
                ],
            )

    def test_generate_refuses_version_regressions(self):
        with tempfile.TemporaryDirectory() as directory:
            _, first = self.generate(directory, version="0.3.0")
            result, _ = self.generate(directory, version="0.2.0", existing=first)
            self.assertNotEqual(result.returncode, 0)
            self.assertIn("refusing", result.stderr)
            equal, _ = self.generate(directory, version="0.3.0", existing=first)
            self.assertIn("refusing", equal.stderr)
            forced, out = self.generate(directory, version="0.3.0", existing=first, force=True)
            self.assertEqual(forced.returncode, 0, forced.stderr)
            self.assertEqual(json.loads(out.read_text())["version"], "0.3.0")

    def test_check_accepts_a_generated_manifest(self):
        with tempfile.TemporaryDirectory() as directory:
            _, out = self.generate(directory)
            result = self.run_script("check", "--manifest", str(out))
            self.assertEqual(result.returncode, 0, result.stderr)

    def test_non_zip_and_incomplete_archives_never_get_a_channel_manifest(self):
        import zipfile
        with tempfile.TemporaryDirectory() as directory:
            archive = Path(directory) / "Aster-darwin-aarch64-v0.2.0.zip"
            out = Path(directory) / "desktop-latest.json"
            for payload in (b"not a ZIP", None):
                if payload is not None:
                    archive.write_bytes(payload)
                else:
                    with zipfile.ZipFile(archive, "w") as package:
                        package.writestr("Aster.app/Contents/MacOS/Studio", b"host only")
                for force in ([], ["--force"]):
                    result = self.run_script("generate", "--zip", str(archive), "--asset", archive.name,
                                             "--version", "0.2.0", "--out", str(out), *force)
                    self.assertNotEqual(result.returncode, 0)
                    self.assertIn("invalid desktop archive", result.stderr)
                    self.assertFalse(out.exists())

    def test_asset_name_and_bundle_version_must_match_the_manifest_version(self):
        with tempfile.TemporaryDirectory() as directory:
            archive = self.make_archive(directory)
            out = Path(directory) / "desktop-latest.json"
            for asset, version in (("wrong.zip", "0.2.0"), ("Aster-darwin-aarch64-v0.3.0.zip", "0.3.0")):
                with self.subTest(asset=asset, version=version):
                    result = self.run_script("generate", "--zip", str(archive), "--asset", asset,
                                             "--version", version, "--out", str(out))
                    self.assertNotEqual(result.returncode, 0)
                    self.assertFalse(out.exists())

    def test_check_names_every_refusal(self):
        base = {
            "product": "aster",
            "channel": "stable",
            "version": "0.2.0",
            "published_at": "2026-10-22T00:00:00Z",
            "notes": "",
            "platforms": {
                "darwin-aarch64": {
                    "url": "https://github.com/aster-ai-lab/aster/releases/download/desktop-stable/a.zip",
                    "sha256": "ab" * 32,
                    "size": 4,
                }
            },
        }
        cases = {
            "wrong product": {"product": "toyou"},
            "wrong channel": {"channel": "beta"},
            "bad version": {"version": "0.2"},
            "negative version": {"version": "0.2.-1"},
            "signed version": {"version": "+0.2.0"},
            "unicode version": {"version": "０.2.0"},
            "missing published_at": {"published_at": ""},
            "incomplete language markers": {"notes": "<!--LANG:en-->\nEnglish only"},
            "history not an array": {"history": {}},
            "history duplicate": {
                "history": [
                    {"version": "0.2.0", "notes": ""},
                    {"version": "0.2.0", "notes": ""},
                ]
            },
            "history newer than manifest": {
                "history": [
                    {"version": "0.2.0", "notes": ""},
                    {"version": "0.3.0", "notes": ""},
                ]
            },
            "history missing current": {"history": [{"version": "0.1.0", "notes": ""}]},
            "history current notes mismatch": {"history": [{"version": "0.2.0", "notes": "different"}]},
            "history malformed entry": {"history": [{"version": "0.2.0"}]},
            "empty platforms": {"platforms": {}},
            "unsupported platform": {"platforms": {"darwin-unknown": base["platforms"]["darwin-aarch64"]}},
            "digest not hex": {
                "platforms": {"darwin-aarch64": {**base["platforms"]["darwin-aarch64"], "sha256": "zz" * 32}}
            },
            "digest uppercase": {
                "platforms": {"darwin-aarch64": {**base["platforms"]["darwin-aarch64"], "sha256": "AB" * 32}}
            },
            "url off channel": {
                "platforms": {"darwin-aarch64": {**base["platforms"]["darwin-aarch64"], "url": "https://example.test/a.zip"}}
            },
            "size zero": {
                "platforms": {"darwin-aarch64": {**base["platforms"]["darwin-aarch64"], "size": 0}}
            },
        }
        with tempfile.TemporaryDirectory() as directory:
            for name, override in cases.items():
                manifest = {**base, **override}
                path = Path(directory) / "m.json"
                path.write_text(json.dumps(manifest))
                result = self.run_script("check", "--manifest", str(path))
                self.assertNotEqual(result.returncode, 0, name)
                self.assertTrue(result.stderr.strip(), name)


class DesktopPublishTests(unittest.TestCase):
    def publish(self, download_exit=0, incomplete=False):
        lines = (REPO / ".github/workflows/release-aster-desktop.yml").read_text().splitlines()
        scripts = []
        for label in ("Regenerate the channel manifest with regression refusal", "Publish to the desktop-stable channel release"):
            start = lines.index("      - name: " + label)
            start = lines.index("        run: |", start) + 1
            end = start
            while end < len(lines) and (not lines[end].strip() or lines[end].startswith("          ")):
                end += 1
            scripts.append(textwrap.dedent("\n".join(lines[start:end])))
        with tempfile.TemporaryDirectory(prefix="desktop-publish-") as directory:
            root = Path(directory)
            dist, commands = root / "dist", root / "commands"
            dist.mkdir()
            commands.mkdir()
            (root / "scripts").symlink_to(REPO / "scripts", target_is_directory=True)
            archive = ManifestContractTests().make_archive(dist)
            if incomplete:
                archive.write_bytes(b"not a desktop bundle")
            current = root / "existing.json"
            current.write_text(json.dumps({"product":"aster", "channel":"stable", "version":"0.1.0",
                "published_at":"2026-10-20T00:00:00Z", "notes":"previous notes", "platforms":{"darwin-aarch64":{
                    "url":"https://github.com/aster-ai-lab/aster/releases/download/desktop-stable/previous.zip",
                    "sha256":"ab"*32, "size":4}}}))
            gh = commands / "gh"
            gh.write_text('''#!/bin/sh
case "$1:$2" in
 release:download)
  [ "$TEST_DOWNLOAD_EXIT" = 0 ] || exit "$TEST_DOWNLOAD_EXIT"
  cp "$TEST_CURRENT" current.json ;;
 release:view) exit 0 ;;
 release:upload|release:create) printf '%s\\n' "$@" > "$TEST_RECEIPT" ;;
 *) exit 91 ;;
esac
''')
            gh.chmod(0o755)
            receipt = root / "publish.args"
            env = dict(os.environ, PATH=str(commands)+os.pathsep+os.environ["PATH"],
                GH_REPO="fixture/aster", GH_TOKEN="synthetic", DESKTOP_VERSION="0.2.0",
                DESKTOP_NOTES_EN="New fixture release", DESKTOP_NOTES_ZH_CN="测试发布",
                TEST_CURRENT=str(current), TEST_RECEIPT=str(receipt), TEST_DOWNLOAD_EXIT=str(download_exit))
            result = subprocess.run(["bash", "-c", "\n".join(scripts)], cwd=root, env=env,
                                    capture_output=True, text=True, timeout=30)
            return result, receipt.read_text() if receipt.exists() else None, (dist / "desktop-latest.json").exists()

    def test_complete_archive_reaches_publication(self):
        result, receipt, manifest = self.publish()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertTrue(manifest)
        self.assertIn("upload", receipt)

    def test_unreadable_published_manifest_stops_before_publication(self):
        result, receipt, manifest = self.publish(download_exit=47)
        self.assertEqual(result.returncode, 47, result.stderr)
        self.assertIsNone(receipt)
        self.assertFalse(manifest)

    def test_incomplete_archive_stops_before_manifest_and_publication(self):
        result, receipt, manifest = self.publish(incomplete=True)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("invalid desktop archive", result.stderr)
        self.assertIsNone(receipt)
        self.assertFalse(manifest)


if __name__ == "__main__":
    unittest.main()
