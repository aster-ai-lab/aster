#!/usr/bin/env python3
"""Finite paired qualification of existing private processes; never serving configuration."""
import argparse
from collections import namedtuple
import datetime
import hashlib
import http.client
import json
import math
import os
from pathlib import Path
import signal
import socket
import subprocess
import sys
import tempfile
import threading
import time

EVIDENCE = Path(__file__).resolve().parent
sys.path.insert(0, str(EVIDENCE.parents[3] / 'scripts'))
import power_serving_qualification as q

PRIVATE = Path('/code/Power/.git/optimization-20261021')
Rig = namedtuple('Rig', 'index private engine_port power_ports gateway_ports gpu_index key_path')
RIGS = {
    7: Rig(7, PRIVATE, 19100, {'baseline': 19200, 'candidate': 19210},
           {'baseline': 19880, 'candidate': 19881}, 7, PRIVATE / 'benchmark.key'),
    6: Rig(6, PRIVATE / 'rig6', 19110, {'baseline': 19300, 'candidate': 19310},
           {'baseline': 19882, 'candidate': 19883}, 6, PRIVATE / 'benchmark.key'),
    5: Rig(5, PRIVATE / 'rig5', 19120, {'baseline': 19400, 'candidate': 19410},
           {'baseline': 19884, 'candidate': 19885}, 5, PRIVATE / 'benchmark.key'),
}
DEFAULT_RIG = RIGS[7]
PROC = Path('/proc')
ENGINE_PORT = 19100
MODEL_PATH = '/data/models/Ternary-Bonsai-2-27B-PTQ1_0.gguf'
MODEL_ALIAS = 'prism27b'
CONTEXT_PER_SEQUENCE = 32768
POWER_PORTS = {'baseline': 19200, 'candidate': 19210}
GATEWAY_PORTS = {'baseline': 19880, 'candidate': 19881}
MAX_OBSERVATION = 1024 * 1024
COUNTER_NAMES = frozenset({
    'llamacpp:tokens_predicted_total', 'llamacpp:tokens_predicted_seconds_total',
    'llamacpp:prompt_tokens_total', 'llamacpp:prompt_tokens_cached_total',
    'llamacpp:prompt_seconds_total', 'llamacpp:requests_processing',
    'llamacpp:requests_deferred', 'llamacpp:spec_decode_num_draft_tokens_total',
    'llamacpp:spec_decode_num_accepted_tokens_total',
})
GAUGES = frozenset({'llamacpp:requests_processing', 'llamacpp:requests_deferred'})
SECONDS = frozenset({'llamacpp:tokens_predicted_seconds_total', 'llamacpp:prompt_seconds_total'})


class GuardError(RuntimeError):
    """Only fixed, content-free diagnostics belong in persisted errors."""


def read_limited(path, limit):
    with Path(path).open('rb') as stream:
        value = stream.read(limit + 1)
    if len(value) > limit:
        raise GuardError('identity or input observation exceeds byte limit')
    return value


def digest(path):
    result = hashlib.sha256()
    with Path(path).open('rb') as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b''):
            result.update(block)
    return result.hexdigest()


# Capture the instrument files when this process imports them, before dispatch.
TOOL_IDENTITY = {'runner_sha256': digest(Path(q.__file__).resolve()),
                 'matrix_sha256': digest(Path(__file__).resolve())}


