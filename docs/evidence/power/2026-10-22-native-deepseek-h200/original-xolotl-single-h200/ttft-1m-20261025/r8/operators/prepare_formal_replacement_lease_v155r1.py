"""Correct only the accidentally rewritten exact C10 guard name before any submission."""
import ast
import json
import time
from xolotl_trial_common import BASE
if (BASE/'xolotl-single1m-cont11-lease-pending.json').exists():
    raise RuntimeError('submission already has a handle; inspect instead')
text = (BASE/'submit_xolotl_single1m_cont11_lease_v155.frozen.py').read_text()
wrong = 'aster-xolotl-1mc11-d0ed8472c851-76441859'
right = 'aster-xolotl-1mc10-d0ed8472c851-76441859'
if text.count(wrong) != 2:
    raise RuntimeError('exact incorrect preflight-only C10 role references differ')
text = text.replace(wrong,right)
ast.parse(text)
with (BASE/'submit_xolotl_single1m_cont11_lease_v155r1.frozen.py').open('x') as output:
    output.write(text)
proof = dict(unix=time.time(),phase='preflight_guard_repaired',prior_failed_operator='submit_xolotl_single1m_cont11_lease_v155.frozen.py',
    previous_error='Guard queried a nonexistent C11 job name while retaining C10 UID; generated global prefix replacement was too broad',
    old_failure_before_submission_intent=True,old_failure_before_remote_mutation=True,new_gpu_submitted_by_failed_attempt=False,
    exact_existing_optimization_job=right,maximum_authorized_h200=2)
(BASE/'xolotl-formal-replacement-v155r1-repair.json').write_text(json.dumps(proof,indent=2)+'\n')
print(json.dumps(proof))
