"""Inspect declared lifecycle policy and CLI options without changing any job."""
import json
import time
from xolotl_trial_common import BASE,jump
path=BASE/'xolotl-rjob-lifecycle-policy-v218.json'
if path.exists():raise RuntimeError('exact lifecycle-policy inspection already recorded')
code='''import hashlib,json,shlex,subprocess,time
def cli(args):return subprocess.run(['bash','-lc','. /etc/profile.d/ssh-init.sh >/dev/null 2>&1; exec '+shlex.join(args)],capture_output=True,text=True,timeout=20)
q=cli(['brainctl','get','rjob','aster-xolotl-1mc11-6ef84ac5b528-78529266','-n','ailab-hrprdduanyan','-o','json'])
if q.returncode or len(q.stdout)>1048576:raise RuntimeError('bounded exact old-formal lifecycle metadata exceeded')
job=json.loads(q.stdout)
if job['metadata']['uid']!='1cfa236c-40ba-408f-a45e-003e0261bdf5' or job['status']['phase']!='Stopped':raise RuntimeError('exact old formal lifecycle identity differs')
help_result=cli(['rjob','submit','--help'])
if len(help_result.stdout)>262144 or len(help_result.stderr)>65536:raise RuntimeError('bounded declared resource CLI help exceeded')
spec=job['spec'];tasks=spec.get('taskSpecs',{})
result=dict(unix=time.time(),job_uid=job['metadata']['uid'],job_spec_non_tasks={key:value for key,value in spec.items() if key!='taskSpecs'},task_fields={name:{key:value for key,value in task.items() if key!='template'} for name,task in tasks.items()},declared_labels={key:value for key,value in job['metadata']['labels'].items() if key in ['rjob.brainpp.cn/auto-delete-policy','rjob.brainpp.cn/auto-delete-policy-version','quotagroup.brainpp.cn/preemptible','volcano.brainpp.cn/interruptible','rjob.brainpp.cn/zs-task-type']},submit_help=help_result.stdout,submit_help_stderr=help_result.stderr,submit_help_returncode=help_result.returncode,policy_change_executed=False,actor_identified=False)
print(json.dumps(result))
'''
result=jump(code);raw=(json.dumps(result,indent=2)+'\n').encode()
keys=[p.read_bytes().strip() for p in BASE.glob('access-private-*/api-key.txt')]
if not keys or any(not key or key in raw for key in keys):raise RuntimeError('actual credential negative control refused policy receipt')
with path.open('xb') as out:out.write(raw)
print(json.dumps({k:v for k,v in result.items() if k not in ['submit_help','submit_help_stderr']},indent=2))
print(json.dumps(dict(declared_help_lines=[line for line in result['submit_help'].splitlines() if any(word in line.lower() for word in ['idle','delete','timeout','duration','task-type','stop','time-limit','inference'])]),indent=2))
