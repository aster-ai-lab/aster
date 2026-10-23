#!/usr/bin/env python3
"""Finite generic Gateway canary; production native is observed, never managed.

Preparation artifact: execution requires actual wrapper identity/provenance and
observable native slots/metrics. No service, model registration or signal API is
provided. All generated traffic uses the separate benchmark fixture identity.
"""
import argparse
import concurrent.futures
import copy
import http.client
import importlib.util
import json
import math
import socket
import sys
import threading
import time
import types
from pathlib import Path

HERE = Path(__file__).resolve().parent
SPEC = importlib.util.spec_from_file_location('canary_matrix_reference', HERE / 'matrix-campaign.py')
m = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(m)
q = m.q
POWER_GAUGES = frozenset({'power_active_requests', 'power_requests_waiting', 'power_requests_running'})
PORTS = {'native': 18107, 'power': 19507, 'gateway': 19897}
MAX_TRACE = 1800
MODEL_ALIAS = 'prism-27b'
CORPUS_SHA256 = '1791c2cc7f6496454c69c903f2c3a82b61240b0e46d6b9b690747a6e7b113256'


class Unsupported(m.GuardError):
    """Required evidence is unavailable; no inference may be dispatched."""


class Budget:
    def __init__(self, seconds=160):
        self.started = time.monotonic()
        self.deadline = self.started + seconds

    def left(self, maximum):
        remaining = min(maximum, self.deadline - time.monotonic())
        if remaining <= 0:
            raise m.GuardError('finite canary budget exhausted')
        return remaining


class OwnedRequests:
    """Abort only this helper's inference transports; readers close responses.

    A private function globals copy injects a connection factory into the frozen
    parser. No imported module/class is patched, including other running tools.
    Retaining the transport handles Connection:close responses whose connection
    has already relinquished its socket to HTTPResponse.
    """
    def __init__(self):
        self.lock = threading.Lock()
        self.sockets = {}
        self.children = []
        self.aborted = threading.Event()
        self.reason = None
        owner = self

        class Connection(http.client.HTTPConnection):
            def connect(self):
                if owner.aborted.is_set():
                    raise OSError('owned dispatch aborted')
                super().connect()
                with owner.lock:
                    owner.sockets[self] = self.sock
                    aborted = owner.aborted.is_set()
                if aborted:
                    owner._shutdown(self.sock)
                    raise OSError('owned dispatch aborted')

        self.connection = Connection
        namespace = dict(q.request_once.__globals__)
        namespace['http'] = types.SimpleNamespace(client=types.SimpleNamespace(
            HTTPConnection=Connection, HTTPException=http.client.HTTPException))
        self.request_once = types.FunctionType(q.request_once.__code__, namespace,
                                              q.request_once.__name__, q.request_once.__defaults__)
        self.request_once.__kwdefaults__ = q.request_once.__kwdefaults__

    @staticmethod
    def _shutdown(transport):
        if transport is not None:
            try:
                transport.shutdown(socket.SHUT_RDWR)
            except OSError:
                pass

    def abort(self, reason):
        with self.lock:
            self.reason = self.reason or reason
            self.aborted.set()
            transports = list(self.sockets.values())
            children = list(self.children)
        for transport in transports:
            self._shutdown(transport)
        for child in children:
            child.abort(reason)

    def child(self):
        with self.lock:
            self.require_dispatch()
            if len(self.children) >= 32:
                raise m.GuardError('bounded owned request scope count exhausted')
            child = OwnedRequests()
            self.children.append(child)
            return child

    def require_dispatch(self):
        if self.aborted.is_set():
            raise m.GuardError('owned request dispatch aborted')

    def rejection(self, port, body, key, timeout):
        """Retain only the closed expected error-code match, never messages."""
        self.require_dispatch()
        connection = self.connection('127.0.0.1', port, timeout=timeout)
        response, transport = None, [None]

        def expire():
            self._shutdown(connection.sock or transport[0])

        timer = threading.Timer(timeout, expire)
        timer.daemon = True
        timer.start()
        try:
            connection.connect()
            transport[0] = connection.sock
            connection.request('POST', '/v1/chat/completions', body=json.dumps(body).encode(),
                               headers={'Content-Type': 'application/json', 'Authorization': 'Bearer ' + key,
                                        'Connection': 'close'})
            response = connection.getresponse()
            raw = response.read(m.MAX_OBSERVATION + 1)
            if len(raw) > m.MAX_OBSERVATION:
                raise m.GuardError('bounded refusal response exceeded byte limit')
            value = json.loads(raw)
            error = value.get('error') if isinstance(value, dict) else None
            return {'http_status': response.status, 'body_bytes': len(raw),
                    'queue_full_code': isinstance(error, dict) and error.get('code') == 'admission_queue_full',
                    'rate_limit_type': isinstance(error, dict) and error.get('type') == 'rate_limit_error'}
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
                raise Unsupported('invalid native or Power metric observation')
            if name not in m.SECONDS and (not value.is_integer() or value > 2**53):
                raise Unsupported('invalid native or Power metric count')
            result[name] = value
    if set(result) != names:
        raise Unsupported('complete native and Power metrics are required')
    return result


