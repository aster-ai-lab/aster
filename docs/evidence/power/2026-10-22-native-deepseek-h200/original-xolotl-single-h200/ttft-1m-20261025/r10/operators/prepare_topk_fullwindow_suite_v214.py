"""Reuse the complete20-call real1M/1024-output suite for all three exact model profiles."""
import ast
import hashlib
import json
from xolotl_trial_common import BASE

source=(BASE/'manage_literal_fullwindow_suite_v150.py').read_text()
changes=[
    ("choices=['control','literal']", "choices=['control_a','topk','control_b']"),
    ("prefix = 'xolotl-phase-simd-v139-control_b' if profile == 'control' else 'xolotl-literal-service-v149-literal'", "prefix = 'xolotl-topk-native-v212-' + profile"),
    ("extended_prefix = 'xolotl-phase-simd-extended-v139-control_b' if profile == 'control' else 'xolotl-literal-extended-v149-literal'", "extended_prefix = 'xolotl-topk-native-extended-v212-' + profile"),
    ("    if profile == 'literal':\n        api_sha256 = json.loads((BASE / 'api-adapter-v148/overlay-plan.json').read_text())['patched_sha256']\n", ''),
]
for before,after in changes:
    if source.count(before)!=1:raise RuntimeError('closed fullwindow suite source anchor differs: '+before[:90])
    source=source.replace(before,after)
source=source.replace('xolotl-literal-fullwindow-v150-','xolotl-topk-fullwindow-v214-').replace('qualification-canonical-literal-v150','qualification-canonical-topk-v214')
ast.parse(source)
name='manage_topk_fullwindow_suite_v214.py'
with (BASE/name).open('x') as output:output.write(source)
raw=(BASE/name).read_bytes()
with (BASE/'xolotl-topk-fullwindow-v214-prepared.json').open('x') as output:output.write(json.dumps(dict(file=name,bytes=len(raw),sha256=hashlib.sha256(raw).hexdigest(),parent='manage_literal_fullwindow_suite_v150.py',fullwindow_worker='xolotl_code_fullwindow_long_worker_v119.frozen.py',suite_worker='xolotl_literal_fullwindow_suite_worker_v150.frozen.py',profiles=['control_a','topk','control_b'],calls_per_profile=20,actual1m_calls_per_profile=18,output_tokens=1024,full_output_identity_and_concurrency_gates_changed=False,formal_changed=False),indent=2)+'\n')
print(json.dumps(dict(prepared=True,full20_acceptance_unchanged=True,GPU_work_started=False)))
