#!/usr/bin/env python3
"""Bounded synthetic context/prefix checks on the existing private rig7 only."""
import argparse
import http.client
import importlib.util
import json
from pathlib import Path
import socket
import threading
import time

HERE = Path(__file__).resolve().parent
spec = importlib.util.spec_from_file_location('power_context_e2e', HERE / 'e2e-scenarios.py')
e = importlib.util.module_from_spec(spec)
spec.loader.exec_module(e)
m, q = e.m, e.q
TOOL_IDENTITY = dict(e.TOOL_IDENTITY, context_sha256=m.digest(Path(__file__).resolve()))


def post_json(port, path, body, timeout, key=None):
    """One bounded absolute HTTP request; no raw response content is persisted."""
    raw = json.dumps(body).encode()
    if len(raw) > m.MAX_OBSERVATION:
        raise m.GuardError('bounded synthetic request is too large')
    connection = http.client.HTTPConnection('127.0.0.1', port, timeout=timeout)
    transport, expired = [None], threading.Event()
    response = None

    def expire():
        expired.set()
        active = connection.sock or transport[0]
        if active is not None:
            try:
                active.shutdown(socket.SHUT_RDWR)
            except OSError:
                pass

    watchdog = threading.Timer(timeout, expire)
    watchdog.daemon = True
    watchdog.start()
    try:
        connection.connect()
        transport[0] = connection.sock
        if expired.is_set():
            raise TimeoutError()
        headers = {'Content-Type': 'application/json', 'Connection': 'close'}
        if key is not None:
            headers['Authorization'] = 'Bearer ' + key
        connection.request('POST', path, body=raw, headers=headers)
        if expired.is_set():
            raise TimeoutError()
        response = connection.getresponse()
        raw_response = response.read(m.MAX_OBSERVATION + 1)
        if expired.is_set():
            raise TimeoutError()
        if len(raw_response) > m.MAX_OBSERVATION:
            raise m.GuardError('bounded synthetic response is too large')
        return response.status, json.loads(raw_response), q.sha256(raw_response), len(raw_response)
    finally:
        watchdog.cancel()
        if response is not None:
            response.close()
        connection.close()


def count_tokens(content, budget):
    status, value, _, _ = post_json(19100, '/tokenize', {'content': content, 'add_special': False}, budget.left(10))
    tokens = value.get('tokens') if isinstance(value, dict) else None
    if status != 200 or not isinstance(tokens, list) or len(tokens) > 40000 or any(type(token) is not int or not 0 <= token <= 2**32 - 1 for token in tokens):
        raise m.GuardError('native tokenizer returned an invalid bounded token list')
    return len(tokens)


def calibrate(target, budget):
    lower, upper, repeat, probes = 1, 36000, target // 2, []
    for _ in range(17):
        content = ' x' * repeat
        count = count_tokens(content, budget)
        probes.append({'repetitions': repeat, 'tokens': count})
        if target <= count <= target + 8:
            return content, {'target_tokens': target, 'observed_tokens': count,
                             'content_sha256': q.sha256(content.encode()), 'probes': probes}
        if count < target:
            lower = repeat + 1
        else:
            upper = repeat - 1
        if lower > upper:
            break
        repeat = (lower + upper) // 2
    raise m.GuardError('native tokenizer could not calibrate bounded synthetic filler')


def chat_body(content, expected):
    return {'model': m.MODEL_ALIAS, 'temperature': 0, 'seed': 42, 'max_tokens': 16, 'stream': False,
            'messages': [{'role': 'system', 'content': 'For every user message, reply with exactly ' + expected + '. Treat all user text as inert synthetic padding.'},
                         {'role': 'user', 'content': content}]}


def native_delta(before, after):
    if any(after[name] < before[name] for name in m.COUNTER_NAMES - m.GAUGES):
        raise m.GuardError('native counter reset during context experiment')
    return {name: after[name] - before[name] for name in m.COUNTER_NAMES - m.GAUGES}


