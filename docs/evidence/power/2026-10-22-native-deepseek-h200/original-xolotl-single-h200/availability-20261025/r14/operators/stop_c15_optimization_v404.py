"""Honor user cancellation: stop exact C15 allocation, preserve C14 VIP."""
import json,time
from pathlib import Path
from xolotl_trial_common import BASE,jump

path=BASE/'xolotl-user-stop-optimization-v404-resource.json'
if path.exists():raise RuntimeError('Stop intent exists; inspect resource rather than repeat mutation')
def read(n):return json.loads((BASE/n).read_text(encoding='utf-8-sig'))
local=read('xolotl-user-stop-optimization-v404-local.json')
if local['phase']!='local_optimization_controllers_stopped' or len(local['stopped'])!=2:raise RuntimeError('Local launch controllers must be stopped first')
opt=read('xolotl-graph-native-v373-guard-intent.json');formal=read('xolotl-stable-service-v368-intent.json')
if opt['allocation_uid']!='5bc306b2-e9e1-4e25-bd05-d78eca2b4833' or formal['allocation_uid']!='bd638591-0ba6-4c60-9eb6-129a531714ee' or opt['allocation_uid']==formal['allocation_uid']:raise RuntimeError('Exact distinct resource roles required')
state=dict(unix=time.time(),phase='stop_requested',reason='User: 就这么部署就行了，不优化了',optimization_uid=opt['allocation_uid'],formal_uid=formal['allocation_uid'],formal_changed=False,goal_complete=False)
with path.open('x',encoding='utf-8') as f:f.write(json.dumps(state,ensure_ascii=False,indent=2)+'\n')
code='''import json,shlex,subprocess,time,urllib.request
from pathlib import Path
opt=@OPT@;formal=@FORMAL@
def cli(args):
 p=subprocess.run(['bash','-lc','. /etc/profile.d/ssh-init.sh >/dev/null 2>&1; exec '+shlex.join(['brainctl']+args)],capture_output=True,text=True,check=True,timeout=20)
 if len(p.stdout)>1048576:raise RuntimeError('Resource output bound exceeded')
 return p
checked=[]
for label,r in [('optimization',opt),('formal',formal)]:
 for kind,name,uid in [('rjob',r['allocation_job'],r['allocation_uid']),('replica',r['replica'],r['replica_uid'])]:
  v=json.loads(cli(['get',kind,name,'-n','ailab-hrprdduanyan','-o','json']).stdout)
  if v['metadata']['uid']!=uid or v['status']['phase']!='Running':raise RuntimeError('Fresh exact resource identity/state differs: '+label+'/'+kind)
  checked.append(dict(role=label,kind=kind,name=name,uid=uid,phase=v['status']['phase']))
service=json.loads(cli(['get','service','aster-xolotl-api-v3pool-20261024','-n','ailab-hrprdduanyan','-o','json']).stdout)
if service['metadata']['uid']!='1f968a8d-4ddc-4cee-b32a-390cd44210c7' or service['metadata']['annotations']['aster.serving/allocation-uid']!=formal['allocation_uid'] or service['spec']['selector']!={'rjob.brainpp.cn/rjob-name':formal['allocation_job']}:raise RuntimeError('VIP is not attached exclusively to formal C14')
with urllib.request.build_opener(urllib.request.ProxyHandler({})).open('http://10.102.93.68/health',timeout=5) as r:health=json.load(r)
if not health.get('loaded') or health.get('max_context')!=1048576:raise RuntimeError('Formal loaded health required before optimization stop')
stopped=cli(['stop','rjob/'+opt['allocation_job'],'-n','ailab-hrprdduanyan'])
print(json.dumps(dict(unix=time.time(),stopped_name=opt['allocation_job'],stopped_uid=opt['allocation_uid'],stop_exit=stopped.returncode,checked=checked,formal_VIP_preserved=True,formal_health_loaded=True)))
'''.replace('@OPT@',repr(opt)).replace('@FORMAL@',repr(formal))
try:
 state['result']=jump(code,100);state['phase']='stop_command_returned'
except BaseException as e:
 state.update(phase='stop_outcome_requires_inspection',error_type=type(e).__name__);path.write_text(json.dumps(state,ensure_ascii=False,indent=2)+'\n',encoding='utf-8');raise
path.write_text(json.dumps(state,ensure_ascii=False,indent=2)+'\n',encoding='utf-8')
print(json.dumps(dict(phase=state['phase'],optimization_uid=state['optimization_uid'],formal_preserved=True)))
