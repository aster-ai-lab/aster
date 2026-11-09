"""Keep every runtime component lock in sync with the Studio bundled manifest.

Regression guard for the 0.2.1 release: commit 99aa11588f removed the sandbox
component from COMPONENTS but only *renamed* the matching lock entry (kbox /
aster-sandbox) instead of deleting it. validate_pin() then rejected every lock
with an opaque "invalid runtime plugin pin", and the slim release pipeline
failed at the very last packaging step.
"""
import json
import sys
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "apps/studio/scripts"))
from package_runtime_components import COMPONENTS, validate_pin  # noqa: E402

# The registry mirror is a local, untracked copy of aster-ai-lab/plugins; only
# assert on the locks that exist in this checkout.
LOCKS = (
    ROOT / "apps/studio/resources/runtime-components.lock.json",
    ROOT / "plugins/registry/locks/runtime/darwin-arm64.json",
)


class RuntimeComponentLockTests(unittest.TestCase):
    def test_locks_match_bundled_components(self):
        expected = {row[0] for row in COMPONENTS}
        checked = 0
        for lock in LOCKS:
            if not lock.is_file():
                continue
            pin = json.loads(lock.read_text(encoding="utf-8"))
            with self.subTest(lock=str(lock.relative_to(ROOT))):
                self.assertEqual(set(pin.get("components", {})), expected)
                validate_pin(pin)
            checked += 1
        self.assertGreater(checked, 0, "no runtime component lock found in this checkout")

    def test_validation_error_names_the_drift(self):
        pin = json.loads((ROOT / "apps/studio/resources/runtime-components.lock.json").read_text(encoding="utf-8"))
        pin["components"]["retired-sandbox"] = {"version": "0.1.0"}
        with self.assertRaisesRegex(ValueError, r"extra=.*retired-sandbox"):
            validate_pin(pin)

    def test_component_sha_identities_are_identical_across_locks(self):
        """Both locks must describe the same published binaries, not two builds."""
        present = [p for p in LOCKS if p.is_file()]
        if len(present) < 2:
            self.skipTest("registry mirror lock not present in this checkout")
        locks = [json.loads(p.read_text(encoding="utf-8"))["components"] for p in present]
        for name in {row[0] for row in COMPONENTS}:
            with self.subTest(component=name):
                self.assertEqual(locks[0][name], locks[1][name])


if __name__ == "__main__":
    unittest.main()
