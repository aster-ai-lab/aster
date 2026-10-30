"""Stop only the private packed service after both exact full-window clients finish."""
import json
import time
from xolotl_trial_common import BASE,observe,owned,jump
ip=BASE/'xolotl-resident-v41-drain.json'
if ip.exists():raise RuntimeError('drain already has a record; inspect exact serving process')
r=json.loads((BASE/'xolotl-resident-compare-v37-v35_packed-intent.json').read_text());o=observe()
if o['job_uid']!=r['allocation_uid']:raise RuntimeError('exact private comparison allocation differs')
clients=[json.loads((BASE/n).read_text()) for n in ['xolotl-resident-v38-fullwindow-intent.json','xolotl-resident-fullwindow-load-v41-intent.json']]
code='''import json,time
from pathlib import Path
clients=@CLIENTS@;out=[]
for c in clients:
 root=Path(c['root']);q=json.loads((root/'request-result.json').read_text());p=Path('/proc/'+str(c['process']['pid'])+'/stat')
 alive=p.exists() and p.stat().st_uid==10250 and int(p.read_text().split(') ',1)[1].split()[19])==c['process']['start_ticks'] and p.read_text().split(') ',1)[1].split()[0]!='Z'
 if q['phase']!='passed' or alive:raise RuntimeError('exact full-window client must finish and pass before drain')
 out.append({'root':str(root),'phase':q['phase'],'exact_client_alive':False,'observed_unix':time.time()})
print(json.dumps(out))
'''.replace('@CLIENTS@',repr(clients))
proof=jump(code)
record=dict(unix=time.time(),phase='prepared',allocation_uid=r['allocation_uid'],gpu_uuid=r['gpu_uuid'],clients=proof)
with ip.open('x') as f:f.write(json.dumps(record,indent=2)+'\n')
code='''import json,os,signal,time,urllib.request
from pathlib import Path
r=@R@;root=Path(r['root']);q=json.loads((root/'service-longctx-v1-result.json').read_text())
if q['phase']!='serving' or q['gpu_uuid']!=r['gpu_uuid'] or q['binary_sha256']!=r['plan']['binary_sha256']:raise RuntimeError('exact private packed backend differs')
key=(root/'credentials/api-key.txt').read_text().strip()
req=urllib.request.Request('http://127.0.0.1:30003/slots',headers={'Authorization':'Bearer '+key})
with urllib.request.urlopen(req,timeout=4) as response:slots=json.load(response)
if not isinstance(slots,list) or not slots or any(s['is_processing'] for s in slots):raise RuntimeError('private model slots missing/busy')
p=Path('/proc/'+str(r['process']['pid'])+'/stat')
if not p.exists() or p.stat().st_uid!=10250 or int(p.read_text().split(') ',1)[1].split()[19])!=r['process']['start_ticks']:raise RuntimeError('exact packed supervisor differs')
os.kill(r['process']['pid'],signal.SIGTERM)
print(json.dumps({'unix':time.time(),'signal':'SIGTERM','exact_supervisor':r['process'],'idle_slots':len(slots)}))
'''.replace('@R@',repr(r))
record['phase']='drain_requested';ip.write_text(json.dumps(record,indent=2)+'\n')
record['signal']=owned(code);ip.write_text(json.dumps(record,indent=2)+'\n');print(json.dumps(record))
