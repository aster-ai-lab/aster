#!/usr/bin/env python3
"""Finite qualification-only supervisor; no native/service lifecycle operations."""
import argparse
import hashlib
import importlib.util
import json
from pathlib import Path
import selectors
import subprocess
import sys
import time

HERE = Path(__file__).resolve().parent
spec = importlib.util.spec_from_file_location('qualification_driver_eight', HERE / 'eight-card-qualification.py')
e = importlib.util.module_from_spec(spec)
spec.loader.exec_module(e)
m, q = e.m, e.q
PRIVATE = m.PRIVATE / 'eight-replica'
POWER_SHA = '75afe5de8d423547fdb9b9bb302b17ca9e168d6f6fe13a18d29fbce9492bf1c1'
PINS = {'runner_sha256': '576b2d13811482024522e790f901647ac79dc6cc4368a2622a71adef856fe546',
        'matrix_sha256': 'b1e24fd648da9b7ae31cb3d3ace44c4b9b91dc6c793a0d9de7ac114ba1b821e4',
        'canary_sha256': '1960de0dd2c8dbec7338f808ddbd207972fb63844fa022607bb676f1069c459e',
        'eight_card_sha256': '9385bf26ec9dd6ea971612356214ff2691e319318a87bbddaa3eaded5996675e'}
CELL_CACHE = {}
CELL_NAMES = frozenset(f'candidate-c{c}-r{r}.json' for r in (1, 2, 3) for c in (1, 8, 16, 32))
RUN_TAGS = ('canary-v4', 'canary-v4-isolated', 'canary-v4-isolated-warm')
FUNCTIONAL_NAMES = frozenset(tag + '-functional.json' for tag in RUN_TAGS)


def load(path):
    return json.loads(m.read_limited(path, 16 * m.MAX_OBSERVATION))


def current_tools():
    result = dict(m.TOOL_IDENTITY, canary_sha256=m.digest(HERE / 'canary-scenarios.py'),
                  eight_card_sha256=m.digest(HERE / 'eight-card-qualification.py'))
    if result != PINS:
        raise m.GuardError('qualification instrument differs from the reviewed frozen files')
    return result


def inputs(slots):
    corpus = m.read_limited(HERE / 'eight-card-corpus.jsonl', 65536)
    provenance = q.load_provenance(PRIVATE / 'worker-7-provenance.json', corpus)
    identity = load(PRIVATE / 'worker-7-identity.json')
    ready = load(PRIVATE / 'worker-7-readiness.json')
    if (provenance['arm'] != 'candidate' or provenance['binary_sha256'] != POWER_SHA
            or provenance['gpu_indices'] != [7] or provenance['slots'] != slots
            or provenance['context_per_sequence'] != 32768
            or ready.get('status') != 'healthy-idle' or ready.get('slots') != slots
            or ready.get('identity') != identity):
        raise m.GuardError('actual v4 worker7 readiness/provenance receipts do not match')
    return provenance, identity


def functional_ok(result, provenance, identity):
    if result.get('status') != 'passed' or result.get('provenance') != provenance:
        return False
    if result.get('tool_identity') != {key: value for key, value in PINS.items() if key != 'eight_card_sha256'}:
        return False
    actual = result.get('identity', {})
    if set(actual) != set(identity):
        return False
    for kind, record in actual.items():
        if set(record) != {'pid', 'start_ticks', 'cmdline_sha256', 'exe_sha256', 'listener_inodes', 'port'} or any(identity[kind].get(key) != value for key, value in record.items()):
            return False
    cleanup = result.get('cleanup', {})
    return (cleanup.get('own_requests_completed') is True
            and cleanup.get('observed_all_native_and_Power_idle') is True
            and result.get('monitor_terminated') is True
            and len(result.get('cases', [])) == 4
            and all(row.get('passed') is True for row in result['cases']))


