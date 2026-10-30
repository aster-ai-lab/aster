"""Recover on exact C11 after C9 terminal proof, preserving the active C10 comparison."""
import ast
import hashlib
import json
import time
from xolotl_trial_common import BASE

reason = json.loads((BASE/'xolotl-stable-v97-early-stop-v156-reason.json').read_text())
new = json.loads((BASE/'xolotl-single1m-cont11-lease-pending.json').read_text())
if reason['rjob']['status']['phase']!='Stopped' or reason['replica']['status']['phase']!='Stopped' or new['phase']!='submitted':
    raise RuntimeError('exact terminal predecessor/submitted C11 resource required')
outputs = []
def freeze(name, text):
    raw=text.encode()
    ast.parse(raw,filename=name)
    with (BASE/name).open('xb') as output:
        output.write(raw)
    outputs.append(dict(file=name,bytes=len(raw),sha256=hashlib.sha256(raw).hexdigest()))
def rename(text):
    return text.replace('xolotl_recovery_common_v97','xolotl_recovery_common_v157').replace('stable-recovery-v97','stable-recovery-v157').replace('stable-extended-v97','stable-extended-v157').replace('stable-edge-suite-v97','stable-edge-suite-v157').replace('qualification-basic-v97','qualification-basic-v157').replace('qualification-extended-v97','qualification-extended-v157')
common=(BASE/'xolotl_recovery_common_v97.py').read_text()
common=common.replace('xolotl-single1m-cont9-lease-pending.json','xolotl-single1m-cont11-lease-pending.json').replace('xolotl-cont9-resource-v97-latest.json','xolotl-cont11-resource-v157-latest.json')
common=common.replace('3a66564e-0a12-4c17-9b41-741fd78e0017',new['submitted']['uid']).replace('exact C9 recovery','exact C11 recovery')
freeze('xolotl_recovery_common_v157.py',common)
sealed=(BASE/'xolotl_sealed_bootstrap_v139.py').read_text()
sealed=sealed.replace('from xolotl_trial_common import BASE, jump, lease, observe, owned','from xolotl_recovery_common_v157 import BASE, jump, lease, observe, owned')
old="LABELS={'phase-simd-v139-control_a','phase-simd-v139-simd','phase-simd-v139-control_b'}"
if sealed.count(old)!=1:
    raise RuntimeError('unique sealed recovery label set differs')
sealed=sealed.replace(old,"LABELS={'stable-recovery-v157'}")
freeze('xolotl_sealed_bootstrap_v157.py',sealed)
launcher=rename((BASE/'launch_xolotl_stable_recovery_v97.frozen.py').read_text())
launcher=launcher.replace("old = json.loads((BASE / 'xolotl-stable-service-v58-intent.json').read_text())", "old = json.loads((BASE / 'xolotl-stable-service-v97-intent.json').read_text())")
launcher=launcher.replace('optimization = old  # Explicit negative binding to the stopped C8 formal GPU.',
    "optimization = json.loads((BASE / 'xolotl-phase-simd-v139-control_b-intent.json').read_text())")
anchor='from xolotl_recovery_common_v157 import BASE, lease, observe, owned\n'
if launcher.count(anchor)!=1 or launcher.count("record['process'] = owned(code)")!=1:
    raise RuntimeError('unique exact C11 sealed activation anchors differ')
launcher=launcher.replace(anchor,anchor+'from xolotl_sealed_bootstrap_v157 import sealed_owned\n')
launcher=launcher.replace("record['process'] = owned(code)", "record['process'] = sealed_owned(code,'stable-recovery-v157')")
freeze('launch_xolotl_stable_recovery_v157.frozen.py',launcher)
for source in ['manage_xolotl_stable_recovery_v97.frozen.py','manage_xolotl_stable_extended_v97.frozen.py','manage_xolotl_stable_edge_suite_v97.frozen.py']:
    freeze(source.replace('v97','v157'),rename((BASE/source).read_text()))
controller=rename((BASE/'run_xolotl_stable_recovery_v97.frozen.py').read_text())
controller=controller.replace('existing_optimization_gpu_repurposed=True','existing_optimization_gpu_repurposed=False, newly_allocated_formal_replacement=True, maximum_h200_roles=2')
controller=controller.replace('timeout=110','timeout=200')
freeze('run_xolotl_stable_recovery_v157.frozen.py',controller)
promotion=rename((BASE/'promote_xolotl_stable_recovery_v97.frozen.py').read_text())
promotion=promotion.replace("old = json.loads((BASE / 'xolotl-stable-service-v58-intent.json').read_text())", "old = json.loads((BASE / 'xolotl-stable-service-v97-intent.json').read_text())")
promotion=promotion.replace('-before-v97.frozen.json','-before-v157.frozen.json').replace('xolotl-single1m-cont9-lease-pending.json','xolotl-single1m-cont11-lease-pending.json')
anchor="(BASE / 'xolotl-stable-service-v97-intent.json').write_text"
if promotion.count(anchor)!=1:
    raise RuntimeError('unique new C11 formal intent writer differs')
promotion=promotion.replace(anchor,"(BASE / 'xolotl-stable-service-v157-intent.json').write_text")
freeze('promote_xolotl_stable_recovery_v157.frozen.py',promotion)
finalize=rename((BASE/'finalize_xolotl_stable_recovery_v97.frozen.py').read_text()).replace('xolotl-stable-service-v97-intent.json','xolotl-stable-service-v157-intent.json')
freeze('finalize_xolotl_stable_recovery_v157.frozen.py',finalize)
receipt=dict(unix=time.time(),phase='prepared',files=outputs,maximum_h200_roles=2,
    prior_formal_allocation_uid='3a66564e-0a12-4c17-9b41-741fd78e0017',new_formal_allocation_uid=new['submitted']['uid'],
    independent_optimization_allocation_uid='7455d989-8255-4f7c-ae8f-3cc0aa5928e4',optimization_restarted=False,
    selected_profile='nativefa/S128/API f062/exact-token-memo1/GPFS, unchanged qualified formal profile',
    fresh_gates=['basic28','native-tool/1024output12','strict132097','Windows VIP backend nonce/cache reuse'],
    actual1m_new_instance_claimed=False,experimental_candidates_promoted=False)
with (BASE/'xolotl-stable-recovery-v157-prepared.json').open('x') as output:
    output.write(json.dumps(receipt,indent=2)+'\n')
print(json.dumps(dict(phase='prepared',scripts=len(outputs),exact_C11_uid=new['submitted']['uid'],maximum_h200_roles=2)))
