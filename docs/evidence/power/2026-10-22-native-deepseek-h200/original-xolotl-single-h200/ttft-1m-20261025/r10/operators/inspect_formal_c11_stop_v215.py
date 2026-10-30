"""Freeze exact unexpectedly terminal C11 metadata; make no actor/cause attribution."""
import json
import time
from xolotl_trial_common import BASE,jump
path=BASE/'xolotl-formal-c11-stop-v215.json'
if path.exists():raise RuntimeError('exact C11 terminal evidence already retained')
r=json.loads((BASE/'xolotl-stable-service-v157-intent.json').read_text());watch=json.loads((BASE/'xolotl-readiness-watch-v159-intent.json').read_text())
code='''import json,shlex,subprocess,time
from pathlib import Path
r=@R@;watch=@WATCH@
def get(kind,name):
 q=subprocess.run(['bash','-lc','. /etc/profile.d/ssh-init.sh >/dev/null 2>&1; exec '+shlex.join(['brainctl','get',kind,name,'-n','ailab-hrprdduanyan','-o','json'])],capture_output=True,text=True,check=True,timeout=15)
 if len(q.stdout)>1048576:raise RuntimeError('bounded exact stopped formal metadata exceeded')
 return json.loads(q.stdout)
result=dict(unix=time.time(),cause_identified=False,actor_identified=False,restart_requested=False)
for kind,name,uid in [('rjob',r['allocation_job'],r['allocation_uid']),('replica',r['replica'],r['replica_uid'])]:
 q=get(kind,name)
 if q['metadata']['uid']!=uid or q['status']['phase'] not in ['Stopped','Succeeded','Failed']:raise RuntimeError('exact old formal terminal identity differs')
 result[kind]=dict(metadata=q['metadata'],status=q['status'])
q=get('service','aster-xolotl-api-v3pool-20261024')
if q['metadata']['uid']!='1f968a8d-4ddc-4cee-b32a-390cd44210c7' or q['metadata']['annotations']['aster.serving/allocation-uid']!=r['allocation_uid']:raise RuntimeError('exact existing formal VIP owner differs')
result['service']=dict(metadata=q['metadata'],spec=q['spec'],status=q['status'])
for label,path in [('formal_worker',Path(r['root'])/'service-longctx-v1-result.json'),('watch',Path(watch['root'])/'watch-result.json')]:
 if path.is_symlink() or path.stat().st_uid!=10250 or path.stat().st_size>1048576:raise RuntimeError('bounded exact formal/watch receipt differs')
 result[label]=dict(modified_unix=path.stat().st_mtime,result=json.loads(path.read_text()))
result['recorded_lease_ends_unix']=r['plan']['lease_ends_unix'];result['terminal_before_recorded_lease']=time.time()<r['plan']['lease_ends_unix']
print(json.dumps(result))
'''.replace('@R@',repr(r)).replace('@WATCH@',repr(watch))
result=jump(code);raw=(json.dumps(result,indent=2)+'\n').encode();keys=[p.read_bytes().strip() for p in BASE.glob('access-private-*/api-key.txt')]
if not keys or any(not key or key in raw for key in keys):raise RuntimeError('actual credential negative control refused stop metadata')
with path.open('xb') as out:out.write(raw)
print(json.dumps(dict(phase=result['rjob']['status']['phase'],terminal_before_lease=result['terminal_before_recorded_lease'],status=result['rjob']['status'],replica_status=result['replica']['status'],cause_identified=False,actor_identified=False),indent=2))
