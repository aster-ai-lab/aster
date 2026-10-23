#!/usr/bin/env python3
"""Finite real Gateway/Power/llama qualification; no configuration or process changes."""
import argparse
import concurrent.futures
import hashlib
import http.client
import importlib.util
import json
import math
from pathlib import Path
import socket
import sys
import threading
import time

HERE = Path(__file__).resolve().parent
spec = importlib.util.spec_from_file_location('power_qualification_matrix_e2e', HERE / 'matrix-campaign.py')
m = importlib.util.module_from_spec(spec)
spec.loader.exec_module(m)
q = m.q
SOURCE_FILES = ('src/api/openai/chat.rs', 'src/api/openai/chat_runtime.rs',
                'src/api/openai/mod.rs', 'src/server/limiter.rs', 'src/server/auth.rs')
TOOL_IDENTITY = dict(m.TOOL_IDENTITY, e2e_sha256=m.digest(Path(__file__).resolve()))
POWER_GAUGES = frozenset({'power_active_requests', 'power_requests_waiting', 'power_requests_running'})


class Budget:
    def __init__(self, seconds=160):
        self.started = time.monotonic()
        self.deadline = self.started + seconds

    def left(self, maximum):
        remaining = min(maximum, self.deadline - time.monotonic())
        if remaining <= 0:
            raise m.GuardError('finite scenario budget exhausted')
        return remaining


def raw_post(port, raw, key, timeout):
    """Read bounded rejection evidence without persisting server messages or headers."""
    connection = http.client.HTTPConnection('127.0.0.1', port, timeout=timeout)
    transport, expired = [None], threading.Event()
    response = None
    started = time.monotonic()

    def expire():
        expired.set()
        active = connection.sock or transport[0]
        if active is not None:
            try:
                active.shutdown(socket.SHUT_RDWR)
            except OSError:
                pass

        # Only the reader thread may close HTTPResponse's parser state.

    timer = threading.Timer(timeout, expire)
    timer.daemon = True
    timer.start()
    try:
        connection.connect()
        transport[0] = connection.sock
        if expired.is_set():
            raise TimeoutError()
        headers = {'Content-Type': 'application/json', 'Connection': 'close'}
        if key is not None:
            headers['Authorization'] = 'Bearer ' + key
        connection.request('POST', '/v1/chat/completions', body=raw, headers=headers)
        if expired.is_set():
            raise TimeoutError()
        response = connection.getresponse()
        body = response.read(m.MAX_OBSERVATION + 1)
        if expired.is_set():
            raise TimeoutError()
        if len(body) > m.MAX_OBSERVATION:
            raise m.GuardError('bounded rejection response exceeded byte limit')
        try:
            value = json.loads(body)
        except ValueError:
            value = None
        error = value.get('error') if isinstance(value, dict) else None
        # Only the fixed expected public code/type are retained by the caller.
        return {'http_status': response.status, 'body_bytes': len(body),
                'body_sha256': hashlib.sha256(body).hexdigest(),
                'json_error': isinstance(error, dict) or isinstance(error, str),
                'error_shape': 'object' if isinstance(error, dict) else 'string' if isinstance(error, str) else 'other',
                '_code': error.get('code') if isinstance(error, dict) else None,
                '_type': error.get('type') if isinstance(error, dict) else None,
                'elapsed_ms': 1000 * (time.monotonic() - started)}
    finally:
        timer.cancel()
        if response is not None:
            response.close()
        connection.close()


def parse_metrics(raw, names):
    result = {}
    for line in raw.decode('utf-8').splitlines():
        fields = line.split()
        if len(fields) == 2 and fields[0] in names:
            name, value = fields[0], float(fields[1])
            if name in result or not math.isfinite(value) or value < 0:
                raise m.GuardError('invalid live metric observation')
            if name not in m.SECONDS and (not value.is_integer() or value > 2**53):
                raise m.GuardError('invalid live metric count')
            result[name] = value
    if set(result) != names:
        raise m.GuardError('incomplete live metric observation')
    return result


def observe(arm, budget):
    slots = json.loads(m.get('/slots', timeout=budget.left(.75)))
    if not isinstance(slots, list) or len(slots) != 1:
        raise m.GuardError('one actual private slot is required for E2E cancellation proof')
    slot = slots[0]
    if not isinstance(slot, dict) or slot.get('id') != 0 or type(slot.get('is_processing')) is not bool or slot.get('n_ctx') != 32768 or type(slot.get('id_task')) is not int:
        raise m.GuardError('invalid live native slot observation')
    power = parse_metrics(m.get('/metrics', port=m.POWER_PORTS[arm], timeout=budget.left(.75)), POWER_GAUGES)
    native = parse_metrics(m.get('/metrics', timeout=budget.left(.75)), m.COUNTER_NAMES)
    return {'at_ms': 1000 * (time.monotonic() - budget.started),
            'slot': {key: slot[key] for key in ('id', 'n_ctx', 'id_task', 'is_processing')},
            'power': power, 'native': native}


