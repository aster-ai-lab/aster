"""Read exact corrected gate and retain the full public numeric receipt."""
import json
from xolotl_trial_common import BASE, observe, owned
r=json.loads((BASE/'xolotl-resident-mmq-gpu-gate-v36r2-intent.json').read_text())
o=observe()
if o['job_uid']!=r['allocation_uid']:raise RuntimeError('exact corrected gate allocation differs')
code='''import json,time
from pathlib import Path
r=@R@;root=Path(r['root']);out={'unix':time.time()}
if root.resolve()!=root or root.stat().st_uid!=10250:raise RuntimeError('exact gate root differs')
p=root/'gpu-result.json'
if p.exists():
 if p.is_symlink() or p.stat().st_size>1048576:raise RuntimeError('bounded public gate receipt exceeded')
 out['result']=json.loads(p.read_text())
p=Path('/proc/'+str(r['process']['pid'])+'/stat')
out['exact_process_alive']=p.exists() and p.stat().st_uid==10250 and int(p.read_text().split(') ',1)[1].split()[19])==r['process']['start_ticks'] and p.read_text().split(') ',1)[1].split()[0]!='Z'
print(json.dumps(out))
'''.replace('@R@',repr(r))
v=owned(code)
(BASE/'xolotl-resident-mmq-gpu-gate-v36r2-latest.json').write_text(json.dumps(v,indent=2)+'\n')
q=v.get('result',{})
print(json.dumps({'phase':q.get('phase'),'exact_process_alive':v['exact_process_alive'],'error':q.get('error'),'runs':[{'name':x['name'],'returncode':x['returncode'],'resident_packing_waves':x.get('resident_packing_waves'),'layer_timings':x.get('layer_timings'),'numeric_summary':[line for line in x['stdout'].splitlines() if line.startswith(('stats:','ok   L0 chunk','ok   L39 chunk'))],'packing_summary':[line for line in x['stderr'].splitlines() if line.startswith('resident MMQ stage:')]} for x in q.get('runs',[])]},indent=2))
