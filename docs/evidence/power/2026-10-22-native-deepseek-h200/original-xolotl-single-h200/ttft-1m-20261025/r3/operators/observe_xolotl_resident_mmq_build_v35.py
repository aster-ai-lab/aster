"""Read the exact V35 build handle; a polling timeout never restarts it."""
import json
from xolotl_trial_common import BASE, observe, owned
r = json.loads((BASE/'xolotl-resident-mmq-build-v35-intent.json').read_text())
o = observe()
if o['job_uid']!=r['allocation_uid']:
    raise RuntimeError('exact V35 allocation changed')
code = '''import json,time
from pathlib import Path
r=@R@;root=Path(r['root']);out={'unix':time.time()}
if root.resolve()!=root or root.stat().st_uid!=10250:raise RuntimeError('exact V35 root differs')
for name in ['result.json','gpu-result.json']:
 p=root/name
 if p.exists():
  if p.is_symlink() or p.stat().st_uid!=10250 or p.stat().st_size>1048576:raise RuntimeError('V35 receipt bound differs')
  out[name]=json.loads(p.read_text())
p=Path('/proc/'+str(r['process']['pid'])+'/stat')
out['exact_build_process_alive']=p.exists() and p.stat().st_uid==10250 and int(p.read_text().split(') ',1)[1].split()[19])==r['process']['start_ticks'] and p.read_text().split(') ',1)[1].split()[0]!='Z'
for name in ['checked-moe-compile.log','cache-policy-test.log','resident-stage-policy-compile.log','resident-stage-policy-test.log','real-weight-gate-compile.log','worker.log']:
 p=root/name
 if p.exists():
  if p.is_symlink() or p.stat().st_size>1048576:raise RuntimeError('V35 compile log bound differs')
  out[name]=p.read_text(errors='replace')[-4000:]
print(json.dumps(out))
'''.replace('@R@',repr(r))
v = owned(code)
(BASE/'xolotl-resident-mmq-build-v35-latest.json').write_text(json.dumps(v,indent=2)+'\n')
print(json.dumps(v,indent=2))
