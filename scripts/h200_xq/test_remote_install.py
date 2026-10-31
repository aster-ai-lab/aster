"""Nonweight preparation must be complete before reserving the GPU job."""
import gzip
import hashlib
import json
import tempfile
import unittest
import subprocess
from contextlib import ExitStack
from pathlib import Path
from unittest.mock import patch

import remote_install


class PreparationTests(unittest.TestCase):
    def fixture(self, root, stack):
        expected = {}
        for name in remote_install.STAGING_SHA256:
            path = root/name
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes(('fixture '+name).encode())
        payload = b'ggml source'
        source = root/'source/llama/ggml/example.c'
        source.parent.mkdir(parents=True)
        source.write_bytes(payload)
        tree = {'sha': remote_install.LLAMA_REVISION, 'truncated': False, 'tree': [
            {'type': 'blob', 'mode': '100644', 'path': 'ggml/example.c', 'size': len(payload),
             'sha': hashlib.sha1(f'blob {len(payload)}\0'.encode()+payload).hexdigest()}]}
        (root/'source/llama-tree.json').write_text(json.dumps(tree))
        for name in remote_install.STAGING_SHA256:
            expected[name] = remote_install.digest(root/name)
        (root/'source/SOURCES_READY').write_text(remote_install.XOLOTL_REVISION+'\n'+remote_install.LLAMA_REVISION+'\n')
        with gzip.open(root/'aster-power-glibc235.gz', 'wb') as output:
            output.write(b'prepared Power')
        operator = root/'service.py'
        operator.write_bytes(b'operator source')
        (root/'operator-files.json').write_text(json.dumps([
            {'file': 'service.py', 'sha256': remote_install.digest(operator)}]))
        stack.enter_context(patch.object(remote_install, 'STAGING_SHA256', expected))
        stack.enter_context(patch.object(remote_install, 'POWER_SHA256', hashlib.sha256(b'prepared Power').hexdigest()))
        stack.enter_context(patch.object(remote_install, 'STAGING', root))

    def test_preparation_succeeds_without_any_gguf_weights(self):
        with tempfile.TemporaryDirectory() as directory, ExitStack() as stack:
            self.fixture(Path(directory), stack)
            record = remote_install.run_preflight()
            self.assertTrue(record['nonweight_ready'])
            self.assertFalse(record['main_weights_checked'])
            self.assertFalse(record['gpu_inference_tested'])
            self.assertEqual(record['ggml_verified_files'], 1)

    def test_corrupt_power_binary_is_refused(self):
        with tempfile.TemporaryDirectory() as directory, ExitStack() as stack:
            root = Path(directory); self.fixture(root, stack)
            with gzip.open(root/'aster-power-glibc235.gz', 'wb') as output:
                output.write(b'wrong binary')
            with self.assertRaisesRegex(RuntimeError, 'Power binary checksum'):
                remote_install.verify_staging(root)

    def test_same_length_ggml_corruption_is_refused(self):
        with tempfile.TemporaryDirectory() as directory, ExitStack() as stack:
            root = Path(directory); self.fixture(root, stack)
            (root/'source/llama/ggml/example.c').write_bytes(b'GGML source')
            with self.assertRaisesRegex(RuntimeError, 'Git blob mismatch'):
                remote_install.verify_staging(root)

    def test_changed_operator_bundle_is_refused(self):
        with tempfile.TemporaryDirectory() as directory, ExitStack() as stack:
            root = Path(directory); self.fixture(root, stack)
            (root/'service.py').write_bytes(b'changed operator')
            with self.assertRaisesRegex(RuntimeError, 'operator checksum'):
                remote_install.verify_staging(root)

    def test_failed_preflight_replaces_a_previous_ready_report(self):
        with tempfile.TemporaryDirectory() as directory, ExitStack() as stack:
            root = Path(directory); self.fixture(root, stack)
            remote_install.run_preflight()
            (root/'assets/tokenizer/vocab.json').write_bytes(b'corrupt tokenizer')
            with self.assertRaisesRegex(RuntimeError, 'artifact checksum'):
                remote_install.run_preflight()
            record = json.loads((root/'logs/nonweight-preflight.json').read_text())
            self.assertFalse(record['nonweight_ready'])
            self.assertEqual(record['error_type'], 'RuntimeError')

    def test_future_software_bundle_can_be_verified_before_activation(self):
        with tempfile.TemporaryDirectory() as directory, ExitStack() as stack:
            root = Path(directory); self.fixture(root, stack)
            bundle = root/'prepared-release'
            bundle.mkdir()
            for name in ('aster-power-glibc235.gz', 'operator-files.json', 'service.py'):
                (bundle/name).write_bytes((root/name).read_bytes())
            (root/'service.py').write_bytes(b'old active operator version')
            record = remote_install.verify_staging(root, bundle)
            self.assertEqual(record['software_bundle'], 'prepared-release')
            self.assertTrue(record['nonweight_ready'])
            with self.assertRaisesRegex(RuntimeError, 'operator checksum'):
                remote_install.verify_staging(root)
            with self.assertRaisesRegex(RuntimeError, 'inside the owned staging'):
                remote_install.verify_staging(root, root.parent)

    def test_failed_campaign_still_restores_normal_frontend_privacy(self):
        failure = subprocess.CalledProcessError(1, ['synthetic-campaign'])
        with patch.object(remote_install.subprocess, 'run', side_effect=[failure, None]) as run:
            with self.assertRaises(subprocess.CalledProcessError):
                remote_install.run_autotune()
            self.assertEqual(run.call_count, 2)
            self.assertEqual(run.call_args_list[1].args[0][-1], 'set-usage-mode')
            self.assertNotIn('--exact-benchmark-usage', run.call_args_list[1].args[0])


if __name__ == '__main__':
    unittest.main()
