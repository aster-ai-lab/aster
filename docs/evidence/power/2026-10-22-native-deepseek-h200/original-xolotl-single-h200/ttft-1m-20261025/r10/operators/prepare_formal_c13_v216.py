"""Prepare one replacement only after exact C11 terminal/C12 running proofs."""
import ast
import hashlib
import json
from xolotl_trial_common import BASE
parent=BASE/'submit_xolotl_single1m_cont11_lease_v155r1.frozen.py';raw=parent.read_text();tree=ast.parse(raw)
node=next(n for n in tree.body if isinstance(n,ast.Assign) and any(isinstance(t,ast.Name) and t.id=='GUARD' for t in n.targets));guard=ast.literal_eval(node.value);g=ast.parse(guard)
for name,value in [('old',dict(allocation_job='aster-xolotl-1mc11-6ef84ac5b528-78529266',allocation_uid='1cfa236c-40ba-408f-a45e-003e0261bdf5',replica='aster-xolotl-1mc11-6ef84ac5b528-78529266-v4lkj',replica_uid='51a633ec-5c55-4544-bf11-94aeb9283ebd')),('opt',dict(allocation_job='aster-xolotl-1mc12-00686222ad57-74805472',allocation_uid='b37b7201-2c9c-45a2-ae1f-e522319324ca',replica='aster-xolotl-1mc12-00686222ad57-74805472-4rc4h',replica_uid='2232f72c-7ef9-403d-bebf-803dc39d6d22'))]:
    assignment=next(n for n in g.body if isinstance(n,ast.Assign) and any(isinstance(t,ast.Name) and t.id==name for t in n.targets));guard=guard.replace(ast.get_source_segment(ast.literal_eval(node.value),assignment),name+'='+repr(value))
compile(guard,'exact terminal C11/live C12 role guard','exec')
raw=raw.replace(ast.get_source_segment(raw,node),'GUARD='+repr(guard)).replace("'xolotl-single1m-cont11-lease-pending.json'","'xolotl-single1m-cont13-lease-pending.json'").replace("name='aster-xolotl-1mc11-'","name='aster-xolotl-1mc13-'").replace('proven-terminal C9; preserve C10 optimization','proven-terminal C11; preserve C12 optimization')
ast.parse(raw);target=BASE/'submit_xolotl_single1m_cont13_lease_v216.frozen.py'
with target.open('x') as out:out.write(raw)
receipt=dict(parent=parent.name,parent_sha256=hashlib.sha256(parent.read_bytes()).hexdigest(),file=target.name,sha256=hashlib.sha256(target.read_bytes()).hexdigest(),submission_executed=False,old_formal_uid='1cfa236c-40ba-408f-a45e-003e0261bdf5',live_optimization_uid='b37b7201-2c9c-45a2-ae1f-e522319324ca',maximum_task_H200=2)
with (BASE/'xolotl-formal-c13-v216-prepared.json').open('x') as out:out.write(json.dumps(receipt,indent=2)+'\n')
print(json.dumps(receipt))
