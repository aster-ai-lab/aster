#!/usr/bin/env python3
"""Exactly two controlled preparations for the frozen canary's warm-slot precondition."""
import concurrent.futures
import importlib.util
import json
from pathlib import Path
import subprocess
import sys
import threading
import time

HERE = Path(__file__).resolve().parent
spec = importlib.util.spec_from_file_location('frozen_slot_warmup_eight', HERE / 'eight-card-qualification.py')
e = importlib.util.module_from_spec(spec)
spec.loader.exec_module(e)
m, q = e.m, e.q
TARGET = m.PRIVATE / 'canary-v4-isolated-warm-preparation.json'


def inactive_production():
    def bus(args):
        value = subprocess.run(['busctl', '--json=short'] + args, capture_output=True, timeout=3)
        if value.returncode or len(value.stdout) > 65536:
            raise m.GuardError('bounded production isolation observation failed')
        return json.loads(value.stdout)
    obj = bus(['call', 'org.freedesktop.systemd1', '/org/freedesktop/systemd1',
               'org.freedesktop.systemd1.Manager', 'GetUnit', 's', 'a3s-power@7.service'])
    if obj.get('type') != 'o' or not isinstance(obj.get('data'), list) or len(obj['data']) != 1:
        raise m.GuardError('production isolation unit shape differs')
    unit = obj['data'][0]
    pid = bus(['get-property', 'org.freedesktop.systemd1', unit, 'org.freedesktop.systemd1.Service', 'MainPID'])
    active = bus(['get-property', 'org.freedesktop.systemd1', unit, 'org.freedesktop.systemd1.Unit', 'ActiveState'])
    if pid != {'type': 'u', 'data': 0} or active != {'type': 's', 'data': 'inactive'}:
        raise m.GuardError('actual production Power7 is not stopped')
    return {'unit': 'a3s-power@7.service', 'main_pid': 0, 'active_state': 'inactive'}


def safe_slots(raw):
    rows = json.loads(raw)
    if not isinstance(rows, list) or len(rows) != 2:
        raise m.GuardError('warm preparation requires actual two native slots')
    result = []
    for row in rows:
        if (not isinstance(row, dict) or type(row.get('id')) is not int
                or row.get('n_ctx') != 32768 or type(row.get('is_processing')) is not bool
                or row.get('speculative') is not False):
            raise m.GuardError('actual warm preparation slot geometry differs')
        value = {key: row[key] for key in ('id', 'n_ctx', 'is_processing')}
        if 'id_task' in row:
            if type(row['id_task']) is not int or row['id_task'] < 0:
                raise m.GuardError('observed native task identity is invalid')
            value['id_task'] = row['id_task']
        elif row['is_processing']:
            raise m.GuardError('processing native slot lacks task identity')
        result.append(value)
    if sorted(row['id'] for row in result) != [0, 1]:
        raise m.GuardError('native slot identities differ')
    return sorted(result, key=lambda row: row['id'])


def gauges():
    result = {}
    for line in m.get('/metrics', port=19507, timeout=.6).decode().splitlines():
        fields = line.split()
        if len(fields) == 2 and fields[0] in ('power_active_requests', 'power_requests_waiting', 'power_requests_running'):
            if fields[0] in result:
                raise m.GuardError('duplicate private Power gauge')
            value = float(fields[1])
            if not value.is_integer() or not 0 <= value <= 2**53:
                raise m.GuardError('invalid private Power gauge')
            result[fields[0]] = value
    if len(result) != 3:
        raise m.GuardError('private Power gauge is missing')
    return result


def valid_record(row):
    usage = q.valid_usage(row.get('usage'))
    return (row.get('outcome') == 'success' and row.get('http_status') == 200
            and usage is not None and usage['completion_tokens'] > 0
            and row.get('usage_source') == 'upstream-usage' and row.get('wire_usage') == usage
            and row.get('wire_usage_mismatch') is False and row.get('quality_source') == 'expected-text'
            and row.get('finish_observed') is True and row.get('terminal_observed') is True)


