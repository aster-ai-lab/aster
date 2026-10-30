"""Finish exact existing1M suite, then a bounded single-GPU controlled SIMD A/B/A.

Known gate failures remain recorded. No formal service is restarted/promoted.
"""
import json
import subprocess
import sys
import time
from xolotl_trial_common import BASE

PATH = BASE / 'xolotl-phase-simd-v140-controller.json'
STATE = dict(unix=time.time(), phase='observing_exact_existing_code1m_suite', profiles=[],
             formal_service_unchanged=True, maximum_h200_roles=2, gpu_300_qualified=False,
             model_gain_claimed=False, goal_complete=False)
with PATH.open('x') as output:
    output.write(json.dumps(STATE, indent=2) + '\n')

def save():
    STATE['observed_unix'] = time.time()
    temp = PATH.with_suffix('.tmp')
    temp.write_text(json.dumps(STATE, indent=2) + '\n')
    for i in range(12):
        try:
            temp.replace(PATH)
            return
        except PermissionError:
            if i == 11:
                raise
            time.sleep(.1 * (i + 1))

def run(script, *args, timeout=100):
    result = subprocess.run([sys.executable, '-X', 'utf8', str(BASE / script), *args],
                            cwd=BASE, capture_output=True, text=True, timeout=timeout)
    if result.returncode:
        raise RuntimeError(script + ' ' + str(args) + ': ' + result.stderr[-3500:])
    return json.loads(result.stdout)

def wait_stopped(script, profile):
    deadline = time.monotonic() + 170
    while time.monotonic() < deadline:
        q = run(script, profile, 'observe')
        service = q.get('service-longctx-v1-result.json', {})
        if not q['exact_supervisor_alive']:
            if service.get('phase') != 'stopped' or not service.get('api_reaped'):
                raise RuntimeError('exact prior comparison model did not stop/reap cleanly')
            return
        time.sleep(10)
    raise RuntimeError('drain observation expired; inspect same exact supervisor, do not restart')

def wait_client(script, profile, action, expected, seconds):
    deadline = time.monotonic() + seconds
    last = None
    while time.monotonic() < deadline:
        q = run(script, profile, action)
        row = (q.get('phase'), q.get('tests'), q.get('exact_client_alive'))
        STATE.update(client_phase=row[0], client_tests=row[1], client_alive=row[2])
        save()
        if row != last:
            print(json.dumps(dict(profile=profile, stage=STATE['phase'], phase=row[0], tests=row[1], alive=row[2])), flush=True)
            last = row
        if not row[2]:
            if row[0] != 'passed' or row[1] != expected:
                raise RuntimeError('unchanged standard model acceptance failed: ' + str(q.get('error')))
            return q
        time.sleep(20)
    raise RuntimeError('finite client observation expired; inspect same exact client, do not restart')

try:
    deadline = time.monotonic() + 3000
    last = None
    while time.monotonic() < deadline:
        q = run('manage_code_fullwindow_suite_v119.frozen.py', 'observe')
        row = (q.get('phase'), q.get('current_stage'), q.get('exact_client_alive'), q.get('children'))
        STATE.update(prior_suite_phase=row[0], prior_suite_client_alive=row[2], prior_suite_children=row[3])
        save()
        if row != last:
            print(json.dumps(dict(prior_suite_phase=row[0], stage=row[1], alive=row[2], children=row[3])), flush=True)
            last = row
        if not row[2]:
            if row[0] not in ['passed', 'failed']:
                raise RuntimeError('exact original suite ended without a terminal receipt; inspect same handle')
            STATE['prior_suite_error'] = q.get('error')
            break
        time.sleep(30)
    else:
        raise RuntimeError('original suite observation expired; inspect same exact handle, do not restart')
    STATE['phase'] = 'draining_completed_code1m_service'
    save()
    run('manage_code_basic_v118.frozen.py', 'full1m_code', 'drain')
    wait_stopped('manage_code_basic_v118.frozen.py', 'full1m_code')
    STATE['phase'] = 'linking_controlled_native_variants'
    save()
    if not (BASE / 'xolotl-native-phase-simd-v138-intent.json').exists():
        run('manage_native_phase_simd_v138.py', 'launch', timeout=160)
    deadline = time.monotonic() + 340
    while time.monotonic() < deadline:
        q = run('manage_native_phase_simd_v138.py', 'observe')
        STATE.update(native_link_phase=q.get('phase'), native_link_alive=q.get('alive'))
        save()
        if not q.get('alive'):
            if q.get('phase') != 'linked' or not q.get('control_byte_equal'):
                raise RuntimeError('controlled current9a native link failed: ' + str(q.get('error')))
            print(json.dumps(dict(native_link_phase=q['phase'], variants=q['variants'])), flush=True)
            break
        time.sleep(10)
    else:
        raise RuntimeError('native link observation expired; inspect same exact handle')
    for profile in ['control_a', 'simd', 'control_b']:
        STATE.update(phase='starting_fresh_profile', current_profile=profile)
        save()
        run('launch_phase_simd_service_v139.frozen.py', profile, timeout=180)
        deadline = time.monotonic() + 700
        while time.monotonic() < deadline:
            q = run('manage_phase_simd_basic_v139.frozen.py', profile, 'observe')
            phase = q.get('service-longctx-v1-result.json', {}).get('phase')
            STATE['service_phase'] = phase
            save()
            if phase == 'serving' and q['exact_supervisor_alive']:
                break
            if phase == 'failed' or not q['exact_supervisor_alive']:
                raise RuntimeError('exact fresh SIMD/control service failed; inspect same handle')
            time.sleep(15)
        else:
            raise RuntimeError('service startup observation expired; inspect same exact handle')
        STATE['phase'] = 'basic28'
        save()
        run('manage_phase_simd_basic_v139.frozen.py', profile, 'launch_client')
        basic = wait_client('manage_phase_simd_basic_v139.frozen.py', profile, 'observe_client', 28, 1150)
        STATE['phase'] = 'native_tool_long12'
        save()
        run('manage_phase_simd_extended_v139.frozen.py', profile, 'launch')
        extended = wait_client('manage_phase_simd_extended_v139.frozen.py', profile, 'observe', 12, 1350)
        if not extended.get('native_tool_roundtrip_passed') or not extended.get('reference_matched'):
            raise RuntimeError('unchanged native-tool and exact1024-output references must pass')
        STATE['profiles'].append(dict(profile=profile, basic=basic, extended=extended, model_requests=40))
        save()
        if profile != 'control_b':
            run('manage_phase_simd_basic_v139.frozen.py', profile, 'drain')
            wait_stopped('manage_phase_simd_basic_v139.frozen.py', profile)
    STATE.update(phase='comparison_completed', model_requests=120, finished_unix=time.time(),
                 final_optimization_service='control_b', formal_promoted=False)
    save()
    print(json.dumps(dict(phase=STATE['phase'], model_requests=120, goal_complete=False)), flush=True)
except BaseException as error:
    STATE.update(phase='requires_inspection', error=type(error).__name__ + ': ' + str(error), finished_unix=time.time())
    save()
    raise
