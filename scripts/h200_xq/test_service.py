import json
import hashlib
import os
import signal
import subprocess
import sys
import tempfile
import unittest
from contextlib import ExitStack
from pathlib import Path
from unittest.mock import MagicMock, patch

import service


class ActivationBoundaryTests(unittest.TestCase):
    def environment(self, directory, stack):
        root = Path(directory)/'nvme'
        private = Path(directory)/'private'
        stack.enter_context(patch.object(service, 'ROOT', root))
        stack.enter_context(patch.object(service, 'PRIVATE', private))
        stack.enter_context(patch.object(service, 'STAGING', Path(directory)/'staging'))
        stack.enter_context(patch.object(service, 'require_hardware', return_value=[]))
        stack.enter_context(patch.object(sys, 'argv', ['service.py', 'start']))
        return root, private

    def test_missing_weight_identity_never_starts_or_stops_a_service(self):
        with tempfile.TemporaryDirectory() as directory, ExitStack() as stack:
            _, private = self.environment(directory, stack)
            private.mkdir(parents=True)
            (private/'power.acl').write_text('prior startup configuration')
            start = stack.enter_context(patch.object(service, 'start_process'))
            stop = stack.enter_context(patch.object(service, 'stop_owned'))
            with self.assertRaisesRegex(RuntimeError, 'checksum gate'):
                service.main()
            start.assert_not_called()
            stop.assert_not_called()
            self.assertEqual((private/'power.acl').read_text(), 'prior startup configuration')

    def test_a_changed_model_cannot_replace_an_existing_service(self):
        with tempfile.TemporaryDirectory() as directory, ExitStack() as stack:
            root, _ = self.environment(directory, stack)
            (root/'models').mkdir(parents=True)
            (root/'models/V41-XQ-1017-v3.gguf').write_bytes(b'changed')
            (root/'models/verified-model.json').write_text(json.dumps(
                {'sha256': service.MODEL_SHA256, 'size': 1, 'mtime_ns': 1}))
            start = stack.enter_context(patch.object(service, 'start_process'))
            stop = stack.enter_context(patch.object(service, 'stop_owned'))
            with self.assertRaisesRegex(RuntimeError, 'changed after checksum verification'):
                service.main()
            start.assert_not_called()
            stop.assert_not_called()

    def test_an_unverified_draft_model_cannot_enable_speculation(self):
        with tempfile.TemporaryDirectory() as directory, ExitStack() as stack:
            self.environment(directory, stack)
            staging = Path(directory)/'staging/assets'
            staging.mkdir(parents=True)
            (staging/'V41-MTP-Vision-1017.Q2_K.gguf').write_bytes(b'incomplete draft')
            with self.assertRaisesRegex(RuntimeError, 'sidecar failed the pinned SHA-256'):
                service.engine_config('split8', 'dspark', 131072)

    def test_batching_is_enabled_only_without_layer_splitting(self):
        for parallel in (4, 8, 16):
            with self.subTest(parallel=parallel):
                config = service.engine_config('replica8x1', 'exact', 32768, parallel=parallel)
                self.assertEqual(config['replicas'], 8)
                self.assertEqual(config['parallel'], parallel)
                self.assertNotIn('--layer-split', config['args'])
                self.assertNotIn('layer_split', config)
            for layout in ('split8', 'replica4x2'):
                with self.subTest(layout=layout, parallel=parallel), self.assertRaisesRegex(ValueError, 'batch slots require'):
                    service.engine_config(layout, 'exact', 32768, parallel=parallel)
        for invalid in (0, 3, 17, True):
            with self.subTest(invalid=invalid), self.assertRaisesRegex(ValueError, 'parallel slots'):
                service.engine_config('replica8x1', 'exact', 32768, parallel=invalid)

    def test_http_200_without_loaded_weights_does_not_pass_engine_readiness(self):
        process = MagicMock()
        process.poll.side_effect = [None, 3]
        response = MagicMock()
        response.__enter__.return_value = response
        response.status = 200
        response.read.return_value = b'{"status":"ok","loaded":false}'
        with patch.object(service.urllib.request, 'urlopen', return_value=response), patch.object(service.time, 'sleep'):
            with self.assertRaisesRegex(RuntimeError, 'exited before becoming healthy'):
                service.wait_health(process, 8150, 30, require_loaded=True)

    @unittest.skipUnless(sys.platform == 'linux', 'Linux process ownership gate')
    def test_owned_children_are_stopped_even_after_the_launcher_exits(self):
        with tempfile.TemporaryDirectory() as directory, ExitStack() as stack:
            root, private = self.environment(directory, stack)
            (root/'logs').mkdir(parents=True)
            private.mkdir(parents=True)
            child_file = Path(directory)/'child.pid'
            script = Path(directory)/'launcher.py'
            script.write_text(
                'import subprocess,sys\nfrom pathlib import Path\n'
                'p=subprocess.Popen([sys.executable,"-c","import time;time.sleep(60)"])\n'
                f'Path({str(child_file)!r}).write_text(str(p.pid))\n')
            parent = service.start_process('xolotl', [sys.executable, str(script)], dict(os.environ))
            child = None
            try:
                parent.wait(timeout=5)
                child = int(child_file.read_text())
                self.assertTrue(Path(f'/proc/{child}/cmdline').read_bytes())
                service.stop_owned('xolotl', script)
                command = Path(f'/proc/{child}/cmdline')
                self.assertFalse(command.exists() and bool(command.read_bytes()),
                                 'the exited launcher left its owned child running')
            finally:
                try:
                    os.killpg(parent.pid, signal.SIGKILL)
                except ProcessLookupError:
                    pass
                if parent.poll() is None:
                    parent.wait(timeout=5)

    @unittest.skipUnless(sys.platform == 'linux', 'Linux process ownership gate')
    def test_a_matching_command_with_a_different_owner_is_not_stopped(self):
        with tempfile.TemporaryDirectory() as directory, patch.object(service, 'PRIVATE', Path(directory)):
            script = Path(directory)/'other.py'
            script.write_text('import time;time.sleep(60)')
            child = subprocess.Popen([sys.executable, str(script)], start_new_session=True)
            try:
                (Path(directory)/'power.pid').write_text(json.dumps({'pid': child.pid, 'marker': 'f'*32}))
                with self.assertRaisesRegex(RuntimeError, 'unrelated process group'):
                    service.stop_owned('power', script)
                self.assertIsNone(child.poll())
            finally:
                child.terminate()
                child.wait(timeout=5)

    @unittest.skipUnless(sys.platform == 'linux', 'Linux process ownership gate')
    def test_a_reused_pid_does_not_terminate_an_unrelated_process(self):
        with tempfile.TemporaryDirectory() as directory, patch.object(service, 'PRIVATE', Path(directory)):
            child = subprocess.Popen([sys.executable, '-c', 'import time; time.sleep(60)'], start_new_session=True)
            try:
                (Path(directory)/'power.pid').write_text(str(child.pid))
                with self.assertRaisesRegex(RuntimeError, 'unrelated'):
                    service.stop_owned('power', Path('/not/this/deployment/aster-power'))
                self.assertIsNone(child.poll())
            finally:
                child.terminate()
                child.wait(timeout=5)


