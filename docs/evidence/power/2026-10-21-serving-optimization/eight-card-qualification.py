#!/usr/bin/env python3
"""Candidate-only qualification of explicit benchmark wrappers and real natives.

Importing this module performs no network, process control, or model-file reads.
The production native processes are observed and are never stopped by this tool.
"""
import argparse
import contextlib
import copy
import http.client
import importlib.util
import json
import math
from pathlib import Path
import socket
import subprocess
import threading
import time

HERE = Path(__file__).resolve().parent
spec = importlib.util.spec_from_file_location('power_eight_matrix', HERE / 'matrix-campaign.py')
m = importlib.util.module_from_spec(spec)
spec.loader.exec_module(m)
q = m.q
PRIVATE = m.PRIVATE / 'eight-replica'
ALIAS = 'prism-27b'
TOOL_IDENTITY = dict(m.TOOL_IDENTITY, eight_card_sha256=m.digest(Path(__file__).resolve()))


class Budget:
    def __init__(self, seconds=5370):
        self.deadline = time.monotonic() + seconds

    def left(self, maximum):
        result = min(maximum, self.deadline - time.monotonic())
        if result <= 0:
            raise m.GuardError('finite eight-card qualification budget exhausted')
        return result


def ports(gpu):
    if type(gpu) is not int or gpu not in range(8):
        raise m.GuardError('physical GPU is outside the closed eight-card set')
    return {'native': 18100 + gpu, 'power': 19500 + gpu, 'gateway': 19890 + gpu}


def load_corpus():
    original = [json.loads(line) for line in m.read_limited(HERE / 'corpus.jsonl', 2 * m.MAX_OBSERVATION).decode().splitlines() if line.strip()]
    expected = copy.deepcopy(original)
    for row in expected:
        row['request']['model'] = ALIAS
    raw = m.read_limited(HERE / 'eight-card-corpus.jsonl', 2 * m.MAX_OBSERVATION)
    actual = [json.loads(line) for line in raw.decode().splitlines() if line.strip()]
    if actual != expected:
        raise m.GuardError('eight-card corpus must differ only by the known production model alias')
    return raw, actual


def balanced_schedule(corpus, devices):
    # q uses the same global index for URL and corpus. Repeating each case once
    # per endpoint gives every selected GPU every case, including at c1.
    return [item for item in corpus for _ in devices]


def read_workers(devices, corpus_bytes):
    manifests, fixed = {}, {}
    for gpu in devices:
        prov = q.load_provenance(PRIVATE / ('worker-' + str(gpu) + '-provenance.json'), corpus_bytes)
        if prov['arm'] != 'candidate' or prov['gpu_indices'] != [gpu] or prov['context_per_sequence'] != 32768:
            raise m.GuardError('worker provenance differs from selected GPU or context')
        identity = json.loads(m.read_limited(PRIVATE / ('worker-' + str(gpu) + '-identity.json'), 65536))
        if not isinstance(identity, dict) or set(identity) != {'native', 'power', 'gateway'}:
            raise m.GuardError('worker identity receipt has an unknown shape')
        manifests[gpu], fixed[gpu] = prov, identity
    reference = manifests[devices[0]]
    common = ('source_head', 'source_diff_sha256', 'binary_sha256', 'model_sha256', 'runtime_sha256',
              'context_per_sequence', 'sampling', 'cache_mode', 'slots', 'tokenizer_sha256',
              'template_sha256', 'profile', 'build_features', 'warmup_requests', 'speculation',
              'admission', 'numa_policy', 'cpu_affinity')
    reference_flags = {k: v for k, v in reference['runtime_flags'].items() if k not in ('--port', '--main-gpu')}
    for prov in manifests.values():
        if any(prov.get(field) != reference.get(field) for field in common):
            raise m.GuardError('selected workers have incompatible candidate profiles')
        if {k: v for k, v in prov['runtime_flags'].items() if k not in ('--port', '--main-gpu')} != reference_flags:
            raise m.GuardError('selected workers have undeclared runtime flag differences')
    return manifests, fixed


