"""Runtime discovery uses owned temporary descriptors; never opens a window."""

import json
import os
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

from window import runtime_for


class RuntimeDiscoveryTests(unittest.TestCase):
    def test_fork_discovery_never_reads_another_tenants_descriptor(self):
        with tempfile.TemporaryDirectory() as directory, patch.dict(os.environ, {"TMPDIR": directory}, clear=True):
            self.write(Path(directory) / "a3s-studio/aster/runtime-fork-42.json", {"pid": 42, "token": "aster"})
            self.write(Path(directory) / "a3s-studio/toyou/runtime-fork-42.json", {"pid": 42, "token": "toyou"})
            self.assertEqual(runtime_for("aster", 42)["token"], "aster")
            self.assertEqual(runtime_for("toyou", 42)["token"], "toyou")
            self.assertIsNone(runtime_for("aster", 43))

    def test_allocated_instance_and_explicit_override_remain_bound_to_the_pid(self):
        with tempfile.TemporaryDirectory() as directory, patch.dict(os.environ, {"TMPDIR": directory}, clear=True):
            self.write(Path(directory) / "a3s-studio/toyou/instances/fork1/runtime.json", {"pid": 42})
            self.assertEqual(runtime_for("toyou", 42)["pid"], 42)
            explicit = Path(directory) / "explicit.json"
            self.write(explicit, {"pid": 43})
            os.environ["STUDIO_RUNTIME_FILE"] = str(explicit)
            self.assertIsNone(runtime_for("toyou", 42))
            self.assertEqual(runtime_for("toyou", 43)["pid"], 43)

    @staticmethod
    def write(path, data):
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(json.dumps(data))


if __name__ == "__main__":
    unittest.main()
