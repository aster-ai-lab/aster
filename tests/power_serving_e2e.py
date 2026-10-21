"""Small hermetic guards for the bounded real E2E qualification driver."""
import importlib.util
import json
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
import threading
import time
import unittest

ROOT = Path(__file__).resolve().parents[1]
PATH = ROOT / 'docs/evidence/power/2026-10-21-serving-optimization/e2e-scenarios.py'
spec = importlib.util.spec_from_file_location('power_serving_e2e', PATH)
e = importlib.util.module_from_spec(spec)
spec.loader.exec_module(e)


class Fixture(BaseHTTPRequestHandler):
    def log_message(self, *_args):
        pass

    def do_POST(self):
        raw = self.rfile.read(int(self.headers['Content-Length']))
        try:
            if raw == b'slow':
                for byte in b'HTTP/1.1 400 Bad Request\r\nConnection: close\r\n\r\n':
                    self.connection.sendall(bytes([byte]))
                    time.sleep(.006)
                return
            body = json.dumps({'error': {'code': 'unauthorized', 'type': 'invalid_request_error',
                                         'message': 'api_key:DO_NOT_RECORD_THIS_SECRET'}}).encode()
            self.send_response(401)
            self.send_header('Content-Length', str(len(body)))
            self.send_header('Connection', 'close')
            self.end_headers()
            self.wfile.write(body)
            self.close_connection = True
        except OSError:
            pass


class E2EGuards(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.server = ThreadingHTTPServer(('127.0.0.1', 0), Fixture)
        cls.thread = threading.Thread(target=cls.server.serve_forever, daemon=True)
        cls.thread.start()

    @classmethod
    def tearDownClass(cls):
        cls.server.shutdown()
        cls.server.server_close()
        cls.thread.join()

    def test_rejection_records_type_code_without_raw_content_or_credentials(self):
        row = e.raw_post(self.server.server_port, b'{}', 'NEVER_RECORD_BEARER', 1)
        self.assertEqual(row['http_status'], 401)
        self.assertEqual(row['_code'], 'unauthorized')
        self.assertTrue(row['json_error'])
        serialized = json.dumps(row)
        self.assertNotIn('DO_NOT_RECORD_THIS_SECRET', serialized)
        self.assertNotIn('NEVER_RECORD_BEARER', serialized)

    def test_rejection_headers_obey_absolute_deadline(self):
        started = time.monotonic()
        with self.assertRaises(Exception):
            e.raw_post(self.server.server_port, b'slow', None, .03)
        self.assertLess(time.monotonic() - started, .18)

    def test_complete_finite_live_metrics_and_exact_idle_state(self):
        names = e.POWER_GAUGES
        raw = '\n'.join(name + ' 0' for name in names).encode()
        gauges = e.parse_metrics(raw, names)
        for invalid in (b'power_requests_running NaN', raw + b'\npower_requests_running 0', raw.replace(b' 0', b' .5', 1)):
            with self.assertRaises(e.m.GuardError):
                e.parse_metrics(invalid, names)
        sample = {'slot': {'is_processing': False}, 'power': gauges,
                  'native': {name: 0 for name in e.m.GAUGES}}
        self.assertTrue(e.idle(sample))
        sample['power']['power_active_requests'] = 2
        self.assertFalse(e.idle(sample), 'legacy active-gauge leak must remain a failed gate')
        self.assertTrue(e.quiescent(sample), 'released permits/native idle allow independent next-case observation')
        sample['power']['power_active_requests'] = 0
        sample['power']['power_requests_waiting'] = 1
        self.assertFalse(e.idle(sample))
        self.assertFalse(e.quiescent(sample))
        sample['power']['power_requests_waiting'] = 0
        sample['native']['llamacpp:requests_deferred'] = 1
        self.assertFalse(e.idle(sample))

    def test_budget_and_instrument_are_bounded(self):
        with self.assertRaises(e.m.GuardError):
            e.Budget(0).left(1)
        self.assertEqual(set(e.TOOL_IDENTITY), {'runner_sha256', 'matrix_sha256', 'e2e_sha256'})
        self.assertTrue(all(len(value) == 64 for value in e.TOOL_IDENTITY.values()))


if __name__ == '__main__':
    unittest.main()