def native_profile(command, environment, gpu, manifest):
    required = ((('--port',), str(ports(gpu)['native'])), (('--host',), '127.0.0.1'),
                (('-m', '--model'), m.MODEL_PATH), (('--alias',), ALIAS),
                (('--parallel', '-np'), str(manifest['slots'])),
                (('-c', '--ctx-size'), str(32768 * manifest['slots'])),
                (('-ngl', '--n-gpu-layers'), '99'), (('-fa', '--flash-attn'), 'on'),
                (('-t', '--threads'), '8'), (('--reasoning',), 'off'))
    if any(m.command_flag(command, aliases) != value for aliases, value in required):
        raise m.GuardError('production native command differs from selected candidate profile')
    for flag in ('--jinja', '--cache-prompt', '--metrics', '--slots'):
        if command.count(flag) != 1:
            raise m.GuardError('required native capability flag is missing or ambiguous')
    held = {}
    for index, token in enumerate(command[1:], 1):
        if token in q.RUNTIME_FLAG_NAMES:
            if token in held:
                raise m.GuardError('native runtime flag is duplicated')
            held[token] = command[index + 1] if index + 1 < len(command) and not command[index + 1].startswith('-') else True
    if held != manifest['runtime_flags']:
        raise m.GuardError('actual native runtime flags differ from observed provenance')
    if any(flag.startswith('--spec-') and (flag != '--spec-type' or value != 'none') for flag, value in held.items()):
        raise m.GuardError('candidate profile requires disabled native speculation')
    binding = [item.split(b'=', 1)[1] for item in environment if item.startswith(b'CUDA_VISIBLE_DEVICES=')]
    if binding != [str(gpu).encode()]:
        raise m.GuardError('production native CUDA binding differs from physical GPU')
    controls = (b'GGML_CUDA_DISABLE_GRAPHS=', b'GGML_CUDA_GRAPH_OPT=',
                b'GGML_CUDA_PTQ1_0_MMQ_MAX_BATCH=', b'LLAMA_ARG_SPEC_')
    if any(item.startswith(controls) for item in environment):
        raise m.GuardError('native CUDA or speculation environment differs from default profile')


def worker_identity(gpu, manifest, expected):
    result = {}
    for kind, port in ports(gpu).items():
        snapshot = expected[kind]
        if not isinstance(snapshot, dict) or snapshot.get('port') != port:
            raise m.GuardError('worker receipt port differs from closed port assignment')
        actual = m.process_identity(snapshot.get('pid'), port)
        if kind == 'native':
            pid = actual['pid']
            command = m.read_limited(m.PROC / str(pid) / 'cmdline', 65536).decode().rstrip('\0').split('\0')
            environment = m.read_limited(m.PROC / str(pid) / 'environ', 65536).split(b'\0')
            native_profile(command, environment, gpu, manifest)
            actual['cuda_visible_devices'] = str(gpu)
            if actual['exe_sha256'] != manifest['runtime_sha256']:
                raise m.GuardError('native binary differs from pinned worker provenance')
        elif kind == 'power' and actual['exe_sha256'] != manifest['binary_sha256']:
            raise m.GuardError('Power binary differs from pinned worker provenance')
        if actual != snapshot:
            raise m.GuardError('actual worker listener or process identity changed')
        result[kind] = actual
    return result


def observation_timeout(deadline):
    if deadline is None:
        return 3
    remaining = min(3, deadline - time.monotonic())
    if remaining <= 0:
        raise m.GuardError('absolute aggregate observation deadline exceeded')
    return remaining


def native_state(gpu, slots, deadline=None):
    port = ports(gpu)['native']
    props = json.loads(m.get('/props', port=port, timeout=observation_timeout(deadline)))
    observed = json.loads(m.get('/slots', port=port, timeout=observation_timeout(deadline)))
    if not isinstance(props, dict) or props.get('model_path') != m.MODEL_PATH or props.get('model_alias') != ALIAS or type(props.get('total_slots')) is not int or props['total_slots'] != slots:
        raise m.GuardError('actual native model or slot count differs')
    if not isinstance(observed, list) or len(observed) != slots or not observed:
        raise m.GuardError('actual native slots endpoint is absent or incomplete')
    identifiers = set()
    for slot in observed:
        if not isinstance(slot, dict) or type(slot.get('id')) is not int or slot['id'] in identifiers or type(slot.get('n_ctx')) is not int or slot['n_ctx'] != 32768 or type(slot.get('is_processing')) is not bool or slot['is_processing'] or slot.get('speculative') is not False:
            raise m.GuardError('native slot context or idle observation is invalid')
        identifiers.add(slot['id'])
    if identifiers != set(range(slots)):
        raise m.GuardError('native slot identity differs from selected profile')
    return {'model_alias': ALIAS, 'model_path': m.MODEL_PATH, 'total_slots': slots,
            'slots': [{key: row[key] for key in ('id', 'n_ctx', 'is_processing')} for row in observed]}


