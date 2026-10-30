"""Finite same-GPU native flag0 -> flag1 -> flag0 API/model comparison. No promotion."""
import json
import os
from aster_observer_retry_v236r1 import observe_with_retries,run_json
from pathlib import Path
import subprocess
import sys
import time
import urllib.request
from xolotl_serving_common import observe as observe_formal

B=Path(__file__).resolve().parent
PATH=B/'xolotl-graph-native-model-v375-controller.json'
if PATH.exists():raise RuntimeError('exact model comparison has a handle; inspect it')
STATE=dict(unix=time.time(),phase='starting',profiles=['control_a','guard','control_b'],maximum_task_H200=2,
    formal_service_changed=False,candidate_promoted=False,goal_complete=False,model_speed_claimed=False,
    native_binary_sha256='b7124c78d711d3f48b928a292bd6423af4137816533e0acfbeb5114644508185',
    flag_modes=['0','1','0'],ordinary_requests_per_profile=40,fullwindow_calls_per_profile=20,
    actual1m_calls_per_profile=18,output_tokens=1024,acceptance_changed=False,completed_profiles=[],local_pid=os.getpid(),local_namespace='Windows',graph_source_type_modes=['0','1','0'],TopK_mode_constant=1)
LAST_FORMAL=0
def save():
    for attempt in range(12):
        try:
            tmp=PATH.with_suffix('.tmp');tmp.write_text(json.dumps(STATE,indent=2)+'\n');tmp.replace(PATH);return
        except PermissionError:
            if attempt==11:raise
            time.sleep((attempt+1)*.1)
def read(name):return json.loads((B/name).read_text())
def retry_record(label,ordinal,error):
    rows=STATE.setdefault('observer_retries',[])
    if len(rows)>=64:raise RuntimeError('bounded observer retry journal exceeded')
    rows.append(dict(unix=time.time(),operation=label,attempt=ordinal,error_type=type(error).__name__,error=str(error)[:600]));save()

def run(name,*args,read_only=False):
    return run_json([sys.executable,'-B','-X','utf8',str(B/name),*args],read_only=read_only,
        runner=lambda command:subprocess.run(command,cwd=B,capture_output=True,text=True,timeout=200),
        record=lambda n,e:retry_record(name,n,e))

def formal_health(force=False):
    global LAST_FORMAL
    if not force and time.monotonic()-LAST_FORMAL<60:return
    q=observe_with_retries(observe_formal,read_only=True,record=lambda n,e:retry_record("formal-resource",n,e))
    if q['job_uid']!='bd638591-0ba6-4c60-9eb6-129a531714ee' or q['phase']!='Running':raise RuntimeError('independent exact formal C14 changed; inspect before more experimental work')
    with urllib.request.build_opener(urllib.request.ProxyHandler({})).open('http://10.102.93.68/health',timeout=5) as response:
        raw=response.read(65537)
        if response.status!=200 or len(raw)>65536:raise RuntimeError('formal direct Windows health failed')
        health=json.loads(raw)
    if health.get('loaded') is not True or health.get('max_context')!=1048576:raise RuntimeError('independent formal model/context is not ready')
    STATE.update(formal_UID=q['job_uid'],formal_health_status=200,formal_health_checked_unix=time.time());LAST_FORMAL=time.monotonic();save()
def service(profile,terminal=False):
    start=time.monotonic()
    while time.monotonic()-start<720:
        formal_health();v=run('manage_graph_native_basic_v373.frozen.py',profile,'observe',read_only=True);q=v.get('service-longctx-v1-result.json',{})
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
        formal_health();row=run(name,profile,observation,read_only=True)
        STATE.update(client_phase=row.get('phase'),client_tests=row.get('tests'),client_alive=row.get('exact_client_alive'),observed_unix=time.time());save()
        print(json.dumps(dict(profile=profile,phase=row.get('phase'),tests=row.get('tests'),alive=row.get('exact_client_alive'))),flush=True)
        if row.get('phase') in ['passed','failed'] and not row.get('exact_client_alive'):
            if row['phase']!='passed' or row['tests']!=expected:raise RuntimeError('unchanged '+str(expected)+' model request acceptance failed')
            STATE.setdefault('ordinary_results',{}).setdefault(profile,{})[str(expected)]=row;save()
            return
        time.sleep(20)
    raise RuntimeError('finite ordinary comparison observation elapsed; preserve client')
