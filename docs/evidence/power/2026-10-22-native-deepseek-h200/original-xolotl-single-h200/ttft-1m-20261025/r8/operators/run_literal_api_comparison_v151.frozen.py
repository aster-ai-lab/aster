"""Finite exact baseline -> same-native literal API qualification; no formal changes."""
import json
import subprocess
import sys
import time
from pathlib import Path

B = Path(__file__).resolve().parent
PATH = B / 'xolotl-literal-api-v151-controller.json'
if PATH.exists():
    raise RuntimeError('exact comparison controller has a handle; inspect it')
STATE = dict(unix=time.time(), phase='baseline_fullwindow', formal_service_unchanged=True,
    maximum_h200_roles=2, model_math_changed=False, acceptance_changed=False,
    goal_complete=False, gpu_300_qualified=False, native_binary_sha256='99f3eee8c7eeee54c304120db50e8e632a7c001f9ac90fadf0983040fef92c25')

def save():
    for attempt in range(12):
        try:
            temp = PATH.with_suffix('.tmp')
            temp.write_text(json.dumps(STATE, indent=2) + '\n')
            temp.replace(PATH)
            return
        except PermissionError:
            if attempt == 11:
                raise
            time.sleep((attempt+1)*.1)

def run(name, *args):
    result = subprocess.run([sys.executable, '-X', 'utf8', str(B/name), *args],
        cwd=B, capture_output=True, text=True, timeout=200)
    if result.returncode:
        raise RuntimeError(name + ' failed: ' + result.stderr[-3500:])
    if len(result.stdout) > 2*1024**2:
        raise RuntimeError('bounded exact observer output exceeded')
    return json.loads(result.stdout)

def wait_full(profile):
    while True:
        row = run('manage_literal_fullwindow_suite_v150.py', 'observe', '--profile', profile)
        STATE.update(profile=profile, suite_phase=row['phase'], suite_stage=row['current_stage'],
                     suite_alive=row['exact_client_alive'], suite_error=row.get('error'), children=row['children'], observed_unix=time.time())
        save()
        print(json.dumps(dict(profile=profile, phase=row['phase'], stage=row['current_stage'], alive=row['exact_client_alive'], children=row['children'])), flush=True)
        if row['phase'] in ['passed','failed'] and not row['exact_client_alive']:
            proof = json.loads((B/('xolotl-literal-fullwindow-v150-'+profile+'-latest.json')).read_text())
            q = proof.get('fullwindow/request-result.json',{})
            if not q.get('full_1m_length_qualified') or not q.get('full1m_cache_restore_passed') or not q.get('exact_restored_long_output_matched') or len(q.get('tests',[])) != 20:
                raise RuntimeError('complete real1M cold/restore/c1/c2 structural observation required before transition')
            if row['phase']=='failed' and q.get('error')!='ValueError: full-window long-output identity differs':
                raise RuntimeError('baseline/candidate fullwindow failure requires inspection')
            STATE[profile+'_identity_gate_passed'] = row['phase']=='passed'
            save()
            return
        time.sleep(30)

def observe_service(name, profile, terminal=False):
    start = time.monotonic()
    while time.monotonic()-start < 720:
        v = run(name, profile, 'observe')
        q = v.get('service-longctx-v1-result.json',{})
        STATE.update(service_phase=q.get('phase'), service_supervisor_alive=v.get('exact_supervisor_alive'), observed_unix=time.time())
        save()
        if terminal:
            if q.get('phase')=='stopped' and q.get('api_reaped') and not v.get('exact_supervisor_alive'):
                return
        elif q.get('phase')=='serving':
            return
        elif q.get('phase')=='failed':
            raise RuntimeError('literal service failed startup: '+str(q.get('error')))
        time.sleep(15)
    raise RuntimeError('bounded service observation elapsed; inspect exact handles without relaunch')

def qualify(name, intent_name, launch, observation, expected):
    if not (B/intent_name).exists():
        run(name, 'literal', launch)
    while True:
        row = run(name, 'literal', observation)
        STATE.update(client_phase=row.get('phase'), client_tests=row.get('tests'), client_alive=row.get('exact_client_alive'), observed_unix=time.time())
        save()
        print(json.dumps(row), flush=True)
        if row.get('phase') in ['passed','failed'] and not row.get('exact_client_alive'):
            if row['phase']!='passed' or row['tests']!=expected:
                raise RuntimeError('literal API unchanged '+str(expected)+'-request qualification failed')
            return
        time.sleep(20)

try:
    save()
    wait_full('control')
    STATE['phase']='draining_baseline';save()
    if not (B/'xolotl-phase-simd-v139-control_b-drain.json').exists():
        run('manage_phase_simd_basic_v139.frozen.py','control_b','drain')
    observe_service('manage_phase_simd_basic_v139.frozen.py','control_b',terminal=True)
    STATE['phase']='candidate_launch';save()
    if not (B/'xolotl-literal-service-v149-literal-intent.json').exists():
        run('launch_literal_service_v149.frozen.py','literal')
    observe_service('manage_literal_basic_v149.frozen.py','literal')
    STATE['phase']='candidate_basic28';save()
    qualify('manage_literal_basic_v149.frozen.py','xolotl-literal-service-v149-literal-qualification-intent.json','launch_client','observe_client',28)
    STATE['phase']='candidate_tool_long12';save()
    qualify('manage_literal_extended_v149.frozen.py','xolotl-literal-extended-v149-literal-intent.json','launch','observe',12)
    STATE['phase']='candidate_fullwindow';save()
    if not (B/'xolotl-literal-fullwindow-v150-literal-intent.json').exists():
        run('manage_literal_fullwindow_suite_v150.py','launch','--profile','literal')
    wait_full('literal')
    STATE.update(phase='comparison_completed', model_speed_claimed=False, candidate_promoted=False, finished_unix=time.time())
    save()
    print(json.dumps(dict(phase=STATE['phase'], goal_complete=False)),flush=True)
except BaseException as error:
    STATE.update(phase='requires_inspection', error=type(error).__name__+': '+str(error)[:4000], finished_unix=time.time())
    save()
    raise
