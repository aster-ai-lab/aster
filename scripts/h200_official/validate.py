#!/usr/bin/env python3
"""Isolated Power frontend checks using the already running official H200 engine."""

import argparse
import concurrent.futures
import hashlib
import json
import os
import secrets
import socket
import statistics
import subprocess
import sys
import threading
import time
import urllib.error
import urllib.request
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from h200_xq import service as ownership
from power_serving_qualification import request_once
from h200_official.loopback_recipe import MODEL_PATH

ROOT = Path('/root/aster-h200-official-20261022')
MODEL = 'deepseek-v4.1-flash-1017'
POWER_SHA256 = '0076e2a46734b54a51c6aea7b3e56dedb5326c3ed4b32b0f0ead919122c45d76'
RECIPE_PYTHON = Path('/root/deepseek-v4-1-flash-1017/recipe-venv/bin/python')
CONCURRENCY_CHOICES = (1, 2, 4, 8, 16, 32)


def publish(name, record):
    destination = ROOT/'run'/name
    temporary = destination.with_suffix('.tmp')
    temporary.write_text(json.dumps(record, indent=2))
    temporary.replace(destination)


def read_json(url, key=None):
    headers = {'Authorization': 'Bearer '+key} if key else {}
    with urllib.request.urlopen(urllib.request.Request(url, headers=headers), timeout=10) as response:
        data = response.read(4*1024**2+1)
    if len(data) > 4*1024**2:
        raise RuntimeError('server response exceeds the inspection limit')
    return json.loads(data)


def active_requests():
    with urllib.request.urlopen('http://127.0.0.1:30102/metrics', timeout=5) as response:
        payload = response.read(1024**2+1)
    if len(payload) > 1024**2:
        raise RuntimeError('metrics exceed the inspection limit')
    for line in payload.decode().splitlines():
        fields = line.split()
        if len(fields) == 2 and fields[0] == 'power_active_requests':
            return int(fields[1])
    raise RuntimeError('active-request gauge is missing')


def safe_engine_state():
    key = Path('/root/deepseek-v4-1-flash-1017/backend-api-key').read_text().strip()
    info = read_json('http://127.0.0.1:30000/get_server_info', key)
    if info.get('model_path') != MODEL_PATH or info.get('tp_size') != 8:
        raise RuntimeError('existing backend is not the pinned official eight-card model')
    names = ('model_path', 'version', 'tp_size', 'ep_size', 'mem_fraction_static',
             'max_running_requests', 'chunked_prefill_size', 'max_prefill_tokens',
             'enable_mixed_chunk', 'enable_strict_thinking', 'attention_backend',
             'moe_runner_backend', 'disable_radix_cache', 'disable_overlap_schedule',
             'disable_cuda_graph', 'speculative_algorithm', 'speculative_num_steps',
             'speculative_num_draft_tokens', 'enable_eplb', 'ep_num_redundant_experts',
             'enable_dp_attention', 'dp_size', 'kv_cache_dtype', 'moe_a2a_backend')
    allowed = ('num_running_reqs', 'num_queue_reqs', 'last_gen_throughput',
               'memory_usage', 'effective_max_running_requests_per_dp')
    return {'configuration': {name: info.get(name) for name in names},
            'workers': [{name: state[name] for name in allowed if name in state}
                        for state in info.get('internal_states', []) if isinstance(state, dict)]}


def ensure_ports_free(ports):
    for port in ports:
        with socket.socket() as listener:
            listener.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
            listener.bind(('127.0.0.1', port))


def serving_limit(engine, max_concurrent):
    if type(max_concurrent) is not int or max_concurrent not in CONCURRENCY_CHOICES:
        raise ValueError('concurrency must be 1, 2, 4, 8, 16, or 32')
    capacity = engine['configuration'].get('max_running_requests')
    if type(capacity) is not int or capacity < max_concurrent:
        raise RuntimeError('requested admission exceeds the verified upstream capacity')
    return max_concurrent


