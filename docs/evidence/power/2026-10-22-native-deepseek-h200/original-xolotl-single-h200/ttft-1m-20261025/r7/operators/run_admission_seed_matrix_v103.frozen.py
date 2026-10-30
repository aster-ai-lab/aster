"""Finite off/on/off-repeat admission-seed diagnostic; never adopts a counterfactual or changes the qualified-output gate."""
import json
import subprocess
import sys
import time
from xolotl_trial_common import BASE

PATH=BASE/'xolotl-admission-seed-v103-controller.json'
STATE=dict(unix=time.time(),phase='starting',profiles=[],formal_service_unchanged=True,actual1m_input_tested=False,
           diagnostic_only=True,performance_qualified=False,goal_complete=False)
with PATH.open('x') as output:output.write(json.dumps(STATE,indent=2)+'\n')


def save():
    STATE['observed_unix']=time.time();p=PATH.with_suffix('.tmp');p.write_text(json.dumps(STATE,indent=2)+'\n');p.replace(PATH)


def run(script,*args):
    result=subprocess.run([sys.executable,'-X','utf8',str(BASE/script),*args],cwd=BASE,capture_output=True,text=True,timeout=100)
    if result.returncode:raise RuntimeError(script+' '+str(args)+' exit'+str(result.returncode)+': '+result.stderr[-3500:])


def wait_model(profile,target,seconds):
    end=time.monotonic()+seconds
    while time.monotonic()<end:
        run('manage_admission_seed_matrix_v102.frozen.py',profile,'observe')
        value=json.loads((BASE/('xolotl-admission-service-v101-'+profile+'-latest.json')).read_text());q=value.get('service-longctx-v1-result.json',{})
        STATE['service_phase']=q.get('phase');save()
        if target=='serving' and q.get('phase')=='serving' and value['exact_supervisor_alive']:return
        if target=='stopped' and q.get('phase')=='stopped' and q.get('api_reaped') and not value['exact_supervisor_alive']:return
        if q.get('phase')=='failed' or target=='serving' and not value['exact_supervisor_alive']:raise RuntimeError('exact matrix model failed/disappeared; inspect same handle')
        time.sleep(15)
    raise RuntimeError('matrix model observation budget exceeded; inspect same handle without relaunch')


try:
    for profile in ['off','on','off_repeat']:
        STATE.update(phase='loading_profile',profile=profile);save()
        path=BASE/('xolotl-admission-service-v101-'+profile+'-intent.json')
        run('launch_admission_seed_service_v101.frozen.py',profile)
        wait_model(profile,'serving',420)
        STATE['phase']='diagnostic_requests';save();run('manage_admission_seed_matrix_v102.frozen.py',profile,'launch_client')
        end=time.monotonic()+480
        while time.monotonic()<end:
            run('manage_admission_seed_matrix_v102.frozen.py',profile,'observe_client')
            v=json.loads((BASE/('xolotl-admission-client-v102-'+profile+'-latest.json')).read_text());q=v.get('result',{})
            STATE.update(client_phase=q.get('phase'),client_tests=len(q.get('tests',[])));save()
            if not v['exact_client_alive']:
                if not q.get('diagnostic_completed') or len(q['tests'])!=10:raise RuntimeError('exact diagnostic stopped without all10 calls: '+str(q.get('error')))
                STATE['profiles'].append(dict(profile=profile,phase=q['phase'],diagnostic_completed=True,tests=10,
                    qualified_reference_gate_passed=q['qualified_reference_gate_passed'],profile_serial_repeat_passed=q['profile_serial_repeat_passed'],
                    profile_concurrent_reference_passed=q['profile_concurrent_reference_passed'],profile_post_serial_passed=q['profile_post_serial_passed'],pairs=q['pairs']))
                save();break
            time.sleep(20)
        else:raise RuntimeError('diagnostic observation budget exceeded; inspect same handle')
        STATE['phase']='draining_completed_profile';save();run('manage_admission_seed_matrix_v102.frozen.py',profile,'drain');wait_model(profile,'stopped',120)
    STATE.update(phase='diagnostic_matrix_completed',finished_unix=time.time());save()
except BaseException as error:
    STATE.update(phase='requires_inspection',error=type(error).__name__+': '+str(error),finished_unix=time.time());save();raise
