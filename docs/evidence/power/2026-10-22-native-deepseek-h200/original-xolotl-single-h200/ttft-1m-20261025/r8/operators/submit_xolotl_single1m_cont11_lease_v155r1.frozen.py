"""Replace the invalid selector with a bounded, reviewable allocation probe."""
import argparse,base64,hashlib,json,shlex,subprocess,time,uuid
from pathlib import Path

p=argparse.ArgumentParser();p.add_argument('--gpus',type=int,choices=[1],required=True);p.add_argument('--attempt',type=int,choices=[1],default=1);args=p.parse_args()
scratch=Path('D:/code/aster/target/h200-native');worktree=Path('D:/code/aster/.wt-h200-native-integration')
from xolotl_trial_common import jump
GUARD="import json,shlex,subprocess,time\nold={'allocation_job': 'aster-xolotl-1mc9-e81ede480e07-8789662', 'allocation_uid': '3a66564e-0a12-4c17-9b41-741fd78e0017', 'replica': 'aster-xolotl-1mc9-e81ede480e07-8789662-5s74q', 'replica_uid': 'c19ea7d8-bb55-4d85-966a-f10cd42a08cb'};opt={'allocation_job': 'aster-xolotl-1mc10-d0ed8472c851-76441859', 'allocation_uid': '7455d989-8255-4f7c-ae8f-3cc0aa5928e4', 'replica': 'aster-xolotl-1mc10-d0ed8472c851-76441859-qmx9q', 'replica_uid': 'e96e8d68-2420-45a8-84f1-510b9f556f16'}\ndef get(kind,name):\n p=subprocess.run(['bash','-lc','. /etc/profile.d/ssh-init.sh >/dev/null 2>&1; exec '+shlex.join(['brainctl','get',kind,name,'-n','ailab-hrprdduanyan','-o','json'])],capture_output=True,text=True,check=True,timeout=15)\n if len(p.stdout)>1048576:raise RuntimeError('bounded exact role metadata exceeded')\n return json.loads(p.stdout)\nproof={}\nfor role,r in [('terminal_formal',old),('independent_optimization',opt)]:\n row={}\n for kind,name,uid in [('rjob',r['allocation_job'],r['allocation_uid']),('replica',r['replica'],r['replica_uid'])]:\n  v=get(kind,name)\n  if v['metadata']['uid']!=uid:raise RuntimeError('exact bounded role UID differs')\n  phase=v['status']['phase']\n  if role=='terminal_formal' and phase not in ['Stopped','Failed','Succeeded']:raise RuntimeError('old formal allocation still live or uncertain; do not allocate')\n  if role=='independent_optimization' and phase!='Running':raise RuntimeError('optimization state changed; inspect exact handle')\n  row[kind]=dict(uid=uid,phase=phase)\n proof[role]=row\ns=get('service','aster-xolotl-api-v3pool-20261024')\nif s['metadata']['uid']!='1f968a8d-4ddc-4cee-b32a-390cd44210c7' or s['metadata']['annotations']['aster.serving/allocation-uid']!=old['allocation_uid'] or s['spec']['selector']!={'rjob.brainpp.cn/rjob-name':old['allocation_job']}:raise RuntimeError('existing terminal formal VIP ownership differs')\nproof.update(unix=time.time(),maximum_authorized_h200=2,new_role='formal-replacement',optimization_changed=False)\nprint(json.dumps(proof))\n"
previous_terminal=jump(GUARD)