def start(exact_benchmark_usage=False, max_concurrent=4):
    ownership.ROOT = ROOT
    ownership.PRIVATE = ROOT
    for name in ('logs', 'run', 'bin'):
        (ROOT/name).mkdir(parents=True, exist_ok=True)
    ROOT.chmod(0o700)
    ensure_ports_free((30102, 30103))
    power = ROOT/'bin/aster-power'
    if hashlib.sha256(power.read_bytes()).hexdigest() != POWER_SHA256:
        raise RuntimeError('Power binary differs from the H200-validated build')
    engine = safe_engine_state()
    serving_limit(engine, max_concurrent)
    key_path = ROOT/'api.key'
    if not key_path.exists():
        key_path.write_text(secrets.token_hex(32))
    key_path.chmod(0o600)
    key_hash = hashlib.sha256(key_path.read_text().strip().encode()).hexdigest()
    config = ROOT/'power.acl'
    redact = 'false' if exact_benchmark_usage else 'true'
    config.write_text(f'host = "0.0.0.0"\nport = 30102\ndata_dir = "{ROOT}/data"\n'
                      f'api_keys = ["{key_hash}"]\nredact_logs = {redact}\nnum_parallel = 1\n'
                      f'max_concurrent_requests = {max_concurrent}\n'
                      'max_queued_requests = 8\nqueue_timeout_ms = 300000\n'
                      f'proxy_upstream "{MODEL}" {{\n url = "http://127.0.0.1:30103"\n}}\n')
    config.chmod(0o600)
    environment = dict(os.environ)
    environment['ASTER_POWER_HOME'] = str(ROOT/'data')
    bridge_path = Path(__file__).with_name('loopback_recipe.py')
    bridge = ownership.start_process('official-recipe', [str(RECIPE_PYTHON), str(bridge_path)], environment)
    try:
        ownership.wait_health(bridge, 30103, 60)
        frontend = ownership.start_process('official-power', [str(power), 'serve', '--config', str(config)], environment)
        ownership.wait_health(frontend, 30102, 60)
    except BaseException:
        ownership.stop_owned('official-power', power)
        ownership.stop_owned('official-recipe', bridge_path)
        raise
    record = {'unix': time.time(), 'power_pid': frontend.pid, 'recipe_pid': bridge.pid,
              'power_sha256': POWER_SHA256, 'power_port': 30102, 'recipe_loopback_port': 30103,
              'existing_sglang_port': 30000, 'model': MODEL, 'engine': engine,
              'existing_engine_restarted': False, 'inference_tested': False,
              'max_concurrent_requests': max_concurrent,
              'exact_benchmark_usage': exact_benchmark_usage,
              'synthetic_requests_only': exact_benchmark_usage}
    publish('frontend.json', record)
    print(json.dumps(record), flush=True)


