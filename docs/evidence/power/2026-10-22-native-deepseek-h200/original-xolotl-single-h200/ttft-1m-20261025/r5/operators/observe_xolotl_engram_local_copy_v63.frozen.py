"""Observe the exact existing low-priority disk task in its optimization pod namespace."""
import json
from xolotl_trial_common import BASE, owned

record = json.loads((BASE / 'xolotl-engram-local-copy-v63-intent.json').read_text())
code = '''import json,time
from pathlib import Path
r=@R@;root=Path(r['root']);out={'unix':time.time()}
if root.resolve()!=root or root.stat().st_uid!=10250:raise RuntimeError('exact local-copy receipt owner differs')
p=root/'copy-result.json'
if p.exists():
 if p.is_symlink() or p.stat().st_size>65536:raise RuntimeError('bounded local-copy result exceeded')
 out['result']=json.loads(p.read_text())
p=Path('/proc/'+str(r['process']['pid'])+'/stat')
out['exact_copy_process_alive']=p.exists() and p.stat().st_uid==10250 and int(p.read_text().split(') ',1)[1].split()[19])==r['process']['start_ticks'] and p.read_text().split(') ',1)[1].split()[0]!='Z'
if out.get('result',{}).get('phase')=='failed':
 p=root/'copy-worker.log'
 if p.exists() and p.stat().st_size<65536:out['failure_log']=p.read_text()[-3000:]
print(json.dumps(out))
'''.replace('@R@', repr(record))
result = owned(code)
(BASE / 'xolotl-engram-local-copy-v63-latest.json').write_text(json.dumps(result, indent=2) + '\n')
q = result.get('result', {})
print(json.dumps(dict(phase=q.get('phase'), exact_copy_process_alive=result['exact_copy_process_alive'],
                      files=q.get('files'), error=q.get('error'), gpu_executed=False,
                      live_service_changed=False), indent=2))
