"""Hermetic boundaries for the optional real context qualification helper."""
import importlib.util
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
import threading
import time
import unittest
from unittest import mock

PATH = Path(__file__).resolve().parents[1] / 'docs/evidence/power/2026-10-21-serving-optimization/context-scenarios.py'
spec = importlib.util.spec_from_file_location('power_serving_context', PATH)
c = importlib.util.module_from_spec(spec)
spec.loader.exec_module(c)


class ContextGuards(unittest.TestCase):
    def test_http_post_has_an_absolute_slow_body_deadline(self):
        class SlowFixture(BaseHTTPRequestHandler):
            def log_message(self, *_args):
                pass

            def do_POST(self):
                self.rfile.read(int(self.headers['Content-Length']))
                try:
                    self.send_response(200)
                    self.send_header('Connection', 'close')
                    self.end_headers()
                    self.close_connection = True
                    for byte in b'{"tokens":[1,2,3,4,5,6]}':
                        self.wfile.write(bytes([byte]))
                        self.wfile.flush()
                        time.sleep(.006)
                except OSError:
                    pass

        server = ThreadingHTTPServer(('127.0.0.1', 0), SlowFixture)
        thread = threading.Thread(target=server.serve_forever, daemon=True)
        thread.start()
        try:
            started = time.monotonic()
            with self.assertRaises(Exception):
                c.post_json(server.server_port, '/tokenize', {'content': 'synthetic'}, .03)
            self.assertLess(time.monotonic() - started, .18)
        finally:
            server.shutdown()
            server.server_close()
            thread.join()

    def test_near_limit_requires_actual_usage_and_reserved_output(self):
        record = {'outcome': 'success', 'ttft_ms': None, 'usage': {'prompt_tokens': 31744}}
        delta = {'llamacpp:prompt_tokens_total': 31000, 'llamacpp:prompt_tokens_cached_total': 744}
        actual, checks = c.prompt_checks(record, delta, True)
        self.assertEqual(actual, 31744)
        self.assertTrue(all(checks.values()))
        for reported, computed in [(31744, 8192), (32760, 32760), (31743, 31744)]:
            with self.subTest(reported=reported, computed=computed):
                record['usage']['prompt_tokens'] = reported
                delta['llamacpp:prompt_tokens_total'] = computed
                delta['llamacpp:prompt_tokens_cached_total'] = 0
                self.assertFalse(all(c.prompt_checks(record, delta, True)[1].values()))

    def test_tokenizer_does_not_accept_malformed_or_unbounded_shapes(self):
        for status, value in [(400, {'tokens': [1]}), (200, {'tokens': [True]}), (200, {'tokens': [1.5]}), (200, {'error': {}}), (200, {'tokens': [1] * 40001})]:
            with mock.patch.object(c, 'post_json', return_value=(status, value, 'digest', 0)):
                with self.assertRaises(c.m.GuardError):
                    c.count_tokens('synthetic', c.e.Budget(1))

    def test_calibration_uses_actual_tokenizer_counts(self):
        with mock.patch.object(c, 'count_tokens', side_effect=lambda text, _budget: len(text) // 2):
            text, proof = c.calibrate(31744, c.e.Budget(1))
        self.assertEqual(len(text) // 2, 31744)
        self.assertEqual(proof['observed_tokens'], 31744)
        self.assertGreater(len(proof['probes']), 1, 'must calibrate from tokenizer counts instead of character length')


if __name__ == '__main__':
    unittest.main()