def get(path, *, port=None, timeout=3, rig=DEFAULT_RIG):
    """Absolute watchdog includes connection, headers and slow-drip body."""
    connection = http.client.HTTPConnection('127.0.0.1', rig.engine_port if port is None else port,
                                             timeout=timeout)
    transport = [None]
    expired = threading.Event()
    response = None

    def expire():
        expired.set()
        active = connection.sock or transport[0]
        if active is not None:
            try:
                active.shutdown(socket.SHUT_RDWR)
            except OSError:
                pass

        # The reader thread closes HTTPResponse; mutating its parser here races
        # HTTPResponse's chunked-body cleanup.

    watchdog = threading.Timer(timeout, expire)
    watchdog.daemon = True
    watchdog.start()
    try:
        connection.connect()
        transport[0] = connection.sock
        if expired.is_set():
            raise GuardError('absolute engine observation deadline exceeded')
        connection.request('GET', path, headers={'Connection': 'close'})
        if expired.is_set():
            raise GuardError('absolute engine observation deadline exceeded')
        response = connection.getresponse()
        if response.status != 200:
            raise GuardError('engine observation returned non-200 status')
        raw = response.read(MAX_OBSERVATION + 1)
        if expired.is_set():
            raise GuardError('absolute engine observation deadline exceeded')
        if len(raw) > MAX_OBSERVATION:
            raise GuardError('engine observation exceeds byte limit')
        return raw
    finally:
        watchdog.cancel()
        if response is not None:
            response.close()
        connection.close()


def parse_listeners(raw, port):
    found = set()
    for line in raw.decode('ascii').splitlines()[1:]:
        fields = line.split()
        if len(fields) >= 10 and fields[3] == '0A':
            address, separator, encoded_port = fields[1].rpartition(':')
            if separator and int(encoded_port, 16) == port:
                found.add(fields[9])
    return found


def listener_inodes(port):
    found = set()
    for name in ('tcp', 'tcp6'):
        found.update(parse_listeners(read_limited(PROC / 'net' / name, 4 * MAX_OBSERVATION), port))
    if not found:
        raise GuardError('expected private listener is absent')
    return found


def socket_inodes(pid):
    found = set()
    for descriptor in (PROC / str(pid) / 'fd').iterdir():
        try:
            target = os.readlink(descriptor)
        except FileNotFoundError:
            continue
        if target.startswith('socket:[') and target.endswith(']'):
            found.add(target[8:-1])
    return found


def require_listener(pid, port):
    listeners = listener_inodes(port)
    if not listeners.issubset(socket_inodes(pid)):
        raise GuardError('private port is owned by a different process')
    return sorted(listeners)


def start_ticks(raw):
    fields = raw.decode('utf-8').rsplit(')', 1)[1].split()
    value = int(fields[19])  # proc stat field 22, after parenthesized comm.
    if value <= 0:
        raise GuardError('invalid process start ticks')
    return value


def process_identity(pid, port):
    if type(pid) is not int or not 0 < pid <= 2**31 - 1:
        raise GuardError('invalid private process PID')
    directory = PROC / str(pid)
    command = read_limited(directory / 'cmdline', 65536)
    return {'pid': pid, 'start_ticks': start_ticks(read_limited(directory / 'stat', 65536)),
            'cmdline_sha256': hashlib.sha256(command).hexdigest(),
            'exe_sha256': digest(directory / 'exe'),
            'listener_inodes': require_listener(pid, port), 'port': port}


def command_flag(command, aliases):
    values = []
    for index, token in enumerate(command):
        if token in aliases:
            if index + 1 >= len(command):
                raise GuardError('private engine command flag lacks a value')
            values.append(command[index + 1])
        for alias in aliases:
            if token.startswith(alias + '='):
                values.append(token[len(alias) + 1:])
    if len(values) != 1:
        raise GuardError('private engine command flag is missing or ambiguous')
    return values[0]


