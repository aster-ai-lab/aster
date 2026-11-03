"""Refuse changed Box inputs before staging or invoking the native builder."""
import importlib.util
from pathlib import Path
import tempfile
import unittest

PATH = Path(__file__).resolve().parents[1] / "recipes/aster-box/darwin-arm64.py"
SPEC = importlib.util.spec_from_file_location("box_recipe", PATH)
RECIPE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(RECIPE)


class BoxRecipeTest(unittest.TestCase):
    def test_wrong_length_is_refused(self):
        with tempfile.TemporaryDirectory(prefix="Box 空格 ") as temporary:
            archive = Path(temporary) / "runtime.tar.gz"
            archive.write_bytes(b"wrong archive")
            with self.assertRaisesRegex(ValueError, "length mismatch"):
                RECIPE.verify_runtime(archive)

    def test_same_length_different_content_is_refused(self):
        with tempfile.TemporaryDirectory(prefix="Box 空格 ") as temporary:
            archive = Path(temporary) / "runtime.tar.gz"
            with archive.open("wb") as stream:
                stream.truncate(RECIPE.RUNTIME_BYTES)
            with self.assertRaisesRegex(ValueError, "digest mismatch"):
                RECIPE.verify_runtime(archive)


if __name__ == "__main__":
    unittest.main()
