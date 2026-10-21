"""Cold lifecycle witnesses and strict measurement guards, without production calls."""
import contextlib
import copy
import hashlib
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import importlib.util
import json
from pathlib import Path
import tempfile
import threading
import time
import unittest
from unittest import mock

ROOT = Path(__file__).resolve().parents[1]
HERE = ROOT / 'docs/evidence/power/2026-10-21-serving-optimization'
spec = importlib.util.spec_from_file_location('live_v2', HERE / 'eight-card-live-qualification-v2.py')
l = importlib.util.module_from_spec(spec)
spec.loader.exec_module(l)
spec = importlib.util.spec_from_file_location('unchanged_live_v1_fixture_helpers', ROOT / 'tests/power_serving_live_eight_card.py')
base = importlib.util.module_from_spec(spec)
spec.loader.exec_module(base)
CORPUS, MANIFEST, record, counters, health = base.CORPUS, dict(base.MANIFEST, model_sha256='53107f530aa52eb00912263ab1ee29bd199261c87cd7b4ad4ca1318c1fe33ee3'), base.record, base.counters, base.health


def actual_receipt():
    return (HERE / 'production-fleet-v4-rollout.json').read_bytes()


def parse_fixture(value):
    raw = json.dumps(value).encode()
    return l.parse_readiness(raw, hashlib.sha256(raw).hexdigest())


