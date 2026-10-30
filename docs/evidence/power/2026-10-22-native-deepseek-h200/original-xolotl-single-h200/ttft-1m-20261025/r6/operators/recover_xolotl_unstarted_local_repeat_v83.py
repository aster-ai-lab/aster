"""Recover the proved unstarted repeat with a finite420s request budget and unchanged resource expiry."""
import ast
import json
import time
from xolotl_trial_common import BASE, observe, owned

intent=BASE/'xolotl-engram-local-repeat-recovery-v83-intent.json'
if intent.exists():raise RuntimeError('exact repeat recovery handle exists; inspect it')
path=BASE/'xolotl-engram-local-repeat-v79-local_repeat-intent.json';raw=path.read_bytes();service=json.loads(raw)
if service['phase']!='launch_outcome_requires_inspection' or service.get('process') is not None:
    raise RuntimeError('exact original unstarted repeat bootstrap required')
prior=json.loads((BASE/'xolotl-engram-local-repeat-v82-controller.json').read_text())
if prior['phase']!='requires_inspection' or 'finite7-request repeat deadline insufficient' not in prior['error']:
    raise RuntimeError('exact original finite repeat refusal required')
if (BASE/'xolotl-engram-local-repeat-v80-local_repeat-intent.json').exists():raise RuntimeError('exact repeat client already has a handle')
resource=observe()
if resource['job_uid']!=service['allocation_uid']:raise RuntimeError('exact repeat recovery resource differs')
(BASE/'xolotl-engram-local-repeat-v79-bootstrap-refused.json').write_bytes(raw)
record=dict(unix=time.time(),phase='prepared',root=service['root'],allocation_uid=service['allocation_uid'],
            reason='Original720s startup guard refused before creating a child; recover same staged seven-request control with420s client budget/480s lease guard',
            original_refusal_retained=True,resource_deadline_extended=False)
tree=ast.parse((BASE/'recover_xolotl_unstarted_gpfs_b_v71r1.py').read_text())
bodies=[node.value for node in ast.walk(tree) if isinstance(node,ast.Constant) and isinstance(node.value,str) and 'r=@R@;s=@S@;root=Path(r[\'root\'])' in node.value]
if len(bodies)!=1 or bodies[0].count('if deadline<900:')!=1:raise RuntimeError('exact unstarted-child recovery checks differ')
code=bodies[0].replace('if deadline<900:','if deadline<480:').replace('@R@',repr(record)).replace('@S@',repr(service))
compile(code,'exact-unstarted-local-repeat-recovery-v83','exec')
with intent.open('x') as output:output.write(json.dumps(record,indent=2)+'\n')
record['phase']='launch_requested';intent.write_text(json.dumps(record,indent=2)+'\n')
try:record.update(process=owned(code),phase='launched')
except BaseException:
    record['phase']='launch_outcome_requires_inspection';intent.write_text(json.dumps(record,indent=2)+'\n');raise
intent.write_text(json.dumps(record,indent=2)+'\n')
service.update(process=record['process'],phase='launched',original_refusal_retained=True,recovery_intent=intent.name)
path.write_text(json.dumps(service,indent=2)+'\n')
print(json.dumps(dict(phase=record['phase'],process=record['process']),indent=2))