def cells(tag, smoke):
    expected = ['candidate-c1-r1.json'] if smoke else [f'candidate-c{c}-r{r}.json' for r in (1, 2, 3) for c in (1, 8, 16, 32)]
    result = {}
    for name in expected:
        path = PRIVATE / tag / name
        if path.exists():
            stamp = (path.stat().st_mtime_ns, path.stat().st_size)
            cached = CELL_CACHE.get(path)
            if cached is None or cached[0] != stamp:
                value = load(path)
                cached = (stamp, {'status': value.get('qualification_status'),
                                  'outcomes': value.get('summary', {}).get('outcomes', {})})
                CELL_CACHE[path] = cached
            result[name] = cached[1]
    return result


def safe_line(raw):
    # Never mirror arbitrary child diagnostics, headers or errors to evidence.
    row = {'line_sha256': hashlib.sha256(raw).hexdigest(), 'line_bytes': len(raw)}
    try:
        value = json.loads(raw)
    except (ValueError, UnicodeError):
        return row
    if not isinstance(value, dict):
        return row
    if value.get('status') in ('passed', 'failed', 'unsupported'):
        row['status'] = value['status']
    if isinstance(value.get('file'), str) and value['file'] in CELL_NAMES | FUNCTIONAL_NAMES:
        row['file'] = value['file']
    outcomes = value.get('outcomes')
    if isinstance(outcomes, dict) and set(outcomes) <= {'success', 'refused', 'http_error', 'error', 'timeout', 'cancelled', 'invalid_stream', 'invalid_output'} and all(type(n) is int and 0 <= n <= 100000 for n in outcomes.values()):
        row['outcomes'] = outcomes
    if type(value.get('cases')) is int and 0 <= value['cases'] <= 64:
        row['cases'] = value['cases']
    return row