def engine_identity(pid, slots, rig=DEFAULT_RIG):
    result = process_identity(pid, rig.engine_port)
    command = read_limited(PROC / str(pid) / 'cmdline', 65536).decode().rstrip('\0').split('\0')
    expected = ((('--port',), str(rig.engine_port)), (('--host',), '127.0.0.1'),
                (('-m', '--model'), MODEL_PATH), (('--alias',), MODEL_ALIAS),
                (('-c', '--ctx-size'), str(CONTEXT_PER_SEQUENCE * slots)),
                (('--parallel', '-np'), str(slots)))
    if any(command_flag(command, aliases) != value for aliases, value in expected):
        raise GuardError('private engine command identity differs from experiment')
    environment = read_limited(PROC / str(pid) / 'environ', 65536).split(b'\0')
    bindings = [item.split(b'=', 1)[1] for item in environment if item.startswith(b'CUDA_VISIBLE_DEVICES=')]
    if bindings != [str(rig.gpu_index).encode()]:
        raise GuardError('private engine CUDA binding differs from rig')
    result['cuda_visible_devices'] = str(rig.gpu_index)
    return result


def identity(arm, slots, rig=DEFAULT_RIG):
    power_pid = int(read_limited(rig.private / (arm + '.pid'), 32))
    gateway_pid = int(read_limited(rig.private / ('gateway-' + arm + '.pid'), 32))
    engine_pid = int(read_limited(rig.private / 'engine.pid', 32))
    return {'power': process_identity(power_pid, rig.power_ports[arm]),
            'gateway': process_identity(gateway_pid, rig.gateway_ports[arm]),
            'engine': engine_identity(engine_pid, slots, rig)}


def engine_state(expected_slots, rig=DEFAULT_RIG):
    props = json.loads(get('/props', rig=rig))
    slots = json.loads(get('/slots', rig=rig))
    if not isinstance(props, dict) or props.get('model_path') != MODEL_PATH or props.get('model_alias') != MODEL_ALIAS:
        raise GuardError('actual engine model path or alias differs')
    if type(props.get('total_slots')) is not int or props['total_slots'] != expected_slots:
        raise GuardError('actual engine slot count differs')
    if not isinstance(slots, list) or len(slots) != expected_slots or not slots:
        raise GuardError('actual engine slot observation is empty or incomplete')
    identifiers = set()
    for slot in slots:
        if not isinstance(slot, dict) or type(slot.get('id')) is not int or slot['id'] in identifiers:
            raise GuardError('actual engine slot identity is invalid')
        identifiers.add(slot['id'])
        if type(slot.get('n_ctx')) is not int or slot['n_ctx'] != CONTEXT_PER_SEQUENCE:
            raise GuardError('actual per-sequence context differs from 32768')
        if type(slot.get('is_processing')) is not bool or slot['is_processing']:
            raise GuardError('private engine is occupied or its state is invalid')
    if identifiers != set(range(expected_slots)):
        raise GuardError('actual engine slot identity differs from expected slots')
    settings = props.get('default_generation_settings', {})
    default_context = settings.get('n_ctx') if isinstance(settings, dict) else None
    if default_context is not None and (type(default_context) is not int or default_context <= 0):
        raise GuardError('default engine context observation is invalid')
    return {'model_path': props['model_path'], 'model_alias': props['model_alias'],
            'total_slots': props['total_slots'],
            'default_generation_context': default_context,
            'slots': [{key: slot[key] for key in ('id', 'n_ctx', 'is_processing')} for slot in slots]}


def counters(rig=DEFAULT_RIG):
    result = {}
    for line in get('/metrics', rig=rig).decode('utf-8').splitlines():
        fields = line.split()
        if len(fields) == 2 and fields[0] in COUNTER_NAMES:
            name, value = fields[0], float(fields[1])
            if name in result or not math.isfinite(value) or value < 0:
                raise GuardError('duplicate or invalid native counter')
            if name not in SECONDS and (not value.is_integer() or value > 2**53):
                raise GuardError('native count is fractional or loses integer precision')
            result[name] = value
    if set(result) != COUNTER_NAMES:
        raise GuardError('native counter observation is incomplete')
    if any(result[name] != 0 for name in GAUGES):
        raise GuardError('private engine has processing or deferred requests')
    if result['llamacpp:spec_decode_num_accepted_tokens_total'] > result['llamacpp:spec_decode_num_draft_tokens_total']:
        raise GuardError('native speculative counter observation is inconsistent')
    return result