def parse_slots(raw, count):
    slots = json.loads(raw)
    if not isinstance(slots, list) or len(slots) != count:
        raise Unsupported('actual native slot count differs from selected one or two')
    safe = []
    for slot in slots:
        if (not isinstance(slot, dict) or type(slot.get('id')) is not int
                or type(slot.get('id_task')) is not int or slot.get('n_ctx') != 32768
                or type(slot.get('is_processing')) is not bool):
            raise Unsupported('actual native slot geometry or task state unavailable')
        value = {key: slot[key] for key in ('id', 'id_task', 'n_ctx', 'is_processing')}
        total, processed = slot.get('n_prompt_tokens'), slot.get('n_prompt_tokens_processed')
        next_token = slot.get('next_token')
        decoded = next_token[0].get('n_decoded') if isinstance(next_token, list) and len(next_token) == 1 and isinstance(next_token[0], dict) else None
        valid = (type(total) is int and type(processed) is int and 0 <= processed <= total
                 and type(decoded) is int and decoded >= 0)
        value['phase'] = ('idle' if not slot['is_processing'] else
                          'prefill' if valid and processed < total and decoded == 0 else
                          'decode' if valid and decoded > 0 else 'unknown')
        safe.append(value)
    if sorted(slot['id'] for slot in safe) != list(range(count)):
        raise Unsupported('actual native slot identities differ from closed slot set')
    return sorted(safe, key=lambda slot: slot['id'])


def idle(sample):
    return (all(not slot['is_processing'] for slot in sample['slots'])
            and all(value == 0 for value in sample['power'].values())
            and all(sample['native'][name] == 0 for name in m.GAUGES))


def filled(sample, count):
    return (sum(slot['is_processing'] for slot in sample['slots']) == count
            and sample['power']['power_requests_running'] == count
            and sample['native']['llamacpp:requests_processing'] == count
            and sample['native']['llamacpp:requests_deferred'] == 0)


def task_vector(sample):
    return [(slot['id'], slot['id_task']) for slot in sample['slots']]


def never_dispatched(active, queued_release, drained, count):
    return (queued_release is not None and drained is not None
            and filled(queued_release, count) and idle(drained)
            and task_vector(active) == task_vector(queued_release) == task_vector(drained))