def native_counters(gpu, deadline=None):
    result = {}
    for line in m.get('/metrics', port=ports(gpu)['native'], timeout=observation_timeout(deadline)).decode().splitlines():
        fields = line.split()
        if len(fields) != 2 or fields[0] not in m.COUNTER_NAMES:
            continue
        name, value = fields[0], float(fields[1])
        if name in result or not math.isfinite(value) or value < 0 or (name not in m.SECONDS and (not value.is_integer() or value > 2**53)):
            raise m.GuardError('invalid native counter observation')
        result[name] = value
    if set(result) != m.COUNTER_NAMES or any(result[name] != 0 for name in m.GAUGES):
        raise m.GuardError('native metrics are missing or contain external active work')
    if result['llamacpp:spec_decode_num_accepted_tokens_total'] > result['llamacpp:spec_decode_num_draft_tokens_total']:
        raise m.GuardError('native speculative counters are inconsistent')
    return result


def gpu_samples(devices):
    proc = subprocess.run(['nvidia-smi', '--id=' + ','.join(map(str, devices)),
                           '--query-gpu=index,memory.used,memory.total,utilization.gpu,temperature.gpu,power.draw',
                           '--format=csv,noheader,nounits'], capture_output=True, text=True, check=True, timeout=3)
    result = {}
    for line in proc.stdout.strip().splitlines():
        columns = [value.strip() for value in line.split(',')]
        if len(columns) != 6:
            raise m.GuardError('GPU observation has an unexpected shape')
        gpu = int(columns[0])
        if gpu not in devices or gpu in result:
            raise m.GuardError('GPU observation differs from selected physical devices')
        result[gpu] = m.validate_gpu(dict(zip(('used_mib', 'total_mib', 'gpu_percent', 'temperature_c', 'power_w'), map(float, columns[1:]))))
    if set(result) != set(devices):
        raise m.GuardError('GPU observation is incomplete')
    return result


class OwnedRequests:
    """Cancel only this process's sockets to explicit private Gateway ports."""
    def __init__(self, gateway_ports):
        self.ports, self.sockets, self.lock = set(gateway_ports), {}, threading.Lock()
        self.stopped = threading.Event()

    def abort(self):
        self.stopped.set()
        with self.lock:
            sockets = list(self.sockets.values())
        for transport in sockets:
            try:
                transport.shutdown(socket.SHUT_RDWR)
            except OSError:
                pass

    @contextlib.contextmanager
    def track(self):
        original, owner = http.client.HTTPConnection, self

        class Tracked(original):
            def connect(self):
                if self.port in owner.ports and owner.stopped.is_set():
                    raise ConnectionAbortedError()
                super().connect()
                if self.port in owner.ports:
                    with owner.lock:
                        owner.sockets[self] = self.sock
                    if owner.stopped.is_set():
                        owner.abort()

            def getresponse(self):
                self._reading_headers = True
                try:
                    return super().getresponse()
                finally:
                    self._reading_headers = False

            def close(self):
                # HTTPConnection closes its socket object at headers for a
                # Connection: close response. Preserve that exact socket for
                # guard shutdown while the response owns its makefile. The
                # request reader closes both after reading or cancellation.
                if getattr(self, '_reading_headers', False) and self in owner.sockets:
                    self.sock = None
                super().close()
                if not getattr(self, '_reading_headers', False):
                    with owner.lock:
                        transport = owner.sockets.pop(self, None)
                    if transport is not None:
                        transport.close()

        http.client.HTTPConnection = Tracked
        try:
            yield self
        finally:
            self.abort()
            http.client.HTTPConnection = original


def monitor(stop, abort, owned, devices, samples, errors):
    while not stop.is_set():
        try:
            current = gpu_samples(devices)
            samples.append({'at_monotonic': time.monotonic(), 'devices': current})
            if any(m.vram_exceeded(sample) for sample in current.values()):
                raise m.GuardError('whole-GPU VRAM guard exceeded')
        except Exception as exc:
            errors.append({'exception': type(exc).__name__})
            abort.set()
            owned.abort()
            stop.set()
        stop.wait(1)