class ColdLifecycle(unittest.TestCase):
    def test_real_root_pinned_receipt_accepts_actual_gpu7_power_restart_only(self):
        raw = actual_receipt()
        self.assertEqual(hashlib.sha256(raw).hexdigest(), l.READINESS_SHA)
        parsed = l.parse_readiness(raw)
        self.assertEqual(set(parsed), set(range(8)))
        gpu7 = json.loads(raw)['gpus'][7]
        self.assertNotEqual(gpu7['candidate_readiness']['readiness']['power']['pid'], parsed[7]['power']['pid'])
        self.assertEqual(gpu7['candidate_readiness']['readiness']['native']['pid'], parsed[7]['native']['pid'])

    def test_wrong_root_receipt_hash_refuses_before_semantic_acceptance(self):
        with self.assertRaises(l.m.GuardError): l.parse_readiness(actual_receipt() + b' ')

    def test_receipt_requires_auth_alias_output_loaded_geometry_and_every_gpu(self):
        original = json.loads(actual_receipt())
        for change in ('alias', 'output', 'loaded', 'auth-status', 'creds', 'native-generation', 'geometry', 'duplicate', 'missing', 'power-artifact'):
            value = copy.deepcopy(original)
            row = value['gpus'][0]
            probe = row['current_authenticated_probe']
            if change == 'alias': probe['exact_alias_present'] = False
            elif change == 'output': probe['expected_output'] = False
            elif change == 'loaded': probe['actual_production_worker_loaded'] = False
            elif change == 'auth-status': probe['models_http_status'] = 401
            elif change == 'creds': probe['credentials_output'] = True
            elif change == 'native-generation': row['candidate_readiness']['readiness']['native']['pid'] += 1
            elif change == 'geometry': row['candidate_readiness']['readiness']['slots'] = 1
            elif change == 'duplicate': value['gpus'][1]['gpu'] = 0
            elif change == 'missing': value['gpus'].pop()
            else: probe['power']['exe_sha256'] = '0' * 64
            with self.subTest(change=change), self.assertRaises(l.m.GuardError): parse_fixture(value)

    def test_current_live_native_and_power_both_match_authenticated_generation(self):
        historical = l.parse_readiness(actual_receipt())
        native, power = historical[7]['native'], historical[7]['power']
        witness = l.match_readiness(7, historical, native, power)
        self.assertTrue(witness['native_geometry_same_generation'])
        self.assertTrue(witness['power_apply_stamp_is_historical'])
        self.assertFalse(witness['current_production_registry_requeried'])
        for kind in ('native', 'power'):
            for field in ('pid', 'start_ticks', 'exe_sha256'):
                changed = dict(historical[7][kind])
                changed[field] = '0' * 64 if field == 'exe_sha256' else changed[field] + 1
                with self.subTest(kind=kind, field=field), self.assertRaises(l.m.GuardError):
                    l.match_readiness(7, historical, changed if kind == 'native' else native, changed if kind == 'power' else power)

    def test_unreviewed_readiness_producer_refuses(self):
        with mock.patch.object(l.m, 'digest', return_value='0' * 64), self.assertRaises(l.m.GuardError): l.trusted_readiness()

    def test_production_cold_requires_real_matched_auth_witness_and_live_geometry(self):
        historical = l.parse_readiness(actual_receipt())
        native, power = historical[7]['native'], historical[7]['power']
        fixed = {7: {'native': native}}
        gateway = {'exe_sha256': l.GATEWAY_SHA, 'pid': 43}
        with mock.patch.object(l, 'trusted_readiness', return_value=historical), mock.patch.object(l, 'systemd_pid', side_effect=[native['pid'], power['pid'], 43]), mock.patch.object(l.m, 'process_identity', side_effect=[power, gateway]), mock.patch.object(l.m, 'get', return_value=json.dumps(dict(health(), loaded_models=0)).encode()):
            result = l.production_proof([7], {7: MANIFEST}, fixed)
        self.assertEqual(result['power-7']['health']['model_residency'], 'cold-unloaded')
        self.assertTrue(result['power-7']['authenticated_cold_readiness']['historical_loaded_proof'])
        with mock.patch.object(l, 'trusted_readiness', return_value=historical), mock.patch.object(l, 'systemd_pid', side_effect=[native['pid'], power['pid']]), mock.patch.object(l.m, 'process_identity', return_value=dict(power, start_ticks=power['start_ticks'] + 1)), self.assertRaises(l.m.GuardError):
            l.production_proof([7], {7: MANIFEST}, fixed)

    def test_cold_allowance_never_accepts_invalid_health_or_measurement_cold(self):
        raw = json.dumps(dict(health(), loaded_models=0)).encode()
        l.power_health(raw, MANIFEST, True, False)
        with self.assertRaises(l.m.GuardError): l.power_health(raw, MANIFEST, True, True)
        for loaded in (-1, True, '0', None):
            with self.subTest(loaded=loaded), self.assertRaises(l.m.GuardError):
                l.power_health(json.dumps(dict(health(), loaded_models=loaded)).encode(), MANIFEST, True, False)

    def test_private_registry_requires_exact_alias_format_and_digest(self):
        good = {'data': [{'id': l.e.ALIAS, 'format': 'gguf', 'sha256': MANIFEST['model_sha256']}]}
        l.registered_model(json.dumps(good).encode(), MANIFEST)
        for change in ('alias', 'format', 'sha', 'duplicate', 'missing'):
            value = copy.deepcopy(good)
            if change == 'alias': value['data'][0]['id'] = 'wrong'
            elif change == 'format': value['data'][0]['format'] = 'remote'
            elif change == 'sha': value['data'][0]['sha256'] = '0' * 64
            elif change == 'duplicate': value['data'] *= 2
            else: value['data'] = []
            with self.subTest(change=change), self.assertRaises(l.m.GuardError): l.registered_model(json.dumps(value).encode(), MANIFEST)

    def test_private_registry_is_bound_to_actual_fresh_home_manifest_and_path(self):
        with tempfile.TemporaryDirectory() as name:
            root = Path(name)
            private, proc = root / 'private', root / 'proc'
            home = private / 'worker-7-home'
            manifest_path = home / 'models/manifests/prism-27b.json'
            manifest_path.parent.mkdir(parents=True)
            environment = proc / '42/environ'
            environment.parent.mkdir(parents=True)
            environment.write_bytes(b'A3S_POWER_HOME=' + str(home).encode() + b'\0')
            value = {'name': l.e.ALIAS, 'format': 'gguf', 'path': l.m.MODEL_PATH, 'sha256': MANIFEST['model_sha256']}
            manifest_path.write_text(json.dumps(value))
            api = json.dumps({'data': [{'id': l.e.ALIAS, 'format': 'gguf', 'sha256': MANIFEST['model_sha256']}]}).encode()
            with mock.patch.object(l.e, 'PRIVATE', private), mock.patch.object(l.m, 'PROC', proc), mock.patch.object(l, 'authenticated_models', return_value=api):
                proof = l.private_registrations([7], {7: MANIFEST}, {7: {'power': {'pid': 42}}}, 'fixture-key')
                self.assertNotIn('fixture-key', json.dumps(proof))
                for field, wrong in (('path', '/different/model.gguf'), ('sha256', '0' * 64), ('name', 'wrong')):
                    manifest_path.write_text(json.dumps(dict(value, **{field: wrong})))
                    with self.subTest(field=field), self.assertRaises(l.m.GuardError):
                        l.private_registrations([7], {7: MANIFEST}, {7: {'power': {'pid': 42}}}, 'fixture-key')
                manifest_path.write_text(json.dumps(value))
                environment.write_bytes(b'A3S_POWER_HOME=/foreign/home\0')
                with self.assertRaises(l.m.GuardError): l.private_registrations([7], {7: MANIFEST}, {7: {'power': {'pid': 42}}}, 'fixture-key')

    def test_private_cold_before_warmup_but_strict_residency_after_warmup(self):
        production = {'power-7': {'authenticated_cold_readiness': {'receipt_sha256': l.READINESS_SHA, 'same_generation_authenticated_alias_and_output': True}}}
        zero = json.dumps(dict(health(), loaded_models=0)).encode()
        metrics = '\n'.join(k + ' ' + str(v) for k, v in counters().items()).encode()
        gauges = b'power_active_requests 0\npower_requests_waiting 0\npower_requests_running 0'
        def get(path, port, **kwargs): return metrics if port == 18107 else gauges if path == '/metrics' else zero
        with mock.patch.object(l, 'native_state', return_value={}), mock.patch.object(l.m, 'get', side_effect=get):
            l.snapshot([7], {7: MANIFEST}, time.monotonic() + 5, True, production, False)
            with self.assertRaises(l.m.GuardError): l.snapshot([7], {7: MANIFEST}, time.monotonic() + 5, True, production, True)
            with self.assertRaises((l.m.GuardError, KeyError)): l.snapshot([7], {7: MANIFEST}, time.monotonic() + 5, True, {}, False)

    def test_monitor_switches_from_cold_warmup_to_strict_measured_residency(self):
        stop, abort, warm = mock.Mock(), threading.Event(), threading.Event()
        stop.is_set.side_effect = [False, False, True]
        samples, errors, policies = [], [], []
        def observed(*args):
            policies.append(args[-1])
            warm.set()
            return {'native_counters': {7: counters()}}
        with mock.patch.object(l.e, 'gpu_samples', return_value={7: {'used_mib': 10000, 'total_mib': 24000}}), mock.patch.object(l, 'verify_stamps'), mock.patch.object(l, 'snapshot', side_effect=observed):
            l.monitor(stop, abort, mock.Mock(), [7], {}, {}, {}, {}, samples, errors, warm)
        self.assertEqual(policies, [False, True])
        self.assertEqual([row['private_residency_required'] for row in samples], [False, True])
        self.assertFalse(errors)

    def test_cleanup_keeps_zero_permits_and_loaded_model_requirements_separate(self):
        gauges = b'power_active_requests 0\npower_requests_waiting 0\npower_requests_running 0'
        zero = json.dumps(dict(health(), loaded_models=0)).encode()
        with mock.patch.object(l.m, 'get', side_effect=lambda path, **kwargs: gauges if path == '/metrics' else zero):
            before = l.cleanup_owned([7], {7: MANIFEST}, require_loaded=False)
            after = l.cleanup_owned([7], {7: MANIFEST}, require_loaded=True)
        self.assertTrue(before['all_private_permits_zero'])
        self.assertTrue(before['private_model_readiness_passed'])
        self.assertTrue(after['all_private_permits_zero'])
        self.assertFalse(after['private_model_readiness_passed'])

    def test_authenticated_registry_absolute_deadline_covers_headers_and_detached_body(self):
        for fault in ('headers', 'body'):
            class Handler(BaseHTTPRequestHandler):
                def log_message(self, *args): pass
                def do_GET(self):
                    self.assertion = self.headers.get('Authorization') == 'Bearer fixture-key'
                    try:
                        if fault == 'headers': time.sleep(.25)
                        self.send_response(200)
                        self.send_header('Content-Length', '9999')
                        self.send_header('Connection', 'close')
                        self.end_headers()
                        for _ in range(200): self.wfile.write(b' '); self.wfile.flush(); time.sleep(.01)
                    except OSError: pass
            server = ThreadingHTTPServer(('127.0.0.1', 0), Handler)
            thread = threading.Thread(target=server.serve_forever, daemon=True)
            thread.start()
            started = time.monotonic()
            try:
                with mock.patch.object(l.e, 'ports', return_value={'power': server.server_port}), self.assertRaises((l.m.GuardError, OSError, ValueError, l.http.client.HTTPException)):
                    l.authenticated_models(7, 'fixture-key', .08)
                self.assertLess(time.monotonic() - started, .35)
            finally:
                server.shutdown(); server.server_close(); thread.join(timeout=1)

    def test_accounting_and_unknown_work_claims_are_unchanged(self):
        result = l.accounting([record()], {7: counters()}, {7: counters(12, 2)}, [7], CORPUS)
        self.assertEqual(result[7]['unattributed_native_completion_delta'], 5)
        with self.assertRaises(l.m.GuardError): l.accounting([record()], {7: counters()}, {7: counters(6)}, [7], CORPUS)
        with self.assertRaises(l.m.GuardError): l.validate_claims(dict(l.claims(), shared_native_cancellation='passed'))

    def test_complete_cell_requires_warmup_before_strict_measurement_and_cleanup(self):
        for cleanup_ready in (True, False):
            with self.subTest(cleanup_ready=cleanup_ready), tempfile.TemporaryDirectory() as name:
                owner = mock.Mock()
                owner.track.return_value = contextlib.nullcontext(owner)
                owner.once.side_effect = lambda *args, **kwargs: record(case=next(item['name'] for item in CORPUS if item['request'] == args[1]))
                policies, lifecycle = [], []
                def observed(*args, **kwargs):
                    required = kwargs.get('private_require_loaded', True)
                    policies.append(required)
                    lifecycle.append(('snapshot', required, owner.once.call_count))
                    tokens = (0, 28, 55)[len(policies) - 1]
                    return {'native_counters': {7: counters(tokens, 1)}}
                def sampled(*args):
                    if len(args) == 11:
                        self.assertFalse(args[-1].is_set())
                        args[-3].append({'fixture': 'observed'})
                    else:
                        args[-2].append({'fixture': 'observed'})
                def cleanup(*args, **kwargs):
                    lifecycle.append(('cleanup', kwargs['require_loaded'], owner.once.call_count))
                    return {'all_private_permits_zero': True, 'private_model_readiness_passed': cleanup_ready}
                measured = {'records': [record()], 'summary': l.q.summarize([record()], .02), 'request_budget_exhausted': False, 'aborted': False}
                with mock.patch.object(l, 'Requests', return_value=owner), mock.patch.object(l, 'verify_stamps'), mock.patch.object(l, 'private_registrations', return_value={7: {'registered': True}}), mock.patch.object(l, 'drained_snapshot', side_effect=observed), mock.patch.object(l.e, 'gpu_samples', return_value={7: {'used_mib': 10000, 'total_mib': 24000}}), mock.patch.object(l, 'monitor', side_effect=sampled), mock.patch.object(l.e, 'monitor', side_effect=sampled), mock.patch.object(l, 'campaign', return_value=measured), mock.patch.object(l, 'cleanup_owned', side_effect=cleanup):
                    arguments = ([7], {7: MANIFEST}, {}, {}, {}, CORPUS, 'fixture-key', Path(name), 8, 1, 1, l.e.Budget(), l.current_tools())
                    if cleanup_ready:
                        result = l.run_one(*arguments)
                        self.assertEqual(result['qualification_status'], 'passed')
                    else:
                        with self.assertRaises(l.m.GuardError): l.run_one(*arguments)
                        result = json.loads((Path(name) / 'shared-c8-r1.json').read_text())
                        self.assertEqual(result['qualification_status'], 'failed')
                self.assertEqual(policies, [False, True, True])
                self.assertEqual(lifecycle, [('snapshot', False, 0), ('snapshot', True, 4), ('snapshot', True, 4), ('cleanup', True, 4)])
                owner.abort.assert_called_once()

    def test_v1_source_and_fixture_are_preserved_and_v2_matrix_remains_fixed(self):
        self.assertEqual(l.m.digest(HERE / 'eight-card-live-qualification.py'), l.V1_SHA)
        self.assertEqual(l.m.digest(ROOT / 'tests/power_serving_live_eight_card.py'), l.V1_FIXTURES_SHA)
        self.assertEqual(l.matrix(True), [(1, 8, 1)])
        self.assertEqual(len(l.matrix(False)), 12)
        l.current_tools()


if __name__ == '__main__':
    unittest.main()
