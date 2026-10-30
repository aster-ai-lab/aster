"""Wait for the exact existing comparison, then launch one bounded isolated native trace."""
import json
import subprocess
import sys
import time
from xolotl_trial_common import BASE

PATH=BASE/'xolotl-critical-path-v122-controller.json'
STATE=dict(unix=time.time(),phase='waiting_existing_matrix',formal_service_unchanged=True,
           actual1m_input_executed=False,performance_300_qualified=False,goal_complete=False)
with PATH.open('x') as output:output.write(json.dumps(STATE,indent=2)+'\n')

def save():
    STATE['observed_unix']=time.time();p=PATH.with_suffix('.tmp');p.write_text(json.dumps(STATE,indent=2)+'\n')
    for i in range(12):
        try:p.replace(PATH);return
        except PermissionError:
            if i==11:raise
            time.sleep(.1*(i+1))

def run(name,seconds=100):
    p=subprocess.run([sys.executable,'-X','utf8',str(BASE/name)],cwd=BASE,capture_output=True,text=True,timeout=seconds)
    if p.returncode:raise RuntimeError(name+' failed: '+p.stderr[-4000:])
    return json.loads(p.stdout)

try:
    deadline=time.monotonic()+1500;last=None
    while time.monotonic()<deadline:
        q=json.loads((BASE/'xolotl-pool-seed-v112r2-controller.json').read_text())
        if q['phase']=='requires_inspection':raise RuntimeError('exact existing matrix needs inspection: '+q['error'])
        row=(q['phase'],q.get('profile'),q.get('client_tests'))
        if row!=last:
            print(json.dumps(dict(waiting_matrix_phase=row[0],profile=row[1],tests=row[2])),flush=True);last=row
        STATE.update(matrix_phase=q['phase'],matrix_profile=q.get('profile'),matrix_tests=q.get('client_tests'));save()
        if q['phase']=='comparison_completed':break
        time.sleep(15)
    else:raise RuntimeError('finite matrix observation exceeded; inspect same handles')
    STATE['phase']='launching_native_trace';save()
    if not (BASE/'xolotl-native-profile-v117-intent.json').exists():run('launch_native_profile_v117r1.frozen.py',180)
    STATE['phase']='observing_native_trace';save();deadline=time.monotonic()+1350;last=None
    while time.monotonic()<deadline:
        q=run('observe_native_profile_v117.frozen.py');row=(q.get('phase'),q.get('requests'),q.get('alive'))
        STATE.update(trace_phase=row[0],trace_requests=row[1],trace_worker_alive=row[2]);save()
        if row!=last:print(json.dumps(dict(trace_phase=row[0],requests=row[1],worker_alive=row[2])),flush=True);last=row
        if not q.get('alive'):
            if q.get('phase')!='complete' or not q.get('engine_reaped') or not q.get('native_target_gone') or not q.get('trace'):
                raise RuntimeError('current native trace failed or is incomplete: '+str(q.get('error')))
            STATE.update(phase='trace_collected',trace=q['trace'],finished_unix=time.time());save();break
        time.sleep(15)
    else:raise RuntimeError('finite profiler observation exceeded; inspect exact handle')
except BaseException as error:
    STATE.update(phase='requires_inspection',error=type(error).__name__+': '+str(error),finished_unix=time.time());save();raise
