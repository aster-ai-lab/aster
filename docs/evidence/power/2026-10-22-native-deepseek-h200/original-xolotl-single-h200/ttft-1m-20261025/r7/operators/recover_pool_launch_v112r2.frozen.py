"""Retry only after fresh exact resource and absent remote-root proof; preserve uncertain receipt."""
import hashlib
import json
import subprocess
import sys
import time
from xolotl_trial_common import BASE

primary=BASE/'xolotl-pool-service-v108-pool_off_repeat-intent.json'
old=primary.read_bytes();record=json.loads(old)
proof=json.loads((BASE/'xolotl-pool-launch-uncertainty-v112r2-inspection.json').read_text())
if record['phase']!='launch_outcome_requires_inspection' or record.get('process') or proof['observation']['root_exists'] or time.time()-proof['unix']>300:
    raise RuntimeError('recent exact no-root/no-process proof required; no uncertain launch is duplicated')
if proof['allocation_uid']!=record['allocation_uid'] or proof['fresh_resource_phase']!='Running':raise RuntimeError('fresh exact resource differs')
with (BASE/'xolotl-pool-service-v108-pool_off_repeat-before-v112r2.frozen.json').open('xb') as output:output.write(old)
source=(BASE/'launch_pool_service_v108.frozen.py').read_text()
before="intent=BASE/('xolotl-pool-service-v108-'+profile+'-intent.json')"
after="intent=BASE/('xolotl-pool-service-v108-'+profile+'-retry-v112r2-intent.json')"
if source.count(before)!=1:raise RuntimeError('pinned launch intent anchor differs')
source=source.replace(before,after)
path=BASE/'launch_pool_service_retry_v112r2.frozen.py'
with path.open('x',encoding='utf-8',newline='\n') as output:output.write(source)
compile(source,str(path),'exec')
p=subprocess.run([sys.executable,'-X','utf8',str(path),'pool_off_repeat'],cwd=BASE,capture_output=True,text=True,timeout=100)
receipt=dict(unix=time.time(),source=path.name,source_sha256=hashlib.sha256(path.read_bytes()).hexdigest(),exit=p.returncode,
             prior_uncertainty_preserved=True,new_gpu_allocated=False,formal_service_unchanged=True)
if p.returncode:
    receipt['error']=p.stderr[-4000:]
else:
    recovered=json.loads((BASE/'xolotl-pool-service-v108-pool_off_repeat-retry-v112r2-intent.json').read_text())
    if recovered['phase']!='launched' or not recovered.get('process') or recovered['root']!=record['root'] or recovered['allocation_uid']!=record['allocation_uid']:
        raise RuntimeError('exact recovered launch handle differs')
    recovered['uncertain_predecessor']='xolotl-pool-service-v108-pool_off_repeat-before-v112r2.frozen.json'
    recovered['recovery_receipt']='xolotl-pool-launch-v112r2-recovery.json'
    primary.write_text(json.dumps(recovered,indent=2)+'\n')
    receipt['process']=recovered['process'];receipt['exact_root_reused_after_absent_proof']=True
with (BASE/'xolotl-pool-launch-v112r2-recovery.json').open('x') as output:output.write(json.dumps(receipt,indent=2)+'\n')
print(json.dumps(receipt,indent=2))
if p.returncode:raise RuntimeError('retry outcome retained; inspect exact new handle before any further action')
