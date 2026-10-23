#!/usr/bin/env python3
"""Own verified API goodput on shared production natives; never exclusive capacity.

No service/native process control. Private wrappers form a second admission pool.
Global native counters cannot attribute work or prove shared cancellation.
"""
import argparse
import concurrent.futures
import contextlib
import hashlib
import http.client
import importlib.util
import json
import math
import os
from pathlib import Path
import subprocess
import threading
import time

HERE = Path(__file__).resolve().parent
spec = importlib.util.spec_from_file_location('frozen_eight_live_library', HERE / 'eight-card-qualification.py')
e = importlib.util.module_from_spec(spec)
spec.loader.exec_module(e)
m, q = e.m, e.q
POWER_SHA = '75afe5de8d423547fdb9b9bb302b17ca9e168d6f6fe13a18d29fbce9492bf1c1'
GATEWAY_SHA = 'eaf9f4ca8baf38074d1548badebbd3b42bd08ab5174e2f9136101ccbcfb6c1fd'
PINS = {'runner_sha256': '576b2d13811482024522e790f901647ac79dc6cc4368a2622a71adef856fe546',
        'matrix_sha256': 'b1e24fd648da9b7ae31cb3d3ace44c4b9b91dc6c793a0d9de7ac114ba1b821e4',
        'canary_sha256': '1960de0dd2c8dbec7338f808ddbd207972fb63844fa022607bb676f1069c459e',
        'eight_card_sha256': '9385bf26ec9dd6ea971612356214ff2691e319318a87bbddaa3eaded5996675e'}
GAUGES = frozenset(('power_active_requests', 'power_requests_waiting', 'power_requests_running'))
REFUSALS = frozenset(('admission_queue_full', 'admission_queue_timeout'))
SCOPE = 'own verified private-wrapper API goodput on eight shared production natives'


class NotDrained(m.GuardError):
    pass


def current_tools(expected=None):
    actual = {'runner_sha256': m.digest(Path(q.__file__)), 'matrix_sha256': m.digest(Path(m.__file__)),
              'canary_sha256': m.digest(HERE / 'canary-scenarios.py'),
              'eight_card_sha256': m.digest(HERE / 'eight-card-qualification.py')}
    if actual != PINS:
        raise m.GuardError('frozen dependency hash differs')
    result = dict(actual, live_qualification_sha256=m.digest(Path(__file__)),
                  live_fixtures_sha256=m.digest(HERE.parents[3] / 'tests/power_serving_live_eight_card.py'))
    if expected is not None and result != expected:
        raise m.GuardError('qualification source or fixtures changed since preflight')
    return result


def parse_native_metrics(raw):
    values = {}
    for line in raw.decode().splitlines():
        fields = line.split()
        if len(fields) != 2 or fields[0] not in m.COUNTER_NAMES:
            continue
        name, value = fields[0], float(fields[1])
        if (name in values or not math.isfinite(value) or value < 0
                or (name not in m.SECONDS and (not value.is_integer() or value > 2**53))):
            raise m.GuardError('native metric is invalid')
        values[name] = value
    return validate_counters(values)


def validate_counters(values):
    if set(values) != m.COUNTER_NAMES or any(type(value) not in (int, float) or not math.isfinite(value) or value < 0
            or (name not in m.SECONDS and (value != int(value) or value > 2**53)) for name, value in values.items()):
        raise m.GuardError('native metric set is incomplete')
    if values['llamacpp:spec_decode_num_accepted_tokens_total'] > values['llamacpp:spec_decode_num_draft_tokens_total']:
        raise m.GuardError('native speculative counters are inconsistent')
    return values


def monotonic(before, after):
    validate_counters(before)
    validate_counters(after)
    for name in m.COUNTER_NAMES - m.GAUGES:
        if after[name] < before[name]:
            raise m.GuardError('native counter reset or regressed')


