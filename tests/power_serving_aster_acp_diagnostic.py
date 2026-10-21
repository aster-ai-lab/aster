"""Bounded diagnostic observations; the real ACP talks only to a loopback fixture."""
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import importlib.util
import io
import json
from pathlib import Path
import socket
import struct
import tempfile
import threading
import time
import unittest
from unittest import mock

PATH = Path(__file__).resolve().parents[1] / 'docs/evidence/power/2026-10-21-serving-optimization/aster-acp-diagnostic.py'
spec = importlib.util.spec_from_file_location('aster_acp_diagnostic_fixture', PATH)
d = importlib.util.module_from_spec(spec)
spec.loader.exec_module(d)
KEY = 'fixture-only-diagnostic-marker-51767d98'
MOCK_METADATA = []


class DiagnosticQualification(unittest.TestCase):
    def test_partial_output_is_counted_without_a_completed_prompt_response(self):
        observer = d.Observer(KEY)
        observer.observe(json.dumps({'method': 'session/prompt', 'id': 3}).encode(), False)
        for text in (' ', 'READY'):
            observer.observe(json.dumps({'method': 'session/update', 'params': {'update': {
                'sessionUpdate': 'agent_message_chunk', 'content': {'type': 'text', 'text': text}}}}).encode(), True)
        result = observer.summary()
        self.assertEqual(result['text_by_turn'][1]['deltas'], 2)
        self.assertEqual(result['text_by_turn'][1]['meaningful_deltas'], 1)
        self.assertEqual(result['text_by_turn'][1]['chars'], 6)
        self.assertEqual(result['text_by_turn'][1]['output_sha256'], d.a.digest(b' READY'))
        self.assertNotIn('READY', json.dumps(result))
        self.assertIn(3, observer.pending)

    def test_observer_failure_preserves_original_io_and_never_exports_error_text(self):
        observer = d.Observer(KEY)
        with mock.patch.object(observer, 'observe', side_effect=RuntimeError(KEY)):
            self.assertEqual(observer.pipe(io.BytesIO(b'{}\n'), True).readline(), b'{}\n')
            output = io.BytesIO()
            self.assertEqual(observer.pipe(output, False).write(b'{}\n'), 3)
            self.assertEqual(output.getvalue(), b'{}\n')
        self.assertEqual(len(observer.summary()['observer_errors']), 2)
        self.assertNotIn(KEY, json.dumps(observer.summary()))

    def test_created_child_is_returned_to_cleanup_owner_when_observer_setup_fails(self):
        child = mock.Mock(stdin=io.BytesIO(), stdout=io.BytesIO())
        received = []
        def dialogue(*args, **kwargs):
            owned = d.subprocess.Popen([str(d.a.BINARY)])
            received.append(owned)
            try:
                return {'status': 'passed'}
            finally:
                owned.terminate()
                owned.wait(timeout=1)
        with mock.patch.object(d.subprocess, 'Popen', return_value=child), mock.patch.object(d.a, 'dialogue', side_effect=dialogue), mock.patch.object(d.Observer, 'pipe', side_effect=RuntimeError(KEY)):
            result = d.run(Path('/unused'), 'http://127.0.0.1:9/v1', KEY)
        self.assertEqual(received, [child])
        child.terminate.assert_called_once()
        child.wait.assert_called_once_with(timeout=1)
        self.assertEqual(result['diagnostic_status'], 'unsupported')
        self.assertNotIn(KEY, json.dumps(result))

    def test_30second_checkpoint_keeps_partial_meaningful_count(self):
        observer = d.Observer(KEY)
        observer.turn = 1
        observer.pending[3] = time.monotonic() - 30.5
        observer.text[1]['meaningful_deltas'] = 2
        child = mock.Mock(pid=42)
        child.poll.side_effect = [None, 0]
        with mock.patch.object(d, 'creation_ticks', return_value=1234), mock.patch.object(d, 'owned_tcp', return_value=[]), mock.patch.object(d.time, 'sleep'):
            observer.follow(child, ('117.162.0.179', 8080))
        checkpoint = [row for row in observer.events if row['kind'] == 'prompt-incomplete-at30seconds']
        self.assertEqual(len(checkpoint), 1)
        self.assertEqual(checkpoint[0]['meaningful_deltas'], 2)
        self.assertEqual(observer.tcp[0]['creation_filetime'], 1234)

    def test_exited_owned_process_identity_race_is_normal(self):
        observer, child = d.Observer(KEY), mock.Mock(pid=42)
        child.poll.return_value = 0
        with mock.patch.object(d, 'creation_ticks', side_effect=OSError('already exited')):
            observer.follow(child, ('117.162.0.179', 8080))
        self.assertEqual(observer.errors, [])

    def test_tcp_rows_filter_exact_owner_and_hash_other_endpoints(self):
        def row(pid, host, port, state=5):
            return struct.pack('<6I', state, 0, socket.htons(55123), struct.unpack('<I', socket.inet_aton(host))[0], socket.htons(port), pid)
        raw = struct.pack('<I', 3) + row(42, '117.162.0.179', 8080, 3) + row(43, '117.162.0.179', 8080) + row(42, '192.0.2.77', 99)
        result = d.tcp_rows(raw, 42, ('117.162.0.179', 8080))
        self.assertEqual(len(result), 2)
        self.assertEqual(result[0], {'state': 'SynSent', 'local_port': 55123, 'remote_class': 'public-gateway', 'remote_port': 8080})
        self.assertNotIn('192.0.2.77', json.dumps(result))
        with self.assertRaises(d.a.Failure):
            d.tcp_rows(raw[:-1], 42, ('117.162.0.179', 8080))

    def test_metadata_uses_known_scalars_and_hashes_text_and_rejects_secret(self):
        raw = json.dumps({'messages': [{'role': 'user', 'content': 'synthetic-only'}], 'stream': True,
                          'max_tokens': 128, 'thinking': {'type': 'enabled', 'budget_tokens': 64},
                          'stop': ['PRIVATE-STOP'], 'arbitrary': KEY}).encode()
        with self.assertRaises(d.a.Failure):
            d.request_metadata(raw, KEY)
        raw = raw.replace(KEY.encode(), b'not-a-secret')
        result = d.request_metadata(raw, KEY)
        self.assertEqual(result['known_options']['max_tokens'], 128)
        self.assertTrue(result['known_options']['stream'])
        self.assertNotIn('PRIVATE-STOP', json.dumps(result))
        self.assertNotIn('synthetic-only', json.dumps(result))
        self.assertNotIn('arbitrary', result)

    @unittest.skipUnless(d.a.BINARY.is_file(), 'existing pinned Windows ACP unavailable')
    def test_actual_unchanged_acp_loopback_records_safe_request_options_and_tcp(self):
        class Handler(BaseHTTPRequestHandler):
            protocol_version = 'HTTP/1.1'
            def do_POST(self):
                if self.path != '/v1/chat/completions' or self.headers.get('Authorization') != 'Bearer ' + KEY:
                    raise AssertionError('wrong mock route or authentication')
                size = int(self.headers['Content-Length'])
                if not 0 < size <= 1024 * 1024 or len(MOCK_METADATA) >= 16:
                    raise AssertionError('mock request count or size bound')
                raw_request = self.rfile.read(size)
                MOCK_METADATA.append(d.request_metadata(raw_request, KEY))
                chunk = {'id': 'fixture', 'object': 'chat.completion.chunk', 'model': 'prism-27b',
                         'choices': [{'index': 0, 'delta': {'role': 'assistant', 'content': 'READY'}, 'finish_reason': None}]}
                finish = dict(chunk, choices=[{'index': 0, 'delta': {}, 'finish_reason': 'stop'}])
                usage = dict(chunk, choices=[], usage={'prompt_tokens': 31, 'completion_tokens': 3, 'total_tokens': 34})
                streaming = json.loads(raw_request).get('stream') is True
                raw = (''.join('data: ' + json.dumps(row) + '\n\n' for row in (chunk, finish, usage)) + 'data: [DONE]\n\n').encode() if streaming else json.dumps({
                    'id': 'fixture', 'object': 'chat.completion', 'model': 'prism-27b',
                    'choices': [{'index': 0, 'message': {'role': 'assistant', 'content': 'READY'}, 'finish_reason': 'stop'}],
                    'usage': {'prompt_tokens': 31, 'completion_tokens': 3, 'total_tokens': 34}}).encode()
                self.send_response(200)
                self.send_header('Content-Type', 'text/event-stream' if streaming else 'application/json')
                self.send_header('Content-Length', str(len(raw)))
                self.send_header('Connection', 'close')
                self.end_headers()
                self.wfile.write(raw)
                self.wfile.flush()
            def log_message(self, *args):
                pass
        MOCK_METADATA.clear()
        server = ThreadingHTTPServer(('127.0.0.1', 0), Handler)
        server.daemon_threads = True
        thread = threading.Thread(target=server.serve_forever, daemon=True)
        thread.start()
        try:
            with tempfile.TemporaryDirectory() as directory, mock.patch.object(d.a, 'EXPECTED', ('READY', 'READY')):
                result = d.run(Path(directory) / 'fresh', 'http://127.0.0.1:' + str(server.server_port) + '/v1', KEY, turn_seconds=5, total_seconds=15)
            self.assertEqual(result['status'], 'passed', result)
            self.assertEqual(result['diagnostic_status'], 'observed', result)
            self.assertTrue(result['own_child_reaped'])
            self.assertTrue(result['diagnostic_threads_terminated'])
            self.assertGreater(len(result['diagnostic']['tcp_samples']), 0)
            self.assertEqual(len([row for row in result['diagnostic']['events'] if row['kind'] == 'owned-child']), 1)
            self.assertGreaterEqual(len(MOCK_METADATA), 2)
            self.assertTrue(all(row['tools_count'] == 0 for row in MOCK_METADATA))
            self.assertGreaterEqual(sum(row['known_options'].get('stream') is True for row in MOCK_METADATA), 2)
            self.assertTrue(all(row['known_options']['max_tokens'] == 128 for row in MOCK_METADATA))
            self.assertTrue(any(row['known_options'].get('stream') is True and row['role_counts'].get('assistant', 0) > 0 for row in MOCK_METADATA))
            self.assertNotIn(KEY, json.dumps(result) + json.dumps(MOCK_METADATA))
        finally:
            server.shutdown()
            server.server_close()
            thread.join(timeout=2)


if __name__ == '__main__':
    suite = unittest.defaultTestLoader.loadTestsFromTestCase(DiagnosticQualification)
    result = unittest.TextTestRunner(verbosity=2).run(suite)
    print(json.dumps({'mock_actual_acp_request_metadata': MOCK_METADATA}))
    raise SystemExit(0 if result.wasSuccessful() else 1)