class Canary:
    def __init__(self, count, key, requests, budget, ports=None, observe_get=m.get):
        if type(count) is not int or count not in (1, 2):
            raise ValueError('canary slots must be one or two')
        self.count, self.key, self.requests, self.budget = count, key, requests, budget
        self.ports = dict(PORTS if ports is None else ports)
        self.get = observe_get
        self.trace = []
        self.futures = []
        self.pool = concurrent.futures.ThreadPoolExecutor(max_workers=12)
        self.url = 'http://127.0.0.1:' + str(self.ports['gateway']) + '/v1'
        self.short = {'model': MODEL_ALIAS, 'temperature': 0, 'seed': 42, 'max_tokens': 16,
                      'stream': True, 'stream_options': {'include_usage': True},
                      'messages': [{'role': 'user', 'content': 'Reply with exactly OK.'}]}

    def observe(self, cleanup=False):
        if len(self.trace) >= MAX_TRACE:
            raise m.GuardError('bounded canary observation count exhausted')
        timeout = .6 if cleanup else self.budget.left(.6)
        try:
            slots = parse_slots(self.get('/slots', port=self.ports['native'], timeout=timeout), self.count)
            native = parse_metrics(self.get('/metrics', port=self.ports['native'], timeout=timeout), m.COUNTER_NAMES)
            power = parse_metrics(self.get('/metrics', port=self.ports['power'], timeout=timeout), POWER_GAUGES)
        except Unsupported:
            raise
        except (m.GuardError, OSError, ValueError, TypeError, KeyError):
            raise Unsupported('native slots and complete metrics must be observable') from None
        sample = {'at_ms': 1000 * (time.monotonic() - self.budget.started),
                  'slots': slots, 'native': native, 'power': power}
        self.trace.append(sample)
        return sample

    def until(self, predicate, seconds):
        deadline = time.monotonic() + self.budget.left(seconds)
        while True:
            self.requests.require_dispatch()
            sample = self.observe()
            if predicate(sample):
                return sample
            if time.monotonic() >= deadline:
                return None
            time.sleep(min(.04, max(0, deadline - time.monotonic())))

    def submit(self, body, cancel_ms=None, deltas=None, expected=None, scope=None):
        self.requests.require_dispatch()
        if len(self.futures) >= 32:
            raise m.GuardError('finite canary request count exhausted')
        owner = self.requests if scope is None else scope
        future = self.pool.submit(owner.request_once, self.url, body, self.key,
                                  self.budget.left(10), cancel_ms, deltas, expected)
        self.futures.append(future)
        return future

    def collect(self, future):
        while not future.done():
            self.requests.require_dispatch()
            self.observe()
            time.sleep(self.budget.left(.04))
        return future.result()

    def blockers(self):
        body = copy.deepcopy(self.short)
        body['max_tokens'] = 1600
        body['messages'][0]['content'] = 'Print all integers 1 through 2048 inclusive, one per line, in order. Output no other text.'
        futures = [self.submit(body, 8000) for _ in range(self.count)]
        active = self.until(lambda s: filled(s, self.count), 4)
        if active is None:
            raise m.GuardError('every actual native slot must hold a benchmark blocker')
        return active, futures

    def recover(self):
        before = self.until(idle, 12)
        if before is None:
            raise m.GuardError('Power permits or native work did not drain')
        record = self.collect(self.submit(self.short, expected={'text': 'OK', 'normalize_whitespace': True}))
        after = self.until(idle, 12)
        usage = record.get('usage')
        valid = (record['outcome'] == 'success' and q.valid_usage(usage)
                 and record.get('usage_source', '').startswith('upstream-')
                 and record.get('wire_usage') == usage and not record.get('wire_usage_mismatch'))
        if not valid or after is None:
            raise m.GuardError('recovery expected output, authoritative usage or permit release failed')
        native_delta = after['native']['llamacpp:tokens_predicted_total'] - before['native']['llamacpp:tokens_predicted_total']
        if native_delta != usage['completion_tokens']:
            raise m.GuardError('recovery native output differs from authoritative usage; possible concurrent traffic')
        return {'probe': record, 'native_completion_delta': native_delta, 'all_gauges_zero': True}

    def run_case(self, name, row=None):
        start = len(self.trace)
        if not idle(self.observe()):
            raise m.GuardError('canary requires initially idle native and Power; concurrent work is not owned')
        if row is None:
            row = {'case': name, 'passed': False}
        if name in ('queued-cancel', 'overload'):
            active, blockers = self.blockers()
            if name == 'queued-cancel':
                queued = self.submit(self.short, 400)
                observed = self.until(lambda s: s['power']['power_requests_waiting'] == 1 and filled(s, self.count), .35)
                record = self.collect(queued)
                released = self.until(lambda s: s['power']['power_requests_waiting'] == 0 and filled(s, self.count), 1)
                row.update(request=record, checks={'queue_observed': observed is not None,
                           'cancelled_before_meaningful_output': record['outcome'] == 'cancelled' and record['meaningful_deltas'] == 0})
            else:
                queued = [self.submit(self.short, 800) for _ in range(8)]
                observed = self.until(lambda s: s['power']['power_requests_waiting'] == 8 and filled(s, self.count), .55)
                if observed is None:
                    raise m.GuardError('all eight bounded queue entries must be observed before overload probe')
                self.requests.require_dispatch()
                record = self.requests.rejection(self.ports['gateway'], self.short, self.key, self.budget.left(2))
                queue_records = [self.collect(future) for future in queued]
                released = self.until(lambda s: s['power']['power_requests_waiting'] == 0 and filled(s, self.count), 1)
                row.update(request=record, queued=queue_records,
                           checks={'full_queue_observed': True, 'overload_http_429': record['http_status'] == 429,
                                   'explicit_admission_queue_full': record['queue_full_code'] and record['rate_limit_type'],
                                   'all_queued_clients_cancelled': all(r['outcome'] == 'cancelled' and r['meaningful_deltas'] == 0 for r in queue_records)})
            row['blockers'] = [self.collect(future) for future in blockers]
            drained = self.until(idle, 12)
            row['checks'].update(all_blockers_cancelled=all(r['outcome'] == 'cancelled' for r in row['blockers']),
                                 queued_never_dispatched_all_slots=never_dispatched(active, released, drained, self.count),
                                 all_Power_permits_and_native_work_released=drained is not None)
        else:
            body = copy.deepcopy(self.short)
            if name == 'pre-first-meaningful-cancel':
                body['messages'][0]['content'] = 'Synthetic unique prefill ' + str(time.monotonic_ns()) + ': ' + ('alpha beta gamma delta ' * 2200) + '\nReply with exactly OK.'
                scope = self.requests.child()
                future = self.submit(body, scope=scope)
                prefill = self.until(lambda s: any(slot['is_processing'] and slot['phase'] == 'prefill' for slot in s['slots'])
                                      and s['power']['power_requests_running'] == 1, 5)
                if prefill is None:
                    scope.abort('prefill phase not observed')
                    raise m.GuardError('actual unfinished native prefill must be observed before controlled cancellation')
                if future.done():
                    scope.abort('request ended before prefill cancellation')
                    raise m.GuardError('request must remain pending at observed prefill cancellation')
                scope.abort('observed native prefill cancellation')
                row['cancel_trigger'] = {'kind': 'observed-native-prefill', 'at_ms': prefill['at_ms'],
                                         'tasks': task_vector(prefill)}
            elif name == 'midstream-cancel':
                body['max_tokens'] = 1600
                body['messages'][0]['content'] = 'Print all integers 1 through 2048 inclusive, one per line, in order. Output no other text.'
                future = self.submit(body, deltas=12)
            else:
                raise ValueError('unknown canary scenario')
            record = self.collect(future)
            if name == 'pre-first-meaningful-cancel' and record['outcome'] != 'success':
                # The frozen parser does not know this external socket control.
                # Retain its outcome and label only our explicitly issued cancel.
                record = dict(record, parser_outcome=record['outcome'], outcome='cancelled')
            drained = self.until(idle, 12)
            native_started = any(any(slot['is_processing'] for slot in s['slots']) for s in self.trace[start:])
            row.update(request=record, checks={'native_work_observed': native_started,
                       'client_cancelled': record['outcome'] == 'cancelled',
                       'all_Power_permits_and_native_work_released': drained is not None})
            row['checks']['meaningful_timing_contract'] = (record['meaningful_deltas'] == 0 if name == 'pre-first-meaningful-cancel' else record['meaningful_deltas'] >= 12)
        row['recovery'] = self.recover()
        row['checks']['recovery_passed'] = True
        row['passed'] = all(row['checks'].values())
        row['trace_range'] = [start, len(self.trace)]
        if not row['passed']:
            raise m.GuardError('canary scenario acceptance gate failed')
        return row

    def cleanup(self):
        self.requests.abort('helper cleanup')
        deadline = time.monotonic() + 12
        while any(not future.done() for future in self.futures) and time.monotonic() < deadline:
            time.sleep(.02)
        self.pool.shutdown(wait=False, cancel_futures=True)
        drained = None
        while time.monotonic() < deadline:
            try:
                drained = self.observe(cleanup=True)
                if idle(drained):
                    break
            except (m.GuardError, OSError):
                break
            time.sleep(.04)
        return {'own_requests_completed': all(f.done() for f in self.futures),
                'observed_all_native_and_Power_idle': drained is not None and idle(drained),
                'scope': 'owned HTTP sockets only; native is never signaled or stopped'}


