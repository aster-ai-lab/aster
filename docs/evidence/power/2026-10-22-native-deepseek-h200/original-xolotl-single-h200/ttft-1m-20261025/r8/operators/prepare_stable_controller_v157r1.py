"""Repair stale operator basenames only after exact prelaunch local observer termination."""
import ast
import hashlib
import json
import time
from xolotl_trial_common import BASE

stop = json.loads((BASE/'xolotl-stable-recovery-v157r1-local-observer-stop.json').read_text())
if stop['gpu_or_model_signal_sent'] or (BASE/'xolotl-stable-recovery-v157-stable-intent.json').exists():
    raise RuntimeError('exact prelaunch observer-only repair required')
text = (BASE/'run_xolotl_stable_recovery_v157.frozen.py').read_text()
changed = text.count('_v97.frozen.py')
if changed < 1:
    raise RuntimeError('stale old underscored operator reference absent')
text = text.replace('_v97.frozen.py','_v157.frozen.py')
tree = ast.parse(text)
operators = []
for node in ast.walk(tree):
    if isinstance(node,ast.Call) and isinstance(node.func,ast.Name) and node.func.id=='command':
        name=node.args[0].value
        if '_v97.' in name or not (BASE/name).is_file():
            raise RuntimeError('exact recovery operator unavailable: '+name)
        operators.append(name)
path=BASE/'run_xolotl_stable_recovery_v157r1.frozen.py'
with path.open('x') as output:
    output.write(text)
record=dict(unix=time.time(),phase='prepared',old_local_observer_terminated_before_model_launch=True,
    original_source_retained='run_xolotl_stable_recovery_v157.frozen.py',changed_operator_references=changed,
    fresh_operator_files_verified=sorted(set(operators)),model_profile_changed=False,new_allocation_added=False,
    worker_sha256=hashlib.sha256(path.read_bytes()).hexdigest())
(BASE/'xolotl-stable-recovery-v157r1-prepared.json').write_text(json.dumps(record,indent=2)+'\n')
print(json.dumps(record,indent=2))
