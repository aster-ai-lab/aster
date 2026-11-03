import importlib.util
import io
import json
import subprocess
import sys
import tempfile
import unittest
from contextlib import redirect_stdout
from pathlib import Path
from unittest import mock

SCRIPT = Path(__file__).parents[1] / "scripts/aster_qualification_cleanup.py"
SPEC = importlib.util.spec_from_file_location("aster_qualification_cleanup", SCRIPT)
MODULE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(MODULE)


class QualificationCleanupTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.root = Path(self.temp.name)
        for name in ["fallback-state-old", "fallback-state-middle", "fallback-state-new"]:
            (self.root / name).mkdir()
        (self.root / "shared").mkdir()

    def tearDown(self):
        self.temp.cleanup()

    def test_plan_keeps_latest_two_protected_and_non_candidates(self):
        keep, delete = MODULE.retention_plan(self.root, {"fallback-state-old"})
        self.assertEqual(
            [path.name for path in keep],
            ["fallback-state-new", "fallback-state-middle", "fallback-state-old"],
        )
        self.assertEqual(delete, [])
        self.assertTrue((self.root / "shared").is_dir())

    def test_plan_keeps_latest_two_in_each_supported_family(self):
        for prefix in ["deloop-changing-feed-", "deloop-independent-records-"]:
            for name in ["old", "middle", "new"]:
                (self.root / f"{prefix}{name}").mkdir()
        keep, delete = MODULE.retention_plan(self.root, set())
        self.assertEqual(
            {path.name for path in delete},
            {
                "fallback-state-old",
                "deloop-changing-feed-old",
                "deloop-independent-records-old",
            },
        )
        self.assertEqual(len(keep), 6)

    def test_plan_preserves_current_tui_assets(self):
        for prefix in ["glm-tui-", "tui-"]:
            for name in ["old", "middle", "new"]:
                (self.root / f"{prefix}{name}").mkdir()
        current = self.root / "tui-current-runtime"
        current.mkdir()
        keep, delete = MODULE.retention_plan(self.root, set())
        self.assertIn(current, keep)
        self.assertNotIn(current, delete)
        self.assertIn("glm-tui-old", {path.name for path in delete})
        self.assertIn("tui-old", {path.name for path in delete})

    def test_plan_keeps_latest_two_diagnostic_candidates(self):
        for prefix in ["rust-multifile-real-diagnostic-", "native-multiturn-diagnostic-"]:
            for name in ["old", "middle", "new"]:
                (self.root / f"{prefix}{name}").mkdir()
        keep, delete = MODULE.retention_plan(self.root, set())
        self.assertEqual(len([path for path in keep if "diagnostic-" in path.name]), 4)
        self.assertIn("rust-multifile-real-diagnostic-old", {path.name for path in delete})
        self.assertIn("native-multiturn-diagnostic-old", {path.name for path in delete})

    def test_execute_deletes_only_expired_candidates(self):
        argv = ["cleanup", "--root", str(self.root), "--execute"]
        with (
            mock.patch.object(MODULE, "active_commands", return_value=""),
            mock.patch.object(sys, "argv", argv),
            redirect_stdout(io.StringIO()),
        ):
            MODULE.main()
        self.assertFalse((self.root / "fallback-state-old").exists())
        self.assertTrue((self.root / "fallback-state-new").is_dir())
        self.assertTrue((self.root / "fallback-state-middle").is_dir())
        self.assertTrue((self.root / "shared").is_dir())

    def test_execute_refuses_a_candidate_referenced_by_a_process(self):
        argv = ["cleanup", "--root", str(self.root), "--execute"]
        active = str(self.root / "fallback-state-old" / "aster-acp")
        with (
            mock.patch.object(MODULE, "active_commands", return_value=active),
            mock.patch.object(sys, "argv", argv),
        ):
            with self.assertRaises(SystemExit):
                MODULE.main()
        self.assertTrue((self.root / "fallback-state-old").is_dir())

    def test_cli_dry_run_does_not_delete(self):
        result = subprocess.run(
            [sys.executable, str(SCRIPT), "--root", str(self.root)],
            capture_output=True,
            text=True,
            check=True,
        )
        payload = json.loads(result.stdout)
        self.assertFalse(payload["executed"])
        self.assertEqual(payload["delete"], ["fallback-state-old"])
        self.assertTrue((self.root / "fallback-state-old").is_dir())


if __name__ == "__main__":
    unittest.main()