def native_state(gpu, slots, deadline):
    props = json.loads(m.get('/props', port=e.ports(gpu)['native'], timeout=e.observation_timeout(deadline)))
    rows = json.loads(m.get('/slots', port=e.ports(gpu)['native'], timeout=e.observation_timeout(deadline)))
    if (not isinstance(props, dict) or props.get('model_path') != m.MODEL_PATH
            or props.get('model_alias') != e.ALIAS or type(props.get('total_slots')) is not int
            or props['total_slots'] != slots or not isinstance(rows, list) or len(rows) != slots):
        raise m.GuardError('shared native model or slot count differs')
    ids = []
    tasks = []
    for row in rows:
        if (not isinstance(row, dict) or type(row.get('id')) is not int
                or type(row.get('n_ctx')) is not int or row['n_ctx'] != 32768
                or type(row.get('is_processing')) is not bool or row.get('speculative') is not False):
            raise m.GuardError('shared native slot geometry differs')
        # Pinned b10787-73bfe6c server-context.cpp:600–630 adds task metadata
        # only for task/task_prev. A fresh idle slot legitimately has no id_task.
        task = row.get('id_task')
        if (row['is_processing'] and type(task) is not int) or ('id_task' in row and type(task) is not int):
            raise m.GuardError('busy native task identity is unavailable or malformed')
        ids.append(row['id'])
        tasks.append({'id': row['id'], 'id_task': task, 'is_processing': row['is_processing'],
                      'task_observation': 'unreported-idle' if task is None else 'upstream-id-task'})
    if sorted(ids) != list(range(slots)):
        raise m.GuardError('shared native slot identifiers differ')
    return {'total_slots': slots, 'processing_slots': sum(row['is_processing'] for row in rows),
            'context_per_sequence': 32768, 'native_idle_required': False, 'task_observations': tasks}


def power_health(raw, manifest, owned_idle):
    value = json.loads(raw)
    admission = value.get('worker', {}).get('admission', {}) if isinstance(value, dict) else {}
    limit = manifest['admission']['active']
    if (not isinstance(value, dict) or value.get('status') != 'ok'
            or value.get('speculative', {}).get('mode') != 'off'
            or type(value.get('loaded_models')) is not int or value['loaded_models'] < 1
            or type(value.get('inference', {}).get('num_parallel')) is not int
            or value['inference']['num_parallel'] != manifest['slots']
            or type(admission.get('active_limit')) is not int or admission['active_limit'] != limit
            or any(type(admission.get(key)) is not int for key in ('active', 'waiting'))
            or not 0 <= admission['active'] <= limit or not 0 <= admission['waiting'] <= q.MAX_U64):
        raise m.GuardError('actual Power health, registration or finite admission differs')
    if owned_idle and (admission['active'] or admission['waiting']):
        raise NotDrained('own Power admission is still active')
    return {'num_parallel': value['inference']['num_parallel'], 'loaded_models': value['loaded_models'],
            'admission': {key: admission[key] for key in ('active_limit', 'active', 'waiting')},
            'waiting_observation': 'admission-attempt gauge; includes brief rejected attempts, not exact queue permits',
            'queue_policy_verified_by_this_health_probe': False,
            'queue_limit_source': 'external root-sealed typed preparation, pinned controller and isolated overload proof; health does not expose queue configuration'}


def power_gauges(raw, active_limit, owned_idle):
    result = {}
    for line in raw.decode().splitlines():
        fields = line.split()
        if len(fields) == 2 and fields[0] in GAUGES:
            value = float(fields[1])
            if fields[0] in result or not math.isfinite(value) or value < 0 or not value.is_integer() or value > 2**53:
                raise m.GuardError('Power gauge is invalid')
            result[fields[0]] = value
    # Outer active HTTP requests include refusal transports (chat.rs:220).
    # Waiting increments before try-queue-full (limiter.rs:111–127), so rejected
    # attempts can overlap true queued work. Only running is a permit bound.
    if set(result) != GAUGES or result['power_requests_running'] > active_limit:
        raise m.GuardError('own Power permits did not drain or finite bounds were exceeded')
    if owned_idle and any(result.values()):
        raise NotDrained('own Power requests are still active')
    return result


