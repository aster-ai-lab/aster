"""Reuse the qualified stable engine and all40/132097 recovery gates for C13."""
import ast
import hashlib
import json
import time
from xolotl_trial_common import BASE

new=json.loads((BASE/'xolotl-single1m-cont13-lease-pending.json').read_text())
reason=json.loads((BASE/'xolotl-formal-c11-stop-v215.json').read_text())
if new['phase']!='submitted' or new['submitted']['uid']!='e808d0b3-65c6-4a48-b121-220ff6fc5eb2' or reason['rjob']['status']['phase']!='Stopped':raise RuntimeError('exact terminal C11/submitted C13 required')
outputs=[]
def freeze(name,text):
    ast.parse(text)
    with (BASE/name).open('x') as out:out.write(text)
    raw=(BASE/name).read_bytes();outputs.append(dict(file=name,bytes=len(raw),sha256=hashlib.sha256(raw).hexdigest()))
common=(BASE/'xolotl_recovery_common_v157.py').read_text().replace('v157','v217').replace('cont11','cont13').replace('C11','C13').replace('1cfa236c-40ba-408f-a45e-003e0261bdf5',new['submitted']['uid'])
freeze('xolotl_recovery_common_v217.py',common)
sealed=(BASE/'xolotl_sealed_bootstrap_v157.py').read_text().replace('v157','v217')
freeze('xolotl_sealed_bootstrap_v217.py',sealed)
launcher=(BASE/'launch_xolotl_stable_recovery_v157.frozen.py').read_text().replace('v157','v217')
launcher=launcher.replace("'xolotl-stable-service-v97-intent.json'","'xolotl-stable-service-v157-intent.json'").replace("'xolotl-phase-simd-v139-control_b-intent.json'","'xolotl-topk-native-v212-control_a-intent.json'")
freeze('launch_xolotl_stable_recovery_v217.frozen.py',launcher)
for name in ['manage_xolotl_stable_recovery_v157.frozen.py','manage_xolotl_stable_extended_v157.frozen.py','manage_xolotl_stable_edge_suite_v157.frozen.py']:
    freeze(name.replace('v157','v217'),(BASE/name).read_text().replace('v157','v217'))
controller=(BASE/'run_xolotl_stable_recovery_v157r1.frozen.py').read_text().replace('v157','v217')
freeze('run_xolotl_stable_recovery_v217.frozen.py',controller)
promotion=(BASE/'promote_xolotl_stable_recovery_v157.frozen.py').read_text().replace('v157','v217').replace('cont11','cont13')
promotion=promotion.replace("'xolotl-stable-service-v97-intent.json'","'xolotl-stable-service-v157-intent.json'")
freeze('promote_xolotl_stable_recovery_v217.frozen.py',promotion)
finalize=(BASE/'finalize_xolotl_stable_recovery_v157.frozen.py').read_text().replace('v157','v217')
freeze('finalize_xolotl_stable_recovery_v217.frozen.py',finalize)
with (BASE/'xolotl-stable-recovery-v217-prepared.json').open('x') as out:
    out.write(json.dumps(dict(unix=time.time(),files=outputs,maximum_H200=2,old_formal_uid='1cfa236c-40ba-408f-a45e-003e0261bdf5',new_formal_uid=new['submitted']['uid'],independent_optimization_uid='b37b7201-2c9c-45a2-ae1f-e522319324ca',qualified_profile='nativefa019/S128/API f062/exact memo1/GPFS cache2M; all existing recovery gates unchanged',experimental_candidate_promoted=False,actual1m_new_instance_claimed=False),indent=2)+'\n')
print(json.dumps(dict(prepared=True,scripts=len(outputs),new_formal_uid=new['submitted']['uid'],maximum_task_H200=2)))
