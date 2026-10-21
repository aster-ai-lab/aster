"""Hermetic shared-production qualification boundaries; never live service calls."""
import copy
import contextlib
import http.client
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import importlib.util
import json
from pathlib import Path
import socket
import subprocess
import tempfile
import threading
import time
import unittest
from unittest import mock

PATH = Path(__file__).resolve().parents[1] / 'docs/evidence/power/2026-10-21-serving-optimization/eight-card-live-qualification.py'
spec = importlib.util.spec_from_file_location('power_serving_live_eight_card', PATH)
l = importlib.util.module_from_spec(spec)
spec.loader.exec_module(l)
CORPUS = l.e.load_corpus()[1]


def counters(predicted=0, processing=0):
    result = dict.fromkeys(l.m.COUNTER_NAMES, 0.)
    result['llamacpp:tokens_predicted_total'] = float(predicted)
    result['llamacpp:requests_processing'] = float(processing)
    return result


def record(index=0, tokens=7, case='short-stream'):
    return {'endpoint_index': index, 'case': case, 'outcome': 'success', 'http_status': 200,
            'quality_source': 'expected-tool' if case == 'tool-stream' else 'expected-text',
            'usage_source': 'upstream-usage', 'usage': {'prompt_tokens': 10, 'completion_tokens': tokens, 'total_tokens': 10 + tokens},
            'wire_usage': {'prompt_tokens': 10, 'completion_tokens': tokens, 'total_tokens': 10 + tokens},
            'wire_usage_mismatch': False, 'finish_observed': True, 'terminal_observed': True,
            'ttft_ms': 1, 'elapsed_ms': 2, 'first_tool_ms': None,
            'tool_valid': case == 'tool-stream', 'tool_calls': 1 if case == 'tool-stream' else 0}


def health(active=0, waiting=0):
    return {'status': 'ok', 'inference': {'num_parallel': 2}, 'loaded_models': 1,
            'worker': {'admission': {'active_limit': 2, 'active': active, 'waiting': waiting}},
            'speculative': {'mode': 'off'}}


MANIFEST = {'slots': 2, 'binary_sha256': l.POWER_SHA, 'admission': {'active': 2, 'waiting_limit': 8, 'queue_timeout_ms': 1000}}


