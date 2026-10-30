"""Read-only terminal resource proof and one Windows API smoke request."""
import json,time,urllib.request
from pathlib import Path
from xolotl_trial_common import BASE,jump

def read(n):return json.loads((BASE/n).read_text(encoding='utf-8-sig'))
opt=read('xolotl-graph-native-v373-guard-intent.json');formal=read('xolotl-stable-service-v368-intent.json')
record=read('xolotl-user-stop-optimization-v404-resource.json')
if record['phase']!='stop_command_returned':raise RuntimeError('Inspect exact stop receipt first')
code='''import json,shlex,subprocess,time
opt=@OPT@;formal=@FORMAL@
def cli(args):
 p=subprocess.run(['bash','-lc','. /etc/profile.d/ssh-init.sh >/dev/null 2>&1; exec '+shlex.join(['brainctl']+args)],capture_output=True,text=True,check=True,timeout=20)
 if len(p.stdout)>1048576:raise RuntimeError('Bounded resource proof exceeded')
 return json.loads(p.stdout)
out=dict(unix=time.time(),roles={})
for label,r in [('optimization',opt),('formal',formal)]:
 rows={}
 for kind,name,uid in [('rjob',r['allocation_job'],r['allocation_uid']),('replica',r['replica'],r['replica_uid'])]:
  v=cli(['get',kind,name,'-n','ailab-hrprdduanyan','-o','json'])
  if v['metadata']['uid']!=uid:raise RuntimeError('Fresh exact role UID differs')
  rows[kind]=dict(name=name,uid=uid,phase=v['status']['phase'])
 expected='Stopped' if label=='optimization' else 'Running'
 if any(row['phase']!=expected for row in rows.values()):raise RuntimeError('Resource has not reached required phase: '+label+' '+str(rows))
 out['roles'][label]=rows
service=cli(['get','service','aster-xolotl-api-v3pool-20261024','-n','ailab-hrprdduanyan','-o','json'])
if service['metadata']['uid']!='1f968a8d-4ddc-4cee-b32a-390cd44210c7' or service['metadata']['annotations']['aster.serving/allocation-uid']!=formal['allocation_uid'] or service['spec']['selector']!={'rjob.brainpp.cn/rjob-name':formal['allocation_job']}:raise RuntimeError('Formal VIP selector differs')
out.update(optimization_released=True,formal_VIP_unchanged=True)
print(json.dumps(out))
'''.replace('@OPT@',repr(opt)).replace('@FORMAL@',repr(formal))
resources=jump(code,90)
key=(BASE/'access-private-xolotl-9c996c5a434b/api-key.txt').read_text().strip()
if not key:raise RuntimeError('Existing local credential is missing')
opener=urllib.request.build_opener(urllib.request.ProxyHandler({}));headers={'Authorization':'Bearer '+key,'Content-Type':'application/json'}
def get(path):
 req=urllib.request.Request('http://10.102.93.68'+path,headers=headers)
 with opener.open(req,timeout=8) as r:
  raw=r.read(65537)
  if r.status!=200 or len(raw)>65536:raise RuntimeError('Bounded HTTP proof failed')
  return json.loads(raw)
health=get('/health');models=get('/v1/models')
model='deepseek-v4.1-flash-1017-xq'
if not health.get('loaded') or health.get('max_context')!=1048576 or model not in [m['id'] for m in models['data']]:raise RuntimeError('Loaded model/context differs')
payload=dict(model=model,messages=[dict(role='user',content='Reply with exactly READY.')],temperature=0,max_tokens=32,stream=True)
req=urllib.request.Request('http://10.102.93.68/v1/chat/completions',headers=headers,data=json.dumps(payload).encode(),method='POST')
started=time.perf_counter();first=None;content='';done=False;finish=None;count=0;usage=None
with opener.open(req,timeout=45) as response:
 if response.status!=200:raise RuntimeError('Authenticated generation failed')
 for line in response:
  count+=1
  if count>4096 or len(line)>65536 or time.perf_counter()-started>60:raise RuntimeError('SSE proof exceeded bound')
  if not line.startswith(b'data: '):continue
  raw=line[6:].strip()
  if raw==b'[DONE]':done=True;break
  row=json.loads(raw)
  if row.get('usage'):usage=row['usage']
  for choice in row.get('choices',[]):
   chunk=choice.get('delta',{}).get('content') or ''
   if chunk and first is None:first=time.perf_counter()-started
   content+=chunk
   if len(content)>8192:raise RuntimeError('Smoke reply length bound exceeded')
   finish=choice.get('finish_reason') or finish
elapsed=time.perf_counter()-started
if not done or finish!='stop' or not content.strip():raise RuntimeError('Complete nonempty streaming response required')
out=dict(unix=time.time(),phase='passed',reason='User cancelled further optimization and retained deployed service',resources=resources,
 base_url='http://10.102.93.68/v1',model=model,context_tokens=1048576,same_existing_key_authenticated=True,
 Windows_direct_API_passed=True,Windows_forwarding_required=False,health_status=200,models_status=200,
 smoke=dict(done=done,finish_reason=finish,reply=content[:160],ttft_seconds=first,seconds=elapsed,usage=usage),
 optimization_controllers_stopped=True,queued_profiler_cancelled=True,optimization_H200_released=True,remaining_active_H200_roles=1,
 performance_optimization_cancelled_by_user=True,speed300_met=False,speed1000_met=False,goal_complete=False,
 formal_lease_ends_unix=formal['plan']['lease_ends_unix'])
name='xolotl-user-retain-deployment-v405-verification.json'
with (BASE/name).open('x',encoding='utf-8') as f:f.write(json.dumps(out,ensure_ascii=False,indent=2)+'\n')
print(json.dumps(dict(phase='passed',optimization_released=True,formal_running=True,Windows_API_passed=True,smoke=out['smoke']),ensure_ascii=False))
