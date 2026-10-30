"""Finish exact API A/B, drain only idle C10, then run controlled actual1M hardware profiling."""
import json
from pathlib import Path
import subprocess
import sys
import time
import urllib.request
from xolotl_serving_common import observe as observe_formal

B=Path(__file__).resolve().parent
PATH=B/'xolotl-fullwindow-hardware-v173-controller.json'
if PATH.exists():raise RuntimeError('exact post-comparison profiler controller has a handle; inspect it')
STATE=dict(unix=time.time(),phase='waiting_exact_API_comparison',maximum_h200_roles=2,
    formal_service_changed=False,goal_complete=False,hardware_counters_collected=False)
DEADLINE=time.monotonic()+7200
last_health=0

def save():
    for attempt in range(12):
        try:
            p=PATH.with_suffix('.tmp');p.write_text(json.dumps(STATE,indent=2)+'\n');p.replace(PATH);return
        except PermissionError:
            if attempt==11:raise
            time.sleep((attempt+1)*.1)

def command(script,*args):
    result=subprocess.run([sys.executable,'-X','utf8',str(B/script),*args],cwd=B,capture_output=True,text=True,timeout=200)
    if result.returncode:raise RuntimeError(script+' failed: '+result.stderr[-4000:])
    if len(result.stdout)>2*1024**2:raise RuntimeError('bounded hardware observer output exceeded')
    return json.loads(result.stdout)

def formal_check(force=False):
    global last_health
    if not force and time.monotonic()-last_health<90:return
    r=observe_formal();current=json.loads((B/'xolotl-active-serving-reservation.json').read_text())
    if r['job_uid']!=current['allocation_uid']:raise RuntimeError('current formal reservation and live UID differ')
    STATE.update(formal_phase=r['phase'],formal_uid=r['job_uid'],formal_observed_unix=r['unix']);save()
    if r['phase']!='Running' or len(r['replicas'])!=1 or r['replicas'][0]['phase']!='Running':
        raise RuntimeError('current formal resource is not Running; preserve exact optimization handles and restore service')
    with urllib.request.build_opener(urllib.request.ProxyHandler({})).open('http://10.102.93.68/health',timeout=5) as response:
        raw=response.read(65537)
        if len(raw)>65536:raise RuntimeError('bounded formal health body exceeded')
        health=json.loads(raw)
    if response.status!=200 or not health.get('loaded') or health.get('max_context')!=1048576:
        raise RuntimeError('formal1M API health differs; inspect without restarting solely on this observation')
    STATE.update(formal_health=200,formal_health_checked_unix=time.time());save();last_health=time.monotonic()

def pause(seconds=20):
    if time.monotonic()>DEADLINE:raise RuntimeError('finite controller observation elapsed; keep exact remote handles')
    time.sleep(seconds)

try:
    save();formal_check(True);last=None
    while True:
        formal_check()
        c=json.loads((B/'xolotl-literal-api-v151-controller.json').read_text())
        current=(c['phase'],c.get('suite_phase'),c.get('children',{}).get('fullwindow/request-result.json',{}).get('tests'))
        STATE.update(api_comparison_phase=c['phase'],api_suite_phase=c.get('suite_phase'),api_tests=current[2],observed_unix=time.time());save()
        if current!=last:print(json.dumps(dict(stage=STATE['phase'],comparison=current)),flush=True);last=current
        if c['phase']=='comparison_completed':break
        if c['phase']=='requires_inspection':raise RuntimeError('exact API controller requires inspection; never replace or replay its requests')
        pause()
    row=command('manage_literal_fullwindow_suite_v150.py','observe','--profile','literal')
    q=json.loads((B/'xolotl-literal-fullwindow-v150-literal-latest.json').read_text())['fullwindow/request-result.json']
    if row['exact_client_alive'] or len(q['tests'])!=20 or not q.get('full1m_cache_restore_passed') or not q.get('exact_restored_long_output_matched'):
        raise RuntimeError('exact completed real1M cold/restore/c1/c2 structural proof required before profiler handoff')
    if q['phase']=='failed' and q.get('error')!='ValueError: full-window long-output identity differs':
        raise RuntimeError('unrecognized fullwindow failure; preserve client/model for inspection')
    STATE.update(phase='draining_only_idle_optimization',prior_C2_identity_passed=q['phase']=='passed');save()
    formal_check(True)
    if not (B/'xolotl-literal-service-v149-literal-drain.json').exists():command('manage_literal_basic_v149.frozen.py','literal','drain')
    start=time.monotonic()
    while time.monotonic()-start<600:
        formal_check();row=command('manage_literal_basic_v149.frozen.py','literal','observe')
        q=row.get('service-longctx-v1-result.json',{})
        STATE.update(prior_service_phase=q.get('phase'),prior_supervisor_alive=row.get('exact_supervisor_alive'));save()
        if q.get('phase')=='stopped' and q.get('api_reaped') and not row.get('exact_supervisor_alive'):break
        pause(10)
    else:raise RuntimeError('exact prior model drain observation elapsed; do not launch another model')
    STATE['phase']='controlled_native_link';save()
    if not (B/'xolotl-native-serve-profile-v168-intent.json').exists():command('manage_serve_profile_native_v168.py','launch')
    while True:
        formal_check();row=command('manage_serve_profile_native_v168.py','observe')
        STATE.update(link_phase=row['phase'],link_alive=row['alive']);save()
        if row['phase']=='linked' and not row['alive']:break
        if row['phase']=='failed':raise RuntimeError('exact native profiler link failed: '+str(row.get('error')))
        pause(10)
    STATE['phase']='actual1m_hardware_profile';save();formal_check(True)
    if not (B/'xolotl-ncu-actual1m-v172-intent.json').exists():command('manage_ncu_actual1m_v172.py','launch')
    last=None
    while True:
        formal_check();row=command('manage_ncu_actual1m_v172.py','observe')
        STATE.update(profiler_phase=row['phase'],profiler_alive=row['alive'],profiler_request=row['current'],profiler_state=row['state'],
            profiler_requests=row['requests'],hardware_counters_collected=row['hardware_counters'],observed_unix=time.time());save()
        current=(row['phase'],row['current'],row['state'],row['requests'])
        if current!=last:print(json.dumps(dict(stage=STATE['phase'],profile=row)),flush=True);last=current
        if row['phase'] in ['complete','failed'] and not row['alive']:
            if row['phase']!='complete' or not row['engine_reaped'] or not row['native_gone'] or not row['hardware_counters']:
                raise RuntimeError('actual1M hardware profile did not finish all exact gates: '+str(row.get('error')))
            STATE.update(phase='profile_completed',hot_kernel_families=row['families'],finished_unix=time.time());save();break
        pause(30)
    print(json.dumps(dict(phase=STATE['phase'],goal_complete=False,hardware_counters_collected=True)),flush=True)
except BaseException as error:
    STATE.update(phase='requires_inspection',error=type(error).__name__+': '+str(error)[:4000],finished_unix=time.time());save();raise
