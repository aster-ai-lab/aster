"""Fresh read-only client, optimization-slot and formal readiness proof."""
import json
import time
from pathlib import Path
from xolotl_trial_common import BASE, jump, observe, owned

records = []
for name in ['xolotl-fullwindow-long-suite-v56-intent.json', 'xolotl-post-concurrency-v66-intent.json',
             'xolotl-natural-fixtures-v67-intent.json', 'xolotl-draft-extended-v53-off_repeat-intent.json']:
    row = json.loads((BASE / name).read_text())
    records.append(dict(name=name, root=row['root'], process=row['process']))
code = '''import json,time
from pathlib import Path
rows=@ROWS@;out={'unix':time.time(),'clients':[]}
for row in rows:
 p=Path('/proc/'+str(row['process']['pid'])+'/stat')
 alive=p.exists() and p.stat().st_uid==10250 and int(p.read_text().split(') ',1)[1].split()[19])==row['process']['start_ticks'] and p.read_text().split(') ',1)[1].split()[0]!='Z'
 phases={}
 for name in ['suite-result.json','request-result.json','prepare-result.json']:
  q=Path(row['root'])/name
  if q.exists():
   if q.is_symlink() or q.stat().st_size>1024**2:raise RuntimeError('bounded client result differs')
   phases[name]=json.loads(q.read_text()).get('phase')
 out['clients'].append({'name':row['name'],'alive':alive,'phases':phases})
print(json.dumps(out))
'''.replace('@ROWS@', repr(records))
clients = jump(code)
if any(row['alive'] for row in clients['clients']):
    raise RuntimeError('previous exact client remains live; observe it')
resource = observe()
service = json.loads((BASE / 'xolotl-draft-service-v53-off_repeat-intent.json').read_text())
code = '''import json,time,urllib.request
from pathlib import Path
r=@R@;root=Path(r['root']);state=json.loads((root/'service-longctx-v1-result.json').read_text())
if root.resolve()!=root or root.stat().st_uid!=10250 or state['phase']!='serving' or state['gpu_uuid']!=r['gpu_uuid']:raise RuntimeError('exact previous service differs')
p=Path('/proc/'+str(r['process']['pid'])+'/stat')
alive=p.exists() and p.stat().st_uid==10250 and int(p.read_text().split(') ',1)[1].split()[19])==r['process']['start_ticks'] and p.read_text().split(') ',1)[1].split()[0]!='Z'
key=(root/'credentials/api-key.txt').read_text().strip()
request=urllib.request.Request('http://127.0.0.1:30003/slots',headers={'Authorization':'Bearer '+key})
with urllib.request.build_opener(urllib.request.ProxyHandler({})).open(request,timeout=5) as response:slots=json.load(response)
if not alive or not isinstance(slots,list) or not slots or any(s['is_processing'] for s in slots):raise RuntimeError('previous exact native service not idle')
print(json.dumps({'unix':time.time(),'supervisor_alive':alive,'slots':slots,'phase':state['phase'],'gpu_uuid':state['gpu_uuid']}))
'''.replace('@R@', repr(service))
optimization = owned(code)
watch = json.loads((BASE / 'xolotl-readiness-watch-v65-intent.json').read_text())
formal = jump('''import json,time,urllib.request
from pathlib import Path
r=@R@;root=Path(r['root']);p=root/'readiness-result.json'
out={'unix':time.time(),'observer_result':json.loads(p.read_text()) if p.exists() and p.stat().st_size<1048576 else None}
request=urllib.request.Request('http://10.102.93.68/health')
with urllib.request.build_opener(urllib.request.ProxyHandler({})).open(request,timeout=8) as response:out['health']=json.load(response);out['status']=response.status
print(json.dumps(out))
'''.replace('@R@', repr(watch)))
out = dict(unix=time.time(), phase='passed', clients=clients, optimization=optimization, formal=formal)
(BASE / 'xolotl-engram-transition-v71.json').write_text(json.dumps(out, indent=2)+'\n')
print(json.dumps(dict(phase=out['phase'], clients=clients['clients'], optimization=optimization,
                     formal_status=formal['status'], formal_health=formal['health']), indent=2))