def systemd_pid(unit):
    def bus(args):
        result = subprocess.run(['busctl', '--json=short'] + args, capture_output=True, timeout=3)
        if result.returncode or len(result.stdout) > 65536:
            raise m.GuardError('bounded systemd identity observation failed')
        return json.loads(result.stdout)
    obj = bus(['call', 'org.freedesktop.systemd1', '/org/freedesktop/systemd1',
               'org.freedesktop.systemd1.Manager', 'GetUnit', 's', unit])
    if obj.get('type') != 'o' or not isinstance(obj.get('data'), list) or len(obj['data']) != 1 or not isinstance(obj['data'][0], str):
        raise m.GuardError('systemd GetUnit typed method tuple differs')
    pid = bus(['get-property', 'org.freedesktop.systemd1', obj['data'][0], 'org.freedesktop.systemd1.Service', 'MainPID'])
    active = bus(['get-property', 'org.freedesktop.systemd1', obj['data'][0], 'org.freedesktop.systemd1.Unit', 'ActiveState'])
    if pid.get('type') != 'u' or type(pid.get('data')) is not int or not 0 < pid['data'] < 2**31 or active != {'type': 's', 'data': 'active'}:
        raise m.GuardError('production unit is not typed active with a live MainPID')
    return pid['data']


def production_proof(devices, manifests, fixed):
    proof = {}
    for gpu in devices:
        native = systemd_pid('prism-27b@' + str(gpu) + '.service')
        if native != fixed[gpu]['native']['pid']:
            raise m.GuardError('native receipt is not the actual production systemd process')
        proof['native-' + str(gpu)] = {'unit': 'prism-27b@' + str(gpu) + '.service',
                                      'process': fixed[gpu]['native']}
        pid = systemd_pid('a3s-power@' + str(gpu) + '.service')
        stamp = m.process_identity(pid, 18200 + gpu)
        if stamp['exe_sha256'] != POWER_SHA or stamp['exe_sha256'] != manifests[gpu]['binary_sha256']:
            raise m.GuardError('actual production Power artifact differs')
        health = power_health(m.get('/health', port=18200 + gpu, timeout=3), manifests[gpu], False)
        proof['power-' + str(gpu)] = {'unit': 'a3s-power@' + str(gpu) + '.service',
                                     'process': stamp, 'health': health}
    gateway = m.process_identity(systemd_pid('a3s-gateway.service'), 8080)
    if gateway['exe_sha256'] != GATEWAY_SHA:
        raise m.GuardError('actual production Gateway has not switched to the reviewed artifact')
    proof['gateway'] = {'unit': 'a3s-gateway.service', 'process': gateway,
                        'authenticated_public_inference_tested_here': False}
    return proof


def light_stamp(stamp):
    """No repeated binary hashing inside the measured interval."""
    pid, port = stamp['pid'], stamp['port']
    directory = m.PROC / str(pid)
    current = {'pid': pid, 'port': port, 'start_ticks': m.start_ticks(m.read_limited(directory / 'stat', 65536)),
               'cmdline_sha256': hashlib.sha256(m.read_limited(directory / 'cmdline', 65536)).hexdigest(),
               'listener_inodes': m.require_listener(pid, port)}
    if any(current[key] != stamp[key] for key in current):
        raise m.GuardError('production or private process generation/listener changed')
    info = (directory / 'exe').stat()
    current['exe_metadata'] = [info.st_dev, info.st_ino, info.st_size, info.st_mtime_ns, info.st_ctime_ns]
    current['cpu_affinity'] = sorted(os.sched_getaffinity(pid))
    return current


def observed_stamps(fixed, production):
    stamps = [stamp for worker in fixed.values() for stamp in worker.values()]
    stamps += [entry['process'] for entry in production.values()]
    return {str(stamp['port']): light_stamp(stamp) for stamp in stamps}


def verify_stamps(fixed, production, expected):
    if observed_stamps(fixed, production) != expected:
        raise m.GuardError('process executable metadata or CPU placement changed')


class Requests(e.OwnedRequests):
    """Keep frozen client parsing; inspect only a bounded closed429 envelope."""
    def __init__(self, ports):
        super().__init__(ports)
        self.local = threading.local()

    def once(self, *args, **kwargs):
        self.local.refusal = None
        record = q.request_once(*args, **kwargs)
        record['admission_refusal_code'] = self.local.refusal
        return record

    @contextlib.contextmanager
    def track(self):
        with super().track():
            original, owner = http.client.HTTPConnection, self
            class ClosedRefusal(original):
                def getresponse(self):
                    response = super().getresponse()
                    if self.port in owner.ports and response.status == 429:
                        raw = response.read(8193)
                        try:
                            value = json.loads(raw) if len(raw) <= 8192 else None
                        except ValueError:
                            value = None
                        error = value.get('error') if isinstance(value, dict) else None
                        if isinstance(error, dict) and error.get('type') == 'rate_limit_error' and error.get('code') in REFUSALS:
                            owner.local.refusal = error['code']
                    return response
            http.client.HTTPConnection = ClosedRefusal
            try:
                yield self
            finally:
                http.client.HTTPConnection = original


