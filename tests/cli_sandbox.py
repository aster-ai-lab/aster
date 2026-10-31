"""CLI staging reuses the shared producer and verifies private artifacts before promotion."""

import hashlib
import importlib.util
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[1]
SPEC = importlib.util.spec_from_file_location("aster_sandbox", ROOT / "scripts/aster_sandbox.py")
sandbox = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(sandbox)
from sandbox_assets import FILES


def fixture(root):
    for name in FILES:
        path = root / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(("synthetic runtime " + name).encode())
    receipt = {"schema": 1, "files": {
        name: hashlib.sha256((root / name).read_bytes()).hexdigest() for name in FILES}}
    (root / "manifest.json").write_text(json.dumps(receipt))


class CliSandbox(unittest.TestCase):
    def test_unsupported_staging_host_refuses_before_invoking_cargo(self):
        with patch.object(sandbox.sys, "argv", ["aster_sandbox.py", "stage"]), \
             patch.object(sandbox.sys, "platform", "linux"), \
             patch.object(sandbox.subprocess, "check_output") as cargo:
            with self.assertRaisesRegex(ValueError, "macOS ARM64"):
                sandbox.main()
            cargo.assert_not_called()

    def test_staging_invokes_the_existing_shared_producer(self):
        with tempfile.TemporaryDirectory() as directory:
            target = Path(directory)
            source = target / "debug/studio-resources/agent/sandbox"
            with patch.object(sandbox, "stage_shared", side_effect=lambda *_: fixture(source)) as producer:
                staged = sandbox.stage(target, "debug")
            producer.assert_called_once_with(target, "debug")
            self.assertEqual(sandbox.verify(staged), sandbox.verify(source))

    def test_corrupt_source_does_not_replace_a_working_runtime(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source, dest = root / "source", root / "sandbox"
            fixture(source)
            sandbox.copy_verified(source, dest, sandbox.manifest_digest(source))
            before = sandbox.verify(dest)
            (source / "aster-oci").write_bytes(b"corrupt")
            with self.assertRaisesRegex(ValueError, "checksum mismatch"):
                sandbox.copy_verified(source, dest, sandbox.manifest_digest(source))
            self.assertEqual(sandbox.verify(dest), before)

    def test_development_runtime_lives_outside_the_writable_checkout(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            target = root / "checkout/target"
            fixture(target / "debug/sandbox")
            with patch.object(sandbox.Path, "home", return_value=root / "profile"):
                cached = sandbox.development_cache(target, "debug")
                before = cached.stat().st_mtime_ns
                self.assertEqual(sandbox.development_cache(target, "debug"), cached)
                self.assertEqual(cached.stat().st_mtime_ns, before)
            self.assertFalse(cached.is_relative_to(root / "checkout"))
            self.assertEqual(sandbox.verify(cached), sandbox.verify(target / "debug/sandbox"))

    def test_a_destination_link_cannot_redirect_promotion(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source, foreign = root / "source", root / "foreign"
            fixture(source)
            fixture(foreign)
            before = sandbox.verify(foreign)
            destination = root / "sandbox"
            destination.symlink_to(foreign, target_is_directory=True)
            with self.assertRaisesRegex(ValueError, "regular directory"):
                sandbox.copy_verified(source, destination, sandbox.manifest_digest(source))
            self.assertEqual(sandbox.verify(foreign), before)

    def test_changed_source_cannot_be_published_under_an_older_cache_identity(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            target = root / "checkout/target"
            source = target / "debug/sandbox"
            fixture(source)
            original_copy = sandbox.shutil.copytree

            def replace_before_copy(src, dest, *args, **kwargs):
                if src == source:
                    (source / "aster-oci").write_bytes(b"a different valid runtime")
                    receipt = json.loads((source / "manifest.json").read_text())
                    receipt["files"]["aster-oci"] = hashlib.sha256((source / "aster-oci").read_bytes()).hexdigest()
                    (source / "manifest.json").write_text(json.dumps(receipt))
                return original_copy(src, dest, *args, **kwargs)

            with patch.object(sandbox.Path, "home", return_value=root / "profile"), \
                 patch.object(sandbox.shutil, "copytree", side_effect=replace_before_copy):
                with self.assertRaisesRegex(ValueError, "changed"):
                    sandbox.development_cache(target, "debug")
            cache_root = root / "profile/Library/Caches/aster/sandbox"
            self.assertEqual(list(cache_root.iterdir()), [])

    def test_cache_identity_is_fenced_before_the_copy_starts(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            target = root / "checkout/target"
            source = target / "debug/sandbox"
            fixture(source)
            original_copy = sandbox.copy_verified

            def replace_source(*args):
                (source / "aster-oci").write_bytes(b"new source before copy")
                receipt = json.loads((source / "manifest.json").read_text())
                receipt["files"]["aster-oci"] = hashlib.sha256((source / "aster-oci").read_bytes()).hexdigest()
                (source / "manifest.json").write_text(json.dumps(receipt))
                return original_copy(*args)

            with patch.object(sandbox.Path, "home", return_value=root / "profile"), \
                 patch.object(sandbox, "copy_verified", side_effect=replace_source):
                with self.assertRaisesRegex(ValueError, "changed"):
                    sandbox.development_cache(target, "debug")


if __name__ == "__main__":
    unittest.main()