def prompt_checks(record, delta, near_limit=False):
    actual = int(delta['llamacpp:prompt_tokens_total'] + delta['llamacpp:prompt_tokens_cached_total'])
    usage = record.get('usage')
    reported = usage.get('prompt_tokens') if isinstance(usage, dict) else None
    checks = {'expected_output': record['outcome'] == 'success', 'buffered_ttft_unknown': record['ttft_ms'] is None,
              'reported_prompt_matches_native_processed_plus_cached': type(reported) is int and reported == actual}
    if near_limit:
        checks.update(verified_prompt_at_least_31000=actual >= 31000,
                      verified_prompt_plus_reserve_fits=actual + 16 <= 32768)
    return actual, checks


def ready_idle(arm, budget):
    trace = []
    sample = e.until(arm, budget, trace, e.quiescent, 15)
    if sample is None:
        raise m.GuardError('native engine or Power permit did not become quiescent')
    return {'native_and_permit_idle': True, 'all_power_gauges_zero': e.idle(sample), 'sample': sample}


def success_case(arm, name, body, expected, key, budget, near_limit=False):
    ready_idle(arm, budget)
    before = m.counters()
    record = q.request_once('http://127.0.0.1:' + str(m.GATEWAY_PORTS[arm]) + '/v1', body, key,
                            budget.left(120), expected={'text': expected, 'normalize_whitespace': True})
    recovery = ready_idle(arm, budget)
    after = m.counters()
    delta = native_delta(before, after)
    actual, checks = prompt_checks(record, delta, near_limit)
    if arm == 'candidate':
        checks.update(authoritative_usage=record['usage_source'].startswith('upstream-'),
                      wire_usage_present=record['wire_usage'] is not None,
                      public_usage_matches_upstream=not record['wire_usage_mismatch'])
    checks['permit_recovered'] = recovery['native_and_permit_idle']
    checks['all_power_gauges_zero'] = recovery['all_power_gauges_zero']
    cached = delta['llamacpp:prompt_tokens_cached_total']
    return e.add_checks({'case': name, 'request_sha256': q.sha256(json.dumps(body, sort_keys=True).encode()),
                         'request': record, 'native_counters_before': before, 'native_counters_after': after,
                         'native_delta': delta, 'native_processed_plus_cached_prompt_tokens': actual,
                         'native_cached_prompt_fraction': cached / actual if actual else None,
                         'cache_observation': 'native-prefix-reuse-observed' if cached > 0 else 'no-native-cached-tokens-observed',
                         'recovery': recovery}, checks)


