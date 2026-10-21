"""Private source pins reject missing, non-commit and ambiguous tree records."""

import importlib.util
from pathlib import Path
import unittest
from unittest.mock import patch


ROOT = Path(__file__).resolve().parents[1]
SPEC = importlib.util.spec_from_file_location(
    "gitlink_ref", ROOT / ".github/scripts/gitlink-ref.py"
)
MODULE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(MODULE)
COMMIT = "9aad91f1e934c731e540cf99be25733e25c3a411"


class SourceCheckout(unittest.TestCase):
    def test_reads_only_the_recorded_revision_even_with_path_spaces(self):
        with patch("subprocess.check_output", return_value=(
                f"160000 commit {COMMIT}\tapps/source with spaces\0")) as run:
            self.assertEqual(MODULE.gitlink_ref("apps/source with spaces"), COMMIT)
        run.assert_called_once_with(
            ["git", "ls-tree", "-z", "HEAD", "--", "apps/source with spaces"],
            text=True,
        )

    def test_refuses_missing_non_commit_and_ambiguous_records(self):
        for record in ["", f"100644 blob {COMMIT}\tapps/work\0",
                       f"160000 commit {COMMIT}\tapps/other\0",
                       f"160000 commit {COMMIT}\tapps/work\0" * 2,
                       "160000 commit not-a-commit\tapps/work\0"]:
            with self.subTest(record=record), patch(
                    "subprocess.check_output", return_value=record):
                with self.assertRaises(ValueError):
                    MODULE.gitlink_ref("apps/work")


if __name__ == "__main__":
    unittest.main()