def supervise(command, timeout, status, state_path, events, tag=None, smoke=False):
    process = subprocess.Popen(command, stdin=subprocess.DEVNULL, stdout=subprocess.PIPE,
                               stderr=subprocess.STDOUT, start_new_session=True, bufsize=0)
    reader = None
    try:
        status.update(current_command=command, child_pid=process.pid, child_started_monotonic=time.monotonic())
        m.write_result(state_path, status)
        reader, buffer, total_bytes = selectors.DefaultSelector(), b'', 0
        reader.register(process.stdout, selectors.EVENT_READ)
        deadline = time.monotonic() + timeout
        while reader.get_map() or process.poll() is None:
            if time.monotonic() >= deadline:
                raise m.GuardError('finite qualification child deadline exceeded')
            for key, _mask in reader.select(timeout=min(2, max(.001, deadline - time.monotonic()))):
                chunk = key.fileobj.read(65536)
                if not chunk:
                    reader.unregister(key.fileobj)
                    continue
                total_bytes += len(chunk)
                if total_bytes > 8 * m.MAX_OBSERVATION:
                    raise m.GuardError('qualification child diagnostic stream exceeds byte bound')
                buffer += chunk
                if len(buffer) > 65536:
                    raise m.GuardError('qualification child line exceeds byte bound')
                while b'\n' in buffer:
                    line, buffer = buffer.split(b'\n', 1)
                    event = dict(safe_line(line), at_monotonic=time.monotonic(), child_pid=process.pid)
                    events.write(json.dumps(event) + '\n')
                    events.flush()
                    print(json.dumps(event), flush=True)
            if tag is not None:
                status['cells'] = cells(tag, smoke)
            status['updated_monotonic'] = time.monotonic()
            m.write_result(state_path, status)
        if buffer:
            events.write(json.dumps(safe_line(buffer)) + '\n')
            events.flush()
        code = process.wait(timeout=2)
        status['child_exit_code'] = code
        if code:
            raise m.GuardError('qualification child failed; no automatic retry')
    finally:
        if reader is not None:
            reader.close()
        if process.poll() is None:
            # This is the exact direct qualification child, never a native PID.
            process.terminate()
            try:
                process.wait(timeout=15)
            except subprocess.TimeoutExpired:
                process.kill()
                process.wait(timeout=3)
        process.stdout.close()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--phase', choices=('smoke', 'full'), default='full')
    parser.add_argument('--slots', type=int, choices=(1, 2), required=True)
    parser.add_argument('--run-tag', choices=RUN_TAGS, default='canary-v4')
    args = parser.parse_args()
    state_path = PRIVATE / (args.run_tag + '-driver-' + args.phase + '.json')
    if state_path.exists():
        raise m.GuardError('driver evidence exists; no replacement or automatic retry')
    tools = current_tools()
    provenance, identity = inputs(args.slots)
    status = {'scope': 'qualification only of already-ready GPU7 wrappers; native/service lifecycle absent',
              'status': 'preflight', 'phase': args.phase, 'run_tag': args.run_tag,
              'slots': args.slots, 'tool_identity': tools,
              'driver_sha256': m.digest(Path(__file__)), 'started_monotonic': time.monotonic(),
              'identity_receipt_sha256': m.digest(PRIVATE / 'worker-7-identity.json'),
              'provenance_receipt_sha256': m.digest(PRIVATE / 'worker-7-provenance.json'),
              'readiness_receipt_sha256': m.digest(PRIVATE / 'worker-7-readiness.json')}
    overall_deadline = time.monotonic() + 5600
    functional = m.PRIVATE / (args.run_tag + '-functional.json')
    tag = args.run_tag + '-' + args.phase
    if (PRIVATE / tag).exists():
        raise m.GuardError('qualification output exists; no replacement or automatic retry')
    m.write_result(state_path, status)
    try:
        with (PRIVATE / (args.run_tag + '-driver-' + args.phase + '.jsonl')).open('x', encoding='utf-8') as events:
            if not functional.exists():
                status['status'] = 'functional-scenarios'
                command = [sys.executable, '-u', '-B', str(HERE / 'canary-scenarios.py'), '--tag', args.run_tag.removeprefix('canary-') + '-functional',
                           '--slots', str(args.slots), '--identity', str(PRIVATE / 'worker-7-identity.json'),
                           '--provenance', str(PRIVATE / 'worker-7-provenance.json')]
                supervise(command, 200, status, state_path, events)
            if not functional_ok(load(functional), provenance, identity):
                raise m.GuardError('functional canary gate or cleanup did not pass for this exact worker')
            status.update(status='qualification-matrix', functional_evidence=str(functional))
            command = [sys.executable, '-u', '-B', str(HERE / 'eight-card-qualification.py'), '--gpu', '7', '--tag', tag,
                       '--seconds', '1' if args.phase == 'smoke' else '60', '--repeats', '1' if args.phase == 'smoke' else '3',
                       '--concurrency'] + (['1'] if args.phase == 'smoke' else ['1', '8', '16', '32'])
            supervise(command, min(5400, overall_deadline - time.monotonic()), status, state_path, events, tag, args.phase == 'smoke')
            status['cells'] = cells(tag, args.phase == 'smoke')
            if len(status['cells']) != (1 if args.phase == 'smoke' else 12) or any(row['status'] != 'passed' for row in status['cells'].values()):
                raise m.GuardError('finite qualification matrix is incomplete or failed')
            current_tools()
            if inputs(args.slots) != (provenance, identity):
                raise m.GuardError('worker receipts changed during qualification')
            status['status'] = 'passed'
    except Exception as exc:
        status.update(status='failed', failure_exception=type(exc).__name__)
        if isinstance(exc, m.GuardError):
            status['failure_reason'] = str(exc)
        raise
    finally:
        status['elapsed_seconds'] = time.monotonic() - status['started_monotonic']
        m.write_result(state_path, status)
        print(json.dumps({'file': state_path.name, 'status': status['status'], 'phase': args.phase}), flush=True)


if __name__ == '__main__':
    main()
