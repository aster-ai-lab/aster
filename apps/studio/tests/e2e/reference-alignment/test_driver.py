"""Guard against shell execution in credentials and transport path escape."""

import importlib.util
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch


HERE = Path(__file__).resolve().parent
spec = importlib.util.spec_from_file_location("reference_driver", HERE / "run.py")
driver = importlib.util.module_from_spec(spec)
spec.loader.exec_module(driver)


class DriverBoundaryTests(unittest.TestCase):
    def test_verifier_bytes_stay_pinned_when_the_worktree_changes_during_a_model_turn(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source, job = root / 'case', root / 'job'
            (source / 'tests').mkdir(parents=True)
            job.mkdir()
            for name in ('test.sh', 'test_outputs.py', 'expected.json'):
                (source / 'tests' / name).write_text('original ' + name)
            pinned = driver.pin_verifier_inputs(source, job)
            (source / 'tests/test_outputs.py').write_text('changed grader')
            (source / 'tests/expected.json').write_text('changed expectations')
            self.assertEqual((pinned / 'test_outputs.py').read_text(), 'original test_outputs.py')
            self.assertEqual((pinned / 'expected.json').read_text(), 'original expected.json')
            self.assertNotEqual(driver.sha256(pinned / 'test_outputs.py'), driver.sha256(source / 'tests/test_outputs.py'))

    def test_fixture_inputs_are_copied_and_hashed_without_following_links(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source, workspace = root / "source", root / "workspace"
            (source / "inputs").mkdir(parents=True)
            workspace.mkdir()
            original = source / "inputs/data.csv"
            original.write_text("actual input\n")
            self.assertEqual(driver.copy_inputs(source, workspace), {"data.csv": driver.sha256(original)})
            self.assertEqual((workspace / "inputs/data.csv").read_bytes(), original.read_bytes())
            linked_workspace = root / "linked-workspace"
            linked_workspace.mkdir()
            (source / "inputs/escape").symlink_to(root / "private")
            with self.assertRaises(ValueError):
                driver.copy_inputs(source, linked_workspace)
            self.assertFalse((linked_workspace / "inputs").exists())

    def test_retained_evidence_omits_reasoning_and_keeps_observable_tool_receipts(self):
        report = {"model": "boyue/deepseek-v4-flash", "messages": [{"data": {"parts": [
            {"type": "reasoning", "text": "private"},
            {"type": "text", "text": "result"},
            {"type": "dynamic-tool", "state": "output-available"}]}}]}
        driver.remove_reasoning(report)
        self.assertNotIn("private", json.dumps(report))
        self.assertEqual([part["type"] for part in report["messages"][0]["data"]["parts"]],
                         ["text", "dynamic-tool"])

    def test_original_grader_requires_all_tests_and_no_hidden_failure_or_skip(self):
        report = {"results": {"summary": {"tests": 1, "passed": 1, "failed": 0,
                                          "skipped": 0, "pending": 0, "other": 0},
                              "tests": [{"status": "passed"}]}}
        self.assertTrue(driver.original_tests_passed("regex-log", report))
        self.assertFalse(driver.original_tests_passed("git-leak-recovery", report))
        report["results"]["summary"]["failed"] = 1
        self.assertFalse(driver.original_tests_passed("regex-log", report))
        report["results"]["summary"]["failed"] = 0
        report["results"]["tests"][0]["status"] = "skipped"
        self.assertFalse(driver.original_tests_passed("regex-log", report))

    def test_artifact_hash_is_the_standard_sha256_on_supported_python(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "artifact"
            path.write_bytes(b"abc")
            self.assertEqual(driver.sha256(path), "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad")

    def test_credentials_do_not_execute_shell_syntax_or_change_an_explicit_key(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            marker = root / "must-not-exist"
            source = root / ".env"
            source.write_text(f'BOYUE_API_KEY="$(touch {marker})"\n')
            with patch.dict(os.environ, {}, clear=True):
                self.assertEqual(driver.credential_env(source)["BOYUE_API_KEY"], f"$(touch {marker})")
            self.assertFalse(marker.exists())
            with patch.dict(os.environ, {"BOYUE_API_KEY": "explicit-test-key"}, clear=True):
                self.assertEqual(driver.credential_env(root / "absent")["BOYUE_API_KEY"], "explicit-test-key")

    def test_missing_boyue_credentials_refuse_without_an_alternate_provider(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / ".env"
            path.write_text('OTHER_API_KEY="unrelated"\nBOYUE_API_KEY=""\n')
            with patch.dict(os.environ, {}, clear=True), self.assertRaises(ValueError):
                driver.credential_env(path)

    def test_config_outside_the_owned_runtime_is_refused_before_cli_execution(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            config = root / "outside.acl"
            config.write_text('default_model = "test/test"\n')
            env = {**os.environ, "A3S_STUDIO_REFERENCE_BOX_CLI": str(root / "must-not-run"),
                   "A3S_STUDIO_REFERENCE_BOX": "owned", "A3S_STUDIO_REFERENCE_JOB": str(root)}
            result = subprocess.run([sys.executable, str(HERE / "acp-box.py"), "--config", str(config)],
                                    env=env, capture_output=True, text=True)
            self.assertNotEqual(result.returncode, 0)
            self.assertEqual(result.stdout, "")
            self.assertIn("refuse config outside this job", result.stderr)

    def test_plugin_publication_outside_the_owned_review_job_is_refused(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "state/runtime").mkdir(parents=True)
            config = root / "state/runtime/model.acl"
            config.write_text('default_model = "test/test"\n')
            outside = root / "outside"
            outside.mkdir()
            env = {**os.environ, "A3S_STUDIO_REFERENCE_BOX_CLI": str(root / "must-not-run"),
                   "A3S_STUDIO_REFERENCE_BOX": "owned", "A3S_STUDIO_REFERENCE_JOB": str(root)}
            result = subprocess.run([sys.executable, str(HERE / "acp-box.py"), "--config", str(config), "--plugin-store", str(outside)],
                                    env=env, capture_output=True, text=True)
            self.assertNotEqual(result.returncode, 0)
            self.assertEqual(result.stdout, "")
            self.assertIn("refuse plugin store outside this review job", result.stderr)


if __name__ == "__main__":
    unittest.main()
