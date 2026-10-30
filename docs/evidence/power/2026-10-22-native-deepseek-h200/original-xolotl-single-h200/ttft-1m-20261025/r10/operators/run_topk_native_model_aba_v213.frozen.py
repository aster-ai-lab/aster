"""Finite same-GPU native flag0 -> flag1 -> flag0 API/model comparison. No promotion."""
import json
from pathlib import Path
import subprocess
import sys
import time
import urllib.request
from xolotl_serving_common import observe as observe_formal

B=Path(__file__).resolve().parent
PATH=B/'xolotl-topk-native-model-v213-controller.json'
if PATH.exists():raise RuntimeError('exact model comparison has a handle; inspect it')
STATE=dict(unix=time.time(),phase='starting',profiles=['control_a','topk','control_b'],maximum_task_H200=2,
    formal_service_changed=False,candidate_promoted=False,goal_complete=False,model_speed_claimed=False,
    native_binary_sha256='2bc83dcc3d3993ae7473b4e289454672f04190f21924e672de8dd53e2aff0195',
    flag_modes=['0','1','0'],ordinary_requests_per_profile=40,fullwindow_calls_per_profile=20,
    actual1m_calls_per_profile=18,output_tokens=1024,acceptance_changed=False,completed_profiles=[])
LAST_FORMAL=0
def save():
    for attempt in range(12):
        try:
            tmp=PATH.with_suffix('.tmp');tmp.write_text(json.dumps(STATE,indent=2)+'\n');tmp.replace(PATH);return
        except PermissionError:
            if attempt==11:raise
            time.sleep((attempt+1)*.1)
def read(name):return json.loads((B/name).read_text())
def run(name,*args):
    q=subprocess.run([sys.executable,'-X','utf8',str(B/name),*args],cwd=B,capture_output=True,text=True,timeout=200)
    if q.returncode:raise RuntimeError(name+' failed: '+q.stderr[-3000:])
    if len(q.stdout)>2*1024**2:raise RuntimeError('bounded model comparison observer output exceeded')
    return json.loads(q.stdout)
def formal_health(force=False):
    global LAST_FORMAL
    if not force and time.monotonic()-LAST_FORMAL<60:return
    q=observe_formal()
    if q['job_uid']!='1cfa236c-40ba-408f-a45e-003e0261bdf5' or q['phase']!='Running':raise RuntimeError('independent exact formal C11 changed; inspect before more experimental work')
    with urllib.request.build_opener(urllib.request.ProxyHandler({})).open('http://10.102.93.68/health',timeout=5) as response:
        raw=response.read(65537)
        if response.status!=200 or len(raw)>65536:raise RuntimeError('formal direct Windows health failed')
        health=json.loads(raw)
    if health.get('loaded') is not True or health.get('max_context')!=1048576:raise RuntimeError('independent formal model/context is not ready')
    STATE.update(formal_UID=q['job_uid'],formal_health_status=200,formal_health_checked_unix=time.time());LAST_FORMAL=time.monotonic();save()
def service(profile,terminal=False):
    start=time.monotonic()
    while time.monotonic()-start<720:
        formal_health();v=run('manage_topk_native_basic_v212.frozen.py',profile,'observe');q=v.get('service-longctx-v1-result.json',{})
        STATE.update(service_phase=q.get('phase'),service_supervisor_alive=v.get('exact_supervisor_alive'),observed_unix=time.time());save()
        if terminal:
            if q.get('phase')=='stopped' and q.get('api_reaped') and not v.get('exact_supervisor_alive'):return
        elif q.get('phase')=='serving':return
        elif q.get('phase')=='failed':raise RuntimeError('exact experimental service startup failed: '+str(q.get('error')))
        time.sleep(15)
    raise RuntimeError('service observation elapsed; preserve exact handle')
def qualify(profile,name,intent,launch,observation,expected):
    if not (B/intent).exists():run(name,profile,launch)
    start=time.monotonic()
    while time.monotonic()-start<1500:
        formal_health();row=run(name,profile,observation)
        STATE.update(client_phase=row.get('phase'),client_tests=row.get('tests'),client_alive=row.get('exact_client_alive'),observed_unix=time.time());save()
        print(json.dumps(dict(profile=profile,phase=row.get('phase'),tests=row.get('tests'),alive=row.get('exact_client_alive'))),flush=True)
        if row.get('phase') in ['passed','failed'] and not row.get('exact_client_alive'):
            if row['phase']!='passed' or row['tests']!=expected:raise RuntimeError('unchanged '+str(expected)+' model request acceptance failed')
            return
        time.sleep(20)
    raise RuntimeError('finite ordinary comparison observation elapsed; preserve client')
