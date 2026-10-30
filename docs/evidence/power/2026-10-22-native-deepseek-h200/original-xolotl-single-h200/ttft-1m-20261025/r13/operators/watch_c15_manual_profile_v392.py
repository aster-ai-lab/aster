"""Finite continuation only after the existing exact comparison and all drains complete."""
import json,os,subprocess,sys,time,urllib.request
from pathlib import Path
from aster_observer_retry_v236r1 import run_json,observe_with_retries
from xolotl_trial_common import BASE,observe
from xolotl_serving_common import observe as formal_observe
PATH=BASE/'xolotl-manual-profile-watch-v392.json'
if PATH.exists():raise RuntimeError('exact finite watch has a handle; inspect it')
S=dict(unix=time.time(),phase='waiting_for_existing_comparison375',local_pid=os.getpid(),local_namespace='Windows',formal_changed=False,new_resource_requested=False,goal_complete=False)
def save():
 t=PATH.with_suffix('.tmp');t.write_text(json.dumps(S,indent=2)+'\n');t.replace(PATH)
def read(n):return json.loads((BASE/n).read_text())
def retries(label,n,e):
 rows=S.setdefault('observer_retries',[])
 if len(rows)>=48:raise RuntimeError('finite profiler observer retry journal exceeded')
 rows.append(dict(operation=label,attempt=n,error_type=type(e).__name__,error=str(e)[:400]));save()
def run(name,*args,read_only=False):
 return run_json([sys.executable,'-B','-X','utf8',str(BASE/name),*args],read_only=read_only,runner=lambda cmd:subprocess.run(cmd,cwd=BASE,capture_output=True,text=True,timeout=200),record=lambda n,e:retries(name,n,e))
def formal_health():
 o=observe_with_retries(formal_observe,read_only=True,record=lambda n,e:retries('formal',n,e))
 if o['job_uid']!='bd638591-0ba6-4c60-9eb6-129a531714ee' or o['phase']!='Running':raise RuntimeError('exact formal C14 must remain running')
 with urllib.request.build_opener(urllib.request.ProxyHandler({})).open('http://10.102.93.68/health',timeout=5) as r:h=json.load(r)
 if not h.get('loaded') or h.get('max_context')!=1048576:raise RuntimeError('formal Windows health not ready')
 S['formal_health_checked_unix']=time.time();save()
try:
 save();lease_end=read('xolotl-manual-nsys-actual1m-v391-plan.json')['lease_ends_unix']
 while time.time()<lease_end-3100:
  c=read('xolotl-graph-native-model-v375-controller.json')
  S.update(comparison_phase=c['phase'],comparison_profile=c.get('profile'),observed_unix=time.time());save()
  if c['phase']=='requires_inspection':raise RuntimeError('comparison375 requires inspection; no new profiler work launched')
  if c['phase']=='comparison_completed':break
  time.sleep(45)
 else:raise RuntimeError('finite wait reached exact C15 lease reserve; no GPU work launched')
 if c['completed_profiles']!=['control_a','guard','control_b'] or not c['same_full_1024_output_across_profiles']:raise RuntimeError('unchanged three-profile C1 controls incomplete')
 formal_health();o=observe()
 if o['job_uid']!='5bc306b2-e9e1-4e25-bd05-d78eca2b4833' or o['phase']!='Running':raise RuntimeError('exact C15 role must remain running')
 if time.time()+3100>lease_end:raise RuntimeError('fresh physical-GPU smoke and actual1M diagnostic lease insufficient')
 S['phase']='tiny_GPU_smoke';save()
 if not (BASE/'xolotl-interactive-nsys-smoke-v390-intent.json').exists():run('manage_c15_nsys_smoke_v390.py','launch')
 until=time.monotonic()+240
 while time.monotonic()<until:
  q=run('manage_c15_nsys_smoke_v390.py','observe',read_only=True);S['smoke']=q;save()
  if not q['alive'] and q['phase'] in ['passed','failed']:
   if q['phase']!='passed' or not q['target_reaped'] or not q['GPU_idle_after']:raise RuntimeError('fresh C15 profiler smoke failed')
   break
  time.sleep(15)
 else:raise RuntimeError('fresh tiny smoke observer elapsed; preserve exact worker')
 formal_health();S['phase']='launch_actual1M';save()
 if not (BASE/'xolotl-manual-nsys-actual1m-v391-intent.json').exists():run('manage_c15_manual_actual1m_v391.py','launch')
 S['phase']='observe_actual1M';save();until=time.monotonic()+2720
 while time.monotonic()<until:
  q=run('manage_c15_manual_actual1m_v391.py','observe',read_only=True);S['diagnostic']=q;S['observed_unix']=time.time();save()
  if not q['alive'] and q['phase'] in ['complete','failed']:
   if not q['native_gone'] or not q['engine_reaped']:raise RuntimeError('actual1M diagnostic cleanup unproved')
   S.update(phase='diagnostic_finished',diagnostic_phase=q['phase'],finished_unix=time.time());save();print(json.dumps(S,indent=2));raise SystemExit
  time.sleep(45)
 raise RuntimeError('finite actual1M observer elapsed; preserve exact worker')
except BaseException as e:
 if isinstance(e,SystemExit):raise
 S.update(phase='requires_inspection',error=type(e).__name__+': '+str(e)[:2000],finished_unix=time.time());save();raise