record_path=scratch/'xolotl-single1m-cont11-lease-pending.json'
if args.gpus!=1 or args.attempt!=1:raise RuntimeError('single300 lease requires exactly one single-H200 request')
if record_path.exists():raise RuntimeError('submission intent exists; observe it instead of creating another')
source=worktree/'scripts/h200_xq/resource_probe.py';payload=source.read_bytes()
if len(payload)>65536:raise RuntimeError('allocation probe exceeds source bound')
worker_sha=hashlib.sha256(payload).hexdigest();name='aster-xolotl-1mc11-'+uuid.uuid4().hex[:12]
remote='/mnt/shared-storage-user/yangchao/Workspace/aster-h200-xq/'+name
lease=21600
entry=['python3','-u',remote+'/worker.py','--expected-gpus',str(args.gpus),'--lease-seconds',str(lease),'--worker-sha256',worker_sha]
bootstrap="""from pathlib import Path
import os,subprocess,sys,time,traceback
root=Path(@ROOT@)
try:
 if root.resolve()!=root or root.stat().st_uid!=10250:raise RuntimeError('owned resource stage UID changed')
 if os.getuid()==0:
  os.setgroups([]);os.setgid(root.stat().st_gid);os.setuid(10250)
 if os.getuid()!=10250:raise RuntimeError('probe must run as its artifact owner')
 os.umask(0o077)
 with (root/'worker.log').open('xb',buffering=0) as log:
  os.dup2(log.fileno(),1);os.dup2(log.fileno(),2)
  print('RESOURCE_PROBE_BOOTSTRAP uid='+str(os.getuid()),flush=True)
  result=subprocess.run(@ENTRY@,cwd=root,timeout=21620)
  if result.returncode:
   print('RESOURCE_PROBE_FAILED diagnostic_hold_seconds=600',flush=True);time.sleep(600)
  sys.exit(result.returncode)
except Exception:
 traceback.print_exc();print('RESOURCE_BOOTSTRAP_FAILED diagnostic_hold_seconds=600',flush=True);time.sleep(600);raise
""".replace('@ROOT@',repr(remote)).replace('@ENTRY@',repr(entry))
bootstrap_entry=['python3','-u','-c','import base64;exec(base64.b64decode('+repr(base64.b64encode(bootstrap.encode()).decode())+'))']
command=['rjob','submit','--namespace','ailab-hrprdduanyan','--name',name,'--task-type=normal','--gpu='+str(args.gpus),'--cpu=40','--memory='+str(440*1024),'--charged-group=hrprda3s_gpu_pool','--positive-tags','H200','feature/gpfs=yes','--mount','gpfs://gpfs1/yangchao:/mnt/shared-storage-user/yangchao','gpfs://gpfs2/gpfs2-shared-public:/mnt/shared-storage-gpfs2/gpfs2-shared-public','--private-machine=group','--preemptible=no','--image=registry.h.pjlab.org.cn/ailab-safeag/zoubo:v2.2','--restart-policy=never','--backoff_limit=1','--','bash','-lc','exec timeout --signal=TERM --kill-after=30 21660 '+shlex.join(bootstrap_entry)]
record={'schema':'aster-h200-resource-validation-intent-v2','unix':time.time(),'purpose':'bounded six-hour formal1M single-H200 replacement for proven-terminal C9; preserve C10 optimization and same VIP/key; maximum two H200','showname':name,'remote':remote,'expected_gpus':args.gpus,'lease_seconds_after_pass':lease,'worker_sha256':worker_sha,'bootstrap_uid':10250,'diagnostic_hold_seconds_on_failure':600,'attempt':args.attempt,'command':command,'phase':'prepared','incorrect_selectors_replaced':True,'previous_terminal':previous_terminal}
def save():record_path.write_text(json.dumps(record,indent=2)+'\n',encoding='utf-8')
save()
ssh=['ssh','-C','-o','BatchMode=yes','-o','ConnectTimeout=15','-o','HostName=10.102.254.2','-o','HostKeyAlias=h.pjlab.org.cn','yangchao-dev.yangchao.ailab-hrprdduanyan.ws@h.pjlab.org.cn']
def remote_python(code):
 result=subprocess.run(ssh+['python3 -u -'],input=code,capture_output=True,text=True,timeout=55)
 if result.returncode:raise RuntimeError('owned bounded resource command failed: '+result.stderr[-2000:])
 if len(result.stdout)>2*1024**2:raise RuntimeError('bounded resource response exceeded')
 return json.loads(result.stdout)