def check():
    key = (ROOT/'api.key').read_text().strip()
    checks = []
    for name, credential in [('anonymous', None), ('invalid', 'invalid-synthetic-key')]:
        try:
            read_json('http://127.0.0.1:30102/v1/models', credential)
            passed, code = False, 200
        except urllib.error.HTTPError as error:
            code = error.code
            passed = code == 401
        checks.append({'name': name+'_models', 'passed': passed, 'status': code})
    models = read_json('http://127.0.0.1:30102/v1/models', key)
    checks.append({'name': 'authenticated_models', 'passed': MODEL in [item['id'] for item in models['data']]})
    corpus = [
        ('nonstream_answer', False, 'Compute 17 + 25. Reply with only the integer answer.', {'text': '42', 'normalize_whitespace': True}),
        ('stream_answer', True, 'Compute 9 * 13. Reply with only the integer answer.', {'text': '117', 'normalize_whitespace': True}),
    ]
    for name, streaming, prompt, expected in corpus:
        body = {'model': MODEL, 'messages': [{'role': 'user', 'content': prompt}],
                'temperature': 0, 'max_tokens': 1024, 'stream': streaming}
        if streaming:
            body['stream_options'] = {'include_usage': True}
        result = request_once('http://127.0.0.1:30102/v1', body, key, 180, expected=expected)
        checks.append({'name': name, 'passed': result['outcome'] == 'success', 'result': result})
        publish('qualification.json', {'unix': time.time(), 'complete': False, 'checks': checks})
        print(json.dumps(checks[-1]), flush=True)
    body = {'model': MODEL, 'messages': [{'role': 'user', 'content': 'Call bench_sum with a=19 and b=23.'}],
            'tools': [{'type': 'function', 'function': {'name': 'bench_sum', 'parameters': {
                'type': 'object', 'properties': {'a': {'type': 'integer'}, 'b': {'type': 'integer'}},
                'required': ['a', 'b'], 'additionalProperties': False}}}],
            # The pinned recipe refuses forced functions in its default thinking
            # mode. Exercise its supported automatic tool selection explicitly.
            'tool_choice': 'auto',
            'temperature': 0, 'max_tokens': 1024, 'stream': True, 'stream_options': {'include_usage': True}}
    result = request_once('http://127.0.0.1:30102/v1', body, key, 180,
                          expected={'tool': 'bench_sum'})
    checks.append({'name': 'stream_tool_call', 'passed': result['outcome'] == 'success', 'result': result})
    forced = {**body, 'tool_choice': {'type': 'function', 'function': {'name': 'bench_sum'}}}
    result = request_once('http://127.0.0.1:30102/v1', forced, key, 15)
    checks.append({'name': 'unsupported_forced_function_is_client_error',
                   'passed': result['http_status'] == 400, 'result': result})
    cancellation = {'model': MODEL, 'messages': [{'role': 'user', 'content':
                    'Write a long tutorial about sorting algorithms and keep explaining with examples.'}],
                    'max_tokens': 256, 'stream': True}
    result = request_once('http://127.0.0.1:30102/v1', cancellation, key, 180, cancel_after_deltas=1)
    for _ in range(25):
        remaining = active_requests()
        if remaining == 0:
            break
        time.sleep(.2)
    checks.append({'name': 'client_cancellation_releases_frontend_request',
                   'passed': result['outcome'] == 'cancelled' and remaining == 0,
                   'active_requests_after': remaining, 'result': result})
    report = {'unix': time.time(), 'complete': True, 'passed': all(item['passed'] for item in checks),
              'checks': checks, 'inference_tested': True, 'model': MODEL,
              'tool_choice': 'auto', 'forced_function_in_default_thinking': 'unsupported_by_official_recipe',
              'engine': safe_engine_state()}
    publish('qualification.json', report)
    print(json.dumps(report), flush=True)
    if not report['passed']:
        raise RuntimeError('official model end-to-end qualification failed; inspect the report')