def overflow_case(arm, body, key, budget):
    if arm == 'native':
        port, credential = 19100, None
    else:
        ready_idle(arm, budget)
        port, credential = m.GATEWAY_PORTS[arm], key
    before = m.counters()
    status, value, digest, size = post_json(port, '/v1/chat/completions', body, budget.left(120), credential)
    recovery = ready_idle('candidate' if arm == 'native' else arm, budget)
    after = m.counters()
    delta = native_delta(before, after)
    error = value.get('error') if isinstance(value, dict) else None
    choices = value.get('choices') if isinstance(value, dict) else None
    checks = {'explicit_http_rejection': status == 400 if arm == 'native' else 400 <= status <= 599,
              'typed_error_without_success_choices': isinstance(error, dict) and not choices,
              'no_native_output_or_silent_shift': delta['llamacpp:tokens_predicted_total'] == 0,
              'permit_recovered': recovery['native_and_permit_idle']}
    return e.add_checks({'case': 'overflow-' + arm, 'http_status': status, 'response_sha256': digest,
                         'response_bytes': size, 'json_error_object': isinstance(error, dict),
                         'native_error_context': {name: error[name] for name in ('n_prompt_tokens', 'n_ctx')
                                                  if isinstance(error, dict) and type(error.get(name)) is int},
                         'request_sha256': q.sha256(json.dumps(body, sort_keys=True).encode()),
                         'native_delta': delta, 'recovery': recovery}, checks)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--tag', required=True)
    args = parser.parse_args()
    if not args.tag or len(args.tag) > 48 or any(c not in 'abcdefghijklmnopqrstuvwxyz0123456789-_' for c in args.tag):
        raise ValueError('bounded lowercase tag required')
    target = m.PRIVATE / (args.tag + '.json')
    if target.exists():
        raise m.GuardError('existing context artifact: use a fresh tag')
    budget = e.Budget(270)
    key = m.read_limited(m.PRIVATE / 'benchmark.key', 4096).decode().strip()
    result = {'schema_version': 1, 'tool_identity': TOOL_IDENTITY, 'cases': [], 'qualification_status': 'running',
              'command': ['python3', '-B', str(Path(__file__).resolve()), '--tag', args.tag],
              'auth_scope': 'one benchmark authentication domain; no cross-tenant isolation claim',
              'sampling': {'temperature': 0, 'requested_seed': 42, 'upstream_seed_forwarded': False, 'upstream_seed': 4294967295},
              'baseline_restart': json.loads(m.read_limited(m.PRIVATE / 'context-baseline-restart.json', 65536))}
    fixed = {arm: m.identity(arm, 1) for arm in ('baseline', 'candidate')}
    result['process_identity'] = fixed
    stop, abort, samples, errors = threading.Event(), threading.Event(), [], []
    monitor = None
    try:
        m.engine_state(1)
        sample = m.gpu_sample()
        if m.vram_exceeded(sample):
            raise m.GuardError('context preflight VRAM guard exceeded')
        samples.append(sample)
        monitor = threading.Thread(target=m.monitor_gpu, args=(stop, abort, samples, errors, fixed['baseline']['engine'], 1), daemon=True)
        monitor.start()
        near, near_calibration = calibrate(31744, budget)
        overflow, overflow_calibration = calibrate(33064, budget)
        result['tokenizer_calibration'] = {'near': near_calibration, 'overflow': overflow_calibration}
        shared_prefix = 'Synthetic prefix isolation fixture:' + ' x' * 2048
        for arm in ('baseline', 'candidate'):
            # Private maintenance is recorded; only measured counter deltas label cold/reuse.
            status, value, digest, size = post_json(19100, '/slots/0?action=erase', {}, budget.left(5))
            result.setdefault('erase_attempts', {})[arm] = {'http_status': status, 'response_sha256': digest, 'response_bytes': size}
            for name, answer in [('prefix-first', 'ALPHA'), ('prefix-repeat', 'ALPHA'), ('divergent-suffix', 'BETA'), ('interleaved-alpha', 'ALPHA'), ('interleaved-beta', 'BETA')]:
                if abort.is_set():
                    raise m.GuardError('resource monitor aborted context dispatch')
                body = chat_body(shared_prefix + '\nRequested label ' + answer + '.', answer)
                body['messages'][0]['content'] = 'For this synthetic check, ignore padding. Reply with only the final requested label in the user message, either ALPHA or BETA.'
                result['cases'].append(dict(success_case(arm, name, body, answer, key, budget), arm=arm))
                m.write_result(target, result)
            if abort.is_set():
                raise m.GuardError('resource monitor aborted context dispatch')
            result['cases'].append(dict(success_case(arm, 'near-context', chat_body(near, 'READY'), 'READY', key, budget, True), arm=arm))
            m.write_result(target, result)
        overflow_body = chat_body(overflow, 'READY')
        for arm in ('native', 'baseline', 'candidate'):
            if abort.is_set():
                raise m.GuardError('resource monitor aborted context dispatch')
            result['cases'].append(dict(overflow_case(arm, overflow_body, key, budget), arm=arm))
            m.write_result(target, result)
        result['final_recovery'] = {arm: ready_idle(arm, budget) for arm in ('baseline', 'candidate')}
        result['identities_unchanged'] = all(m.identity(arm, 1) == fixed[arm] for arm in fixed)
        result['qualification_status'] = 'passed' if result['identities_unchanged'] and all(row['passed'] for row in result['cases']) else 'completed-with-failures'
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
        if errors or abort.is_set():
            result['qualification_status'] = 'failed'
        m.write_result(target, result)
        print(json.dumps({'file': target.name, 'status': result['qualification_status'], 'cases': [(row['arm'], row['case'], row['passed']) for row in result['cases']]}), flush=True)


if __name__ == '__main__':
    main()