def verify_identity(expected):
    if not isinstance(expected, dict) or set(expected) != set(PORTS):
        raise m.GuardError('actual three-process wrapper identity required')
    observed = {}
    for name, port in PORTS.items():
        record = expected[name]
        if not isinstance(record, dict) or record.get('port') != port:
            raise m.GuardError('wrapper identity endpoint mismatch')
        actual = m.process_identity(record.get('pid'), port)
        if any(record.get(key) != value for key, value in actual.items()):
            raise m.GuardError('wrapper process identity or listener changed')
        observed[name] = actual
    cuda = [field.split(b'=', 1)[1] for field in m.read_limited(
        m.PROC / str(observed['native']['pid']) / 'environ', 65536).split(b'\0')
        if field.startswith(b'CUDA_VISIBLE_DEVICES=')]
    if cuda != [b'7'] or expected['native'].get('cuda_visible_devices') != '7':
        raise m.GuardError('actual native CUDA placement differs from physical GPU7')
    return observed


def preflight(canary, provenance):
    if (provenance.get('gpu_indices') != [7] or provenance.get('slots') != canary.count
            or provenance.get('context_per_sequence') != 32768
            or provenance.get('admission') != {'active': canary.count, 'waiting_limit': 8, 'queue_timeout_ms': 1000}):
        raise m.GuardError('selected slot geometry or bounded admission provenance differs')
    sample = canary.observe()
    if not idle(sample):
        raise m.GuardError('selected native is not idle; production work must not be cancelled')
    return sample