class LiveEightContracts(unittest.TestCase):
    def test_frozen_dependencies_and_new_instrument_cannot_change_after_preflight(self):
        initial = l.current_tools()
        wrong = dict(initial, live_qualification_sha256='0' * 64)
        with self.assertRaises(l.m.GuardError): l.current_tools(wrong)
        digest = l.m.digest
        def changed(path):
            return '0' * 64 if Path(path).resolve() == Path(l.q.__file__).resolve() else digest(path)
        with mock.patch.object(l.m, 'digest', side_effect=changed), self.assertRaises(l.m.GuardError):
            l.current_tools(initial)

    def test_smoke_is_c8_and_first_wave_covers_every_actual_endpoint(self):
        self.assertEqual(l.matrix(True), [(1, 8, 1)])
        self.assertEqual(l.matrix(False), [(r, c, 60) for r in (1, 2, 3) for c in (1, 8, 16, 32)])
        owner, abort, barrier = mock.Mock(), threading.Event(), threading.Barrier(8)
        def once(*args, **kwargs):
            barrier.wait(timeout=2)
            abort.set()
            return record()
        owner.once.side_effect = once
        result = l.campaign(list(range(8)), CORPUS, None, 8, 1, owner, abort)
        self.assertEqual({row['endpoint_index'] for row in result['records']}, set(range(8)))
        self.assertEqual(len(result['records']), 8)
        self.assertEqual({row['case'] for row in result['records']}, {'short-stream'})

    def test_busy_native_and_unknown_positive_delta_are_allowed_without_attribution(self):
        before, after = {2: counters(10, 2), 7: counters(20, 1)}, {2: counters(20, 1), 7: counters(40, 2)}
        result = l.accounting([record(0, 7), record(1, 11)], before, after, [2, 7], CORPUS)
        self.assertEqual(result[2]['unattributed_native_completion_delta'], 3)
        self.assertEqual(result[7]['unattributed_native_completion_delta'], 9)
        self.assertEqual(result[2]['unattributed_work_classification'], 'unknown; never automatically external')
        self.assertNotIn('unattributed_external_delta', result[2])
        self.assertNotIn('native_output_tokens_per_second', result[2])

    def test_native_shortfall_cannot_be_hidden_by_another_gpu(self):
        with self.assertRaises(l.m.GuardError):
            l.accounting([record(0, 7), record(1, 11)], {2: counters(), 7: counters()},
                         {2: counters(6), 7: counters(1000)}, [2, 7], CORPUS)

    def test_each_gpu_requires_own_verified_success(self):
        with self.assertRaises(l.m.GuardError):
            l.validate_records([record(0)], [2, 7], CORPUS)
        with self.assertRaises(l.m.GuardError):
            l.validate_records([record(True)], [7], CORPUS)

    def test_actual_usage_wire_terminal_and_tool_quality_are_hard_gates(self):
        for change in ('missing', 'estimated', 'timings', 'wire', 'wire-error', 'finish', 'done', 'output', 'tool', 'total', 'negative', 'overflow'):
            row = record(case='tool-stream')
            if change == 'missing': row['usage'] = None
            elif change == 'estimated': row['usage_source'] = 'endpoint-usage'
            elif change == 'timings': row['usage_source'] = 'upstream-timings'
            elif change == 'wire': row['wire_usage']['completion_tokens'] += 1
            elif change == 'wire-error': row['wire_usage_mismatch'] = True
            elif change == 'finish': row['finish_observed'] = False
            elif change == 'done': row['terminal_observed'] = False
            elif change == 'output': row['quality_source'] = 'nonempty-only'
            elif change == 'tool': row['tool_valid'] = False
            elif change == 'total': row['usage']['total_tokens'] += 1
            elif change == 'negative': row['usage']['completion_tokens'] = -1
            else: row['usage'] = {'prompt_tokens': 2**64 - 1, 'completion_tokens': 1, 'total_tokens': 2**64}
            with self.subTest(change=change), self.assertRaises(l.m.GuardError):
                l.validate_records([row], [7], CORPUS)

    def test_warmup_requires_all_gpu_case_pairs_and_no_refusals(self):
        rows = [record(index, case=item['name']) for index in range(2) for item in CORPUS]
        l.validate_records(rows, [2, 7], CORPUS, True)
        with self.assertRaises(l.m.GuardError):
            l.validate_records(rows[:-1], [2, 7], CORPUS, True)
        rows[-1] = dict(rows[-1], outcome='refused', http_status=429, admission_refusal_code='admission_queue_full')
        with self.assertRaises(l.m.GuardError):
            l.validate_records(rows, [2, 7], CORPUS, True)

    def test_closed429_refusals_only_and_other_outcomes_fail(self):
        row = dict(record(), outcome='refused', http_status=429, admission_refusal_code='admission_queue_full')
        l.validate_records([record(), row], [7], CORPUS)
        for code in (None, 'rate_limit', 'admission_unavailable', 'admission_cancelled'):
            with self.subTest(code=code), self.assertRaises(l.m.GuardError):
                l.validate_records([record(), dict(row, admission_refusal_code=code)], [7], CORPUS)
        for outcome in ('http_error', 'error', 'timeout', 'cancelled', 'invalid_stream', 'invalid_output'):
            with self.subTest(outcome=outcome), self.assertRaises(l.m.GuardError):
                l.validate_records([record(), dict(row, outcome=outcome)], [7], CORPUS)

    def test_missing_nonfinite_fractional_and_reset_counters_refuse(self):
        for change in ('missing', 'nan', 'negative', 'fraction', 'accepted', 'reset'):
            before, after = counters(10), counters(20)
            if change == 'missing': after.pop('llamacpp:prompt_tokens_total')
            elif change == 'nan': after['llamacpp:tokens_predicted_total'] = float('nan')
            elif change == 'negative': after['llamacpp:prompt_seconds_total'] = -1
            elif change == 'fraction': after['llamacpp:tokens_predicted_total'] = 20.5
            elif change == 'accepted': after['llamacpp:spec_decode_num_accepted_tokens_total'] = 1
            else: after['llamacpp:tokens_predicted_total'] = 0
            with self.subTest(change=change), self.assertRaises(l.m.GuardError):
                l.accounting([record()], {7: before}, {7: after}, [7], CORPUS)

    def test_native_counter_parser_accepts_external_active_work_but_not_duplicates(self):
        values = counters(20, 2)
        raw = '\n'.join(k + ' ' + str(v) for k, v in values.items()).encode()
        self.assertEqual(l.parse_native_metrics(raw), values)
        with self.assertRaises(l.m.GuardError):
            l.parse_native_metrics(raw + b'\nllamacpp:tokens_predicted_total 20')

    def test_busy_slot_geometry_is_valid_and_changed_context_is_not(self):
        props = {'model_path': l.m.MODEL_PATH, 'model_alias': l.e.ALIAS, 'total_slots': 2}
        rows = [{'id': i, 'id_task': 10 + i, 'n_ctx': 32768, 'is_processing': True, 'speculative': False} for i in range(2)]
        with mock.patch.object(l.m, 'get', side_effect=[json.dumps(props).encode(), json.dumps(rows).encode()]):
            self.assertEqual(l.native_state(7, 2, time.monotonic() + 5)['processing_slots'], 2)
        rows[0]['n_ctx'] = 65536
        with mock.patch.object(l.m, 'get', side_effect=[json.dumps(props).encode(), json.dumps(rows).encode()]), self.assertRaises(l.m.GuardError):
            l.native_state(7, 2, time.monotonic() + 5)

    def test_fresh_idle_slot_omits_task_metadata_but_busy_missing_task_refuses(self):
        props = {'model_path': l.m.MODEL_PATH, 'model_alias': l.e.ALIAS, 'total_slots': 2}
        rows = [{'id': i, 'n_ctx': 32768, 'is_processing': False, 'speculative': False} for i in range(2)]
        with mock.patch.object(l.m, 'get', side_effect=[json.dumps(props).encode(), json.dumps(rows).encode()]):
            observed = l.native_state(7, 2, time.monotonic() + 5)
        self.assertEqual(observed['task_observations'][0]['task_observation'], 'unreported-idle')
        self.assertIsNone(observed['task_observations'][0]['id_task'])
        rows[0]['is_processing'] = True
        with mock.patch.object(l.m, 'get', side_effect=[json.dumps(props).encode(), json.dumps(rows).encode()]), self.assertRaises(l.m.GuardError):
            l.native_state(7, 2, time.monotonic() + 5)

    def test_production_busy_allowed_but_private_permits_must_zero(self):
        l.power_health(json.dumps(health(2, 8)).encode(), MANIFEST, False)
        with self.assertRaises(l.NotDrained):
            l.power_health(json.dumps(health(2, 1)).encode(), MANIFEST, True)
        for mutate in ('parallel', 'registered', 'limit', 'waiting'):
            value = health()
            if mutate == 'parallel': value['inference']['num_parallel'] = 1
            elif mutate == 'registered': value['loaded_models'] = 0
            elif mutate == 'limit': value['worker']['admission']['active_limit'] = 0
            else: value['worker']['admission']['waiting'] = -1
            with self.subTest(mutate=mutate), self.assertRaises(l.m.GuardError):
                l.power_health(json.dumps(value).encode(), MANIFEST, False)
        raw = b'power_active_requests 1\npower_requests_waiting 0\npower_requests_running 1'
        with self.assertRaises(l.NotDrained): l.power_gauges(raw, 2, True)
        l.power_gauges(raw, 2, False)

    def test_outer_http_includes_valid_refusal_transports_without_extra_permits(self):
        raw = b'power_active_requests 11\npower_requests_waiting 8\npower_requests_running 2'
        self.assertEqual(l.power_gauges(raw, 2, False)['power_active_requests'], 11)
        with self.assertRaises(l.NotDrained): l.power_gauges(raw, 2, True)
        refusal = dict(record(), outcome='refused', http_status=429, admission_refusal_code='admission_queue_full')
        l.validate_records([record(), refusal], [7], CORPUS)
        with self.assertRaises(l.m.GuardError):
            l.power_gauges(raw.replace(b'running 2', b'running 3'), 2, False)

    def test_waiting9_can_include_a_rejection_attempt_but_owned_idle_still_requires_zero(self):
        raw = b'power_active_requests 11\npower_requests_waiting 9\npower_requests_running 2'
        self.assertEqual(l.power_gauges(raw, 2, False)['power_requests_waiting'], 9)
        observed = l.power_health(json.dumps(health(2, 9)).encode(), MANIFEST, False)
        self.assertFalse(observed['queue_policy_verified_by_this_health_probe'])
        self.assertIn('admission-attempt', observed['waiting_observation'])
        with self.assertRaises(l.NotDrained): l.power_gauges(raw, 2, True)
        with self.assertRaises(l.NotDrained): l.power_health(json.dumps(health(2, 9)).encode(), MANIFEST, True)
        for waiting in (-1, 1.5, True, None):
            with self.subTest(waiting=waiting), self.assertRaises(l.m.GuardError):
                l.power_health(json.dumps(health(0, waiting)).encode(), MANIFEST, False)
        for changed in (raw.replace(b'waiting 9', b'waiting -1'), raw.replace(b'waiting 9', b'waiting 1.5'), b'power_active_requests 0\npower_requests_running 0'):
            with self.subTest(changed=changed), self.assertRaises(l.m.GuardError):
                l.power_gauges(changed, 2, False)

    def test_drain_retries_only_known_owned_busy_not_missing_metrics(self):
        with mock.patch.object(l, 'snapshot', side_effect=l.m.GuardError('missing metric')) as observe:
            with self.assertRaises(l.m.GuardError): l.drained_snapshot([7], {7: MANIFEST}, .1)
            self.assertEqual(observe.call_count, 1)
        with mock.patch.object(l, 'snapshot', side_effect=[l.NotDrained('owned busy'), {'ok': True}]), mock.patch.object(l.time, 'sleep'):
            self.assertEqual(l.drained_snapshot([7], {7: MANIFEST}), {'ok': True})

    def test_failure_cleanup_observes_only_own_power_and_never_claims_native_idle(self):
        zeros = b'power_active_requests 0\npower_requests_waiting 0\npower_requests_running 0'
        with mock.patch.object(l.m, 'get', return_value=zeros) as observe:
            result = l.cleanup_owned([7], {7: MANIFEST})
        self.assertTrue(result['all_private_permits_zero'])
        self.assertFalse(result['native_idle_claimed'])
        self.assertEqual(observe.call_args.kwargs['port'], 19507)
        with mock.patch.object(l.m, 'get', return_value=b'missing_metric 0'):
            self.assertFalse(l.cleanup_owned([7], {7: MANIFEST})['all_private_permits_zero'])

    def test_existing_evidence_is_never_replaced_or_retried(self):
        with tempfile.TemporaryDirectory() as name:
            directory = Path(name)
            target = directory / 'shared-c1-r1.json'
            target.write_bytes(b'original-failed-evidence')
            with mock.patch.object(l, 'snapshot') as observe, self.assertRaises(l.m.GuardError):
                l.run_one([], {}, {}, {}, {}, CORPUS, 'fixture-key', directory, 1, 1, 1, l.e.Budget())
            observe.assert_not_called()
            self.assertEqual(target.read_bytes(), b'original-failed-evidence')

    def test_unknown_work_cannot_be_relabelled_external_or_native_capacity(self):
        l.validate_claims(l.claims())
        for field, value in [('native_exclusive_capacity', True), ('native_output_tokens_per_second', 100),
                             ('throughput_ratio', 1.5), ('unattributed_external_delta', 10),
                             ('shared_native_cancellation', 'passed')]:
            with self.subTest(field=field), self.assertRaises(l.m.GuardError):
                l.validate_claims(dict(l.claims(), **{field: value}))

    def test_production_systemd_actual_scalar_shapes_and_service_pid(self):
        results = [{'type': 'o', 'data': ['/org/freedesktop/systemd1/unit/fixture']},
                   {'type': 'u', 'data': 42}, {'type': 's', 'data': 'active'}]
        def run(args, **kwargs):
            return subprocess.CompletedProcess(args, 0, json.dumps(results.pop(0)).encode(), b'')
        with mock.patch.object(l.subprocess, 'run', side_effect=run) as command:
            self.assertEqual(l.systemd_pid('a3s-power@7.service'), 42)
            self.assertIn('org.freedesktop.systemd1.Service', command.call_args_list[1].args[0])
        for bad in ({'type': 'u', 'data': [42]}, {'type': 's', 'data': '42'}, {'type': 'u', 'data': True}):
            results = [{'type': 'o', 'data': ['/unit/fixture']}, bad, {'type': 's', 'data': 'active'}]
            with mock.patch.object(l.subprocess, 'run', side_effect=run), self.assertRaises(l.m.GuardError):
                l.systemd_pid('a3s-power@7.service')

    def test_actual_production_native_receipt_and_power_artifact_are_required(self):
        fixed = {7: {'native': {'pid': 40}}}
        with mock.patch.object(l, 'systemd_pid', return_value=41), self.assertRaises(l.m.GuardError):
            l.production_proof([7], {7: MANIFEST}, fixed)
        with mock.patch.object(l, 'systemd_pid', side_effect=[40, 42]), mock.patch.object(l.m, 'process_identity', return_value={'exe_sha256': 'wrong'}), self.assertRaises(l.m.GuardError):
            l.production_proof([7], {7: MANIFEST}, fixed)

    def test_full_production_proof_requires_gateway_candidate_and_records_separate_health(self):
        fixed = {7: {'native': {'pid': 40}}}
        stamps = [{'pid': 42, 'exe_sha256': l.POWER_SHA}, {'pid': 43, 'exe_sha256': l.GATEWAY_SHA}]
        with mock.patch.object(l, 'systemd_pid', side_effect=[40, 42, 43]), mock.patch.object(l.m, 'process_identity', side_effect=stamps), mock.patch.object(l.m, 'get', return_value=json.dumps(health(1)).encode()):
            proof = l.production_proof([7], {7: MANIFEST}, fixed)
        self.assertEqual(proof['power-7']['health']['admission']['active'], 1)
        self.assertFalse(proof['gateway']['authenticated_public_inference_tested_here'])
        with mock.patch.object(l, 'systemd_pid', side_effect=[40, 42, 43]), mock.patch.object(l.m, 'process_identity', side_effect=[stamps[0], {'exe_sha256': 'old-gateway'}]), mock.patch.object(l.m, 'get', return_value=json.dumps(health()).encode()), self.assertRaises(l.m.GuardError):
            l.production_proof([7], {7: MANIFEST}, fixed)

    def test_identity_or_cpu_placement_change_fails(self):
        with mock.patch.object(l, 'observed_stamps', return_value={'8080': {'start_ticks': 20}}), self.assertRaises(l.m.GuardError):
            l.verify_stamps({}, {}, {'8080': {'start_ticks': 10}})

    def test_vram_guard_only_aborts_owned_sockets_and_monitor_stops(self):
        stop, abort = threading.Event(), threading.Event()
        owned, samples, errors = mock.Mock(), [], []
        with mock.patch.object(l.e, 'gpu_samples', return_value={7: {'used_mib': 23000, 'total_mib': 24000}}):
            l.monitor(stop, abort, owned, [7], {}, {}, {}, {}, samples, errors)
        self.assertTrue(abort.is_set())
        self.assertTrue(stop.is_set())
        owned.abort.assert_called_once()
        self.assertTrue(errors)

    def test429_envelope_is_read_without_exporting_raw_body(self):
        class Handler(BaseHTTPRequestHandler):
            code = 'admission_queue_full'
            def do_POST(self):
                self.rfile.read(int(self.headers['Content-Length']))
                body = json.dumps({'error': {'type': 'rate_limit_error', 'code': self.code, 'message': 'fixture-secret-not-exported'}}).encode()
                self.send_response(429)
                self.send_header('Content-Length', str(len(body)))
                self.end_headers()
                self.wfile.write(body)
            def log_message(self, *args): pass
        server = ThreadingHTTPServer(('127.0.0.1', 0), Handler)
        thread = threading.Thread(target=server.serve_forever, daemon=True)
        thread.start()
        try:
            owner = l.Requests([server.server_port])
            with owner.track():
                row = owner.once('http://127.0.0.1:' + str(server.server_port) + '/v1', CORPUS[0]['request'], 'fixture-key', 2)
            self.assertEqual(row['admission_refusal_code'], 'admission_queue_full')
            self.assertNotIn('fixture-secret', json.dumps(row))
            Handler.code = 'unknown-limit'
            owner = l.Requests([server.server_port])
            with owner.track():
                row = owner.once('http://127.0.0.1:' + str(server.server_port) + '/v1', CORPUS[0]['request'], None, 2)
            self.assertIsNone(row['admission_refusal_code'])
        finally:
            server.shutdown()
            server.server_close()
            thread.join(timeout=1)

    def test_goodput_denominator_includes_request_drain_and_not_global_tokens(self):
        owner = mock.Mock()
        owner.once.return_value = record()
        abort = threading.Event()
        observed = {}
        def once(*args, **kwargs):
            observed['start'] = time.monotonic()
            time.sleep(.04)
            observed['end'] = time.monotonic()
            return record()
        owner.once.side_effect = once
        result = l.campaign([7], CORPUS, 'fixture-key', 1, .01, owner, abort)
        self.assertEqual(len(result['records']), 1)
        self.assertGreaterEqual(result['summary']['wall_seconds_including_drain'], observed['end'] - observed['start'])
        self.assertGreater(result['summary']['wall_seconds_including_drain'], result['dispatch_seconds'])
        self.assertEqual(result['summary']['authoritative_output_tokens'], 7)
        self.assertNotIn('native_output_tokens_per_second', result)

    def test_corruption_stops_further_dispatch_and_aborts_only_owned_requests(self):
        owner = mock.Mock()
        owner.once.return_value = dict(record(), terminal_observed=False)
        abort = threading.Event()
        result = l.campaign([7], CORPUS, None, 1, .2, owner, abort)
        self.assertEqual(len(result['records']), 1)
        owner.once.assert_called_once()
        owner.abort.assert_called_once()
        self.assertTrue(abort.is_set())

    def test_absolute_watchdog_only_exits_own_process_and_is_cancelled(self):
        with mock.patch.object(l.threading, 'Timer') as timer, mock.patch.object(l, 'main') as main, mock.patch.object(l.os, '_exit') as exit_own:
            l.cli()
            self.assertEqual(timer.call_args.args[0], 5400)
            timer.call_args.args[1]()
            exit_own.assert_called_once_with(124)
            main.assert_called_once()
            timer.return_value.start.assert_called_once()
            timer.return_value.cancel.assert_called_once()

    def test_complete_shared_cell_keeps_unknown_work_separate_and_cleanup_failure_cannot_pass(self):
        for cleanup_ok in (True, False):
            with self.subTest(cleanup_ok=cleanup_ok), tempfile.TemporaryDirectory() as name:
                owner = mock.Mock()
                owner.track.return_value = contextlib.nullcontext(owner)
                owner.once.side_effect = lambda *args, **kwargs: record(case=next(item['name'] for item in CORPUS if item['request'] == args[1]))
                def observed(tokens): return {'native_counters': {7: counters(tokens, 1)}}
                def sampled(*args): args[-2].append({'fixture': 'observed'})
                measured = {'records': [record()], 'summary': l.q.summarize([record()], .02), 'request_budget_exhausted': False, 'aborted': False}
                with mock.patch.object(l, 'Requests', return_value=owner), mock.patch.object(l, 'verify_stamps'), mock.patch.object(l, 'drained_snapshot', side_effect=[observed(0), observed(28), observed(55)]), mock.patch.object(l.e, 'gpu_samples', return_value={7: {'used_mib': 10000, 'total_mib': 24000}}), mock.patch.object(l, 'monitor', side_effect=sampled), mock.patch.object(l.e, 'monitor', side_effect=sampled), mock.patch.object(l, 'campaign', return_value=measured), mock.patch.object(l, 'cleanup_owned', return_value={'all_private_permits_zero': cleanup_ok}):
                    arguments = ([7], {7: MANIFEST}, {}, {}, {}, CORPUS, 'fixture-key', Path(name), 8, 1, 1, l.e.Budget(), l.current_tools())
                    if cleanup_ok:
                        result = l.run_one(*arguments)
                        self.assertEqual(result['qualification_status'], 'passed')
                        self.assertEqual(result['accounting_by_gpu'][7]['unattributed_native_completion_delta'], 20)
                        self.assertEqual(result['own_authoritative_output_tokens_per_second'], 350)
                    else:
                        with self.assertRaises(l.m.GuardError): l.run_one(*arguments)
                        self.assertEqual(json.loads((Path(name) / 'shared-c8-r1.json').read_text())['qualification_status'], 'failed')
                owner.abort.assert_called_once()


if __name__ == '__main__':
    unittest.main()
