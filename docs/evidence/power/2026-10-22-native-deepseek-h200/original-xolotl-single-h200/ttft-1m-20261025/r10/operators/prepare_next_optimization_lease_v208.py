"""Prepare, but do not submit, a replacement for the exact expiring idle C10."""
import ast
import hashlib
import json
from xolotl_trial_common import BASE

parent=BASE/'submit_xolotl_single1m_cont10_lease_v100.frozen.py'
raw=parent.read_text()
raw=raw.replace("'xolotl-single1m-cont10-lease-pending.json'", "'xolotl-single1m-cont12-lease-pending.json'")
raw=raw.replace("name='aster-xolotl-1mc10-'", "name='aster-xolotl-1mc12-'")
start=raw.index('promotion=json.loads(');end=raw.index('\nguard_code=',start)
raw=raw[:start]+'''promotion=json.loads((scratch/'xolotl-stable-recovery-v157-promotion.json').read_text())
if promotion['phase']!='promoted' or promotion['new_allocation_uid']!='1cfa236c-40ba-408f-a45e-003e0261bdf5' or not promotion['backend_verified_by_unique_nonce_and_kv_reuse']:raise RuntimeError('verified independent C11 formal handoff required')
'''+raw[end:]
tree=ast.parse(raw);assignment=next(n for n in tree.body if isinstance(n,ast.Assign) and any(isinstance(t,ast.Name) and t.id=='guard_code' for t in n.targets))
old_guard=ast.literal_eval(assignment.value)
tree2=ast.parse(old_guard)
old_assignment=next(n for n in tree2.body if isinstance(n,ast.Assign) and any(isinstance(t,ast.Name) and t.id=='old' for t in n.targets))
formal_assignment=next(n for n in tree2.body if isinstance(n,ast.Assign) and any(isinstance(t,ast.Name) and t.id=='formal' for t in n.targets))
old=dict(allocation_job='aster-xolotl-1mc10-d0ed8472c851-76441859',allocation_uid='7455d989-8255-4f7c-ae8f-3cc0aa5928e4',replica='aster-xolotl-1mc10-d0ed8472c851-76441859-qmx9q',replica_uid='e96e8d68-2420-45a8-84f1-510b9f556f16')
formal=dict(allocation_job='aster-xolotl-1mc11-6ef84ac5b528-78529266',allocation_uid='1cfa236c-40ba-408f-a45e-003e0261bdf5',replica='aster-xolotl-1mc11-6ef84ac5b528-78529266-v4lkj',replica_uid='51a633ec-5c55-4544-bf11-94aeb9283ebd')
guard=old_guard.replace(ast.get_source_segment(old_guard,old_assignment),'old='+repr(old)).replace(ast.get_source_segment(old_guard,formal_assignment),'formal='+repr(formal))
guard=guard.replace('Replace terminal C8 capacity after C9 qualified formal handoff; sameVIP/key and maximum two H200','Replace terminal C10 with C12 for the compiled Top-K long-output model A/B; C11 VIP/key unchanged, maximum two H200')
guard=guard.replace('print(json.dumps(proof))', '''import urllib.request
with urllib.request.build_opener(urllib.request.ProxyHandler({})).open('http://10.102.93.68/health',timeout=5) as response:
 raw=response.read(65537)
 if response.status!=200 or len(raw)>65536:raise RuntimeError('independent formal health unavailable')
 health=json.loads(raw)
if health.get('loaded') is not True or health.get('max_context')!=1048576:raise RuntimeError('independent formal model/context not ready')
proof['formal_health']={'status':200,'loaded':True,'context':1048576}
print(json.dumps(proof))''')
compile(guard,'exact C12 resource role guard','exec')
raw=raw.replace(ast.get_source_segment(raw,assignment),'guard_code='+repr(guard))
raw=raw.replace('formalC9 unchanged','formalC11 unchanged').replace('bounded six-hour single-H2001M XQ optimization: isolate short-input concurrent/draft/cache behavior and GPU-host execution, then actual1M natural/long-output gates','bounded six-hour replacement single-H2001M XQ optimization: native Top-K packed-key long-output A/B and actual1M API output identity/TTFT/throughput gates')
ast.parse(raw)
target=BASE/'submit_xolotl_single1m_cont12_lease_v208.frozen.py'
with target.open('x') as output:output.write(raw)
receipt=dict(parent=parent.name,parent_sha256=hashlib.sha256(parent.read_bytes()).hexdigest(),candidate=target.name,candidate_sha256=hashlib.sha256(target.read_bytes()).hexdigest(),submission_executed=False,
    predecessor=old,formal=formal,required_predecessor_phase=['Succeeded','Failed','Stopped'],formal_health_required=True,expected_new_GPUs=1,maximum_task_GPUs=2,windows_forwarding=False)
with (BASE/'xolotl-next-optimization-lease-v208-prepared.json').open('x') as output:output.write(json.dumps(receipt,indent=2)+'\n')
print(json.dumps(receipt,indent=2))
