"""Task-owned SSH transport. Never targets the shared reference allocation."""
import json
from pathlib import Path
import subprocess

BASE=Path('D:/code/aster/target/h200-native')
WORK=Path('D:/code/aster/.wt-h200-native-deploy')
SSH=['ssh','-C','-o','BatchMode=yes','-o','ConnectTimeout=15','-o','HostName=10.102.254.2','-o','HostKeyAlias=h.pjlab.org.cn','yangchao-dev.yangchao.ailab-hrprdduanyan.ws@h.pjlab.org.cn']

def jump(code, timeout=55):
    p=subprocess.run(SSH+['python3 -u -'],input=code,capture_output=True,text=True,timeout=timeout)
    if p.returncode:raise RuntimeError('jump failed: '+p.stderr[-3000:])
    if len(p.stdout)>2*1024**2:raise RuntimeError('jump output bound exceeded')
    return json.loads(p.stdout)

def lease():
    current=json.loads((BASE/'xolotl-single1m-cont13-lease-pending.json').read_text())
    if current.get('phase') not in ('submitted','submission_needs_inspection') or not current.get('submitted') or current.get('expected_gpus')!=1:
        raise RuntimeError('one explicit serving allocation is not submitted')
    if current['submitted']['uid'] != 'e808d0b3-65c6-4a48-b121-220ff6fc5eb2':
        raise RuntimeError('exact C13 recovery allocation differs')
    return current


def observe():
    intent=lease()
    code='''import json,subprocess,shlex,time
from pathlib import Path
intent=@INTENT@
def cli(args):
 p=subprocess.run(['bash','-lc','. /etc/profile.d/ssh-init.sh >/dev/null 2>&1; exec '+shlex.join(args)],capture_output=True,text=True,check=True,timeout=25)
 if len(p.stdout)>1048576:raise RuntimeError('observation exceeds bound')
 return json.loads(p.stdout)
job=cli(['brainctl','get','rjob',intent['submitted']['rjob'],'-n','ailab-hrprdduanyan','-o','json'])
if job['metadata']['uid']!=intent['submitted']['uid']:raise RuntimeError('trial job identity changed')
replicas=cli(['brainctl','get','replicas','-n','ailab-hrprdduanyan','-l','rjob.brainpp.cn/rjob-name='+intent['submitted']['rjob'],'-o','json'])
out=dict(unix=time.time(),job=job['metadata']['name'],job_uid=job['metadata']['uid'],phase=job['status']['phase'],replicas=[dict(name=r['metadata']['name'],uid=r['metadata']['uid'],phase=r['status']['phase'],status=r['status']) for r in replicas.get('items',[])])
root=Path(intent['remote'])
if root.resolve()!=root or root.stat().st_uid!=10250:raise RuntimeError('trial stage owner changed')
for name in ['resource-result.json','worker.log']:
 p=root/name
 if p.exists():
  if p.is_symlink() or p.stat().st_uid!=10250 or p.stat().st_size>262144:raise RuntimeError('resource artifact bound changed')
  out[name]=json.loads(p.read_text()) if name.endswith('.json') else p.read_text()[-8192:]
print(json.dumps(out))
'''.replace('@INTENT@',repr(intent))
    out=jump(code,80)
    (BASE/'xolotl-cont13-resource-v217-latest.json').write_text(json.dumps(out,indent=2)+'\n')
    return out

def owned(code, timeout=55):
    observation=json.loads((BASE/'xolotl-cont13-resource-v217-latest.json').read_text())
    if observation['phase']!='Running' or len(observation['replicas'])!=1 or observation['replicas'][0]['phase']!='Running' or observation.get('resource-result.json',{}).get('phase')!='passed':
        raise RuntimeError('trial allocation has no successful running ownership proof')
    replica=observation['replicas'][0]
    wrapper='''import json,subprocess,shlex
def cli(args):
 return subprocess.run(['bash','-lc','. /etc/profile.d/ssh-init.sh >/dev/null 2>&1; exec '+shlex.join(args)],capture_output=True,text=True,check=True,timeout=25)
for kind,name,uid in [('rjob',@JOB@,@JOB_UID@),('replica',@REPLICA@,@REPLICA_UID@)]:
 v=json.loads(cli(['brainctl','get',kind,name,'-n','ailab-hrprdduanyan','-o','json']).stdout)
 if v['metadata']['uid']!=uid or v['status']['phase']!='Running':raise RuntimeError('fresh trial allocation identity/state refused')
p=cli(['brainctl','exec','-n','ailab-hrprdduanyan','replica/'+@REPLICA@,'--','python3','-c',@CODE@])
if len(p.stdout)>1048576:raise RuntimeError('owned result exceeds bound')
print(p.stdout,end='')
'''
    drop='''import os
if os.getuid()==0:
 os.setgroups([]);os.setgid(10250);os.setuid(10250)
if os.getuid()!=10250:raise RuntimeError('trial must run as artifact owner')
os.umask(0o077)
'''+code
    for k,v in {'JOB':observation['job'],'JOB_UID':observation['job_uid'],'REPLICA':replica['name'],'REPLICA_UID':replica['uid'],'CODE':drop}.items():
        wrapper=wrapper.replace('@'+k+'@',repr(v))
    return jump(wrapper,timeout)

if __name__=='__main__':
    o=observe()
    print(json.dumps(dict(phase=o['phase'],job=o['job'],replicas=[dict(name=r['name'],phase=r['phase']) for r in o['replicas']],proof=o.get('resource-result.json',{}).get('phase')),indent=2))
