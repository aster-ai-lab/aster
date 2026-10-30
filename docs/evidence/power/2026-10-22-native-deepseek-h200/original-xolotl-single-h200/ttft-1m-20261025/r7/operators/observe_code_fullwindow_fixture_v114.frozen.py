"""Observe the exact CPU code fixture process without resubmitting."""
import json
from xolotl_trial_common import BASE, jump

r=json.loads((BASE/'xolotl-code-fullwindow-fixture-v114-intent.json').read_text())
code='''import json,time
from pathlib import Path
r=@R@;root=Path(r['root']);out={'unix':time.time()}
if root.resolve()!=root or root.stat().st_uid!=10250:raise RuntimeError('exact CPU code fixture owner differs')
for name in ['prepare-result.json','fixture-metadata.json','unit-tests.log']:
 p=root/name
 if p.exists():
  if p.is_symlink() or p.stat().st_size>131072:raise RuntimeError('bounded CPU fixture result differs')
  out[name]=json.loads(p.read_text()) if name.endswith('.json') else p.read_text()
p=Path('/proc/'+str(r['process']['pid'])+'/stat')
out['exact_cpu_process_alive']=p.exists() and p.stat().st_uid==10250 and int(p.read_text().split(') ',1)[1].split()[19])==r['process']['start_ticks'] and p.read_text().split(') ',1)[1].split()[0]!='Z'
if out.get('prepare-result.json',{}).get('phase')=='failed':
 p=root/'prepare-worker.log'
 if p.exists() and p.stat().st_size<65536:out['failure_log']=p.read_text()[-4000:]
print(json.dumps(out))
'''.replace('@R@',repr(r))
q=jump(code)
(BASE/'xolotl-code-fullwindow-fixture-v114-latest.json').write_text(json.dumps(q,indent=2)+'\n')
print(json.dumps(dict(phase=q.get('prepare-result.json',{}).get('phase'),alive=q['exact_cpu_process_alive'],
    fixture=q.get('fixture-metadata.json'),error=q.get('prepare-result.json',{}).get('error'),failure_log=q.get('failure_log')),indent=2))
