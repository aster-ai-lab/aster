"""Prepare observers/resumption for the exact replacement formal role; do not start work."""
import ast
import hashlib
import json
from xolotl_trial_common import BASE
rows=[]
def freeze(name,text):
    ast.parse(text)
    with (BASE/name).open('x') as out:out.write(text)
    raw=(BASE/name).read_bytes();rows.append(dict(file=name,bytes=len(raw),sha256=hashlib.sha256(raw).hexdigest()))
observer=(BASE/'observe_current_h200_roles_v211.py').read_text()
observer=observer.replace('from xolotl_serving_common import observe as observe_formal','from xolotl_recovery_common_v217 import observe as observe_formal').replace('1cfa236c-40ba-408f-a45e-003e0261bdf5','e808d0b3-65c6-4a48-b121-220ff6fc5eb2').replace('C11/C12','C13/C12')
before="cp=BASE/'xolotl-topk-native-model-v213-controller.json'"
if observer.count(before)!=1:raise RuntimeError('exact current model-controller observer anchor differs')
observer=observer.replace(before,"cp=BASE/'xolotl-topk-native-model-v213r1-controller.json'\nif not cp.exists():cp=BASE/'xolotl-topk-native-model-v213-controller.json'")
observer=observer.replace('role-progress-v211','role-progress-v219')
freeze('observe_current_h200_roles_v219.py',observer)
controller=(BASE/'run_topk_native_model_aba_v213.frozen.py').read_text().replace('v213','v213r1').replace('1cfa236c-40ba-408f-a45e-003e0261bdf5','e808d0b3-65c6-4a48-b121-220ff6fc5eb2').replace('C11','C13').replace('1792921721.8908198','1792933033.9163594')
anchor="    flag_modes=['0','1','0'],ordinary_requests_per_profile=40,fullwindow_calls_per_profile=20,"
if controller.count(anchor)!=1:raise RuntimeError('exact resumed comparison state anchor differs')
controller=controller.replace(anchor,"    predecessor_controller='xolotl-topk-native-model-v213-controller.json',prior_stop='Independent formal C11 unexpectedly Stopped; no experimental qualification acceptance waived',\n"+anchor)
freeze('run_topk_native_model_aba_v213r1.frozen.py',controller)
with (BASE/'xolotl-post-recovery-v219-prepared.json').open('x') as out:out.write(json.dumps(dict(files=rows,model_resumed=False,formal_promotion_required_before_model_resumption=True,acceptance_unchanged=True),indent=2)+'\n')
print(json.dumps(dict(prepared=True,files=len(rows),model_resumed=False)))