def benchmark(concurrencies=(1, 2, 4), rounds=3, output_name='benchmark.json'):
    if (not concurrencies or any(type(n) is not int or n not in CONCURRENCY_CHOICES for n in concurrencies)
            or len(set(concurrencies)) != len(concurrencies) or not 1 <= rounds <= 3):
        raise ValueError('benchmark requires unique supported concurrencies and one to three rounds')
    if Path(output_name).name != output_name or not output_name.endswith('.json'):
        raise ValueError('benchmark output must be a JSON basename')
    qualification = json.loads((ROOT/'run/qualification.json').read_text())
    if not qualification.get('passed'):
        raise RuntimeError('passing end-to-end qualification is required before benchmarking')
    frontend = json.loads((ROOT/'run/frontend.json').read_text())
    if not frontend.get('exact_benchmark_usage'):
        raise RuntimeError('synthetic benchmark mode is required; privacy-rounded counts cannot certify token speed')
    capacity = frontend.get('max_concurrent_requests', 4)
    if type(capacity) is not int or max(concurrencies) > capacity:
        raise RuntimeError('benchmark concurrency exceeds the owned frontend admission limit')
    key = (ROOT/'api.key').read_text().strip()
    results = []
    for concurrency in concurrencies:
        for round_index in range(rounds):
            before = safe_engine_state()
            nonce = secrets.token_hex(8)
            barrier = threading.Barrier(concurrency+1)
            def request(index):
                prompt = (f'Workload {nonce}, cohort {concurrency}, item {index}.\n'
                          'Write a detailed tutorial about binary search with Python code, invariants, '
                          'boundary cases and tests. Continue until all topics are covered.')
                body = {'model': MODEL, 'messages': [{'role': 'user', 'content': prompt}],
                        'temperature': 0, 'max_tokens': 256, 'stream': True,
                        'stream_options': {'include_usage': True}}
                barrier.wait(timeout=20)
                return request_once('http://127.0.0.1:30102/v1', body, key, 180)
            with concurrent.futures.ThreadPoolExecutor(max_workers=concurrency) as pool:
                jobs = [pool.submit(request, index) for index in range(concurrency)]
                started = time.monotonic()
                barrier.wait(timeout=20)
                records = [job.result() for job in jobs]
                elapsed = time.monotonic()-started
            valid = all(record['outcome'] == 'success' and record.get('usage')
                        and record['usage_source'].startswith('upstream-') for record in records)
            tokens = sum(record['usage']['completion_tokens'] for record in records) if valid else None
            result = {'concurrency': concurrency, 'round': round_index, 'wall_s': elapsed,
                      'completion_tokens': tokens,
                      'aggregate_output_tokens_per_second': tokens/elapsed if valid else None,
                      'passed': valid, 'engine_before': before, 'engine_after': safe_engine_state(),
                      'records': records}
            results.append(result)
            publish(output_name, {'unix': time.time(), 'complete': False, 'frontend': frontend, 'results': results})
            print(json.dumps({name: value for name, value in result.items() if name != 'records'}), flush=True)
            if not valid:
                raise RuntimeError('a benchmark request did not complete with exact usage; results retained')
    report = {'unix': time.time(), 'complete': True, 'model': MODEL, 'results': results,
              'median_output_tokens_per_second': {str(n): statistics.median(
                  row['aggregate_output_tokens_per_second'] for row in results if row['concurrency'] == n)
                  for n in concurrencies},
              'scope': 'Existing shared official backend; owned frontend admission and batch-size comparison',
              'through_power': True, 'frontend': frontend, 'engine': safe_engine_state()}
    publish(output_name, report)
    print(json.dumps({name: value for name, value in report.items() if name != 'results'}), flush=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('action', choices=['start', 'check', 'benchmark', 'stop'])
    parser.add_argument('--exact-benchmark-usage', action='store_true',
                        help='use only synthetic requests with unrounded accounting; restart normally after the benchmark')
    parser.add_argument('--max-concurrent', type=int, choices=CONCURRENCY_CHOICES, default=4)
    parser.add_argument('--concurrency', type=int, choices=CONCURRENCY_CHOICES, nargs='+', default=[1, 2, 4])
    parser.add_argument('--rounds', type=int, choices=(1, 2, 3), default=3)
    parser.add_argument('--output-name', default='benchmark.json')
    args = parser.parse_args()
    if args.action == 'start':
        start(args.exact_benchmark_usage, args.max_concurrent)
    elif args.action == 'check':
        check()
    elif args.action == 'benchmark':
        benchmark(tuple(args.concurrency), args.rounds, args.output_name)
    else:
        ownership.ROOT = ROOT
        ownership.PRIVATE = ROOT
        ownership.stop_owned('official-power', ROOT/'bin/aster-power')
        ownership.stop_owned('official-recipe', Path(__file__).with_name('loopback_recipe.py'))
        print('OWN_OFFICIAL_FRONTEND_STOPPED', flush=True)


if __name__ == '__main__':
    main()
