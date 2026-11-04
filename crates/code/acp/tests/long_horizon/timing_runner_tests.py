import importlib.util
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import time
import unittest

spec = importlib.util.spec_from_file_location("timing_runner", Path(__file__).with_name("timing_runner.py"))
timing = importlib.util.module_from_spec(spec)
spec.loader.exec_module(timing)


class TimingRunnerTests(unittest.TestCase):
    def test_process_first_output_exit_bytes_and_secrets_are_separate(self):
        with tempfile.TemporaryDirectory(prefix="aster timing 値 ") as directory:
            root = Path(directory)
            trace = root / "timing.jsonl"
            class AcknowledgingRecorder(timing.Recorder):
                def event(self, event, phase, **kwargs):
                    span = super().event(event, phase, **kwargs)
                    if phase == "process_first_output":
                        (root / "first-output-seen").write_bytes(b"ack")
                    return span
            recorder = AcknowledgingRecorder(trace)
            secret = "synthetic-private-argument"
            child = "import os,time;os.write(1,b'ready\\n');deadline=time.monotonic()+5\nwhile not os.path.exists('first-output-seen') and time.monotonic()<deadline:time.sleep(.01)\nos.write(2,b'last\\n');raise SystemExit(7 if os.path.exists('first-output-seen') else 11)"
            command = [sys.executable, "-c", child, secret]
            result = recorder.run_logged(command, cwd=root, env=dict(os.environ, FIXTURE_PRIVATE=secret), log=root / "output.log", phase="test_process")
            self.assertEqual(result.returncode, 7)
            self.assertEqual((root / "output.log").read_bytes(), b"ready\nlast\n")
            text = trace.read_text(encoding="utf-8")
            self.assertNotIn(secret, text)
            rows = [json.loads(line) for line in text.splitlines()]
            first = next(row for row in rows if row["phase"] == "process_first_output")
            end = next(row for row in rows if row["phase"] == "process_exit")
            self.assertLessEqual(first["elapsed_us"], end["elapsed_us"])
            self.assertEqual(end["subject"]["exit_code"], 7)
            recorder.close()
            self.assertTrue(timing.summarize([trace])["complete"])

    def test_capture_timeout_remains_terminal_and_is_recorded(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            trace = root / "timing.jsonl"
            recorder = timing.Recorder(trace)
            with self.assertRaises(subprocess.TimeoutExpired):
                recorder.run_capture([sys.executable, "-c", "import time;time.sleep(10)"], cwd=root, phase="identity_query", timeout=.05)
            recorder.close()
            summary = timing.summarize([trace])
            self.assertTrue(summary["complete"])
            self.assertTrue(any(row["phase"] == "process_timeout" for row in summary["points"]))
            self.assertTrue(all(row["subject"]["exit_code"] is not None for row in summary["points"] if row["phase"] == "process_exit"))

    def test_missing_end_is_incomplete_and_nested_time_is_not_added(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            trace = root / "timing.jsonl"
            recorder = timing.Recorder(trace)
            with recorder.phase("whole"):
                with recorder.phase("inner"):
                    time.sleep(.002)
            recorder.close()
            summary = timing.summarize([trace])
            self.assertTrue(summary["complete"])
            self.assertGreater(summary["inclusive_sum_us"], summary["covered_union_us"])
            recorder.event("begin", "interrupted")
            summary = timing.summarize([trace])
            self.assertFalse(summary["complete"])
            self.assertEqual(summary["errors"][0]["kind"], "missing_end")

    def test_lifecycle_and_coverage_gaps_are_explicit(self):
        with tempfile.TemporaryDirectory() as directory:
            trace = Path(directory) / 'timing.jsonl'
            recorder = timing.Recorder(trace)
            recorder.event('point', 'process_started', subject={'process':os.getpid()})
            with recorder.phase('campaign_prepare'):
                time.sleep(.002)
            with recorder.phase('prompt'):
                with recorder.phase('tool'):
                    time.sleep(.002)
            time.sleep(.002)
            recorder.event('point', 'process_exit', subject={'process':os.getpid(),'exit_code':0})
            self.assertFalse(timing.summarize([trace])['complete'], 'writer without a close marker is incomplete')
            recorder.close()
            summary = timing.summarize([trace])
            result = timing.reconcile(summary,os.getpid())
            self.assertTrue(result['complete'])
            self.assertEqual(result['wall_us'],result['sum_us'])
            self.assertGreater(result['partition_us']['unattributed'],0)
            self.assertGreater(result['partition_us']['task'],0)
            self.assertEqual(timing.compare({'time':80,'new':1},{'time':100})['time']['percent'],-20)
            self.assertIsNone(timing.compare({'new':1},{})['new']['percent'])
            recorder.event('point','tool_unfinished')
            self.assertFalse(timing.summarize([trace])['complete'])


if __name__ == "__main__":
    unittest.main()
