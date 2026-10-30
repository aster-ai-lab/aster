"""Sequential matched trace, CPU summary, fresh service gates and exact unique-source1M suite."""
import json
import subprocess
import sys
import time
from xolotl_trial_common import BASE

PATH=BASE/'xolotl-matched-profile-code1m-v127-controller.json'
STATE=dict(unix=time.time(),phase='observing_existing_batch2_trace',formal_service_unchanged=True,
           actual1m_input_executed=False,performance_300_qualified=False,goal_complete=False)
with PATH.open('x') as output:output.write(json.dumps(STATE,indent=2)+'\n')

def save():
    STATE['observed_unix']=time.time();p=PATH.with_suffix('.tmp');p.write_text(json.dumps(STATE,indent=2)+'\n')
    for i in range(12):
        try:p.replace(PATH);return
        except PermissionError:
            if i==11:raise
            time.sleep(.1*(i+1))

def run(script,*args,limit=100):
    p=subprocess.run([sys.executable,'-X','utf8',str(BASE/script),*args],cwd=BASE,capture_output=True,text=True,timeout=limit)
    if p.returncode:raise RuntimeError(script+' '+str(args)+' failed: '+p.stderr[-4000:])
    return json.loads(p.stdout)

def wait_client(script,profile,action,seconds,count):
    deadline=time.monotonic()+seconds;last=None
    while time.monotonic()<deadline:
        q=run(script,profile,action)
        row=(q.get('phase'),q.get('tests'),q.get('exact_client_alive'))
        STATE.update(client_phase=row[0],client_tests=row[1],exact_client_alive=row[2]);save()
        if row!=last:print(json.dumps(dict(stage=STATE['phase'],phase=row[0],tests=row[1],alive=row[2])),flush=True);last=row
        if not q.get('exact_client_alive'):
            if q.get('phase')!='passed' or q.get('tests')!=count:raise RuntimeError('fresh service acceptance failed: '+str(q.get('error')))
            return q
        time.sleep(20)
    raise RuntimeError('finite client observation exceeded; inspect exact existing client')

try:
    deadline=time.monotonic()+1350;last=None
    while time.monotonic()<deadline:
        q=run('observe_native_profile_v125.frozen.py');row=(q.get('phase'),q.get('requests'),q.get('alive'))
        STATE.update(trace_phase=row[0],trace_requests=row[1],trace_worker_alive=row[2]);save()
        if row!=last:print(json.dumps(dict(trace_phase=row[0],requests=row[1],alive=row[2])),flush=True);last=row
        if not q.get('alive'):
            if q.get('phase')!='complete' or not q.get('engine_reaped') or not q.get('native_target_gone') or not q.get('trace'):
                raise RuntimeError('matched batch2 native trace failed: '+str(q.get('error')))
            break
        time.sleep(15)
    else:raise RuntimeError('finite matched trace observation exceeded; inspect exact handle')
    STATE['phase']='matched_trace_statistics';save()
    if not (BASE/'xolotl-native-statistics-v126-intent.json').exists():run('manage_profile_statistics_v126.frozen.py','launch')
    deadline=time.monotonic()+420;last=None
    while time.monotonic()<deadline:
        q=run('manage_profile_statistics_v126.frozen.py','observe');row=(q.get('phase'),q.get('alive'))
        STATE.update(statistics_phase=row[0],statistics_alive=row[1]);save()
        if row!=last:print(json.dumps(dict(statistics_phase=row[0],alive=row[1])),flush=True);last=row
        if not q.get('alive'):
            if q.get('phase')!='passed':raise RuntimeError('matched numeric trace aggregation failed: '+str(q.get('error')))
            STATE['matched_trace_summary_passed']=True;save();break
        time.sleep(10)
    else:raise RuntimeError('finite numeric trace observation exceeded; inspect exact CPU process')
    STATE['phase']='starting_fresh_code1m_service';save()
    if not (BASE/'xolotl-code-service-v118-full1m_code-intent.json').exists():run('launch_code_service_v118r1.frozen.py','full1m_code',limit=180)
    deadline=time.monotonic()+650;last=None
    while time.monotonic()<deadline:
        q=run('manage_code_basic_v118.frozen.py','full1m_code','observe');service=q.get('service-longctx-v1-result.json',{})
        STATE['service_phase']=service.get('phase');save()
        if service.get('phase')!=last:print(json.dumps(dict(service_phase=service.get('phase'))),flush=True);last=service.get('phase')
        if service.get('phase')=='serving' and q['exact_supervisor_alive']:break
        if service.get('phase')=='failed' or not q['exact_supervisor_alive']:raise RuntimeError('exact fresh service failed; inspect same handle')
        time.sleep(15)
    else:raise RuntimeError('finite service startup observation exceeded; inspect exact handle')
    STATE['phase']='fresh_basic28';save()
    if not (BASE/'xolotl-code-service-v118-full1m_code-qualification-intent.json').exists():run('manage_code_basic_v118.frozen.py','full1m_code','launch_client')
    wait_client('manage_code_basic_v118.frozen.py','full1m_code','observe_client',1150,28)
    STATE['phase']='fresh_native_tool_long12';save()
    if not (BASE/'xolotl-code-extended-v118-full1m_code-intent.json').exists():run('manage_code_extended_v118.frozen.py','full1m_code','launch')
    q=wait_client('manage_code_extended_v118.frozen.py','full1m_code','observe',1350,12)
    if not q.get('native_tool_roundtrip_passed') or not q.get('reference_matched'):raise RuntimeError('fresh native tool/complete1024-output references must pass')
    STATE['phase']='unique_code_actual1m_suite';save()
    if not (BASE/'xolotl-code-fullwindow-suite-v119-intent.json').exists():run('manage_code_fullwindow_suite_v119.frozen.py','launch')
    deadline=time.monotonic()+9200;last=None
    while time.monotonic()<deadline:
        q=run('manage_code_fullwindow_suite_v119.frozen.py','observe')
        row=(q.get('phase'),q.get('current_stage'),q.get('exact_client_alive'),q.get('children'))
        STATE.update(suite_phase=row[0],suite_stage=row[1],suite_client_alive=row[2],suite_children=row[3]);save()
        if row!=last:print(json.dumps(dict(suite_phase=row[0],stage=row[1],alive=row[2],children=row[3])),flush=True);last=row
        if not q.get('exact_client_alive'):
            if q.get('phase')!='passed':raise RuntimeError('unchanged actual1M code suite gate failed: '+str(q.get('error')))
            STATE.update(phase='actual1m_code_suite_passed',actual1m_input_executed=True,finished_unix=time.time());save();break
        time.sleep(30)
    else:raise RuntimeError('finite actual1M observation exceeded; inspect exact existing client')
except BaseException as error:
    STATE.update(phase='requires_inspection',error=type(error).__name__+': '+str(error),finished_unix=time.time());save();raise
