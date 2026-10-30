"""Separate idle-resource safety from a failed diagnostic; retain all38 operator gates."""
import ast
import hashlib
import json
from xolotl_trial_common import BASE

latest = json.loads((BASE / 'xolotl-ncu-actual1m-v172r2-latest.json').read_text())
q = latest['result.json']
if latest['exact_worker_alive'] or q['phase'] != 'failed' or not q['engine_reaped'] or not q['native_target_gone'] or q['hardware_counters_collected'] or len(q['requests']) != 3:
    raise RuntimeError('retain exact failed/reaped three-control profiler outcome first')
source = BASE / 'manage_topk_harness_v195r1.py'
raw = source.read_text()
changes = [
    ('v195r1', 'v203'),
    ("if old['phase']!='complete' or not old['hardware_counters_collected'] or not old['engine_reaped'] or not old['native_target_gone'] or len(old['requests'])!=4:raise RuntimeError('exact actual1M hardware profile must finish/reap first')",
     "if old['phase']!='failed' or old['hardware_counters_collected'] or not old['engine_reaped'] or not old['native_target_gone'] or len(old['requests'])!=3:raise RuntimeError('exact retained failed profiler must be terminal/reaped first')"),
    ("or q['phase']!='complete' or not q.get('engine_reaped') or not q.get('native_target_gone') or not q.get('hardware_counters_collected'):",
     "or q['phase']!='failed' or not q.get('engine_reaped') or not q.get('native_target_gone') or q.get('hardware_counters_collected') or len(q['requests'])!=3:"),
]
for before, after in changes:
    if before != 'v195r1' and raw.count(before) != 1:
        raise RuntimeError('closed diagnostic-prerequisite replacement differs')
    raw = raw.replace(before, after)
raw = '# Derived after the retained R2 profiling failure. The38 ordered-ID/graph gates are unchanged.\n' + raw
ast.parse(raw)
target = BASE / 'manage_topk_harness_v203.py'
with target.open('x') as output:
    output.write(raw)
receipt = dict(parent=source.name, parent_sha256=hashlib.sha256(source.read_bytes()).hexdigest(),
    candidate=target.name, candidate_sha256=hashlib.sha256(target.read_bytes()).hexdigest(),
    failed_diagnostic='xolotl-ncu-actual1m-v172r2-latest.json', hardware_counters_collected=False,
    model_requests_started=False, exact38_operator_and_graph_gates_changed=False,
    rationale='Operator correctness does not depend on successful unrelated hardware sampling. Exact old profiler termination, native reap, one owned GPU, idle state and finite lease remain mandatory.')
with (BASE / 'xolotl-topk-harness-prerequisite-v203-prepared.json').open('x') as output:
    output.write(json.dumps(receipt, indent=2) + '\n')
print(json.dumps(receipt, indent=2))