def validate_accounting(records, before, after, devices):
    totals = {}
    for index, gpu in enumerate(devices):
        if any(after[gpu][name] < before[gpu][name] for name in m.COUNTER_NAMES - m.GAUGES):
            raise m.GuardError('native counters reset during qualification')
        successes = [record for record in records if record['endpoint_index'] == index and record['outcome'] == 'success']
        if not successes or any(record.get('quality_source') not in ('expected-text', 'expected-tool') or not record.get('usage_source', '').startswith('upstream-') or record.get('usage') is None or record.get('wire_usage') is None or record.get('wire_usage_mismatch') for record in successes):
            raise m.GuardError('candidate request lacks output or authoritative usage proof')
        known = sum(record['usage']['completion_tokens'] for record in successes)
        delta = after[gpu]['llamacpp:tokens_predicted_total'] - before[gpu]['llamacpp:tokens_predicted_total']
        if known <= 0 or delta != known:
            raise m.GuardError('native token delta differs: external work or usage confound')
        totals[gpu] = delta
    return totals


def power_state(gpu, manifest, deadline=None):
    health = json.loads(m.get('/health', port=ports(gpu)['power'], timeout=observation_timeout(deadline)))
    if not isinstance(health, dict) or health.get('status') != 'ok':
        raise m.GuardError('private candidate Power is not healthy')
    parallel = health.get('inference', {}).get('num_parallel')
    admission = health.get('worker', {}).get('admission', {})
    expected = manifest.get('admission', {})
    if type(parallel) is not int or parallel != manifest['slots'] or type(admission.get('active_limit')) is not int or admission['active_limit'] != expected.get('active'):
        raise m.GuardError('actual Power parallel or active limit differs from candidate provenance')
    if health.get('speculative', {}).get('mode') != 'off' or type(health.get('loaded_models')) is not int or health['loaded_models'] < 1:
        raise m.GuardError('private Power model registration or speculative mode differs')
    if any(type(admission.get(name)) is not int or admission[name] != 0 for name in ('active', 'waiting')):
        raise m.GuardError('actual Power admission did not recover')
    return {'num_parallel': parallel, 'admission': {name: admission[name] for name in ('active_limit', 'active', 'waiting')},
            'spec_mode': 'off', 'loaded_models': health['loaded_models']}


def observe_all(devices, manifests, deadline=None):
    states, counters, power, health = {}, {}, {}, {}
    for gpu in devices:
        states[gpu] = native_state(gpu, manifests[gpu]['slots'], deadline)
        counters[gpu] = native_counters(gpu, deadline)
        health[gpu] = power_state(gpu, manifests[gpu], deadline)
        gauges = {}
        names = {'power_active_requests', 'power_requests_waiting', 'power_requests_running'}
        for line in m.get('/metrics', port=ports(gpu)['power'], timeout=observation_timeout(deadline)).decode().splitlines():
            fields = line.split()
            if len(fields) == 2 and fields[0] in names:
                value = float(fields[1])
                if fields[0] in gauges or not math.isfinite(value) or value != 0:
                    raise m.GuardError('private Power request or permit did not recover')
                gauges[fields[0]] = value
        if set(gauges) != names:
            raise m.GuardError('private Power gauges are incomplete')
        power[gpu] = gauges
    return {'native_states': states, 'native_counters': counters, 'power_gauges': power, 'power_health': health}