def self_test():
    fresh = [{'id': n, 'n_ctx': 32768, 'is_processing': False, 'speculative': False} for n in (0, 1)]
    assert all('id_task' not in row for row in safe_slots(json.dumps(fresh)))
    fresh[0]['is_processing'] = True
    try:
        safe_slots(json.dumps(fresh))
    except m.GuardError:
        pass
    else:
        raise AssertionError('busy slot without task identity accepted')
    usage = {'prompt_tokens': 1, 'completion_tokens': 2, 'total_tokens': 3}
    row = dict(outcome='success', http_status=200, usage=usage, usage_source='upstream-usage',
               wire_usage=usage, wire_usage_mismatch=False, quality_source='expected-text',
               finish_observed=True, terminal_observed=True)
    assert valid_record(row)
    assert not valid_record(dict(row, usage_source='endpoint-usage'))
    assert not valid_record(dict(row, terminal_observed=False))
    print(json.dumps({'self_test': 'passed', 'checks': 5}))


def main():
    if TARGET.exists():
        raise m.GuardError('existing warm preparation; no replacement or automatic retry')
    started = time.monotonic()
    result = {'schema': 'a3s.power.controlled-warm-slot-preparation.v1', 'status': 'preflight',
              'scope': 'preparation only; never a canary pass', 'max_requests': 2,
              'request_deadline_seconds': 15, 'preparation_budget_seconds': 30,
              'cleanup_budget_seconds': 15, 'tool_identity': dict(m.TOOL_IDENTITY,
                  eight_card_sha256=m.digest(HERE / 'eight-card-qualification.py'),
                  warm_preparation_sha256=m.digest(Path(__file__))), 'records': [], 'slot_observations': []}
    owned = e.OwnedRequests([19897])
    stop, abort, samples, errors = threading.Event(), threading.Event(), [], []
    watcher, futures = None, []
    watchdog = threading.Timer(30, owned.abort)
    watchdog.daemon = True
    watchdog.start()
    try:
        result['production_isolation_before'] = inactive_production()
        raw, corpus = e.load_corpus()
        manifests, fixed = e.read_workers([7], raw)
        if (manifests[7]['slots'] != 2
                or manifests[7]['binary_sha256'] != '75afe5de8d423547fdb9b9bb302b17ca9e168d6f6fe13a18d29fbce9492bf1c1'):
            raise m.GuardError('actual wrapper profile must have two slots')
        result['identity_before'] = e.worker_identity(7, manifests[7], fixed[7])
        result['provenance'] = manifests[7]
        result['before_slots'] = safe_slots(m.get('/slots', port=18107, timeout=.6))
        if any(row['is_processing'] for row in result['before_slots']) or any(gauges().values()):
            raise m.GuardError('warm preparation must start fully idle')
        result['before_native'] = e.native_counters(7)
        before_gpu = e.gpu_samples([7])
        if m.vram_exceeded(before_gpu[7]):
            raise m.GuardError('whole-GPU90percent guard exceeded before warm preparation')
        result['before_gpu'] = before_gpu
        item = next(row for row in corpus if row['name'] == 'decode-stream')
        key = m.read_limited(m.PRIVATE / 'benchmark.key', 4096).decode().strip()
        if not key or time.monotonic() - started >= 15:
            raise m.GuardError('warm preparation credential or dispatch budget unavailable')
        with owned.track():
            watcher = threading.Thread(target=e.monitor, args=(stop, abort, owned, [7], samples, errors), daemon=True)
            watcher.start()
            with concurrent.futures.ThreadPoolExecutor(max_workers=2) as pool:
                futures = [pool.submit(q.request_once, 'http://127.0.0.1:19897/v1', item['request'], key, 15,
                                       expected=item['expect']) for _ in range(2)]
                while not all(future.done() for future in futures):
                    if owned.stopped.is_set() or abort.is_set() or time.monotonic() - started >= 30:
                        raise m.GuardError('warm preparation deadline or GPU guard aborted requests')
                    result['slot_observations'].append({'at_monotonic': time.monotonic(),
                        'slots': safe_slots(m.get('/slots', port=18107, timeout=.6))})
                    time.sleep(.04)
                result['records'] = [future.result() for future in futures]
        deadline = time.monotonic() + 15
        while True:
            rows = safe_slots(m.get('/slots', port=18107, timeout=.6))
            current = gauges()
            if all(not row['is_processing'] for row in rows) and not any(current.values()):
                break
            if time.monotonic() >= deadline:
                raise m.GuardError('owned preparation requests or native slots did not drain')
            time.sleep(.04)
        result.update(after_slots=rows, after_power_gauges=current, after_native=e.native_counters(7))
        result['identity_after'] = e.worker_identity(7, manifests[7], fixed[7])
        result['production_isolation_after'] = inactive_production()
        result['both_slots_observed_busy'] = any(all(row['is_processing'] and type(row.get('id_task')) is int
                                                   for row in sample['slots']) for sample in result['slot_observations'])
        delta = result['after_native']['llamacpp:tokens_predicted_total'] - result['before_native']['llamacpp:tokens_predicted_total']
        result['native_completion_delta'] = delta
        if (len(result['records']) != 2 or not all(valid_record(row) for row in result['records'])
                or delta != sum(row['usage']['completion_tokens'] for row in result['records'])
                or any(result['after_native'][name] < result['before_native'][name] for name in m.COUNTER_NAMES - m.GAUGES)
                or not result['both_slots_observed_busy'] or any(type(row.get('id_task')) is not int for row in rows)):
            raise m.GuardError('warm precondition, exact output or authoritative native accounting failed')
        stop.set()
        watcher.join(timeout=4)
        if watcher.is_alive() or errors or abort.is_set():
            raise m.GuardError('whole-GPU monitor failed or did not terminate')
        result['status'] = 'prepared'
    except Exception as exc:
        result.update(status='failed', failure_exception=type(exc).__name__)
        if isinstance(exc, m.GuardError):
            result['failure_reason'] = str(exc)
        raise
    finally:
        watchdog.cancel()
        stop.set()
        owned.abort()
        if watcher is not None:
            watcher.join(timeout=4)
        if not result['records']:
            result['records'] = [future.result() for future in futures if future.done() and not future.cancelled()]
        cleanup_deadline = time.monotonic() + 15
        result['cleanup'] = {'own_power_and_native_idle': False}
        try:
            while True:
                rows = safe_slots(m.get('/slots', port=18107, timeout=.6))
                current = gauges()
                if all(not row['is_processing'] for row in rows) and not any(current.values()):
                    result['cleanup'] = {'own_power_and_native_idle': True, 'slots': rows,
                                         'private_power_gauges': current}
                    break
                if time.monotonic() >= cleanup_deadline:
                    break
                time.sleep(.04)
        except Exception as exc:
            result['cleanup']['failure_exception'] = type(exc).__name__
        if not result['cleanup']['own_power_and_native_idle']:
            result['status'] = 'failed'
        result.update(elapsed_seconds=time.monotonic() - started, gpu_samples=samples, monitor_errors=errors,
                      own_http_complete=all(future.done() for future in futures),
                      monitor_terminated=watcher is None or not watcher.is_alive())
        m.write_result(TARGET, result)
        print(json.dumps({'file': TARGET.name, 'status': result['status'], 'requests': len(result['records']),
                          'elapsed_seconds': result['elapsed_seconds']}), flush=True)
    if result['status'] != 'prepared':
        raise m.GuardError('controlled warm preparation cleanup failed')


if __name__ == '__main__':
    if sys.argv[1:] == ['--self-test']:
        self_test()
    elif sys.argv[1:]:
        raise SystemExit('fixed preparation takes no arguments')
    else:
        main()
