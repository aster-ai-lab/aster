"""Observe exact direct native profiler target; no raw report or process environment is published."""
import json
from xolotl_trial_common import BASE, observe, owned
r=json.loads((BASE/'xolotl-native-profile-v125-intent.json').read_text());o=observe()
if o['job_uid']!=r['allocation_uid']:raise RuntimeError('exact native profile resource differs')
code='''import json,time
from pathlib import Path
r=@R@;root=Path(r['root']);out={'unix':time.time()}
if root.resolve()!=root or root.stat().st_uid!=10250:raise RuntimeError('exact profiler root differs')
p=Path('/proc/'+str(r['process']['pid'])+'/stat')
out['exact_worker_alive']=p.exists() and p.stat().st_uid==10250 and int(p.read_text().split(') ',1)[1].split()[19])==r['process']['start_ticks'] and p.read_text().split(') ',1)[1].split()[0]!='Z'
p=root/'result.json'
if p.exists():
 if p.is_symlink() or p.stat().st_size>1048576:raise RuntimeError('bounded exact profiler result differs')
 out['result']=json.loads(p.read_text())
if out.get('result',{}).get('phase')=='failed':
 p=root/'worker.log'
 if p.exists() and p.stat().st_size<65536:out['failure_log']=p.read_text()[-4000:]
print(json.dumps(out))
'''.replace('@R@',repr(r))
q=owned(code);(BASE/'xolotl-native-profile-v125-latest.json').write_text(json.dumps(q,indent=2)+'\n');p=q.get('result',{})
print(json.dumps(dict(phase=p.get('phase'),alive=q['exact_worker_alive'],requests=len(p.get('requests',[])),
    engine_reaped=p.get('engine_reaped'),native_target_gone=p.get('native_target_gone'),trace=p.get('trace'),
    error=p.get('error'),failure_log=q.get('failure_log')),indent=2))