stage='''from pathlib import Path
import os,json,base64,hashlib
os.umask(0o077)
base=Path('/mnt/shared-storage-user/yangchao/Workspace/aster-h200-xq')
if base.resolve()!=base or base.is_symlink() or base.stat().st_uid!=10250 or os.getuid()!=10250:raise RuntimeError('owned staging root identity changed')
root=Path(@REMOTE@)
if root.parent!=base:raise RuntimeError('resource stage escaped owned base')
root.mkdir(mode=0o700,exist_ok=False)
raw=base64.b64decode(@PAYLOAD@)
if len(raw)>65536 or hashlib.sha256(raw).hexdigest()!=@SHA@:raise RuntimeError('resource worker identity changed')
path=root/'worker.py';path.write_bytes(raw);path.chmod(0o600)
(root/'submission-plan.json').write_text(@PLAN@)
print(json.dumps({'path':str(root),'uid':root.stat().st_uid,'worker_sha256':hashlib.sha256(path.read_bytes()).hexdigest(),'worker_bytes':path.stat().st_size}))
'''.replace('@REMOTE@',repr(remote)).replace('@PAYLOAD@',repr(base64.b64encode(payload).decode())).replace('@SHA@',repr(worker_sha)).replace('@PLAN@',repr(json.dumps(record,indent=2)+'\n'))
record['stage']=remote_python(stage);record['phase']='staged';save()
record['pre_submit_guard']=remote_python(GUARD)
record['phase']='submit_requested';save()
submit='''import subprocess,json,shlex,time
command=@COMMAND@
result=subprocess.run(['bash','-lc','. /etc/profile.d/ssh-init.sh >/dev/null 2>&1; exec '+shlex.join(command)],capture_output=True,text=True,timeout=30)
from pathlib import Path
root=Path(@REMOTE@)
(root/'submit.stdout.log').write_text(result.stdout);(root/'submit.stderr.log').write_text(result.stderr)
listed=subprocess.run(['bash','-lc','. /etc/profile.d/ssh-init.sh >/dev/null 2>&1; brainctl get rjobs -n ailab-hrprdduanyan -o json'],capture_output=True,text=True,timeout=18,check=True)
if len(listed.stdout)>32*1024**2:raise RuntimeError('resource list exceeds verification bound')
matches=[v for v in json.loads(listed.stdout).get('items',[]) if v['metadata'].get('annotations',{}).get('kubebrain.brainpp.cn/showname')==@NAME@]
if len(matches)!=1:raise RuntimeError('submission is ambiguous; never resubmit automatically')
job=matches[0];spec=job.get('spec',{});tasks=spec.get('taskSpecs',{})
if len(tasks)!=1:raise RuntimeError('resource probe unexpectedly has multiple tasks')
task=next(iter(tasks.values()))
if task.get('positiveTags')!=['H200','feature/gpfs=yes']:raise RuntimeError('admitted GPU selector differs from verified node label')
out={'cli_exit':result.returncode,'rjob':job['metadata']['name'],'uid':job['metadata']['uid'],'createdAt':job['metadata'].get('creationTimestamp'),'status':job.get('status'),'positiveTags':task.get('positiveTags'),'resources':[c.get('resources') for c in task.get('template',{}).get('spec',{}).get('containers',[])],'replicas':task.get('replicas'),'restartPolicy':task.get('template',{}).get('spec',{}).get('restartPolicy'),'securityContext':task.get('template',{}).get('spec',{}).get('securityContext'),'observedAtEpoch':int(time.time())}
print(json.dumps(out))
'''.replace('@COMMAND@',repr(command)).replace('@NAME@',repr(name)).replace('@REMOTE@',repr(remote))
try:
 record['submitted']=remote_python(submit);record['phase']='submitted';save()
except Exception:
 record['phase']='submission_needs_inspection';save();raise
print(json.dumps(record['submitted'],ensure_ascii=False))