def idle(sample):
    return (not sample['slot']['is_processing'] and all(value == 0 for value in sample['power'].values())
            and all(sample['native'][name] == 0 for name in m.GAUGES))


def quiescent(sample):
    # A legacy leaked public active gauge is a recorded failure, but does not
    # mean its otherwise idle engine or released permit remains occupied.
    return (not sample['slot']['is_processing']
            and sample['power']['power_requests_running'] == 0
            and sample['power']['power_requests_waiting'] == 0
            and all(sample['native'][name] == 0 for name in m.GAUGES))


def sample_into(arm, budget, trace):
    if len(trace) >= 1600:
        raise m.GuardError('bounded scenario observation count exhausted')
    sample = observe(arm, budget)
    trace.append(sample)
    return sample


def until(arm, budget, trace, predicate, seconds):
    deadline = time.monotonic() + budget.left(seconds)
    while True:
        sample = sample_into(arm, budget, trace)
        if predicate(sample):
            return sample
        if time.monotonic() >= deadline:
            return None
        time.sleep(min(.05, max(0, deadline - time.monotonic())))


def collect(future, arm, budget, trace):
    while not future.done():
        sample_into(arm, budget, trace)
        time.sleep(min(.05, budget.left(.05)))
    return future.result()


def add_checks(row, checks):
    row['checks'] = {name: bool(value) for name, value in checks.items()}
    row['passed'] = all(row['checks'].values())
    return row


def recover(arm, budget, trace, key, short_case):
    sample = until(arm, budget, trace, quiescent, 15)
    if sample is None:
        return {'idle_within_15s': False, 'probe': None}
    record = q.request_once('http://127.0.0.1:' + str(m.GATEWAY_PORTS[arm]) + '/v1',
                            short_case['request'], key, budget.left(10), expected=short_case['expect'])
    restored = until(arm, budget, trace, quiescent, 15)
    return {'idle_within_15s': idle(sample), 'quiescent_within_15s': True, 'idle_at_ms': sample['at_ms'],
            'probe': record, 'idle_after_probe': restored is not None and idle(restored),
            'quiescent_after_probe': restored is not None}