def validate_records(records, devices, corpus, warmup=False):
    known = {item['name']: item for item in corpus}
    sums, coverage = {gpu: 0 for gpu in devices}, {gpu: set() for gpu in devices}
    for row in records:
        index = row.get('endpoint_index')
        if type(index) is not int or not 0 <= index < len(devices) or row.get('case') not in known:
            raise m.GuardError('record endpoint or workload is outside the declared set')
        if row.get('outcome') == 'refused' and not warmup:
            if row.get('http_status') != 429 or row.get('admission_refusal_code') not in REFUSALS:
                raise m.GuardError('unknown429 is not a finite admission refusal')
            continue
        item = known[row['case']]
        tool = item['expect'].get('tool') == 'bench_sum'
        usage = q.valid_usage(row.get('usage'))
        if (row.get('outcome') != 'success' or row.get('http_status') != 200
                or usage is None or usage['completion_tokens'] <= 0
                or row.get('usage_source') != 'upstream-usage'
                or row.get('wire_usage') != row['usage'] or row.get('wire_usage_mismatch') is not False
                or row.get('finish_observed') is not True or row.get('terminal_observed') is not True
                or row.get('quality_source') != ('expected-tool' if tool else 'expected-text')
                or (tool and (row.get('tool_valid') is not True or row.get('tool_calls') != 1))):
            raise m.GuardError('own output, terminal transport or authoritative actual usage failed')
        gpu = devices[index]
        sums[gpu] += usage['completion_tokens']
        coverage[gpu].add(row['case'])
    if any(value <= 0 for value in sums.values()) or (warmup and any(value != set(known) for value in coverage.values())):
        raise m.GuardError('each GPU needs verified success; warmup needs every GPU/workload')
    return sums, {gpu: sorted(value) for gpu, value in coverage.items()}


def accounting(records, before, after, devices, corpus, warmup=False):
    sums, coverage = validate_records(records, devices, corpus, warmup)
    result = {}
    for gpu in devices:
        monotonic(before[gpu], after[gpu])
        delta = after[gpu]['llamacpp:tokens_predicted_total'] - before[gpu]['llamacpp:tokens_predicted_total']
        if delta < sums[gpu]:
            raise m.GuardError('per-GPU native completion delta is less than own verified output')
        result[gpu] = {'own_verified_completion_tokens': sums[gpu], 'global_native_completion_delta': delta,
                       'unattributed_native_completion_delta': delta - sums[gpu],
                       'unattributed_work_classification': 'unknown; never automatically external',
                       'verified_workloads': coverage[gpu]}
    return result


def snapshot(devices, manifests, deadline, owned_idle):
    result = {'native_states': {}, 'native_counters': {}, 'private_power_gauges': {}, 'private_power_health': {}, 'production_power_health': {}}
    for gpu in devices:
        manifest, ports = manifests[gpu], e.ports(gpu)
        result['native_states'][gpu] = native_state(gpu, manifest['slots'], deadline)
        result['native_counters'][gpu] = parse_native_metrics(m.get('/metrics', port=ports['native'], timeout=e.observation_timeout(deadline)))
        result['private_power_health'][gpu] = power_health(m.get('/health', port=ports['power'], timeout=e.observation_timeout(deadline)), manifest, owned_idle)
        result['private_power_gauges'][gpu] = power_gauges(m.get('/metrics', port=ports['power'], timeout=e.observation_timeout(deadline)), manifest['admission']['active'], owned_idle)
        result['production_power_health'][gpu] = power_health(m.get('/health', port=18200 + gpu, timeout=e.observation_timeout(deadline)), manifest, False)
    return result


def drained_snapshot(devices, manifests, seconds=15):
    deadline = time.monotonic() + seconds
    while True:
        try:
            return snapshot(devices, manifests, deadline, True)
        except NotDrained:
            if time.monotonic() >= deadline:
                raise
            time.sleep(min(.1, max(0, deadline - time.monotonic())))


