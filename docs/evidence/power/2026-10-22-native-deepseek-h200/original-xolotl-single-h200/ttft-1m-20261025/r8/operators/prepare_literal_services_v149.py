"""Derive a single literal API candidate; retain the same traced native control and all gates."""
import ast
import hashlib
import json
from xolotl_trial_common import BASE

source = (BASE/'launch_phase_simd_service_v139.frozen.py').read_text()
overlay = json.loads((BASE/'api-adapter-v148/overlay-plan.json').read_text())
def change(before, after):
    global source
    if source.count(before) != 1:
        raise RuntimeError('unique frozen literal-service anchor differs: '+before[:90])
    source = source.replace(before, after)
change('from xolotl_sealed_bootstrap_v139 import sealed_owned', 'from xolotl_sealed_bootstrap_v149 import sealed_owned')
change("choices=['control_a','simd','control_b']", "choices=['literal']")
change("BASE/'xolotl-api-hybrid-v52-intent.json'", "BASE/'xolotl-api-literal-v148-intent.json'")
start = source.index("suite=json.loads((BASE/'xolotl-code-fullwindow-suite-v119-latest.json').read_text())")
end = source.index('resource=observe()', start)
source = source[:start] + '''suite=json.loads((BASE/'xolotl-literal-fullwindow-v150-control-latest.json').read_text())
if suite['exact_client_alive'] or suite['suite-result.json']['phase'] not in ['passed','failed']:
    raise RuntimeError('exact baseline real1M suite must finish first; preserve failed gates')
cpu=json.loads((BASE/'xolotl-api-literal-v148-latest.json').read_text())
if cpu['exact_cpu_process_alive'] or cpu['cpu-preflight-result.json']['phase']!='passed' or cpu['cpu-preflight-result.json']['tests_completed']!=14:
    raise RuntimeError('exact both-mode API preflight must pass before GPU deployment')
''' + source[end:]
source = source.replace('f062d9d39131959da9c981b6704e2ec27f89d8e2806730062dcabc3eb27682ba', overlay['patched_sha256'])
change("ASTER_XQ_PREFILL_PHASE_TRACE='1')", "ASTER_XQ_PREFILL_PHASE_TRACE='1',ASTER_XQ_LITERAL_SCAN='1')")
change("for name in ['serve_ds41.py','aster_token_cache.py']:", "for name in ['serve_ds41.py','aster_token_cache.py','aster_literal_scan.py']:")
source = source.replace("BASE/'api-adapter-v52'", "BASE/'api-adapter-v148'")
anchor = "compile(worker,'bounded-isolation-service-v89','exec')"
intro = '''before=" run([sys.executable,'-m','unittest','serve.test_security']"
if worker.count(before)!=1:raise RuntimeError('exact startup CPU suite anchor differs')
worker=worker.replace(before," env['ASTER_XQ_LITERAL_SCAN']='1'\\n"+before)
'''
change(anchor, intro + anchor)
change("repr('trace_simd' if profile=='simd' else 'trace_control')", "repr('trace_control')")
change("Same GPU099e, trace-control / SIMD / trace-control; API f062/S512/1M/GPFS/cache2M/admission1/pool1. No adoption.",
       "Same GPU099e/native trace-control/S512/1M/GPFS/cache2M/admission1/pool1; exact literal API only; formal unchanged.")
change("sealed_owned(code,'phase-simd-v139-'+profile)", "sealed_owned(code,'literal-service-v149-'+profile)")
source = source.replace('xolotl-phase-simd-v139-', 'xolotl-literal-service-v149-').replace('serve-phase-simd-v139-', 'serve-literal-v149-')
files = {'launch_literal_service_v149.frozen.py': source}
for old, new in [('manage_phase_simd_basic_v139.frozen.py','manage_literal_basic_v149.frozen.py'),
                 ('manage_phase_simd_extended_v139.frozen.py','manage_literal_extended_v149.frozen.py')]:
    text = (BASE/old).read_text().replace("choices=['control_a','simd','control_b']", "choices=['literal']")
    text = text.replace('xolotl-phase-simd-v139-', 'xolotl-literal-service-v149-').replace('xolotl-phase-simd-extended-v139-', 'xolotl-literal-extended-v149-')
    text = text.replace('qualification-basic-v139', 'qualification-basic-v149').replace('qualification-extended-v139', 'qualification-extended-v149')
    files[new] = text
sealed = (BASE/'xolotl_sealed_bootstrap_v139.py').read_text()
before = "LABELS={'phase-simd-v139-control_a','phase-simd-v139-simd','phase-simd-v139-control_b'}"
if sealed.count(before) != 1:
    raise RuntimeError('unique bounded sealed literal label differs')
files['xolotl_sealed_bootstrap_v149.py'] = sealed.replace(before, "LABELS={'literal-service-v149-literal'}")
rows = []
for name, text in files.items():
    raw = text.encode()
    ast.parse(raw, filename=name)
    with (BASE/name).open('xb') as output:
        output.write(raw)
    rows.append(dict(file=name,bytes=len(raw),sha256=hashlib.sha256(raw).hexdigest()))
(BASE/'xolotl-literal-service-v149-prepared.json').write_text(json.dumps(dict(files=rows, native_unchanged=True,
    candidate_api_sha256=overlay['patched_sha256'], acceptance_changed=False, gpu_executed=False, production_changed=False),indent=2)+'\n')
print(json.dumps(dict(phase='prepared',files=len(rows),gpu_executed=False,native_unchanged=True)))
