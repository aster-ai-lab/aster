"""Finite dispatcher gates, content-free events and no automatic failure retry."""
import copy
import importlib.util
import io
import json
from pathlib import Path
import sys
import tempfile
import unittest
from unittest import mock

PATH = Path(__file__).resolve().parents[1] / 'docs/evidence/power/2026-10-21-serving-optimization/canary-qualification-driver.py'
spec = importlib.util.spec_from_file_location('qualification_canary_driver', PATH)
d = importlib.util.module_from_spec(spec)
spec.loader.exec_module(d)


def identity():
    return {kind: {'pid': 100 + index, 'start_ticks': 10, 'cmdline_sha256': 'a' * 64,
                   'exe_sha256': 'b' * 64, 'listener_inodes': ['1'], 'port': port}
            for index, (kind, port) in enumerate([('native', 18107), ('power', 19507), ('gateway', 19897)])}


class Dispatcher(unittest.TestCase):
    def test_prior_functional_evidence_requires_exact_generation_and_cleanup(self):
        fixed, provenance = identity(), {'binary_sha256': d.POWER_SHA}
        fixed['native']['cuda_visible_devices'] = '7'
        observed = copy.deepcopy(fixed)
        observed['native'].pop('cuda_visible_devices')
        result = {'status': 'passed', 'provenance': provenance, 'identity': observed,
                  'tool_identity': {k: v for k, v in d.PINS.items() if k != 'eight_card_sha256'},
                  'cleanup': {'own_requests_completed': True, 'observed_all_native_and_Power_idle': True},
                  'monitor_terminated': True, 'cases': [{'passed': True}] * 4}
        self.assertTrue(d.functional_ok(result, provenance, fixed))
        for failure in ('generation', 'missing-identity', 'cleanup', 'case', 'instrument'):
            bad = copy.deepcopy(result)
            if failure == 'generation':
                bad['identity']['native']['start_ticks'] += 1
            elif failure == 'missing-identity':
                bad['identity']['native'] = {}
            elif failure == 'cleanup':
                bad['cleanup']['observed_all_native_and_Power_idle'] = False
            elif failure == 'case':
                bad['cases'][0]['passed'] = False
            else:
                bad['tool_identity']['canary_sha256'] = 'c' * 64
            with self.subTest(failure=failure):
                self.assertFalse(d.functional_ok(bad, provenance, fixed))

    def test_events_preserve_counts_without_raw_credentials_or_messages(self):
        for raw in [b'Traceback secret-marker', b'{"api_key":"secret-marker","message":"private"}',
                    b'{"file":"secret-marker","status":"failed"}']:
            result = json.dumps(d.safe_line(raw))
            self.assertNotIn('secret-marker', result)
            self.assertNotIn('private', result)
        row = d.safe_line(b'{"file":"candidate-c8-r2.json","status":"passed","outcomes":{"success":9,"refused":2}}')
        self.assertEqual(row['outcomes'], {'success': 9, 'refused': 2})

    def test_failed_functional_artifact_prevents_any_matrix_dispatch(self):
        with tempfile.TemporaryDirectory() as directory:
            root, private = Path(directory), Path(directory) / 'eight-replica'
            private.mkdir()
            (root / 'canary-v4-functional.json').write_text('{"status":"failed"}')
            for name in ('identity', 'provenance', 'readiness'):
                (private / ('worker-7-' + name + '.json')).write_text('{}')
            with mock.patch.object(d, 'PRIVATE', private), mock.patch.object(d.m, 'PRIVATE', root), \
                 mock.patch.object(d, 'current_tools', return_value=d.PINS), \
                 mock.patch.object(d, 'inputs', return_value=({}, {})), \
                 mock.patch.object(d, 'supervise') as child, \
                 mock.patch.object(sys, 'argv', ['driver', '--slots', '2']):
                with self.assertRaises(d.m.GuardError):
                    d.main()
            child.assert_not_called()
            self.assertEqual(json.loads((private / 'canary-v4-driver-full.json').read_text())['status'], 'failed')

    def test_isolated_namespace_preserves_old_failure_and_cannot_retry_itself(self):
        with tempfile.TemporaryDirectory() as directory:
            root, private = Path(directory), Path(directory) / 'eight-replica'
            private.mkdir()
            old_paths = [root / 'canary-v4-functional.json',
                         private / 'canary-v4-driver-smoke.json',
                         private / 'canary-v4-driver-smoke.jsonl']
            old_bytes = b'{"status":"failed","fixture":"original-preserved"}'
            for path in old_paths:
                path.write_bytes(old_bytes)
            old_matrix = private / 'canary-v4-smoke'
            old_matrix.mkdir()
            (old_matrix / 'original-failure.json').write_bytes(old_bytes)
            old_paths.append(old_matrix / 'original-failure.json')
            for name in ('identity', 'provenance', 'readiness'):
                (private / ('worker-7-' + name + '.json')).write_text('{}')
            fixed = identity()
            result = {'status': 'passed', 'provenance': {}, 'identity': fixed,
                      'tool_identity': {k: v for k, v in d.PINS.items() if k != 'eight_card_sha256'},
                      'cleanup': {'own_requests_completed': True, 'observed_all_native_and_Power_idle': True},
                      'monitor_terminated': True, 'cases': [{'passed': True}] * 4}
            commands = []

            def complete(command, _timeout, *_args):
                commands.append(command)
                tag = command[command.index('--tag') + 1]
                if command[3].endswith('canary-scenarios.py'):
                    self.assertEqual(tag, 'v4-isolated-functional')
                    (root / ('canary-' + tag + '.json')).write_text(json.dumps(result))
                else:
                    self.assertEqual(tag, 'canary-v4-isolated-smoke')
                    target = private / tag
                    target.mkdir()
                    (target / 'candidate-c1-r1.json').write_text('{"qualification_status":"passed"}')

            with mock.patch.object(d, 'PRIVATE', private), mock.patch.object(d.m, 'PRIVATE', root), \
                 mock.patch.object(d, 'current_tools', return_value=d.PINS), \
                 mock.patch.object(d, 'inputs', return_value=({}, fixed)), \
                 mock.patch.object(d, 'supervise', side_effect=complete) as child, \
                 mock.patch.object(sys, 'argv', ['driver', '--slots', '2', '--phase', 'smoke',
                                               '--run-tag', 'canary-v4-isolated']):
                d.main()
                self.assertEqual(child.call_count, 2)
                child.reset_mock()
                with self.assertRaises(d.m.GuardError):
                    d.main()
                child.assert_not_called()
            self.assertEqual(len(commands), 2)
            for path in old_paths:
                self.assertEqual(path.read_bytes(), old_bytes)
            state = json.loads((private / 'canary-v4-isolated-driver-smoke.json').read_text())
            self.assertEqual((state['status'], state['run_tag']), ('passed', 'canary-v4-isolated'))
            self.assertEqual(state['functional_evidence'], str(root / 'canary-v4-isolated-functional.json'))

    def test_run_tag_is_closed_before_any_inputs_or_dispatch(self):
        with mock.patch.object(d, 'inputs') as inputs, mock.patch.object(d, 'supervise') as child, \
             mock.patch.object(sys, 'argv', ['driver', '--slots', '2', '--run-tag', 'arbitrary-retry']), \
             mock.patch('sys.stderr', io.StringIO()):
            with self.assertRaises(SystemExit):
                d.main()
        inputs.assert_not_called()
        child.assert_not_called()

    def test_warm_namespace_cannot_reuse_or_replace_isolated_unsupported(self):
        with tempfile.TemporaryDirectory() as directory:
            root, private = Path(directory), Path(directory) / 'eight-replica'
            private.mkdir()
            prior = [root / 'canary-v4-isolated-functional.json',
                     private / 'canary-v4-isolated-driver-smoke.json']
            preserved = b'{"status":"unsupported","completed_request_records":[]}'
            for path in prior:
                path.write_bytes(preserved)
            for name in ('identity', 'provenance', 'readiness'):
                (private / ('worker-7-' + name + '.json')).write_text('{}')

            def fail_new_functional(command, *_args):
                self.assertEqual(command[command.index('--tag') + 1], 'v4-isolated-warm-functional')
                (root / 'canary-v4-isolated-warm-functional.json').write_text('{"status":"failed"}')
                raise d.m.GuardError('new fixture failure; no retry')

            with mock.patch.object(d, 'PRIVATE', private), mock.patch.object(d.m, 'PRIVATE', root), \
                 mock.patch.object(d, 'current_tools', return_value=d.PINS), \
                 mock.patch.object(d, 'inputs', return_value=({}, {})), \
                 mock.patch.object(d, 'supervise', side_effect=fail_new_functional) as child, \
                 mock.patch.object(sys, 'argv', ['driver', '--slots', '2', '--phase', 'smoke',
                                               '--run-tag', 'canary-v4-isolated-warm']):
                with self.assertRaises(d.m.GuardError):
                    d.main()
            child.assert_called_once()
            for path in prior:
                self.assertEqual(path.read_bytes(), preserved)
            state = json.loads((private / 'canary-v4-isolated-warm-driver-smoke.json').read_text())
            self.assertEqual(state['status'], 'failed')
            self.assertFalse((private / 'canary-v4-isolated-warm-smoke').exists())

    def test_cell_snapshots_do_not_reread_unchanged_full_results(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / 'tag').mkdir()
            path = root / 'tag/candidate-c1-r1.json'
            path.write_text('{"qualification_status":"passed","summary":{"outcomes":{"success":2}}}')
            with mock.patch.object(d, 'PRIVATE', root), mock.patch.object(d, 'load', wraps=d.load) as read:
                self.assertEqual(d.cells('tag', True)['candidate-c1-r1.json']['status'], 'passed')
                d.cells('tag', True)
                self.assertEqual(read.call_count, 1)

    @unittest.skipIf(sys.platform == 'win32', 'supervisor runs on the Linux serving host')
    def test_failed_direct_child_is_not_retried_and_diagnostics_are_hashed(self):
        with tempfile.TemporaryDirectory() as directory:
            state, events, path = {}, io.StringIO(), Path(directory) / 'state.json'
            command = [sys.executable, '-u', '-c', 'print("secret-marker",flush=True);raise SystemExit(1)']
            with self.assertRaises(d.m.GuardError):
                d.supervise(command, 2, state, path, events)
            self.assertEqual(state['child_exit_code'], 1)
            self.assertNotIn('secret-marker', events.getvalue())

    def test_status_setup_failure_reaps_exact_owned_child(self):
        child = mock.Mock()
        child.pid = 123
        child.poll.return_value = None
        with mock.patch.object(d.subprocess, 'Popen', return_value=child), \
             mock.patch.object(d.m, 'write_result', side_effect=OSError('fixture disk unavailable')):
            with self.assertRaises(OSError):
                d.supervise(['python', 'fixture-only'], 2, {}, Path('unused'), io.StringIO())
        child.terminate.assert_called_once()
        child.wait.assert_called_once_with(timeout=15)
        child.stdout.close.assert_called_once()


if __name__ == '__main__':
    unittest.main()
