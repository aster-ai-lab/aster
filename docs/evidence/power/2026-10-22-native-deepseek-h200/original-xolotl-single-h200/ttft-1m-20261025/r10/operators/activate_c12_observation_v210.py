"""Change only the explicit optimization observer alias after C10 is terminal."""
import json
import time
from xolotl_trial_common import BASE,jump,observe

path=BASE/'xolotl-current-optimization-lease.json'
old=json.loads(path.read_text());new=json.loads((BASE/'xolotl-single1m-cont12-lease-pending.json').read_text())
if old['submitted']['uid']!='7455d989-8255-4f7c-ae8f-3cc0aa5928e4' or new['submitted']['uid']!='b37b7201-2c9c-45a2-ae1f-e522319324ca' or new['phase']!='submitted' or new['expected_gpus']!=1:raise RuntimeError('exact old/new observer roles differ')
code='''import json,shlex,subprocess,time
def get(name):
 q=subprocess.run(['bash','-lc','. /etc/profile.d/ssh-init.sh >/dev/null 2>&1; exec '+shlex.join(['brainctl','get','rjob',name,'-n','ailab-hrprdduanyan','-o','json'])],capture_output=True,text=True,check=True,timeout=15)
 if len(q.stdout)>1048576:raise RuntimeError('bounded exact role metadata exceeded')
 return json.loads(q.stdout)
old=get('aster-xolotl-1mc10-d0ed8472c851-76441859');formal=get('aster-xolotl-1mc11-6ef84ac5b528-78529266');new=get('aster-xolotl-1mc12-00686222ad57-74805472')
if old['metadata']['uid']!='7455d989-8255-4f7c-ae8f-3cc0aa5928e4' or old['status']['phase'] not in ['Succeeded','Failed','Stopped'] or formal['metadata']['uid']!='1cfa236c-40ba-408f-a45e-003e0261bdf5' or formal['status']['phase']!='Running' or new['metadata']['uid']!='b37b7201-2c9c-45a2-ae1f-e522319324ca':raise RuntimeError('exact terminal old/independent formal/new role identities differ')
print(json.dumps(dict(unix=time.time(),old_phase=old['status']['phase'],formal_phase=formal['status']['phase'],new_phase=new['status']['phase'],maximum_task_GPUs=2)))
'''
proof=jump(code)
backup=BASE/'xolotl-current-optimization-lease-before-v210.frozen.json'
with backup.open('xb') as out:out.write(path.read_bytes())
tmp=path.with_suffix('.v210.tmp');tmp.write_text(json.dumps(new,indent=2)+'\n');tmp.replace(path)
result=observe()
record=dict(unix=time.time(),phase='observer_activated',proof=proof,current_optimization_uid=result['job_uid'],current_optimization_phase=result['phase'],service_changed=False,model_forward_started=False,resource_probe_phase=result.get('resource-result.json',{}).get('phase'))
with (BASE/'xolotl-c12-observer-v210-activation.json').open('x') as out:out.write(json.dumps(record,indent=2)+'\n')
print(json.dumps(record,indent=2))
