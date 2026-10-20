"""Hermetic release-pair packaging and install fault injection; no user binaries."""
import hashlib
import io
import os
from pathlib import Path
import subprocess
import sys
import tarfile
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]
PLATFORM = "macos" if sys.platform == "darwin" else "linux"
IDENTITY = "aster-cli-pair-v1:fixture-source:fixture-target"
NAMES = ("aster", "a3s-code-acp")


@unittest.skipUnless(os.name == "posix", "Unix installer requires a Unix host")
class PairDelivery(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix="aster-pair-")
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.bins = self.root / "bins"
        self.assets = self.root / "assets"
        self.prefix = self.root / "installed"
        self.commands = self.root / "commands"
        for path in (self.bins, self.assets, self.prefix, self.commands):
            path.mkdir()
        self.archive = self.assets / f"aster-cli-v0.1.1-{PLATFORM}.tar.gz"
        for name in NAMES:
            self.binary(name, IDENTITY)
            (self.prefix / name).write_text("old-" + name)

    def binary(self, name, identity):
        path = self.bins / name
        path.write_text(f"#!/bin/sh\nprintf '%s\\n' '{identity}'\n")
        path.chmod(0o755)

    def package(self):
        return subprocess.run(["bash", str(ROOT / "scripts/package-aster-cli.sh"),
            PLATFORM, str(self.bins), str(self.archive)], capture_output=True, text=True, timeout=10)

    def install(self):
        env = dict(os.environ, ASTER_RELEASE_BASE=str(self.assets),
            PATH=str(self.commands) + os.pathsep + os.environ["PATH"])
        env.pop("ASTER_INSTALL_URL", None)
        env.pop("GROK_INSTALL_URL", None)
        return subprocess.run(["bash", str(ROOT / "scripts/install.sh"), "--prefix", str(self.prefix)],
            env=env, capture_output=True, text=True, timeout=10)

    def assert_old(self):
        for name in NAMES:
            self.assertEqual((self.prefix / name).read_text(), "old-" + name)

    def fault(self, command, stage_name):
        path = self.commands / command
        path.write_text(f'#!/bin/sh\nfor arg do\n case "$arg" in */{stage_name}) exit 73 ;; esac\ndone\nexec /bin/{command} "$@"\n')
        path.chmod(0o755)

    def rewrite_archive(self, change):
        with tarfile.open(self.archive) as package:
            entries = {name: package.extractfile(name).read() for name in package.getnames()}
        change(entries)
        with tarfile.open(self.archive, "w:gz") as package:
            for name, content in entries.items():
                entry = tarfile.TarInfo(name)
                entry.mode = 0o755
                entry.size = len(content)
                package.addfile(entry, io.BytesIO(content))

    def test_package_manifest_and_successful_install(self):
        self.assertEqual(self.package().returncode, 0)
        with tarfile.open(self.archive) as package:
            self.assertEqual(set(package.getnames()), {*NAMES, "BUILD_ID", "SHA256SUMS"})
        result = self.install()
        self.assertEqual(result.returncode, 0, result.stderr)
        for name in NAMES:
            self.assertEqual((self.prefix / name).read_bytes(), (self.bins / name).read_bytes())

    def test_package_refuses_different_source_identities(self):
        self.binary("a3s-code-acp", IDENTITY + "-old")
        self.assertNotEqual(self.package().returncode, 0)
        self.assertFalse(self.archive.exists())

    def test_missing_binary_refuses_before_packaging(self):
        (self.bins / "a3s-code-acp").unlink()
        self.assertNotEqual(self.package().returncode, 0)
        self.assertFalse(self.archive.exists())

    def test_staging_failure_preserves_pair(self):
        self.assertEqual(self.package().returncode, 0)
        self.fault("cp", "new-acp")
        self.assertEqual(self.install().returncode, 73)
        self.assert_old()

    def test_second_promotion_failure_rolls_back_first_binary(self):
        self.assertEqual(self.package().returncode, 0)
        self.fault("mv", "new-acp")
        self.assertEqual(self.install().returncode, 73)
        self.assert_old()
        self.assertFalse((self.prefix / ".aster-install.lock").exists())

    def test_failed_first_install_leaves_neither_binary(self):
        self.assertEqual(self.package().returncode, 0)
        for name in NAMES:
            (self.prefix / name).unlink()
        self.fault("mv", "new-acp")
        self.assertEqual(self.install().returncode, 73)
        self.assertFalse(any((self.prefix / name).exists() for name in NAMES))

    def test_rollback_preserves_original_symlink(self):
        self.assertEqual(self.package().returncode, 0)
        target = self.root / "old-target"
        target.write_text("old-aster")
        (self.prefix / "aster").unlink()
        (self.prefix / "aster").symlink_to(target)
        self.fault("mv", "new-acp")
        self.assertEqual(self.install().returncode, 73)
        self.assert_old()
        self.assertTrue((self.prefix / "aster").is_symlink())

    def test_checksum_tampering_never_replaces_a_binary(self):
        self.assertEqual(self.package().returncode, 0)
        self.rewrite_archive(lambda entries: entries.update(aster=b"corrupted"))
        result = self.install()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("checksum mismatch", result.stderr)
        self.assert_old()

    def test_matching_checksums_do_not_allow_mixed_build_identity(self):
        self.assertEqual(self.package().returncode, 0)
        def mix(entries):
            entries["a3s-code-acp"] = entries["a3s-code-acp"].replace(b"fixture-source", b"older-source")
            entries["SHA256SUMS"] = "".join(f"{hashlib.sha256(entries[name]).hexdigest()}  {name}\n" for name in NAMES).encode()
        self.rewrite_archive(mix)
        result = self.install()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("source identity mismatch", result.stderr)
        self.assert_old()


if __name__ == "__main__":
    unittest.main()
