"""Derive unchanged real1M acceptance for the same-native baseline/literal API comparison."""
import ast
import hashlib
import json
from xolotl_trial_common import BASE

text = (BASE / 'manage_code_fullwindow_suite_v119.frozen.py').read_text()
def change(before, after):
    global text
    if text.count(before) != 1:
        raise RuntimeError('unique frozen fullwindow manager anchor differs: ' + before[:90])
    text = text.replace(before, after)
change("action = parser.parse_args().action", "parser.add_argument('--profile', choices=['control','literal'], required=True)\nargs = parser.parse_args()\naction = args.action\nprofile = args.profile")
change("intent = BASE / 'xolotl-code-fullwindow-suite-v119-intent.json'", "intent = BASE / ('xolotl-literal-fullwindow-v150-' + profile + '-intent.json')")
start = text.index("    service = json.loads((BASE / 'xolotl-code-service-v118-full1m_code-intent.json').read_text())")
end = text.index('    observation = observe()', start)
text = text[:start] + '''    prefix = 'xolotl-phase-simd-v139-control_b' if profile == 'control' else 'xolotl-literal-service-v149-literal'
    extended_prefix = 'xolotl-phase-simd-extended-v139-control_b' if profile == 'control' else 'xolotl-literal-extended-v149-literal'
    service = json.loads((BASE / (prefix + '-intent.json')).read_text())
    basic = json.loads((BASE / (prefix + '-qualification-intent.json')).read_text())
    extended = json.loads((BASE / (extended_prefix + '-intent.json')).read_text())
    fixture = json.loads((BASE / 'xolotl-canonical-code-fixture-v147-intent.json').read_text())
    if fixture['phase'] != 'staged' or fixture['body_changed']:
        raise RuntimeError('unchanged canonical actual-API public fixture must be staged')
    metadata = fixture['metadata']
    api_sha256 = 'f062d9d39131959da9c981b6704e2ec27f89d8e2806730062dcabc3eb27682ba'
    if profile == 'literal':
        api_sha256 = json.loads((BASE / 'api-adapter-v148/overlay-plan.json').read_text())['patched_sha256']
''' + text[end:]
change("root = service['root'] + '/qualification-code-fullwindow-suite-v119'", "root = service['root'] + '/qualification-canonical-literal-v150'")
change("'suite-worker.py': (BASE / 'xolotl_code_fullwindow_suite_worker_v119.frozen.py').read_bytes()", "'suite-worker.py': (BASE / 'xolotl_literal_fullwindow_suite_worker_v150.frozen.py').read_bytes()")
change("api.read_bytes()).hexdigest()!='f062d9d39131959da9c981b6704e2ec27f89d8e2806730062dcabc3eb27682ba'", "api.read_bytes()).hexdigest()!=c['api_sha256']")
change("public_fixture_only=True,\n                  files=", "public_fixture_only=True, api_sha256=api_sha256, canonical_api_metadata=True, acceptance_changed=False, profile=profile,\n                  files=")
text = text.replace('9000', '7200')
change("(BASE / 'xolotl-code-fullwindow-suite-v119-latest.json')", "(BASE / ('xolotl-literal-fullwindow-v150-' + profile + '-latest.json'))")
worker = (BASE / 'xolotl_code_fullwindow_suite_worker_v119.frozen.py').read_text()
anchor = "[('remainder', 1100), ('fullwindow', 7900)]"
if worker.count(anchor) != 1:
    raise RuntimeError('unique suite finite-stage budget anchor differs')
worker = worker.replace(anchor, "[('remainder', 600), ('fullwindow', 6600)]")
rows = []
for name, source in [('manage_literal_fullwindow_suite_v150.py', text), ('xolotl_literal_fullwindow_suite_worker_v150.frozen.py', worker)]:
    raw = source.encode()
    ast.parse(raw, filename=name)
    with (BASE / name).open('xb') as output:
        output.write(raw)
    rows.append(dict(file=name, bytes=len(raw), sha256=hashlib.sha256(raw).hexdigest()))
record = dict(schema='aster-literal-fullwindow-v150', files=rows, public_body_changed=False,
    completion_acceptance_changed=False, load_requests_changed=False,
    total_finite_budget_seconds=7200, stage_finite_budget_seconds=dict(remainder=600, fullwindow=6600),
    rationale='Bounded two-hour campaign; unchanged132097 remainder + actual1047544input/1024output/cold/restore/c1/c2 identity acceptance')
(BASE / 'xolotl-literal-fullwindow-v150-prepared.json').write_text(json.dumps(record, indent=2) + '\n')
print(json.dumps(record, indent=2))