def validate_gpu(sample):
    names = ('used_mib', 'total_mib', 'gpu_percent', 'temperature_c', 'power_w')
    if not isinstance(sample, dict) or set(sample) != set(names):
        raise GuardError('GPU observation must contain exactly five fields')
    if any(type(sample[name]) not in (int, float) or not math.isfinite(sample[name]) for name in names):
        raise GuardError('GPU observation contains non-finite values')
    if sample['total_mib'] <= 0 or not 0 <= sample['used_mib'] <= sample['total_mib'] or not 0 <= sample['gpu_percent'] <= 100 or sample['power_w'] < 0:
        raise GuardError('GPU observation is outside physical bounds')
    return sample


def gpu_sample(rig=DEFAULT_RIG):
    result = subprocess.run(['nvidia-smi', '--id=' + str(rig.gpu_index),
                             '--query-gpu=memory.used,memory.total,utilization.gpu,temperature.gpu,power.draw',
                             '--format=csv,noheader,nounits'], capture_output=True, text=True,
                            check=True, timeout=3)
    fields = result.stdout.strip().split(',')
    if len(fields) != 5 or '\n' in result.stdout.strip():
        raise GuardError('GPU observation has unexpected column or device count')
    return validate_gpu(dict(zip(('used_mib', 'total_mib', 'gpu_percent', 'temperature_c', 'power_w'),
                                (float(value.strip()) for value in fields))))


def vram_exceeded(sample):
    return sample['used_mib'] > .9 * sample['total_mib']


def terminate_private_engine(expected, slots, rig=DEFAULT_RIG):
    if engine_identity(expected['pid'], slots, rig) != expected:
        raise GuardError('refused termination of changed private engine')
    os.kill(expected['pid'], signal.SIGTERM)


def monitor_gpu(stop, abort, samples, errors, expected_engine, slots, rig=DEFAULT_RIG):
    while not stop.is_set():
        try:
            sample = gpu_sample(rig)
            exceeded = vram_exceeded(sample)
            sample['observed_at_monotonic'] = time.monotonic()
            samples.append(sample)
            if exceeded:
                abort.set()
                errors.append({'stage': 'gpu_monitor', 'reason': 'VRAM guard exceeded'})
                terminate_private_engine(expected_engine, slots, rig)
                stop.set()
        except Exception as exc:
            abort.set()
            errors.append({'stage': 'gpu_monitor', 'exception': type(exc).__name__})
            stop.set()
        stop.wait(1)


def write_result(path, result):
    data = json.dumps(result, indent=2, allow_nan=False)
    descriptor, temporary = tempfile.mkstemp(prefix=path.name + '.', suffix='.tmp', dir=path.parent)
    try:
        with os.fdopen(descriptor, 'w', encoding='utf-8') as stream:
            stream.write(data)
        os.replace(temporary, path)
    finally:
        if os.path.exists(temporary):
            os.unlink(temporary)


def validate_native_result(arm, result, before, after):
    if any(after[name] < before[name] for name in COUNTER_NAMES - GAUGES):
        raise GuardError('native counters reset')
    successes = [record for record in result['records'] if record['outcome'] == 'success']
    delta = after['llamacpp:tokens_predicted_total'] - before['llamacpp:tokens_predicted_total']
    if not successes or delta <= 0:
        raise GuardError('campaign has no verified successful native output')
    if any(record.get('quality_source') not in ('expected-text', 'expected-tool') for record in successes):
        raise GuardError('campaign success lacks an expected-output proof')
    authoritative = [record for record in successes if record.get('usage') is not None and record.get('usage_source', '').startswith('upstream-')]
    known = sum(record['usage']['completion_tokens'] for record in authoritative)
    if arm == 'candidate':
        if len(authoritative) != len(successes) or delta != known:
            raise GuardError('candidate native token delta differs from authoritative request usage')
        if result['summary']['wire_usage_mismatches']:
            raise GuardError('candidate public usage differs from upstream usage')
    elif known > delta:
        raise GuardError('baseline authoritative subset exceeds native token delta')
    return delta


