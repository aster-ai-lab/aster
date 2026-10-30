"""Bind the newly observed exact supervisor handle, preserving all transport-uncertain receipts."""
import json
import time
from xolotl_trial_common import BASE, observe, owned
r=json.loads((BASE/'xolotl-pool-service-v108-pool_off_repeat-retry-v112r3-intent.json').read_text())
if r['phase']!='launched' or not r.get('process'):raise RuntimeError('exact sealed bootstrap launch required')
o=observe()
if o['job_uid']!=r['allocation_uid']:raise RuntimeError('exact recovered allocation differs')
code='''import json,time
from pathlib import Path
r=@R@;root=Path(r['root']);p=Path('/proc/'+str(r['process']['pid'])+'/stat')
if root.resolve()!=root or root.stat().st_uid!=10250:raise RuntimeError('exact recovered root differs')
alive=p.exists() and p.stat().st_uid==10250 and int(p.read_text().split(') ',1)[1].split()[19])==r['process']['start_ticks'] and p.read_text().split(') ',1)[1].split()[0]!='Z'
if not alive:raise RuntimeError('exact recovered supervisor must be alive')
q=json.loads((root/'service-longctx-v1-result.json').read_text())
print(json.dumps(dict(unix=time.time(),exact_supervisor_alive=alive,phase=q['phase'])))
'''.replace('@R@',repr(r))
proof=owned(code)
r['uncertain_predecessor']='xolotl-pool-service-v108-pool_off_repeat-before-v112r2.frozen.json'
r['recovery_receipt']='xolotl-pool-launch-v112r3-recovery.json'
receipt=dict(unix=time.time(),phase='recovered',proof=proof,process=r['process'],
    same_native_and_bootstrap_inputs=True,changed_transport='owner/hash-checked file execution instead of long inline exec argument',
    cause='long inline exec transport suspected from paired failures and same-payload file-execution success; server reason not captured',
    new_gpu_allocated=False,formal_service_unchanged=True)
with (BASE/'xolotl-pool-launch-v112r3-recovery.json').open('x') as output:
    output.write(json.dumps(receipt,indent=2)+'\n')
(BASE/'xolotl-pool-service-v108-pool_off_repeat-intent.json').write_text(json.dumps(r,indent=2)+'\n')
print(json.dumps(receipt,indent=2))
