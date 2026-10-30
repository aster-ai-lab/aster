"""Read-only formal service proof and explicit current handles; no inference or mutation remotely."""
import json
import time
import urllib.request
from xolotl_serving_common import BASE, observe, jump

resource=observe()
service=json.loads((BASE/'xolotl-stable-service-v97-intent.json').read_text())
watch=json.loads((BASE/'xolotl-readiness-watch-v99-intent.json').read_text())
if resource['job_uid']!=service['allocation_uid']:
    raise RuntimeError('exact current formal allocation differs')
code='''import json,time
from pathlib import Path
r=@R@;root=Path(r['root'])
if root.resolve()!=root or root.stat().st_uid!=10250:raise RuntimeError('exact readiness root differs')
p=root/'watch-result.json'
if p.is_symlink() or p.stat().st_size>1048576:raise RuntimeError('bounded readiness receipt differs')
q=json.loads(p.read_text());p=Path('/proc/'+str(r['process']['pid'])+'/stat')
alive=p.exists() and p.stat().st_uid==10250 and int(p.read_text().split(') ',1)[1].split()[19])==r['process']['start_ticks'] and p.read_text().split(') ',1)[1].split()[0]!='Z'
print(json.dumps(dict(unix=time.time(),result=q,exact_observer_alive=alive)))
'''.replace('@R@',repr(watch))
q=jump(code)
opener=urllib.request.build_opener(urllib.request.ProxyHandler({}))
started=time.monotonic()
with opener.open('http://10.102.93.68/health',timeout=15) as response:
    raw=response.read(65537)
    if len(raw)>65536:raise RuntimeError('bounded formal health response exceeded')
    health=dict(status=response.status,body=json.loads(raw),seconds=time.monotonic()-started)
receipt=dict(unix=time.time(),formal_job=resource['job'],formal_uid=resource['job_uid'],formal_phase=resource['phase'],
             replicas=[dict(name=r['name'],phase=r['phase']) for r in resource['replicas']],
             readiness=q,windows_direct_health=health,inference_sent=False,service_restarted=False)
path=BASE/'xolotl-formal-health-v152.json'
with path.open('x') as output:output.write(json.dumps(receipt,indent=2)+'\n')
print(json.dumps(dict(formal_phase=resource['phase'],health=health,readiness_observer_alive=q['exact_observer_alive'],inference_sent=False,service_restarted=False)))