def validate_manifest(arm, manifest, fixed, rig=DEFAULT_RIG):
    if manifest['arm'] != arm or manifest['gpu_indices'] != [rig.gpu_index]:
        raise GuardError('provenance arm or physical GPU differs from selected rig')
    if manifest['context_per_sequence'] != CONTEXT_PER_SEQUENCE:
        raise GuardError('provenance per-sequence context differs from 32768')
    if manifest['binary_sha256'] != fixed['power']['exe_sha256'] or manifest['runtime_sha256'] != fixed['engine']['exe_sha256']:
        raise GuardError('manifest differs from running process')


def run_one(arm, concurrency, repetition, seconds, directory, corpus, key, manifest, fixed,
            rig=DEFAULT_RIG, factors=('admission',)):
    target = directory / (arm + '-c' + str(concurrency) + '-r' + str(repetition) + '.json')
    if target.exists():
        raise GuardError('existing campaign result: use a new tag')
    slots = manifest['slots']
    result = {'schema_version': 1, 'tool_identity': dict(TOOL_IDENTITY), 'rig': rig.index,
              'experimental_factors': list(factors),
              'provenance': manifest, 'process_identity': fixed,
              'qualification_status': 'preflight', 'warmup_records': [], 'records': []}
    stage = 'preflight'
    try:
        if identity(arm, slots, rig) != fixed:
            raise GuardError('process identity changed before run')
        result['engine_before_warmup'] = engine_state(slots, rig)
        counters(rig)
        sample = gpu_sample(rig)
        if vram_exceeded(sample):
            raise GuardError('preflight VRAM guard exceeded')
        url = 'http://127.0.0.1:' + str(rig.gateway_ports[arm]) + '/v1'
        stage = 'warmup'
        for item in corpus:
            row = q.request_once(url, item['request'], key, 120, expected=item['expect'])
            result['warmup_records'].append(row)
            if row['outcome'] != 'success' or (arm == 'candidate' and row.get('wire_usage_mismatch')):
                raise GuardError('controlled warmup correctness or usage failure')
        result['engine_before'] = engine_state(slots, rig)
        before = counters(rig)
        result['native_counters_before'] = before
        sample = gpu_sample(rig)
        if vram_exceeded(sample):
            raise GuardError('pre-measurement VRAM guard exceeded')
        samples, errors = [dict(sample, observed_at_monotonic=time.monotonic())], []
        stop, abort = threading.Event(), threading.Event()
        monitor = threading.Thread(target=monitor_gpu, args=(stop, abort, samples, errors, fixed['engine'], slots, rig), daemon=True)
        stage = 'requests'
        monitor.start()
        try:
            campaign = q.run_campaign([url], corpus, key, q.Limits(seconds, 120, concurrency, 10000), abort_event=abort)
            result.update(campaign)
        finally:
            stop.set()
            monitor.join(timeout=5)
        result.update(gpu_samples=samples, monitor_errors=errors,
                      qualification_status='postchecks-pending',
                      observed_at_utc=datetime.datetime.now(datetime.timezone.utc).isoformat())
        # Persist completed client records before ANY fallible post-observation.
        write_result(target, result)
        stage = 'postchecks'
        if monitor.is_alive() or errors or abort.is_set():
            raise GuardError('resource monitor aborted dispatch or failed to terminate')
        if identity(arm, slots, rig) != fixed:
            raise GuardError('process identity changed during run')
        result['engine_after'] = engine_state(slots, rig)
        after = counters(rig)
        result['native_counters_after'] = after
        delta = validate_native_result(arm, result, before, after)
        summary = result['summary']
        unexpected = sum(count for outcome, count in summary['outcomes'].items() if outcome not in ('success', 'refused'))
        if unexpected or result['request_budget_exhausted']:
            raise GuardError('campaign correctness or request-budget gate failed')
        result.update(qualification_status='passed', upstream_idle_after_drain=True,
                      native_predicted_token_delta=delta,
                      native_output_tokens_per_second=delta / summary['wall_seconds_including_drain'],
                      native_counter_scope='validated exclusive private engine; after warmup through complete drain')
        write_result(target, result)
        print(json.dumps({'file': target.name, 'outcomes': summary['outcomes'],
                          'native_tok_s': result['native_output_tokens_per_second'],
                          'ttft': summary['ttft_ms'], 'elapsed': summary['elapsed_ms']}), flush=True)
        return result
    except Exception as exc:
        result.update(qualification_status='failed', failure_stage=stage,
                      failure_exception=type(exc).__name__)
        if isinstance(exc, GuardError):
            result['failure_reason'] = str(exc)
        write_result(target, result)
        raise


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--tag', required=True)
    parser.add_argument('--rig', type=int, choices=(7, 6, 5), default=7)
    parser.add_argument('--factor', choices=('slots', 'runtime_flags', 'admission'), nargs='+', default=['admission'])
    parser.add_argument('--seconds', type=float, default=60)
    parser.add_argument('--repeats', type=int, choices=(1, 2, 3), default=3)
    parser.add_argument('--concurrency', type=int, choices=(1, 8, 16, 32), nargs='+', default=[1, 8, 16, 32])
    parser.add_argument('--arms', choices=('baseline', 'candidate'), nargs='+', default=['baseline', 'candidate'])
    args = parser.parse_args()
    if not args.tag or len(args.tag) > 48 or any(character not in 'abcdefghijklmnopqrstuvwxyz0123456789-_' for character in args.tag):
        raise ValueError('bounded lowercase campaign tag required')
    if not 0 < args.seconds <= 60 or len(set(args.arms)) != len(args.arms) or len(set(args.concurrency)) != len(args.concurrency) or len(set(args.factor)) != len(args.factor):
        raise ValueError('invalid finite campaign matrix')
    rig = RIGS[args.rig]
    directory = rig.private / args.tag
    directory.mkdir(exist_ok=True)
    with (directory / 'campaign.lock').open('x', encoding='utf-8') as lock:
        lock.write(str(os.getpid()))
    corpus_bytes = read_limited(EVIDENCE / 'corpus.jsonl', 2 * MAX_OBSERVATION)
    corpus = [json.loads(line) for line in corpus_bytes.decode().splitlines() if line.strip()]
    if not corpus or len(corpus) > 256 or any(not isinstance(item, dict) or not isinstance(item.get('request'), dict) or not isinstance(item.get('expect'), dict) for item in corpus):
        raise GuardError('bounded synthetic corpus with expected output is required')
    key = read_limited(rig.key_path, 4096).decode().strip()
    manifests = {arm: q.load_provenance(rig.private / (arm + '-provenance.json'), corpus_bytes) for arm in args.arms}
    fixed = {arm: identity(arm, manifests[arm]['slots'], rig) for arm in args.arms}
    for arm in args.arms:
        validate_manifest(arm, manifests[arm], fixed[arm], rig)
    if len(args.arms) == 2:
        q.compare({'provenance': manifests['baseline'], 'limits': {}, 'summary': {'successful_output_tokens_per_second': None}},
                  {'provenance': manifests['candidate'], 'limits': {}, 'summary': {'successful_output_tokens_per_second': None}}, args.factor)
    for repetition in range(1, args.repeats + 1):
        for concurrency in args.concurrency:
            arms = args.arms if repetition % 2 else list(reversed(args.arms))
            for arm in arms:
                run_one(arm, concurrency, repetition, args.seconds, directory, corpus, key,
                        manifests[arm], fixed[arm], rig, args.factor)


if __name__ == '__main__':
    main()

