#!/usr/bin/env python3
"""Hermetic slot-aware canary and owned-transport proofs; no GPU or service use."""
import concurrent.futures
import copy
import importlib.util
import json
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
import select
import sys
import tempfile
import threading
import time
import unittest

PATH = Path(__file__).resolve().parents[1] / 'docs/evidence/power/2026-10-21-serving-optimization/canary-scenarios.py'
SPEC = importlib.util.spec_from_file_location('power_canary', PATH)
c = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(c)


class Fixture(ThreadingHTTPServer):
    daemon_threads = True

    def __init__(self, count):
        super().__init__(('127.0.0.1', 0), Handler)
        self.count, self.lock = count, threading.Lock()
        self.slots = [{'id': i, 'id_task': -1, 'is_processing': False, 'n_ctx': 32768,
                       'n_prompt_tokens': 0, 'n_prompt_tokens_processed': 0, 'next_token': [{'n_decoded': 0}]}
                      for i in range(count)]
        self.waiting = self.serial = self.output = self.prompt = self.posts = 0
        self.maximum_running = 0
        self.native_observable = True
        self.role_sent = threading.Event()

    def handle_error(self, *_args):
        pass  # Reader-owned cancellation intentionally closes transports.


class Handler(BaseHTTPRequestHandler):
    protocol_version = 'HTTP/1.1'

    def log_message(self, *_args):
        pass

    def reply(self, status, data):
        self.send_response(status)
        self.send_header('Content-Length', str(len(data)))
        self.send_header('Connection', 'close')
        self.end_headers()
        self.wfile.write(data)
        self.close_connection = True

    def do_GET(self):
        s = self.server
        with s.lock:
            if self.path == '/slots':
                if not s.native_observable:
                    self.reply(404, b'{}')
                else:
                    slots = [dict(slot, prompt='must never be retained') for slot in s.slots]
                    self.reply(200, json.dumps(slots).encode())
                return
            running = sum(slot['is_processing'] for slot in s.slots)
            values = {name: 0 for name in c.m.COUNTER_NAMES}
            values.update({'llamacpp:requests_processing': running, 'llamacpp:tokens_predicted_total': s.output,
                           'llamacpp:prompt_tokens_total': s.prompt})
            values.update({'power_active_requests': running + s.waiting,
                           'power_requests_running': running, 'power_requests_waiting': s.waiting})
            self.reply(200, '\n'.join(f'{name} {value}' for name, value in values.items()).encode())

    def disconnected(self):
        readable, _, _ = select.select([self.connection], [], [], 0)
        return bool(readable) and self.connection.recv(1, __import__('socket').MSG_PEEK) == b''

    def do_POST(self):
        s = self.server
        body = json.loads(self.rfile.read(int(self.headers['Content-Length'])))
        slot, waiting = None, False
        with s.lock:
            s.posts += 1
        try:
            prompt = body['messages'][0]['content']
            if prompt.startswith('Synthetic unique prefill'):
                time.sleep(.35)  # Authentication completes before native admission.
            with s.lock:
                if all(x['is_processing'] for x in s.slots):
                    if s.waiting == 8:
                        self.reply(429, json.dumps({'error': {'code': 'admission_queue_full', 'type': 'rate_limit_error',
                                                              'message': 'not retained'}}).encode())
                        return
                    s.waiting += 1
                    waiting = True
            end = time.monotonic() + 1
            while slot is None:
                if self.disconnected():
                    return
                with s.lock:
                    slot = next((x for x in s.slots if not x['is_processing']), None)
                    if slot is not None:
                        if waiting:
                            s.waiting -= 1
                            waiting = False
                        s.serial += 1
                        slot.update(is_processing=True, id_task=s.serial)
                        slot.update(n_prompt_tokens=9000 if prompt.startswith('Synthetic unique prefill') else 1,
                                    n_prompt_tokens_processed=0, next_token=[{'n_decoded': 0}])
                        s.prompt += 1
                        s.maximum_running = max(s.maximum_running, sum(x['is_processing'] for x in s.slots))
                if slot is None:
                    if time.monotonic() >= end:
                        self.reply(429, b'{"error":{"code":"admission_queue_timeout","type":"rate_limit_error"}}')
                        return
                    time.sleep(.005)
            self.send_response(200)
            self.send_header('Content-Type', 'text/event-stream')
            self.send_header('Connection', 'close')
            self.end_headers()
            self.close_connection = True

            def event(value):
                raw = value if isinstance(value, str) else json.dumps(value)
                self.wfile.write(('data: ' + raw + '\n\n').encode())
                self.wfile.flush()

            event({'choices': [{'delta': {'role': 'assistant'}}]})
            s.role_sent.set()
            if prompt.startswith('Synthetic unique prefill'):
                time.sleep(.5)
            with s.lock:
                slot['n_prompt_tokens_processed'] = slot['n_prompt_tokens']
                slot['next_token'][0]['n_decoded'] = 1
            long = body.get('max_tokens') == 1600
            count = 1200 if long else 1
            for _ in range(count):
                if self.disconnected():
                    return
                event({'choices': [{'delta': {'content': '1\n' if long else 'OK'}}]})
                with s.lock:
                    s.output += 1
                if long:
                    time.sleep(.015)
            event({'choices': [{'delta': {}, 'finish_reason': 'stop'}]})
            usage = {'prompt_tokens': 1, 'completion_tokens': count, 'total_tokens': 1 + count}
            event({'choices': [], 'usage': usage, 'prism_upstream': {'source': 'upstream-reported', 'usage': usage}})
            event('[DONE]')
        except (OSError, ConnectionError):
            pass
        finally:
            with s.lock:
                if waiting:
                    s.waiting -= 1
                if slot is not None:
                    slot['is_processing'] = False