def cancel_case(name, arm, budget, key, pool, short_case, row=None):
    trace, started = [], time.monotonic()
    before = sample_into(arm, budget, trace)
    if not quiescent(before):
        raise m.GuardError('E2E cancellation requires initially quiescent private engine and permits')
    body = {'model': m.MODEL_ALIAS, 'temperature': 0, 'seed': 42, 'max_tokens': 1200,
            'stream': True, 'stream_options': {'include_usage': True},
            'messages': [{'role': 'user', 'content': 'Print all integers 1 through 2048 inclusive, one per line, in order. Output no other text.'}]}
    url = 'http://127.0.0.1:' + str(m.GATEWAY_PORTS[arm]) + '/v1'
    if row is None:
        row = {'case': name, 'passed': False}
    row.update(trace=trace, request_sha256=q.sha256(json.dumps(body, sort_keys=True).encode()))
    if name == 'queued-cancel':
        blocker = pool.submit(q.request_once, url, body, key, budget.left(12), 4000)
        active = until(arm, budget, trace, lambda s: s['slot']['is_processing'] and s['power']['power_requests_running'] == 1, 2)
        if active is None:
            row['blocker'] = collect(blocker, arm, budget, trace)
            row['recovery'] = recover(arm, budget, trace, key, short_case)
            return add_checks(row, {'native_blocker_observed': False})
        queued = pool.submit(q.request_once, url, short_case['request'], key, budget.left(5), 350)
        queued_seen = until(arm, budget, trace, lambda s: s['power']['power_requests_waiting'] >= 1 and s['slot']['is_processing'], .3)
        row['request'] = collect(queued, arm, budget, trace)
        released = until(arm, budget, trace, lambda s: s['power']['power_requests_waiting'] == 0 and s['slot']['is_processing'] and s['slot']['id_task'] == active['slot']['id_task'], 1)
        row['blocker'] = collect(blocker, arm, budget, trace)
        cancel_returned = time.monotonic()
        recovered = until(arm, budget, trace, quiescent, 15)
        # Slot id_task survives idle: a different last task proves queued work was dispatched.
        row['queued_after_blocker'] = {'idle_within_15s': recovered is not None,
                                      'same_last_native_task': recovered is not None and recovered['slot']['id_task'] == active['slot']['id_task']}
        checks = {'native_blocker_observed': True, 'power_queue_observed': queued_seen is not None,
                  'queued_client_cancelled_before_output': row['request']['outcome'] == 'cancelled' and row['request']['meaningful_deltas'] == 0,
                  'queue_released_while_blocker_active': released is not None,
                  'queued_request_never_dispatched_after_blocker': row['queued_after_blocker']['same_last_native_task'],
                  'native_idle_within_15s_after_blocker_cancel': recovered is not None,
                  'power_all_gauges_restored': recovered is not None and idle(recovered)}
    else:
        if name == 'pre-first-meaningful-cancel':
            # Bounded synthetic prefill, well below 32768 tokens; unique content avoids a warm prefix.
            body['messages'][0]['content'] = 'Synthetic prefill for ' + arm + ': ' + ('alpha beta gamma delta ' * 2200) + '\nReply with exactly OK.'
            body['max_tokens'] = 16
            future = pool.submit(q.request_once, url, body, key, budget.left(10), 200)
        else:
            future = pool.submit(q.request_once, url, body, key, budget.left(12), None, 12)
        row['request_sha256'] = q.sha256(json.dumps(body, sort_keys=True).encode())
        row['request'] = collect(future, arm, budget, trace)
        cancel_returned = time.monotonic()
        recovered = until(arm, budget, trace, quiescent, 15)
        native_started = (any(sample['slot']['is_processing'] for sample in trace)
                          or (recovered is not None and recovered['native']['llamacpp:prompt_tokens_total'] > before['native']['llamacpp:prompt_tokens_total']))
        checks = {'native_started': native_started, 'client_cancelled': row['request']['outcome'] == 'cancelled',
                  'native_idle_within_15s_after_client_cancel': recovered is not None,
                  'power_all_gauges_restored': recovered is not None and idle(recovered)}
        if name == 'pre-first-meaningful-cancel':
            checks['no_meaningful_output_observed_by_client'] = row['request']['meaningful_deltas'] == 0
        else:
            checks['meaningful_output_observed_before_cancel'] = row['request']['meaningful_deltas'] >= 12
    row['recovery_ms_after_client_cancel'] = 1000 * (time.monotonic() - cancel_returned)
    row['recovery'] = recover(arm, budget, trace, key, short_case)
    checks['recovery_probe_passed'] = (row['recovery'].get('probe') is not None
                                      and row['recovery']['probe']['outcome'] == 'success'
                                      and row['recovery'].get('idle_after_probe') is True)
    row['elapsed_ms'] = 1000 * (time.monotonic() - started)
    row['cancellation_scope'] = 'client socket closed; observed native slot/counters and Power gauges; no GPU kernel-stop acknowledgement'
    return add_checks(row, checks)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--tag', required=True)
    args = parser.parse_args()
    if not args.tag or len(args.tag) > 48 or any(c not in 'abcdefghijklmnopqrstuvwxyz0123456789-_' for c in args.tag):
        raise ValueError('bounded lowercase tag required')
    target = m.PRIVATE / (args.tag + '.json')
    if target.exists():
        raise m.GuardError('existing scenario artifact: use a fresh tag')
    budget = Budget()
    corpus_bytes = m.read_limited(HERE / 'corpus.jsonl', 2 * m.MAX_OBSERVATION)
    corpus = [json.loads(line) for line in corpus_bytes.decode().splitlines() if line.strip()]
    short_case = next(item for item in corpus if item['name'] == 'short-stream')
    tool = next(item for item in corpus if item['name'] == 'tool-stream')
    buffered = dict(tool['request'], stream=False)
    buffered.pop('stream_options', None)
    corpus.append({'name': 'tool-buffered', 'request': buffered, 'expect': tool['expect']})
    key = m.read_limited(m.PRIVATE / 'benchmark.key', 4096).decode().strip()
    result = {'schema_version': 1, 'tool_identity': TOOL_IDENTITY, 'arms': {},
              'command': ['python3', '-B', str(Path(__file__).resolve()), '--tag', args.tag],
              'source_file_sha256': {name: m.digest(Path('/code/Power') / name) for name in SOURCE_FILES},
              'corpus_sha256': q.sha256(corpus_bytes), 'qualification_status': 'running'}
    stop, abort, samples, errors = threading.Event(), threading.Event(), [], []
    monitor = None
    try:
        fixed_engine = m.identity('baseline', 1)['engine']
        m.engine_state(1)
        sample = m.gpu_sample()
        if m.vram_exceeded(sample):
            raise m.GuardError('E2E preflight VRAM guard exceeded')
        samples.append(sample)
        monitor = threading.Thread(target=m.monitor_gpu, args=(stop, abort, samples, errors, fixed_engine, 1), daemon=True)
        monitor.start()
        for arm in ('baseline', 'candidate'):
            if abort.is_set():
                raise m.GuardError('resource monitor aborted E2E dispatch')
            fixed = m.identity(arm, 1)
            manifest = q.load_provenance(m.PRIVATE / (arm + '-provenance.json'), corpus_bytes)
            if manifest['binary_sha256'] != fixed['power']['exe_sha256'] or manifest['runtime_sha256'] != fixed['engine']['exe_sha256']:
                raise m.GuardError('E2E running artifact differs from provenance')
            current = {'process_identity': fixed, 'provenance': manifest, 'cases': []}
            result['arms'][arm] = current
            url = 'http://127.0.0.1:' + str(m.GATEWAY_PORTS[arm]) + '/v1'
            for item in corpus:
                if abort.is_set():
                    raise m.GuardError('resource monitor aborted E2E dispatch')
                record = q.request_once(url, item['request'], key, budget.left(12), expected=item['expect'])
                checks = {'expected_output_and_terminal': record['outcome'] == 'success'}
                if arm == 'candidate':
                    checks.update(authoritative_usage=record['usage_source'].startswith('upstream-'),
                                  public_usage_present=record['wire_usage'] is not None,
                                  public_usage_matches_upstream=not record['wire_usage_mismatch'])
                current['cases'].append(add_checks({'case': item['name'], 'request': record}, checks))
                m.write_result(target, result)
            rejection = [
                ('invalid-json', b'{', key, 400, None),
                # Gateway auth.rs returns 401 with error:string before Power.
                ('missing-auth', json.dumps(short_case['request']).encode(), None, 401, None),
                ('typed-choice-error', json.dumps(dict(short_case['request'], n=2)).encode(), key, 400, 'unsupported_choice_count'),
            ]
            for name, raw, credential, status, code in rejection:
                observed = raw_post(m.GATEWAY_PORTS[arm], raw, credential, budget.left(5))
                actual_code, actual_type = observed.pop('_code'), observed.pop('_type')
                checks = {'expected_http_status': observed['http_status'] == status}
                if name == 'missing-auth':
                    checks['gateway_auth_json_error'] = observed['json_error'] and observed['error_shape'] == 'string'
                if code is not None:
                    checks.update(typed_json_error=observed['json_error'], expected_error_code=actual_code == code,
                                  expected_error_type=actual_type == 'invalid_request_error')
                observed.update(expected_http_status=status, expected_error_code=code,
                                matched_error_code=actual_code == code if code is not None else None)
                current['cases'].append(add_checks({'case': name, 'response': observed}, checks))
                m.write_result(target, result)
            with concurrent.futures.ThreadPoolExecutor(max_workers=2) as pool:
                for name in ('queued-cancel', 'pre-first-meaningful-cancel', 'mid-generation-cancel'):
                    if abort.is_set():
                        raise m.GuardError('resource monitor aborted E2E dispatch')
                    row = {'case': name, 'passed': False}
                    current['cases'].append(row)
                    cancel_case(name, arm, budget, key, pool, short_case, row)
                    m.write_result(target, result)
            current['identity_unchanged'] = m.identity(arm, 1) == fixed
            current['passed'] = current['identity_unchanged'] and all(row['passed'] for row in current['cases'])
        result['qualification_status'] = 'passed' if all(row['passed'] for row in result['arms'].values()) else 'completed-with-failures'
    except Exception as exc:
        result.update(qualification_status='failed', failure_exception=type(exc).__name__)
        if isinstance(exc, m.GuardError):
            result['failure_reason'] = str(exc)
        raise
    finally:
        stop.set()
        if monitor is not None:
            monitor.join(timeout=4)
        result.update(elapsed_ms=1000 * (time.monotonic() - budget.started), gpu_samples=samples,
                      monitor_errors=errors, monitor_terminated=monitor is None or not monitor.is_alive())
        m.write_result(target, result)
        print(json.dumps({'file': target.name, 'status': result['qualification_status'],
                          'arms': {arm: {'passed': value.get('passed'),
                                         'cases': {row['case']: row['passed'] for row in value['cases']}}
                                   for arm, value in result['arms'].items()}}), flush=True)


if __name__ == '__main__':
    main()