def cleanup_owned(devices, manifests, seconds=15):
    deadline = time.monotonic() + seconds
    while True:
        try:
            gauges = {gpu: power_gauges(m.get('/metrics', port=e.ports(gpu)['power'], timeout=e.observation_timeout(deadline)),
                                      manifests[gpu]['admission']['active'], True) for gpu in devices}
            return {'all_private_permits_zero': True, 'private_power_gauges': gauges,
                    'native_idle_claimed': False}
        except NotDrained:
            if time.monotonic() >= deadline:
                return {'all_private_permits_zero': False, 'failure': 'owned-permit-drain-deadline', 'native_idle_claimed': False}
            time.sleep(min(.1, max(0, deadline - time.monotonic())))
        except Exception as error:
            return {'all_private_permits_zero': False, 'failure': type(error).__name__, 'native_idle_claimed': False}


def campaign(devices, corpus, key, concurrency, seconds, owned, abort):
    started, records, index = time.monotonic(), [], 0
    lock = threading.Lock()
    schedule = e.balanced_schedule(corpus, devices)
    def worker():
        nonlocal index
        while True:
            with lock:
                if abort.is_set() or time.monotonic() - started >= seconds or index >= 10000:
                    return
                current, index = index, index + 1
            item, endpoint = schedule[current % len(schedule)], current % len(devices)
            row = owned.once('http://127.0.0.1:' + str(e.ports(devices[endpoint])['gateway']) + '/v1', item['request'], key, 120, expected=item['expect'])
            row.update(request_index=current, endpoint_index=endpoint, case=item['name'])
            with lock:
                records.append(row)
            try:
                if row.get('outcome') == 'refused':
                    if row.get('http_status') != 429 or row.get('admission_refusal_code') not in REFUSALS:
                        raise m.GuardError('unknown admission refusal')
                else:
                    validate_records([dict(row, endpoint_index=0)], [devices[endpoint]], corpus)
            except m.GuardError:
                abort.set()
                owned.abort()
                return
            if row['outcome'] == 'refused':
                abort.wait(min(.2, max(0, seconds - (time.monotonic() - started))))
    with concurrent.futures.ThreadPoolExecutor(max_workers=concurrency) as pool:
        futures = [pool.submit(worker) for _ in range(concurrency)]
        for future in futures:
            future.result(timeout=seconds + 135)
    wall = time.monotonic() - started
    records.sort(key=lambda row: row['request_index'])
    return {'records': records, 'summary': q.summarize(records, wall), 'dispatch_seconds': seconds,
            'limits': {'concurrency': concurrency, 'request_timeout': 120, 'max_requests': 10000},
            'request_budget_exhausted': index >= 10000, 'aborted': abort.is_set()}


def monitor(stop, abort, owned, devices, manifests, fixed, production, stamps, samples, errors):
    previous = None
    while not stop.is_set():
        try:
            if len(samples) >= 1024:
                raise m.GuardError('bounded observation count exhausted')
            gpu = e.gpu_samples(devices)
            if any(m.vram_exceeded(value) for value in gpu.values()):
                raise m.GuardError('whole-GPU90percent VRAM guard exceeded')
            verify_stamps(fixed, production, stamps)
            observed = snapshot(devices, manifests, time.monotonic() + 10, False)
            if previous is not None:
                for device in devices:
                    monotonic(previous[device], observed['native_counters'][device])
            previous = observed['native_counters']
            samples.append({'at_monotonic': time.monotonic(), 'gpu': gpu, 'observations': observed})
        except Exception as exc:
            errors.append({'exception': type(exc).__name__})
            abort.set()
            owned.abort()
            stop.set()
        stop.wait(1)


def claims():
    return {'scope': SCOPE, 'native_exclusive_capacity': False, 'native_exclusive_tokens_per_second': None,
            'causal_eight_card_before_after': False, 'shared_native_cancellation': 'not_proven',
            'global_production_admission': 'not_proven; private Power is a second admission pool',
            'waiting_metric_semantics': 'admission attempts, including brief refusals; never an exact queue-permit peak',
            'queue_policy_proof_scope': 'external root-sealed typed preparation plus pinned controller and isolated overload; not inferred from waiting metrics',
            'authenticated_public_frontdoor_pressure': 'not_tested_by_this_private_wrapper_tool'}