def run_one(devices, manifests, fixed, corpus, key, directory, concurrency, repetition, seconds, budget):
    target = directory / ('candidate-c' + str(concurrency) + '-r' + str(repetition) + '.json')
    if target.exists():
        raise m.GuardError('existing result: qualification never retries or replaces a run')
    result = {'schema_version': 1, 'scope': 'candidate absolute capacity; no eight-card baseline ratio',
              'selected_gpus': devices, 'tool_identity': TOOL_IDENTITY, 'provenance': manifests,
              'process_identity': fixed, 'warmup_records': [], 'records': [], 'qualification_status': 'preflight',
              'schedule': 'each corpus case covers every selected endpoint before the next case'}
    stop, abort, samples, errors = threading.Event(), threading.Event(), [], []
    monitor_thread = None
    owned = OwnedRequests(ports(gpu)['gateway'] for gpu in devices)
    try:
        for gpu in devices:
            worker_identity(gpu, manifests[gpu], fixed[gpu])
        result['before_warmup'] = observe_all(devices, manifests, time.monotonic() + 15)
        current = gpu_samples(devices)
        if any(m.vram_exceeded(sample) for sample in current.values()):
            raise m.GuardError('whole-GPU preflight VRAM guard exceeded')
        samples.append({'at_monotonic': time.monotonic(), 'devices': current})
        with owned.track():
            monitor_thread = threading.Thread(target=monitor, args=(stop, abort, owned, devices, samples, errors), daemon=True)
            monitor_thread.start()
            for endpoint_index, gpu in enumerate(devices):
                for item in corpus:
                    if abort.is_set():
                        raise m.GuardError('resource monitor aborted qualification')
                    record = q.request_once('http://127.0.0.1:' + str(ports(gpu)['gateway']) + '/v1',
                                            item['request'], key, budget.left(120), expected=item['expect'])
                    record.update(endpoint_index=endpoint_index, case=item['name'])
                    result['warmup_records'].append(record)
                    m.write_result(target, result)
                    if record['outcome'] != 'success':
                        raise m.GuardError('controlled warmup output or transport failed')
            result['before_measurement'] = observe_all(devices, manifests, time.monotonic() + 15)
            validate_accounting(result['warmup_records'], result['before_warmup']['native_counters'],
                                result['before_measurement']['native_counters'], devices)
            if abort.is_set() or budget.left(seconds + 135) < seconds + 135:
                raise m.GuardError('insufficient remaining budget for a complete fixed campaign')
            urls = ['http://127.0.0.1:' + str(ports(gpu)['gateway']) + '/v1' for gpu in devices]
            campaign = q.run_campaign(urls, balanced_schedule(corpus, devices), key,
                                       q.Limits(seconds, 120, concurrency, 10000), abort_event=abort)
            result.update(campaign)
            result['qualification_status'] = 'postchecks-pending'
            # Client evidence survives every subsequent identity/metrics failure.
            m.write_result(target, result)
            post_deadline = time.monotonic() + 15
            stop.set()
            monitor_thread.join(timeout=4)
            if monitor_thread.is_alive() or errors or abort.is_set():
                raise m.GuardError('resource monitoring aborted or failed')
            for gpu in devices:
                worker_identity(gpu, manifests[gpu], fixed[gpu])
            result['after_measurement'] = observe_all(devices, manifests, post_deadline)
            totals = validate_accounting(result['records'], result['before_measurement']['native_counters'],
                                         result['after_measurement']['native_counters'], devices)
            unexpected = sum(count for outcome, count in result['summary']['outcomes'].items() if outcome not in ('success', 'refused'))
            if unexpected or result['request_budget_exhausted']:
                raise m.GuardError('output, transport, or finite request-budget gate failed')
            result.update(qualification_status='passed', native_predicted_token_delta_by_gpu=totals,
                          native_output_tokens_per_second=sum(totals.values()) / result['summary']['wall_seconds_including_drain'])
    except Exception as exc:
        result.update(qualification_status='failed', failure_exception=type(exc).__name__)
        if isinstance(exc, m.GuardError):
            result['failure_reason'] = str(exc)
        raise
    finally:
        stop.set()
        owned.abort()
        if monitor_thread is not None:
            monitor_thread.join(timeout=4)
        result.update(gpu_samples=samples, monitor_errors=errors,
                      monitor_terminated=monitor_thread is None or not monitor_thread.is_alive(),
                      native_process_control='never performed; guard only closes helper-owned private Gateway sockets')
        m.write_result(target, result)
    print(json.dumps({'file': target.name, 'status': result['qualification_status'], 'outcomes': result['summary']['outcomes']}), flush=True)
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--tag', required=True)
    parser.add_argument('--gpu', type=int, choices=range(8), nargs='+', default=list(range(8)))
    parser.add_argument('--seconds', type=float, default=60)
    parser.add_argument('--repeats', type=int, choices=(1, 2, 3), default=3)
    parser.add_argument('--concurrency', type=int, choices=(1, 8, 16, 32), nargs='+', default=[1, 8, 16, 32])
    args = parser.parse_args()
    if not args.tag or len(args.tag) > 48 or any(c not in 'abcdefghijklmnopqrstuvwxyz0123456789-_' for c in args.tag):
        raise ValueError('bounded lowercase tag required')
    if len(set(args.gpu)) != len(args.gpu) or len(set(args.concurrency)) != len(args.concurrency) or not 0 < args.seconds <= 60:
        raise ValueError('invalid fixed qualification matrix')
    raw, corpus = load_corpus()
    devices = sorted(args.gpu)
    manifests, fixed = read_workers(devices, raw)
    directory = PRIVATE / args.tag
    directory.mkdir(exist_ok=False)
    key = m.read_limited(m.PRIVATE / 'benchmark.key', 4096).decode().strip()
    budget = Budget()
    for repetition in range(1, args.repeats + 1):
        for concurrency in args.concurrency:
            run_one(devices, manifests, fixed, corpus, key, directory, concurrency, repetition, args.seconds, budget)


if __name__ == '__main__':
    main()