def fullwindow(profile):
    manager='manage_topk_fullwindow_suite_v214.py'
    if not (B/('xolotl-topk-fullwindow-v214-'+profile+'-intent.json')).exists():run(manager,'launch','--profile',profile)
    start=time.monotonic()
    while time.monotonic()-start<7250:
        formal_health();row=run(manager,'observe','--profile',profile)
        STATE.update(suite_phase=row['phase'],suite_stage=row['current_stage'],suite_alive=row['exact_client_alive'],suite_error=row.get('error'),suite_children=row['children'],observed_unix=time.time());save()
        print(json.dumps(dict(profile=profile,phase=row['phase'],stage=row['current_stage'],alive=row['exact_client_alive'],children=row['children'])),flush=True)
        if row['phase'] in ['passed','failed'] and not row['exact_client_alive']:
            p=read('xolotl-topk-fullwindow-v214-'+profile+'-latest.json');q=p.get('fullwindow/request-result.json',{})
            if not q.get('full_1m_length_qualified') or not q.get('full1m_cache_restore_passed') or not q.get('exact_restored_long_output_matched') or len(q.get('tests',[]))!=20:raise RuntimeError('all20 real1M cold/restore/c1/c2 structural gates required')
            if row['phase']=='failed' and q.get('error')!='ValueError: full-window long-output identity differs':raise RuntimeError('fullwindow failure requires inspection')
            actual=[t for t in q['tests'] if t['name'].startswith(('cold-full1m','restored-full1m','c1-','c2-'))]
            if len(actual)!=18 or not all(t['status']==200 and t['done'] and t['finish_reason']=='length' and t['usage']['prompt_tokens']==1047544 and t['usage']['completion_tokens']==1024 for t in actual):raise RuntimeError('exact18 actual1M/1024 completion proof differs')
            STATE[profile+'_full_output_identity_passed']=row['phase']=='passed';save();return q
        time.sleep(30)
    raise RuntimeError('finite fullwindow observation elapsed; preserve same exact worker')

try:
    save();summaries={}
    for profile in STATE['profiles']:
        formal_health(force=True)
        if time.time()+2400>1792921721.8908198:raise RuntimeError('formal C11 lease renewal review is due before another long comparison profile')
        STATE.update(profile=profile,phase='service_start');save()
        if not (B/('xolotl-topk-native-v212-'+profile+'-intent.json')).exists():run('launch_topk_native_service_v212.frozen.py',profile)
        service(profile)
        STATE['phase']='basic28';save()
        qualify(profile,'manage_topk_native_basic_v212.frozen.py','xolotl-topk-native-v212-'+profile+'-qualification-intent.json','launch_client','observe_client',28)
        STATE['phase']='tool_long12';save()
        qualify(profile,'manage_topk_native_extended_v212.frozen.py','xolotl-topk-native-extended-v212-'+profile+'-intent.json','launch','observe',12)
        STATE['phase']='actual1m_fullwindow';save();q=fullwindow(profile)
        c1=[r for r in q['levels'] if r['concurrency']==1]
        if len(c1)!=1:raise RuntimeError('qualified same-input sequential full1M level absent')
        summaries[profile]=dict(cold_output_sha256=q['cold_output_sha256'],c1=c1[0],full_output_identity_passed=STATE[profile+'_full_output_identity_passed'])
        STATE['summaries']=summaries;STATE['completed_profiles'].append(profile);STATE['phase']='draining_profile';save()
        if not (B/('xolotl-topk-native-v212-'+profile+'-drain.json')).exists():run('manage_topk_native_basic_v212.frozen.py',profile,'drain')
        service(profile,terminal=True)
    hashes={row['cold_output_sha256'] for row in summaries.values()}
    same_output=len(hashes)==1 and next(iter(hashes))=='7f9b16debd8c73a488bdf1ef1ca517afee6c9e27581e6ca2964b7ef86422e7cf'
    candidate=summaries['topk']['c1']['aggregate_end_to_end_completion_tok_s']
    changes={profile:100*(candidate/summaries[profile]['c1']['aggregate_end_to_end_completion_tok_s']-1) for profile in ['control_a','control_b']}
    STATE.update(phase='comparison_completed',same_full_1024_output_across_profiles=same_output,
        candidate_C1_change_percent=changes,C1_material_improvement_qualified=same_output and all(value>=5 for value in changes.values()),
        complete_acceptance_passed=same_output and all(row['full_output_identity_passed'] for row in summaries.values()),finished_unix=time.time())
    save();print(json.dumps(dict(phase=STATE['phase'],changes=changes,same_output=same_output,formal_changed=False,goal_complete=False)),flush=True)
except BaseException as error:
    STATE.update(phase='requires_inspection',error=type(error).__name__+': '+str(error)[:4000],finished_unix=time.time());save();raise
