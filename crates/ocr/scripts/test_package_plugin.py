"""Check the real OCR staging boundary without executing a payload."""
from pathlib import Path
import hashlib
import tempfile
import unittest
from unittest.mock import patch

import package_plugin as package


class OcrPackageTest(unittest.TestCase):
    def test_complete_inputs_have_stable_versions_and_changed_models_are_refused(self):
        with tempfile.TemporaryDirectory(prefix="OCR 插件 ") as temporary:
            root = Path(temporary)
            binary, runtime = root / "aster-use-ocr", root / "runtime"
            binary.write_bytes(b"native fixture")
            runtime.write_bytes(b"library fixture")
            notices = root / "notices"
            notices.mkdir()
            (notices / "LICENSE").write_text("Fixture license", encoding="utf-8")
            models = root / "models"
            hashes = {}
            for name in package.MODEL_FILES:
                path = models / name
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_bytes(name.encode("utf-8"))
                hashes[name] = hashlib.sha256(path.read_bytes()).hexdigest()
            with patch.object(package, "MODEL_FILES", hashes):
                versions = [package.stage_plugin(root / name, binary, runtime, models, notices, "darwin-arm64")
                            for name in ("first", "second")]
                self.assertEqual(versions[0], versions[1])
                self.assertLessEqual(len(versions[0]), 64)
                self.assertTrue((root / "first/skills/aster-use-ocr/SKILL.md").is_file())
                self.assertIn('requires_mcp = ["ocr"]', (root / "first/aster-use-extension.acl").read_text())
                (models / "det/inference.onnx").write_bytes(b"changed")
                with self.assertRaisesRegex(ValueError, "checksum"):
                    package.stage_plugin(root / "changed", binary, runtime, models, notices, "darwin-arm64")


if __name__ == "__main__":
    unittest.main()