def fullwindow(profile):
    manager='manage_graph_fullwindow_suite_v374.py'
    if not (B/('xolotl-graph-fullwindow-v374-'+profile+'-intent.json')).exists():run(manager,'launch','--profile',profile)
    start=time.monotonic()
    while time.monotonic()-start<7250:
        formal_health();row=run(manager,'observe','--profile',profile,read_only=True)
        STATE.update(suite_phase=row['phase'],suite_stage=row['current_stage'],suite_alive=row['exact_client_alive'],suite_error=row.get('error'),suite_children=row['children'],observed_unix=time.time());save()
        print(json.dumps(dict(profile=profile,phase=row['phase'],stage=row['current_stage'],alive=row['exact_client_alive'],children=row['children'])),flush=True)
        if row['phase'] in ['passed','failed'] and not row['exact_client_alive']:
            p=read('xolotl-graph-fullwindow-v374-'+profile+'-latest.json');q=p.get('fullwindow/request-result.json',{})
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
        if time.time()+2400>1792952301.9907353:raise RuntimeError('formal C14 lease renewal review is due before another long comparison profile')
        STATE.update(profile=profile,phase='service_start');save()
        if not (B/('xolotl-graph-native-v373-'+profile+'-intent.json')).exists():run('launch_graph_native_service_v373.frozen.py',profile)
        service(profile)
        STATE['phase']='basic28';save()
        qualify(profile,'manage_graph_native_basic_v373.frozen.py','xolotl-graph-native-v373-'+profile+'-qualification-intent.json','launch_client','observe_client',28)
        STATE['phase']='tool_long12';save()
        qualify(profile,'manage_graph_native_extended_v373.frozen.py','xolotl-graph-native-extended-v373-'+profile+'-intent.json','launch','observe',12)
        STATE['phase']='actual1m_fullwindow';save();q=fullwindow(profile)
        c1=[r for r in q['levels'] if r['concurrency']==1]
        if len(c1)!=1:raise RuntimeError('qualified same-input sequential full1M level absent')
        summaries[profile]=dict(cold_output_sha256=q['cold_output_sha256'],c1=c1[0],full_output_identity_passed=STATE[profile+'_full_output_identity_passed'])
        STATE['summaries']=summaries;STATE['completed_profiles'].append(profile);STATE['phase']='draining_profile';save()
        if not (B/('xolotl-graph-native-v373-'+profile+'-drain.json')).exists():run('manage_graph_native_basic_v373.frozen.py',profile,'drain')
        service(profile,terminal=True)
    hashes={row['cold_output_sha256'] for row in summaries.values()}
    same_output=len(hashes)==1 and next(iter(hashes))=='7f9b16debd8c73a488bdf1ef1ca517afee6c9e27581e6ca2964b7ef86422e7cf'
    candidate=summaries['guard']['c1']['aggregate_end_to_end_completion_tok_s']
    changes={profile:100*(candidate/summaries[profile]['c1']['aggregate_end_to_end_completion_tok_s']-1) for profile in ['control_a','control_b']}
    STATE.update(phase='comparison_completed',same_full_1024_output_across_profiles=same_output,
        candidate_C1_change_percent=changes,C1_material_improvement_qualified=same_output and all(value>=5 for value in changes.values()),
        complete_acceptance_passed=same_output and all(row['full_output_identity_passed'] for row in summaries.values()),finished_unix=time.time())
    save();print(json.dumps(dict(phase=STATE['phase'],changes=changes,same_output=same_output,formal_changed=False,goal_complete=False)),flush=True)
except BaseException as error:
    STATE.update(phase='requires_inspection',error=type(error).__name__+': '+str(error)[:4000],finished_unix=time.time());save();raise
