"""Retire only the exact completed idle optimization role, retaining GPFS artifacts."""
import json
import time
from xolotl_trial_common import BASE, observe, owned, jump
from xolotl_serving_common import observe as observe_formal

path=BASE/'xolotl-optimization-c10-retirement-v209.json'
if path.exists():raise RuntimeError('exact retirement has a handle; inspect it')
def read(name):return json.loads((BASE/name).read_text())
profile=read('xolotl-ncu-actual1m-v172r2-latest.json');p=profile['result.json']
operator=read('xolotl-topk-harness-v203-gpu-latest.json');o=operator['gpu-result.json']
smoke=read('xolotl-ncu-smoke-v205-latest.json');s=smoke['result.json']
bridge=read('xolotl-topk-native-bridge-v202-latest.json')
if profile['exact_worker_alive'] or p['phase']!='failed' or not p['engine_reaped'] or not p['native_target_gone'] or len(p['requests'])!=3 or operator['alive'] or o['phase']!='passed' or not o['engine_reaped'] or not o['gpu_idle_after'] or len(o['cases'])!=38 or smoke['alive'] or not s['engine_reaped'] or not s['gpu_idle_after'] or bridge['alive'] or bridge['result.json']['phase']!='compiled':raise RuntimeError('all exact optimization workers must be terminal/reaped')
formal=observe_formal();old=observe()
if formal['job_uid']!='1cfa236c-40ba-408f-a45e-003e0261bdf5' or formal['phase']!='Running' or old['job_uid']!='7455d989-8255-4f7c-ae8f-3cc0aa5928e4' or old['phase']!='Running':raise RuntimeError('exact independent formal/running old optimization identities differ')
idle=owned("import json,subprocess; p=subprocess.run(['nvidia-smi','--query-compute-apps=pid,gpu_uuid','--format=csv,noheader,nounits'],capture_output=True,text=True,check=True,timeout=8); print(json.dumps(dict(idle=not p.stdout.strip(),active=p.stdout)))")
if not idle['idle']:raise RuntimeError('optimization GPU is not idle; no retirement')
record=dict(unix=time.time(),phase='stop_requested',job='aster-xolotl-1mc10-d0ed8472c851-76441859',job_uid=old['job_uid'],replica='aster-xolotl-1mc10-d0ed8472c851-76441859-qmx9q',replica_uid='e96e8d68-2420-45a8-84f1-510b9f556f16',idle_proof=idle,formal_uid=formal['job_uid'],GPU_artifacts_retained_on_GPFS=True,reason='All finite profile/operator/CPU-build workers terminal and optimization GPU idle; release expiring C10 before requesting its one-GPU replacement. Formal C11 unchanged, maximum two H200.')
with path.open('x') as out:out.write(json.dumps(record,indent=2)+'\n')
code='''import json,shlex,subprocess,time
r=@R@
def cli(args):return subprocess.run(['bash','-lc','. /etc/profile.d/ssh-init.sh >/dev/null 2>&1; exec '+shlex.join(args)],capture_output=True,text=True,check=True,timeout=20)
def get(kind,name):
 raw=cli(['brainctl','get',kind,name,'-n','ailab-hrprdduanyan','-o','json']).stdout
 if len(raw)>1048576:raise RuntimeError('bounded exact retirement metadata exceeded')
 return json.loads(raw)
j=get('rjob',r['job']);p=get('replica',r['replica']);f=get('rjob','aster-xolotl-1mc11-6ef84ac5b528-78529266')
if j['metadata']['uid']!=r['job_uid'] or p['metadata']['uid']!=r['replica_uid'] or j['status']['phase']!='Running' or p['status']['phase']!='Running' or f['metadata']['uid']!=r['formal_uid'] or f['status']['phase']!='Running':raise RuntimeError('exact old/formal identities changed; no stop')
stop=cli(['rjob','stop','--namespace','ailab-hrprdduanyan',r['job']]);j=get('rjob',r['job']);p=get('replica',r['replica'])
if j['metadata']['uid']!=r['job_uid'] or p['metadata']['uid']!=r['replica_uid']:raise RuntimeError('exact stopped predecessor identities differ')
print(json.dumps(dict(unix=time.time(),job_phase=j['status']['phase'],replica_phase=p['status']['phase'],stop_stdout=stop.stdout)))
'''.replace('@R@',repr(record))
record['observed']=jump(code);record['phase']='stop_observed';path.write_text(json.dumps(record,indent=2)+'\n');print(json.dumps(record,indent=2))
