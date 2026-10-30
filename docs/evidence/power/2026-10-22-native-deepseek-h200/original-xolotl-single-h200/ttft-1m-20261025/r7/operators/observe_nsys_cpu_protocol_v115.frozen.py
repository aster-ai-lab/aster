"""Read only this exact owned protocol; raw profiler reports stay private."""
import json
from xolotl_trial_common import BASE, observe, owned
r=json.loads((BASE/'xolotl-nsys-cpu-protocol-v115-intent.json').read_text());observe()
code='''import json,time
from pathlib import Path
r=@R@;root=Path(r['root']);out={'unix':time.time()}
if root.resolve()!=root or root.stat().st_uid!=10250:raise RuntimeError('exact CPU protocol root differs')
p=Path('/proc/'+str(r['process']['pid'])+'/stat')
out['exact_cpu_process_alive']=p.exists() and p.stat().st_uid==10250 and int(p.read_text().split(') ',1)[1].split()[19])==r['process']['start_ticks'] and p.read_text().split(') ',1)[1].split()[0]!='Z'
q=root.parent/'nsys-interactive-cpu-v115'/'result.json'
if q.exists():
 if q.is_symlink() or q.stat().st_size>65536:raise RuntimeError('bounded exact CPU protocol result differs')
 out['result']=json.loads(q.read_text())
p=root/'worker.log'
if p.exists() and p.stat().st_size<65536:out['worker_log']=p.read_text()[-3000:]
print(json.dumps(out))
'''.replace('@R@',repr(r))
q=owned(code);(BASE/'xolotl-nsys-cpu-protocol-v115-latest.json').write_text(json.dumps(q,indent=2)+'\n')
print(json.dumps(q,indent=2))