def known_corpus():
    corpus = m.read_limited(HERE / 'eight-card-corpus.jsonl', 2 * m.MAX_OBSERVATION)
    if q.sha256(corpus) != CORPUS_SHA256:
        raise m.GuardError('known eight-card qualification corpus differs')
    if any(json.loads(line)['request']['model'] != MODEL_ALIAS for line in corpus.splitlines() if line.strip()):
        raise m.GuardError('known eight-card qualification model alias differs')
    return corpus


def monitor(stop, requests, samples, errors):
    while not stop.is_set():
        try:
            sample = m.gpu_sample()  # GPU7 observation only; never m.monitor_gpu.
            samples.append(sample)
            if m.vram_exceeded(sample):
                errors.append('whole GPU7 memory exceeded 90 percent')
                requests.abort('GPU memory guard')
                return
        except Exception:
            errors.append('GPU observation unavailable')
            requests.abort('GPU observation unavailable')
            return
        stop.wait(.5)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--tag', required=True)
    parser.add_argument('--slots', required=True, type=int, choices=(1, 2))
    parser.add_argument('--identity', required=True, type=Path)
    parser.add_argument('--provenance', required=True, type=Path)
    args = parser.parse_args()
    if not args.tag or len(args.tag) > 48 or any(c not in 'abcdefghijklmnopqrstuvwxyz0123456789-_' for c in args.tag):
        raise ValueError('bounded lowercase tag required')
    target = m.PRIVATE / ('canary-' + args.tag + '.json')
    if target.exists():
        raise m.GuardError('existing canary evidence; use a fresh tag')
    budget, requests, stop = Budget(), OwnedRequests(), threading.Event()
    canary = Canary(args.slots, None, requests, budget)
    result = {'schema': 'a3s.power.generic-production-canary.v1', 'status': 'preflight',
              'slots': args.slots, 'ports': PORTS, 'gpu_indices': [7], 'cases': [],
              'tool_identity': dict(m.TOOL_IDENTITY, canary_sha256=m.digest(Path(__file__).resolve())),
              'budget_seconds': 160, 'cleanup_seconds': 15,
              'not_claimed': ['managed Gateway frontdoor', 'GPU kernel cancellation acknowledgement',
                              'native ownership or production deployment authorization']}
    watcher = None
    watchdog = threading.Timer(160, requests.abort, args=('absolute canary budget',))
    watchdog.daemon = True
    watchdog.start()
    try:
        identity = json.loads(m.read_limited(args.identity, m.MAX_OBSERVATION))
        fixed = verify_identity(identity)
        provenance = q.load_provenance(args.provenance, known_corpus())
        if (provenance['binary_sha256'] != fixed['power']['exe_sha256']
                or provenance['runtime_sha256'] != fixed['native']['exe_sha256']):
            raise m.GuardError('actual canary binaries differ from provenance')
        result.update(identity=fixed, provenance=provenance)
        preflight(canary, provenance)  # Includes slots and metrics; before key read or inference.
        sample = m.gpu_sample()
        if m.vram_exceeded(sample):
            raise m.GuardError('GPU memory guard already exceeded before inference')
        result.update(gpu_samples=[sample], monitor_errors=[])
        canary.key = m.read_limited(m.PRIVATE / 'benchmark.key', 4096).decode().strip()
        if not canary.key:
            raise m.GuardError('fresh benchmark fixture key unavailable')
        watcher = threading.Thread(target=monitor, args=(stop, requests, result['gpu_samples'], result['monitor_errors']), daemon=True)
        watcher.start()
        result['warmup'] = canary.recover()
        for name in ('queued-cancel', 'pre-first-meaningful-cancel', 'midstream-cancel', 'overload'):
            row = {'case': name, 'passed': False}
            result['cases'].append(row)
            canary.run_case(name, row)
            m.write_result(target, result)
            if verify_identity(identity) != fixed:
                raise m.GuardError('canary process identity changed between scenarios')
        if result['monitor_errors']:
            raise m.GuardError('resource monitor stopped owned requests')
        result['status'] = 'passed'
    except Unsupported as exc:
        result.update(status='unsupported', reason=str(exc))
    except Exception as exc:
        result.update(status='failed', exception=type(exc).__name__)
        if isinstance(exc, m.GuardError):
            result['reason'] = str(exc)
    finally:
        stop.set()
        watchdog.cancel()
        result['cleanup'] = canary.cleanup()
        if watcher is not None:
            watcher.join(timeout=2)
        result.update(trace=canary.trace, elapsed_ms=1000 * (time.monotonic() - budget.started),
                      monitor_terminated=watcher is None or not watcher.is_alive(),
                      completed_request_records=[future.result() for future in canary.futures if future.done() and not future.cancelled() and future.exception() is None])
        if result['status'] == 'passed' and (not all(value is True for key, value in result['cleanup'].items() if key != 'scope') or result['elapsed_ms'] > 180000 or not result['monitor_terminated']):
            result['status'] = 'failed'
            result['reason'] = 'bounded cleanup or absolute duration gate failed'
        m.write_result(target, result)
        print(json.dumps({'file': target.name, 'status': result['status'], 'cases': len(result['cases'])}), flush=True)
    return 0 if result['status'] == 'passed' else 1


if __name__ == '__main__':
    sys.exit(main())
