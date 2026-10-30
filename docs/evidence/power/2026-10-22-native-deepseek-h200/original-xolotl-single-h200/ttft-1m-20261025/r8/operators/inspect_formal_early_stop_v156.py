"""Preserve exact C9 terminal metadata, existing VIP identity and read-only watcher evidence."""
import json
import time
import urllib.request
from xolotl_trial_common import BASE, jump

ip = BASE/'xolotl-stable-v97-early-stop-v156-reason.json'
if ip.exists():
    raise RuntimeError('exact C9 early-stop receipt already frozen')
r = json.loads((BASE/'xolotl-stable-service-v97-intent.json').read_text())
watch = json.loads((BASE/'xolotl-readiness-watch-v99-intent.json').read_text())
code = '''import json,shlex,subprocess,time
from pathlib import Path
r=@R@;watch=@WATCH@
def get(kind,name):
 p=subprocess.run(['bash','-lc','. /etc/profile.d/ssh-init.sh >/dev/null 2>&1; exec '+shlex.join(['brainctl','get',kind,name,'-n','ailab-hrprdduanyan','-o','json'])],capture_output=True,text=True,check=True,timeout=15)
 if len(p.stdout)>1048576:raise RuntimeError('bounded exact old formal metadata exceeded')
 return json.loads(p.stdout)
out=dict(unix=time.time(),cause_identified=False,service_restart_requested=False)
for kind,name,uid in [('rjob',r['allocation_job'],r['allocation_uid']),('replica',r['replica'],r['replica_uid'])]:
 v=get(kind,name)
 if v['metadata']['uid']!=uid or v['status']['phase'] not in ['Stopped','Failed','Succeeded']:raise RuntimeError('exact old formal terminal identity differs')
 out[kind]=dict(metadata=v['metadata'],status=v['status'])
s=get('service','aster-xolotl-api-v3pool-20261024')
if s['metadata']['uid']!='1f968a8d-4ddc-4cee-b32a-390cd44210c7':raise RuntimeError('exact formal VIP UID differs')
out['service']=dict(metadata=s['metadata'],spec=s['spec'],status=s['status'])
for name in ['service-longctx-v1-result.json','readiness-watch-v99/watch-result.json']:
 p=Path(r['root'])/name
 if p.is_symlink() or p.stat().st_uid!=10250 or p.stat().st_size>1048576:raise RuntimeError('bounded exact old formal diagnostic differs')
 out[name]=dict(modified_unix=p.stat().st_mtime,result=json.loads(p.read_text()))
p=Path('/proc/'+str(watch['process']['pid'])+'/stat')
out['exact_readiness_observer_alive']=p.exists() and p.stat().st_uid==10250 and int(p.read_text().split(') ',1)[1].split()[19])==watch['process']['start_ticks'] and p.read_text().split(') ',1)[1].split()[0]!='Z'
out['recorded_lease_ends_unix']=r['plan']['lease_ends_unix'];out['observed_terminal_before_lease']=time.time()<r['plan']['lease_ends_unix']
print(json.dumps(out))
'''.replace('@R@',repr(r)).replace('@WATCH@',repr(watch))
result = jump(code,90)
start = time.monotonic()
try:
    with urllib.request.build_opener(urllib.request.ProxyHandler({})).open('http://10.102.93.68/health',timeout=5) as response:
        body = response.read(65537)
        if len(body)>65536:
            raise RuntimeError('bounded direct health body exceeded')
        result['windows_direct_health']=dict(status=response.status,body=json.loads(body),seconds=time.monotonic()-start)
except OSError as error:
    result['windows_direct_health']=dict(phase='failed',error=type(error).__name__+': '+str(error)[:500],seconds=time.monotonic()-start)
raw = (json.dumps(result,indent=2)+'\n').encode()
keys = [p.read_bytes().strip() for p in BASE.glob('access-private-*/api-key.txt')]
if not keys or any(not key or key in raw for key in keys):
    raise RuntimeError('actual credential negative control refused exact stop evidence')
with ip.open('xb') as output:
    output.write(raw)
print(json.dumps(dict(formal_phase=result['rjob']['status']['phase'],replica_phase=result['replica']['status']['phase'],
    old_lease_unelapsed=result['observed_terminal_before_lease'],watch_phase=result['readiness-watch-v99/watch-result.json']['result'].get('phase'),
    windows_health=result['windows_direct_health'],cause_identified=False),indent=2))
