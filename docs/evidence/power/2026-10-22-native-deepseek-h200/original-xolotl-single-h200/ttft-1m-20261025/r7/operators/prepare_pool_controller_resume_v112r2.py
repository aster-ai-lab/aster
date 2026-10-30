"""Resume only the last already-launched repeat; preserve completed prior measurements."""
import hashlib
import json
import time
from xolotl_trial_common import BASE

parent=BASE/'run_pool_seed_comparison_v112r1.frozen.py'
prior=json.loads((BASE/'xolotl-pool-seed-v112r1-controller.json').read_text())
if prior['phase']!='requires_inspection' or prior['profile']!='pool_off_repeat' or len(prior['profiles'])!=2:
    raise RuntimeError('exact completed two-profile predecessor required')
record=json.loads((BASE/'xolotl-pool-service-v108-pool_off_repeat-intent.json').read_text())
if record['phase']!='launched' or not record.get('process') or not record.get('recovery_receipt'):
    raise RuntimeError('exact recovered repeat service must be launched once')
source=parent.read_text()
def replace(before,after):
    global source
    if source.count(before)!=1:raise RuntimeError('pinned resumed controller anchor differs')
    source=source.replace(before,after)
replace("PATH=BASE/'xolotl-pool-seed-v112r1-controller.json'","PATH=BASE/'xolotl-pool-seed-v112r2-controller.json'")
replace("with PATH.open('x') as f:f.write(json.dumps(STATE,indent=2)+'\\n')",
        "STATE.update(profiles="+repr(prior['profiles'])+",resumes='xolotl-pool-seed-v112r1-controller.json',\n"
        "             resume_reason='Exact last profile recovered only after absent root/fresh resource proof; completed measurements preserved')\n"
        "with PATH.open('x') as f:f.write(json.dumps(STATE,indent=2)+'\\n')")
replace("    for profile in ['pool_off','pool_on','pool_off_repeat']:","    for profile in ['pool_off_repeat']:")
replace("        if profile=='pool_off':","        if profile=='pool_off_repeat':")
compile(source,'run_pool_seed_comparison_v112r2.frozen.py','exec')
path=BASE/'run_pool_seed_comparison_v112r2.frozen.py'
with path.open('x',encoding='utf-8',newline='\n') as output:output.write(source)
receipt=dict(unix=time.time(),parent=parent.name,parent_sha256=hashlib.sha256(parent.read_bytes()).hexdigest(),
    resumed=path.name,resumed_sha256=hashlib.sha256(path.read_bytes()).hexdigest(),
    existing_service_relaunched=False,completed_profiles_retained=2,acceptance_gates_unchanged=True)
with (BASE/'xolotl-pool-controller-resume-v112r2.json').open('x') as output:output.write(json.dumps(receipt,indent=2)+'\n')
print(json.dumps(receipt))
