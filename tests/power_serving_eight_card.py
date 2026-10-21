"""Hermetic guards for the optional candidate-only production capacity helper."""
import copy
import http.client
import importlib.util
import json
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
import subprocess
import tempfile
import threading
import time
import unittest
from unittest import mock

PATH = Path(__file__).resolve().parents[1] / 'docs/evidence/power/2026-10-21-serving-optimization/eight-card-qualification.py'
spec = importlib.util.spec_from_file_location('power_serving_eight_card', PATH)
e = importlib.util.module_from_spec(spec)
spec.loader.exec_module(e)


def counters(predicted=0):
    result = dict.fromkeys(e.m.COUNTER_NAMES, 0.)
    result['llamacpp:tokens_predicted_total'] = float(predicted)
    return result


def record(index=0, tokens=7):
    return {'endpoint_index': index, 'outcome': 'success', 'quality_source': 'expected-text',
            'usage_source': 'upstream-timings', 'usage': {'completion_tokens': tokens},
            'wire_usage': {'completion_tokens': tokens}, 'wire_usage_mismatch': False}


class EightCardGuards(unittest.TestCase):
    def test_schedule_covers_every_case_on_every_endpoint(self):
        raw, corpus = e.load_corpus()
        self.assertTrue(raw)
        for devices in ([7], list(range(8)), [2, 7]):
            schedule = e.balanced_schedule(corpus, devices)
            observed = {gpu: set() for gpu in devices}
            for index, item in enumerate(schedule):
                observed[devices[index % len(devices)]].add(item['name'])
            self.assertEqual(list(observed.values()), [set(row['name'] for row in corpus)] * len(devices))
        changed = copy.deepcopy(corpus)
        changed[0]['request']['temperature'] = .7
        original_read = e.m.read_limited

        def mutated(path, maximum):
            if Path(path).name == 'eight-card-corpus.jsonl':
                return '\n'.join(map(json.dumps, changed)).encode()
            return original_read(path, maximum)

        with mock.patch.object(e.m, 'read_limited', side_effect=mutated):
            with self.assertRaises(e.m.GuardError):
                e.load_corpus()

    def test_actual_props_and_every_slot_are_required(self):
        props = {'model_path': e.m.MODEL_PATH, 'model_alias': e.ALIAS, 'total_slots': 2}
        slots = [{'id': i, 'n_ctx': 32768, 'is_processing': False, 'speculative': False} for i in range(2)]
        for bad_props, bad_slots in [(dict(props, model_alias='prism27b'), slots),
                                      (props, []), (dict(props, total_slots=True), slots),
                                      (props, [dict(slots[0], n_ctx=65536), slots[1]]),
                                      (props, [dict(slots[0], speculative=True), slots[1]]),
                                      (props, [slots[0], dict(slots[1], id=0)]),
                                      (props, [slots[0], dict(slots[1], is_processing=True)])]:
            with self.subTest(props=bad_props, slots=bad_slots):
                with mock.patch.object(e.m, 'get', side_effect=[json.dumps(bad_props).encode(), json.dumps(bad_slots).encode()]):
                    with self.assertRaises(e.m.GuardError):
                        e.native_state(7, 2)
        with mock.patch.object(e.m, 'get', side_effect=[json.dumps(props).encode(), json.dumps(slots).encode()]):
            self.assertEqual(e.native_state(7, 2)['total_slots'], 2)

    def test_native_profile_matches_actual_flags_and_default_environment(self):
        flags = {'--host': '127.0.0.1', '--port': '18107', '-m': e.m.MODEL_PATH, '-ngl': '99',
                 '-c': '65536', '-fa': 'on', '--reasoning': 'off', '--alias': e.ALIAS, '-t': '8',
                 '--parallel': '2', '--jinja': True, '--cache-prompt': True, '--metrics': True, '--slots': True}
        command = ['llama-server']
        for key, value in flags.items():
            command.extend([key] if value is True else [key, value])
        manifest = {'slots': 2, 'runtime_flags': flags}
        e.native_profile(command, [b'CUDA_VISIBLE_DEVICES=7'], 7, manifest)
        for change in ('flag-mismatch', 'wrong-gpu', 'graphs-env', 'spec-env', 'spec-cli', 'missing-slots'):
            actual, env, prov = list(command), [b'CUDA_VISIBLE_DEVICES=7'], copy.deepcopy(manifest)
            if change == 'flag-mismatch':
                prov['runtime_flags']['-t'] = '16'
            elif change == 'wrong-gpu':
                env = [b'CUDA_VISIBLE_DEVICES=6']
            elif change == 'graphs-env':
                env.append(b'GGML_CUDA_DISABLE_GRAPHS=0')
            elif change == 'spec-env':
                env.append(b'LLAMA_ARG_SPEC_NGRAM_ADAPT=2')
            elif change == 'spec-cli':
                actual.extend(['--spec-type', 'ngram-mod'])
                prov['runtime_flags']['--spec-type'] = 'ngram-mod'
            else:
                actual.remove('--slots')
            with self.subTest(change=change):
                with self.assertRaises(e.m.GuardError):
                    e.native_profile(actual, env, 7, prov)

    def test_cross_worker_admission_cpu_and_numa_must_match(self):
        base = {'arm': 'candidate', 'gpu_indices': [0], 'context_per_sequence': 32768, 'runtime_flags': {},
                'admission': {'active': 2}, 'cpu_affinity': 'unbound', 'numa_policy': 'default'}
        for field, value in [('admission', {'active': 1}), ('cpu_affinity', '0-7'), ('numa_policy', 'local')]:
            second = dict(base, gpu_indices=[7], **{field: value})
            with mock.patch.object(e.q, 'load_provenance', side_effect=[base, second]), \
                 mock.patch.object(e.m, 'read_limited', return_value=b'{"native":{},"power":{},"gateway":{}}'):
                with self.assertRaises(e.m.GuardError):
                    e.read_workers([0, 7], b'fixture')

    def test_native_metrics_missing_nonfinite_and_external_work_fail_closed(self):
        good = counters(9)
        invalid = []
        missing = dict(good)
        missing.pop('llamacpp:tokens_predicted_total')
        invalid.append(missing)
        invalid.extend(dict(good, **{field: value}) for field, value in [
            ('llamacpp:tokens_predicted_total', float('nan')),
            ('llamacpp:tokens_predicted_total', 1.5),
            ('llamacpp:requests_processing', 1),
            ('llamacpp:requests_deferred', 1),
            ('llamacpp:spec_decode_num_accepted_tokens_total', 1)])
        for values in invalid:
            with mock.patch.object(e.m, 'get', return_value='\n'.join(k + ' ' + str(v) for k, v in values.items()).encode()):
                with self.assertRaises(e.m.GuardError):
                    e.native_counters(7)
        with mock.patch.object(e.m, 'get', return_value='\n'.join(k + ' ' + str(v) for k, v in good.items()).encode()):
            self.assertEqual(e.native_counters(7), good)

    def test_each_gpu_reconciles_authoritative_usage_and_rejects_confounds(self):
        before = {2: counters(10), 7: counters(20)}
        after = {2: counters(17), 7: counters(31)}
        records = [record(0, 7), record(1, 11)]
        self.assertEqual(e.validate_accounting(records, before, after, [2, 7]), {2: 7, 7: 11})
        for change in ('external-output', 'wire-mismatch', 'missing-usage', 'counter-reset'):
            altered, observed = copy.deepcopy(records), copy.deepcopy(after)
            if change == 'external-output':
                observed[7]['llamacpp:tokens_predicted_total'] += 1
            elif change == 'wire-mismatch':
                altered[1]['wire_usage_mismatch'] = True
            elif change == 'missing-usage':
                altered[0]['usage'] = None
            else:
                observed[2]['llamacpp:tokens_predicted_total'] = 0
            with self.subTest(change=change):
                with self.assertRaises(e.m.GuardError):
                    e.validate_accounting(altered, before, observed, [2, 7])

    def test_power_effective_admission_parallel_and_spec_mode_match_profile(self):
        health = {'status': 'ok', 'inference': {'num_parallel': 2}, 'loaded_models': 1,
                  'worker': {'admission': {'active_limit': 2, 'active': 0, 'waiting': 0}},
                  'speculative': {'mode': 'off'}}
        manifest = {'slots': 2, 'admission': {'active': 2}}
        with mock.patch.object(e.m, 'get', return_value=json.dumps(health).encode()):
            self.assertEqual(e.power_state(7, manifest)['num_parallel'], 2)
        for change in ('parallel', 'limit', 'leak', 'registration', 'spec'):
            bad = copy.deepcopy(health)
            if change == 'parallel':
                bad['inference']['num_parallel'] = 1
            elif change == 'limit':
                bad['worker']['admission']['active_limit'] = None
            elif change == 'leak':
                bad['worker']['admission']['waiting'] = 1
            elif change == 'registration':
                bad['loaded_models'] = 0
            else:
                bad['speculative']['mode'] = 'auto'
            with self.subTest(change=change), mock.patch.object(e.m, 'get', return_value=json.dumps(bad).encode()):
                with self.assertRaises(e.m.GuardError):
                    e.power_state(7, manifest)

    def test_guard_closes_only_helper_private_gateway_sockets(self):
        class Transport:
            def __init__(self):
                self.shutdowns = 0

            def shutdown(self, _direction):
                self.shutdowns += 1

            def close(self):
                pass

            def fileno(self):
                return -1

            def dup(self):
                return self

        original = http.client.HTTPConnection

        def connect(connection):
            connection.sock = Transport()

        owner = e.OwnedRequests([19897])
        with mock.patch.object(original, 'connect', connect):
            with owner.track():
                native = http.client.HTTPConnection('127.0.0.1', 18107)
                private = http.client.HTTPConnection('127.0.0.1', 19897)
                power = http.client.HTTPConnection('127.0.0.1', 19507)
                for connection in (native, private, power):
                    connection.connect()
                owner.abort()
                self.assertEqual(native.sock.shutdowns, 0)
                self.assertEqual(power.sock.shutdowns, 0)
                self.assertEqual(private.sock.shutdowns, 1)
                with self.assertRaises(ConnectionAbortedError):
                    http.client.HTTPConnection('127.0.0.1', 19897).connect()
                for connection in (native, private, power):
                    connection.close()
        self.assertIs(http.client.HTTPConnection, original)

    def test_guard_interrupts_body_after_http_connection_detaches_socket(self):
        headers, release = threading.Event(), threading.Event()

        class DetachedFixture(BaseHTTPRequestHandler):
            def log_message(self, *_args):
                pass

            def do_POST(self):
                self.rfile.read(int(self.headers['Content-Length']))
                try:
                    self.send_response(200)
                    self.send_header('Connection', 'close')
                    self.end_headers()
                    self.close_connection = True
                    self.wfile.write(b'{')
                    self.wfile.flush()
                    headers.set()
                    end = time.monotonic() + 2
                    while not release.wait(.005) and time.monotonic() < end:
                        self.wfile.write(b' ')
                        self.wfile.flush()
                except OSError:
                    pass

        server = ThreadingHTTPServer(('127.0.0.1', 0), DetachedFixture)
        serving = threading.Thread(target=server.serve_forever, daemon=True)
        serving.start()
        owner, results = e.OwnedRequests([server.server_port]), []
        try:
            with owner.track():
                reader = threading.Thread(target=lambda: results.append(e.q.request_once(
                    'http://127.0.0.1:' + str(server.server_port) + '/v1', {}, 'fixture', 1.5)))
                reader.start()
                self.assertTrue(headers.wait(.5))
                time.sleep(.02)  # Let getresponse detach the HTTP/1.0 socket.
                started = time.monotonic()
                owner.abort()
                reader.join(.3)
                self.assertFalse(reader.is_alive(), 'guard must interrupt a detached response body')
                self.assertLess(time.monotonic() - started, .3)
                self.assertNotEqual(results[0]['outcome'], 'success')
        finally:
            release.set()
            server.shutdown()
            server.server_close()
            serving.join()

    def test_gpu_samples_require_exact_selected_devices_and_finite_columns(self):
        for output in ['7, 23000, 24564, 0, 50, 100\n',
                       '6, 10000, 24564, 0, 50, 100\n',
                       '7, nan, 24564, 0, 50, 100\n',
                       '7, 10000, 0, 0, 50, 100\n',
                       '7, 10000, 24564, 0, 50\n']:
            completed = subprocess.CompletedProcess([], 0, output, '')
            with mock.patch.object(e.subprocess, 'run', return_value=completed):
                if '23000' in output:
                    self.assertTrue(e.m.vram_exceeded(e.gpu_samples([7])[7]))
                else:
                    with self.assertRaises((e.m.GuardError, ValueError)):
                        e.gpu_samples([7])

    def test_monitor_aborts_own_requests_without_native_process_control(self):
        stop, abort, owner = threading.Event(), threading.Event(), mock.Mock()
        samples, errors = [], []
        with mock.patch.object(e, 'gpu_samples', side_effect=OSError('fixture observer unavailable')):
            with mock.patch.object(e.m, 'terminate_private_engine') as terminate:
                e.monitor(stop, abort, owner, [7], samples, errors)
        self.assertTrue(stop.is_set() and abort.is_set())
        owner.abort.assert_called_once()
        terminate.assert_not_called()
        self.assertEqual(errors, [{'exception': 'OSError'}])

    def test_post_observation_failure_preserves_completed_client_records(self):
        devices, manifest, fixed = [7], {7: {'slots': 1}}, {7: {}}
        corpus = [{'name': 'fixture', 'request': {}, 'expect': {'text': 'READY'}}]
        before = {'native_counters': {7: counters(0)}}
        warmed = {'native_counters': {7: counters(7)}}
        request = record(0, 7)
        campaign = {'records': [record(0, 7)], 'summary': {'outcomes': {'success': 1}, 'wall_seconds_including_drain': 1},
                    'request_budget_exhausted': False}
        sample = {7: {'used_mib': 10000, 'total_mib': 24564, 'gpu_percent': 0, 'temperature_c': 50, 'power_w': 100}}
        with tempfile.TemporaryDirectory() as directory:
            with mock.patch.object(e, 'worker_identity'), mock.patch.object(e, 'gpu_samples', return_value=sample), \
                 mock.patch.object(e, 'monitor', side_effect=lambda stop, *_args: stop.wait(1)), \
                 mock.patch.object(e, 'observe_all', side_effect=[before, warmed, OSError('private-secret-error')]), \
                 mock.patch.object(e.q, 'request_once', return_value=request), \
                 mock.patch.object(e.q, 'run_campaign', return_value=campaign):
                with self.assertRaises(OSError):
                    e.run_one(devices, manifest, fixed, corpus, 'private-secret-key', Path(directory), 1, 1, 1, e.Budget(200))
            raw = (Path(directory) / 'candidate-c1-r1.json').read_text()
            result = json.loads(raw)
            self.assertEqual(result['qualification_status'], 'failed')
            self.assertEqual(result['failure_exception'], 'OSError')
            self.assertEqual(len(result['records']), 1)
            self.assertNotIn('private-secret', raw)


if __name__ == '__main__':
    unittest.main()