class UsageModeTests(unittest.TestCase):
    def fixture(self, directory, stack):
        root = Path(directory)/'root'
        private = Path(directory)/'private'
        private.mkdir()
        (root/'run').mkdir(parents=True)
        (private/'api.key').write_text('a'*64)
        profile = {'model': service.MODEL_NAME, 'port': service.POWER_PORT,
                   'upstream_port': service.UPSTREAM_PORT, 'max_concurrent': 4,
                   'power_pid': 100, 'xolotl_pid': 777, 'exact_benchmark_usage': False}
        (root/'run/active-profile.json').write_text(json.dumps(profile))
        stack.enter_context(patch.object(service, 'ROOT', root))
        stack.enter_context(patch.object(service, 'PRIVATE', private))
        stack.enter_context(patch.object(service, 'assert_frontend_owned'))
        return root, private

    def test_acl_has_private_normal_default_and_explicit_exact_mode(self):
        with tempfile.TemporaryDirectory() as directory, patch.object(service, 'PRIVATE', Path(directory)):
            normal = service.write_power_acl(4, 'b'*64).read_text()
            self.assertIn('redact_logs = true', normal)
            exact = service.write_power_acl(4, 'b'*64, True).read_text()
            self.assertIn('redact_logs = false', exact)
            for concurrent in (0, True, 129):
                with self.assertRaises(ValueError):
                    service.write_power_acl(concurrent, 'b'*64)
            with self.assertRaises(ValueError):
                service.write_power_acl(4, 'b'*64, 'true')

    def test_usage_mode_restarts_only_the_owned_power_frontend(self):
        with tempfile.TemporaryDirectory() as directory, ExitStack() as stack:
            root, private = self.fixture(directory, stack)
            stack.enter_context(patch.object(service, 'frontend_is_idle', return_value=True))
            stop = stack.enter_context(patch.object(service, 'stop_owned'))
            start = stack.enter_context(patch.object(service, 'start_process', side_effect=[MagicMock(pid=101), MagicMock(pid=102)]))
            stack.enter_context(patch.object(service, 'wait_health'))
            exact = service._set_frontend_usage_mode(True)
            self.assertTrue(exact['exact_benchmark_usage'])
            self.assertEqual(exact['xolotl_pid'], 777)
            normal = service._set_frontend_usage_mode(False)
            self.assertFalse(normal['exact_benchmark_usage'])
            self.assertEqual(normal['xolotl_pid'], 777)
            self.assertTrue(all(call.args[0] == 'power' for call in stop.call_args_list))
            self.assertTrue(all(call.args[0] == 'power' for call in start.call_args_list))
            self.assertIn('redact_logs = true', (private/'power.acl').read_text())

    def test_busy_frontend_cannot_change_configuration_or_restart(self):
        with tempfile.TemporaryDirectory() as directory, ExitStack() as stack:
            _, private = self.fixture(directory, stack)
            stack.enter_context(patch.object(service, 'frontend_is_idle', return_value=False))
            start = stack.enter_context(patch.object(service, 'start_process'))
            stop = stack.enter_context(patch.object(service, 'stop_owned'))
            with self.assertRaisesRegex(RuntimeError, 'busy'):
                service._set_frontend_usage_mode(True)
            self.assertFalse((private/'power.acl').exists())
            start.assert_not_called()
            stop.assert_not_called()

    def test_failed_start_restores_previous_privacy_profile(self):
        with tempfile.TemporaryDirectory() as directory, ExitStack() as stack:
            root, private = self.fixture(directory, stack)
            stack.enter_context(patch.object(service, 'frontend_is_idle', return_value=True))
            stack.enter_context(patch.object(service, 'stop_owned'))
            start = stack.enter_context(patch.object(service, 'start_process', side_effect=[MagicMock(pid=101), MagicMock(pid=102)]))
            stack.enter_context(patch.object(service, 'wait_health', side_effect=[RuntimeError('new frontend failed'), None]))
            with self.assertRaisesRegex(RuntimeError, 'new frontend failed'):
                service._set_frontend_usage_mode(True)
            self.assertEqual(start.call_count, 2)
            self.assertFalse(json.loads((root/'run/active-profile.json').read_text())['exact_benchmark_usage'])
            self.assertIn('redact_logs = true', (private/'power.acl').read_text())

    def test_failed_normal_restore_stops_frontend_without_reenabling_exact_mode(self):
        with tempfile.TemporaryDirectory() as directory, ExitStack() as stack:
            root, _ = self.fixture(directory, stack)
            profile = root/'run/active-profile.json'
            record = json.loads(profile.read_text())
            record['exact_benchmark_usage'] = True
            profile.write_text(json.dumps(record))
            stack.enter_context(patch.object(service, 'frontend_is_idle', return_value=True))
            stack.enter_context(patch.object(service, 'stop_owned'))
            start = stack.enter_context(patch.object(service, 'start_process', return_value=MagicMock(pid=101)))
            stack.enter_context(patch.object(service, 'wait_health', side_effect=RuntimeError('cannot restore')))
            with self.assertRaisesRegex(RuntimeError, 'cannot restore'):
                service._set_frontend_usage_mode(False)
            self.assertEqual(start.call_count, 1)
            retained = json.loads(profile.read_text())
            self.assertTrue(retained['frontend_unavailable'])
            self.assertFalse(retained['normal_privacy_restored'])

    def test_changed_acl_cannot_be_trusted_as_the_running_usage_mode(self):
        with tempfile.TemporaryDirectory() as directory, ExitStack() as stack:
            root = Path(directory)/'root'
            private = Path(directory)/'private'
            (root/'bin').mkdir(parents=True)
            private.mkdir()
            (root/'bin/aster-power').write_bytes(b'pinned synthetic binary')
            (private/'power.acl').write_text('changed after startup')
            stack.enter_context(patch.object(service, 'ROOT', root))
            stack.enter_context(patch.object(service, 'PRIVATE', private))
            stack.enter_context(patch.object(service, 'POWER_SHA256', hashlib.sha256(b'pinned synthetic binary').hexdigest()))
            with self.assertRaisesRegex(RuntimeError, 'ACL changed'):
                service.assert_frontend_owned({'power_acl_sha256': '0'*64})

    def test_synthetic_context_restores_on_body_or_enable_failure(self):
        for enable_failure in (False, True):
            state = {}
            effects = [RuntimeError('enable failed'), {}] if enable_failure else [{}, {}]
            with patch.object(service, 'set_frontend_usage_mode', side_effect=effects) as update:
                with self.assertRaises(RuntimeError):
                    with service.synthetic_usage_mode(state):
                        raise RuntimeError('request failed')
                self.assertEqual([call.args[0] for call in update.call_args_list], [True, False])
                self.assertTrue(state['normal_privacy_restored'])

    def test_failed_restoration_is_never_reported_as_normal_privacy(self):
        state = {}
        with patch.object(service, 'set_frontend_usage_mode', side_effect=[{}, OSError('restore failed')]):
            with self.assertRaises(OSError):
                with service.synthetic_usage_mode(state):
                    pass
        self.assertFalse(state['normal_privacy_restored'])
        self.assertEqual(state['restoration_error_type'], 'OSError')

    def test_idle_requires_complete_unambiguous_admission_metrics(self):
        def idle(raw):
            response = MagicMock()
            response.__enter__.return_value = response
            response.read.return_value = raw.encode()
            with patch.object(service.urllib.request, 'urlopen', return_value=response):
                return service.frontend_is_idle()
        normal = 'power_active_requests 0\npower_requests_waiting 0\npower_requests_running 0\n'
        self.assertTrue(idle(normal))
        self.assertFalse(idle(normal.replace('power_requests_waiting 0', 'power_requests_waiting 1')))
        for raw in ('power_active_requests 0\n', normal+'power_active_requests 0\n',
                    normal.replace('power_requests_running 0', 'power_requests_running -1')):
            with self.subTest(raw=raw), self.assertRaises(RuntimeError):
                idle(raw)


if __name__ == '__main__':
    unittest.main()
