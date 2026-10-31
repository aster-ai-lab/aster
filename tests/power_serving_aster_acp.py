"""Actual existing ACP against a hermetic OpenAI fixture; no public requests."""
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import importlib.util
import json
from pathlib import Path
import tempfile
import threading
import unittest
from unittest import mock

PATH = Path(__file__).resolve().parents[1] / 'docs/evidence/power/2026-10-21-serving-optimization/aster-acp-two-turn.py'
spec = importlib.util.spec_from_file_location('aster_acp_two_turn_fixture', PATH)
a = importlib.util.module_from_spec(spec)
spec.loader.exec_module(a)
KEY = 'fixture-only-private-marker-83c1b627'


class AcpQualification(unittest.TestCase):
    def test_acl_keeps_environment_expression_and_child_has_no_inherited_home_or_proxies(self):
        root = Path('/fresh-isolated')
        config = a.acl(root, 'http://127.0.0.1:9/v1')
        self.assertIn('api_key = env("ASTER_GATEWAY_KEY")', config)
        self.assertNotIn(KEY, config)
        env = a.child_environment(root, KEY)
        self.assertEqual(env['ASTER_GATEWAY_KEY'], KEY)
        self.assertEqual(env['ASTER_LOG_FILE'], '0')
        self.assertEqual(env['RUST_LOG'], 'off')
        for name in ('HOME', 'HOMEDRIVE', 'HOMEPATH', 'HTTP_PROXY', 'HTTPS_PROXY', 'ALL_PROXY'):
            self.assertNotIn(name, env)

    def test_state_scan_detects_resolved_raw_and_encoded_secret_without_exporting_it(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            path = root / 'state.json'
            path.write_bytes(b'{"safe":"synthetic"}')
            self.assertFalse(a.scan_state(root, KEY)['resolved_credential_found'])
            for variant in a.secret_variants(KEY):
                path.write_bytes(variant)
                with self.assertRaises(a.Failure) as error:
                    a.scan_state(root, KEY)
                self.assertNotIn(KEY, str(error.exception))

    @unittest.skipUnless(a.BINARY.is_file(), 'existing pinned Windows ACP binary unavailable')
    def test_actual_acp_two_turns_preserve_history_and_never_persist_resolved_secret(self):
        captured = []
        class Handler(BaseHTTPRequestHandler):
            protocol_version = 'HTTP/1.1'
            def do_POST(self):
                self.assert_request()
                text = 'READY'
                chunk = {'id': 'fixture', 'object': 'chat.completion.chunk', 'model': 'prism-27b',
                         'choices': [{'index': 0, 'delta': {'role': 'assistant', 'content': text}, 'finish_reason': None}]}
                finish = dict(chunk, choices=[{'index': 0, 'delta': {}, 'finish_reason': 'stop'}])
                usage = dict(chunk, choices=[], usage={'prompt_tokens': 31, 'completion_tokens': 3, 'total_tokens': 34})
                body = ''.join('data: ' + json.dumps(row) + '\n\n' for row in (chunk, finish, usage)) + 'data: [DONE]\n\n'
                raw = body.encode()
                self.send_response(200)
                self.send_header('Content-Type', 'text/event-stream')
                self.send_header('Content-Length', str(len(raw)))
                self.send_header('Connection', 'close')
                self.end_headers()
                self.wfile.write(raw)
                self.wfile.flush()
            def assert_request(self):
                if self.path != '/v1/chat/completions' or self.headers.get('Authorization') != 'Bearer ' + KEY:
                    raise AssertionError('unexpected fixture route or authentication')
                length = int(self.headers['Content-Length'])
                if not 0 < length <= 1024 * 1024:
                    raise AssertionError('fixture body exceeds bound')
                captured.append(json.loads(self.rfile.read(length)))
                if len(captured) > 8:
                    raise AssertionError('bounded fixture model request count exhausted')
            def log_message(self, *args):
                pass
        server = ThreadingHTTPServer(('127.0.0.1', 0), Handler)
        server.daemon_threads = True
        thread = threading.Thread(target=server.serve_forever, daemon=True)
        thread.start()
        try:
            with tempfile.TemporaryDirectory() as directory:
                # ACP can issue internal planning/final-response model calls.
                # The fixture tests protocol/history, with the same deterministic
                # response for every internal request; live expectations stay fixed.
                with mock.patch.object(a, 'EXPECTED', ('READY', 'READY')):
                    result = a.dialogue(Path(directory) / 'fresh-session',
                                        'http://127.0.0.1:' + str(server.server_port) + '/v1', KEY)
                self.assertEqual(result['status'], 'passed', result)
                self.assertGreaterEqual(len(captured), 2)
                self.assertLessEqual(len(captured), 8)
                self.assertTrue(all(not body.get('tools') for body in captured))
                messages = captured[-1]['messages']
                self.assertTrue(any(row.get('role') == 'assistant' and row.get('content') == a.EXPECTED[0] for row in messages))
                self.assertFalse(result['state_privacy']['resolved_credential_found'])
                self.assertTrue(result['own_child_reaped'])
                self.assertNotIn(KEY, json.dumps(result))
        finally:
            server.shutdown()
            server.server_close()
            thread.join(timeout=2)


if __name__ == '__main__':
    unittest.main()