def matrix(smoke):
    # A c1/one-second smoke cannot cover eight endpoints. Its first wave is c8.
    return [(1, 8, 1)] if smoke else [(repeat, concurrency, 60) for repeat in range(1, 4) for concurrency in (1, 8, 16, 32)]


def validate_claims(result):
    if any(result.get(key) != value for key, value in claims().items()) or any(key in result for key in (
            'native_output_tokens_per_second', 'throughput_ratio', 'unattributed_external_delta')):
        raise m.GuardError('shared native work cannot support exclusive or external-attribution claims')


def run_one(devices, manifests, fixed, production, stamps, corpus, key, directory, concurrency, repeat, seconds, budget, expected_tools=None):
    target = directory / ('shared-c' + str(concurrency) + '-r' + str(repeat) + '.json')
    if target.exists():
        raise m.GuardError('existing live result; no replacement or automatic retry')
    if expected_tools is None:
        raise m.GuardError('initial fixed instrument identity is required')
    result = dict(claims(), schema_version=1, qualification_status='preflight', selected_gpus=devices,
                  tool_identity=current_tools(expected_tools), provenance=manifests, private_process_identity=fixed,
                  production_identity=production, warmup_records=[], records=[])
    stop, abort, samples, errors = threading.Event(), threading.Event(), [], []
    owned, thread, vram_thread, vram_samples = Requests(e.ports(gpu)['gateway'] for gpu in devices), None, None, []
    try:
        verify_stamps(fixed, production, stamps)
        result['before_warmup'] = drained_snapshot(devices, manifests)
        result['preflight_gpu'] = e.gpu_samples(devices)
        if any(m.vram_exceeded(value) for value in result['preflight_gpu'].values()):
            raise m.GuardError('whole-GPU90percent preflight VRAM guard exceeded')
        with owned.track():
            # The one-second GPU guard must not wait behind aggregate HTTP health reads.
            vram_thread = threading.Thread(target=e.monitor, args=(stop, abort, owned, devices, vram_samples, errors), daemon=True)
            vram_thread.start()
            thread = threading.Thread(target=monitor, args=(stop, abort, owned, devices, manifests, fixed, production, stamps, samples, errors), daemon=True)
            thread.start()
            for endpoint, gpu in enumerate(devices):
                for item in corpus:
                    if abort.is_set():
                        raise m.GuardError('resource monitor aborted before warmup dispatch')
                    row = owned.once('http://127.0.0.1:' + str(e.ports(gpu)['gateway']) + '/v1', item['request'], key, budget.left(120), expected=item['expect'])
                    row.update(endpoint_index=endpoint, case=item['name'])
                    result['warmup_records'].append(row)
                    m.write_result(target, result)
                    if row['outcome'] != 'success':
                        raise m.GuardError('own controlled warmup failed')
                    validate_records([dict(row, endpoint_index=0)], [gpu], corpus)
            result['before_measurement'] = drained_snapshot(devices, manifests)
            result['warmup_accounting'] = accounting(result['warmup_records'], result['before_warmup']['native_counters'], result['before_measurement']['native_counters'], devices, corpus, True)
            if abort.is_set() or budget.left(seconds + 135) < seconds + 135:
                raise m.GuardError('insufficient absolute campaign budget')
            result.update(campaign(devices, corpus, key, concurrency, seconds, owned, abort))
            m.write_result(target, result)
            result['after_measurement'] = drained_snapshot(devices, manifests)
            result['accounting_by_gpu'] = accounting(result['records'], result['before_measurement']['native_counters'], result['after_measurement']['native_counters'], devices, corpus)
            if result['request_budget_exhausted'] or abort.is_set():
                raise m.GuardError('finite request budget or resource guard failed')
        stop.set()
        thread.join(timeout=15)
        vram_thread.join(timeout=4)
        if thread.is_alive() or vram_thread.is_alive() or errors or not samples or not vram_samples:
            raise m.GuardError('resource monitor failed or did not terminate')
        verify_stamps(fixed, production, stamps)
        current_tools(expected_tools)
        result.update(qualification_status='passed', own_http_complete=True, own_power_permits_zero=True,
                      own_verified_successful_requests_per_second=result['summary']['successful_requests_per_second'],
                      own_authoritative_output_tokens_per_second=result['summary']['successful_output_tokens_per_second'],
                      rate_denominator='client campaign wall_seconds_including_drain; every dispatched own HTTP request ended')
        validate_claims(result)
    except Exception as exc:
        result.update(qualification_status='failed', failure_exception=type(exc).__name__)
        if isinstance(exc, m.GuardError):
            result['failure_reason'] = str(exc)
        raise
    finally:
        stop.set()
        owned.abort()
        if thread is not None:
            thread.join(timeout=15)
        if vram_thread is not None:
            vram_thread.join(timeout=4)
        result['owned_cleanup'] = cleanup_owned(devices, manifests)
        if not result['owned_cleanup']['all_private_permits_zero']:
            result['qualification_status'] = 'failed'
        result.update(observations=samples, vram_guard_samples=vram_samples, monitor_errors=errors,
                      monitor_terminated=(thread is None or not thread.is_alive()) and (vram_thread is None or not vram_thread.is_alive()),
                      native_process_control='never; guard shuts down helper-owned private HTTP sockets only')
        m.write_result(target, result)
    if result['qualification_status'] != 'passed':
        raise m.GuardError('final own-permit cleanup failed')
    print(json.dumps({'file': target.name, 'status': result['qualification_status'], 'outcomes': result['summary']['outcomes']}), flush=True)
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--tag', required=True)
    parser.add_argument('--smoke', action='store_true', help='one-second c8 first wave covers all eight GPUs; never the full qualification')
    args = parser.parse_args()
    if not args.tag or len(args.tag) > 48 or any(c not in 'abcdefghijklmnopqrstuvwxyz0123456789-_' for c in args.tag):
        raise ValueError('bounded lowercase tag required')
    tools = current_tools()
    raw, corpus = e.load_corpus()
    devices = list(range(8))
    manifests, fixed = e.read_workers(devices, raw)
    for gpu in devices:
        if manifests[gpu]['binary_sha256'] != POWER_SHA or manifests[gpu].get('admission') != {'active': 2, 'waiting_limit': 8, 'queue_timeout_ms': 1000} or manifests[gpu]['slots'] != 2:
            raise m.GuardError('selected live profile must be verifiedv4 NP2 with finite2/8/1000 admission')
        e.worker_identity(gpu, manifests[gpu], fixed[gpu])
        if fixed[gpu]['gateway']['exe_sha256'] != GATEWAY_SHA:
            raise m.GuardError('private Gateway artifact differs')
    production = production_proof(devices, manifests, fixed)
    stamps = observed_stamps(fixed, production)
    directory = e.PRIVATE / args.tag
    directory.mkdir(exist_ok=False)
    m.write_result(directory / 'live-manifest.json', dict(claims(), tool_identity=tools, production_identity=production,
                   private_process_identity=fixed, live_process_stamps=stamps, smoke=args.smoke))
    key = m.read_limited(m.PRIVATE / 'benchmark.key', 4096).decode().strip()
    if not key:
        raise m.GuardError('fresh benchmark key is absent')
    budget = e.Budget(5370)
    for repeat, concurrency, seconds in matrix(args.smoke):
        run_one(devices, manifests, fixed, production, stamps, corpus, key, directory, concurrency, repeat, seconds, budget, tools)
    for gpu in devices:
        e.worker_identity(gpu, manifests[gpu], fixed[gpu])
    final = production_proof(devices, manifests, fixed)
    if any(final[key]['process'] != production[key]['process'] for key in production):
        raise m.GuardError('actual production generations changed during live qualification')
    m.write_result(directory / 'live-complete.json', dict(claims(), qualification_status='passed', smoke=args.smoke,
                   cell_count=1 if args.smoke else 12, tool_identity=current_tools(tools), final_production_identity=final))


def cli():
    # This process owns no native/service process. OS close on this process's exit
    # also closes its benchmark HTTP sockets; a deadline is never a passing run.
    hard_deadline = threading.Timer(5400, lambda: os._exit(124))
    hard_deadline.daemon = True
    hard_deadline.start()
    try:
        main()
    finally:
        hard_deadline.cancel()


if __name__ == '__main__':
    cli()