class CanaryTests(unittest.TestCase):
    def test_wrapper_corpus_and_alias_match_and_old_corpus_is_refused(self):
        corpus = c.known_corpus()
        self.assertEqual(c.q.sha256(corpus), c.CORPUS_SHA256)
        self.assertTrue(all(json.loads(line)['request']['model'] == c.MODEL_ALIAS for line in corpus.splitlines()))
        manifest = {name: '0' * 64 for name in c.q.PROVENANCE_FIELDS}
        manifest.update(arm='candidate', corpus_sha256=c.CORPUS_SHA256, context_per_sequence=32768,
                        slots=2, runtime_flags={}, sampling={'temperature': 0, 'seed': 42}, cache_mode='prompt', gpu_indices=[7])
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / 'fixture-provenance.json'
            path.write_text(json.dumps(manifest), encoding='utf8')
            self.assertEqual(c.q.load_provenance(path, corpus)['corpus_sha256'], c.CORPUS_SHA256)
            with self.assertRaises(ValueError):
                c.q.load_provenance(path, (c.HERE / 'corpus.jsonl').read_bytes())

    def fixture(self, count):
        server = Fixture(count)
        thread = threading.Thread(target=server.serve_forever, daemon=True)
        thread.start()
        self.addCleanup(thread.join)
        self.addCleanup(server.server_close)
        self.addCleanup(server.shutdown)
        owned = c.OwnedRequests()
        parser = owned.request_once
        # Keep queue deadlines exact; shorten only the fixture's long blockers.
        owned.request_once = lambda url, body, key, timeout, ms=None, deltas=None, expected=None: parser(
            url, body, key, timeout, 2000 if ms == 8000 else ms, deltas, expected)
        canary = c.Canary(count, 'fixture-only', owned, c.Budget(25),
                          ports={name: server.server_port for name in c.PORTS})
        self.addCleanup(canary.pool.shutdown, wait=True, cancel_futures=True)
        self.addCleanup(owned.abort, 'fixture cleanup')
        return server, canary, owned

    def test_slots_are_actual_and_content_free(self):
        slots = [{'id': i, 'id_task': 3 + i, 'is_processing': True, 'n_ctx': 32768,
                  'prompt': 'secret', 'generated_text': 'secret'} for i in range(2)]
        safe = c.parse_slots(json.dumps(slots), 2)
        self.assertNotIn('secret', json.dumps(safe))
        self.assertEqual(len(safe), 2)
        for bad in ([slots[0]], [slots[0], slots[0]], [dict(slots[0], n_ctx=16384), slots[1]]):
            with self.subTest(bad=bad), self.assertRaises(c.Unsupported):
                c.parse_slots(json.dumps(bad), 2)

    def test_np2_task_vector_rejects_ghost_on_second_slot(self):
        def sample(tasks, processing):
            return {'slots': [{'id': i, 'id_task': task, 'is_processing': processing, 'n_ctx': 32768}
                              for i, task in enumerate(tasks)],
                    'power': {'power_requests_running': 2 if processing else 0},
                    'native': {'llamacpp:requests_processing': 2 if processing else 0, 'llamacpp:requests_deferred': 0}}
        active = sample([9, 10], True)
        self.assertTrue(c.never_dispatched(active, active, sample([9, 10], False), 2))
        self.assertFalse(c.never_dispatched(active, active, sample([9, 11], False), 2))
        half = copy.deepcopy(active)
        half['slots'][1]['is_processing'] = False
        self.assertFalse(c.filled(half, 2))

    def test_metrics_refuse_missing_duplicate_and_nonfinite(self):
        for raw in (b'', b'power_requests_running 1\npower_requests_running 2', b'power_requests_running NaN'):
            with self.subTest(raw=raw), self.assertRaises(c.Unsupported):
                c.parse_metrics(raw, {'power_requests_running'})

    def test_disabled_native_observation_refuses_before_inference(self):
        server, canary, _ = self.fixture(2)
        server.native_observable = False
        manifest = {'gpu_indices': [7], 'slots': 2, 'context_per_sequence': 32768,
                    'admission': {'active': 2, 'waiting_limit': 8, 'queue_timeout_ms': 1000}}
        with self.assertRaises(c.Unsupported):
            c.preflight(canary, manifest)
        self.assertEqual(server.posts, 0)

    def test_np1_and_np2_queue_cancel_and_overload(self):
        for count in (1, 2):
            with self.subTest(slots=count):
                server, canary, _ = self.fixture(count)
                self.assertTrue(canary.run_case('queued-cancel')['passed'])
                overload = canary.run_case('overload')
                self.assertTrue(overload['passed'])
                self.assertTrue(overload['request']['queue_full_code'])
                self.assertEqual(server.maximum_running, count)

    def test_pre_first_and_midstream_cancel_recover(self):
        _, canary, _ = self.fixture(2)
        before = canary.run_case('pre-first-meaningful-cancel')
        self.assertEqual(before['request']['meaningful_deltas'], 0)
        self.assertGreaterEqual(before['request']['elapsed_ms'], 350)
        self.assertEqual(before['cancel_trigger']['kind'], 'observed-native-prefill')
        self.assertTrue(any(slot['phase'] == 'prefill' for sample in canary.trace for slot in sample['slots']))
        middle = canary.run_case('midstream-cancel')
        self.assertEqual(middle['request']['meaningful_deltas'], 12)
        self.assertTrue(before['passed'] and middle['passed'])

    def test_abort_retains_connection_close_transport_and_does_not_patch_parser(self):
        server, canary, owned = self.fixture(1)
        original_class = c.q.http.client.HTTPConnection
        body = copy.deepcopy(canary.short)
        body['messages'][0]['content'] = 'Synthetic unique prefill fixture'
        future = canary.submit(body)
        self.assertTrue(server.role_sent.wait(1))
        owned.abort('fixture GPU guard')
        record = future.result(timeout=1)
        self.assertEqual(record['meaningful_deltas'], 0)
        self.assertNotEqual(record['outcome'], 'success')
        self.assertIs(c.q.http.client.HTTPConnection, original_class)
        with self.assertRaises(c.m.GuardError):
            canary.submit(canary.short)


if __name__ == '__main__':
    unittest.main()
