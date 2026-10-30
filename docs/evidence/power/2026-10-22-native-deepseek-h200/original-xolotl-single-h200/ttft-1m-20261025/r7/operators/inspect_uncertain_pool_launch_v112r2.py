"""Check GPFS artifacts for one transport-uncertain launch; never relaunch on timeout alone."""
import json
import time
from xolotl_trial_common import BASE, observe, jump
r=json.loads((BASE/'xolotl-pool-service-v108-pool_off_repeat-intent.json').read_text())
if r['phase']!='launch_outcome_requires_inspection' or r.get('process'):raise RuntimeError('exact transport-uncertain launch differs')
o=observe()
if o['job_uid']!=r['allocation_uid'] or o['phase']!='Running':raise RuntimeError('exact optimization resource differs')
code='''import json,time
from pathlib import Path
r=@R@;root=Path(r['root']);out={'unix':time.time(),'root_exists':root.exists()}
if root.parent.resolve()!=root.parent or root.parent.stat().st_uid!=10250:raise RuntimeError('exact comparison parent differs')
if root.exists():
 if root.is_symlink() or root.stat().st_uid!=10250:raise RuntimeError('exact comparison root owner differs')
 out['entries']=[p.name for p in root.iterdir()]
 for name in ['service-process.json','service-longctx-v1-result.json']:
  p=root/name
  if p.exists():
   if p.is_symlink() or p.stat().st_size>65536:raise RuntimeError('bounded exact launch receipt differs')
   text=p.read_text()
   if text.endswith('\\n'):text=text[:-2]
   out[name]=json.loads(text)
print(json.dumps(out))
'''.replace('@R@',repr(r))
q=jump(code);receipt=dict(unix=time.time(),fresh_resource_phase=o['phase'],allocation_uid=o['job_uid'],observation=q,
    service_launched=False,allocation_submitted=False)
with (BASE/'xolotl-pool-launch-uncertainty-v112r2-inspection.json').open('x') as output:output.write(json.dumps(receipt,indent=2)+'\n')
print(json.dumps(receipt,indent=2))
