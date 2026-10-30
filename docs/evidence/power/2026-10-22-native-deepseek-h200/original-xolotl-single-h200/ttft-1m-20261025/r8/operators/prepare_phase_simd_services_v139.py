"""Derive a same-GPU A/B/A service matrix from the exact previously qualified adapter."""
import ast
import hashlib
import json
from xolotl_trial_common import BASE

profiles = "['control_a','simd','control_b']"
source = (BASE / 'launch_code_service_v118r1.frozen.py').read_text()

def change(before, after):
    global source
    if source.count(before) != 1:
        raise RuntimeError('exact matrix source anchor differs: ' + before[:90])
    source = source.replace(before, after)

change('from xolotl_sealed_bootstrap_v120 import sealed_owned', 'from xolotl_sealed_bootstrap_v139 import sealed_owned')
change("choices=['full1m_code']", 'choices=' + profiles)
change("BASE/'xolotl-pool-seed-native-v106-intent.json'", "BASE/'xolotl-native-phase-simd-v138-intent.json'")
start = source.index("matrix=json.loads((BASE/'xolotl-pool-seed-v112r2-controller.json').read_text())")
end = source.index('resource=observe()', start)
source = source[:start] + '''suite=json.loads((BASE/'xolotl-code-fullwindow-suite-v119-latest.json').read_text())
if suite['exact_client_alive'] or suite['suite-result.json']['phase'] not in ['passed','failed']:
    raise RuntimeError('exact finite unique-code1M suite must finish first; preserve any failed gate')
''' + source[end:]
start = source.index("root=Path(build['root']);q=json.loads((root/'native-link-result.json').read_text())")
end = source.index("active=subprocess.run(['nvidia-smi'", start)
source = source[:start] + '''root=Path(build['root']);q=json.loads((root/'native-link-result.json').read_text())
if q['phase']!='linked' or not q['control_binary_byte_equal'] or not q['only_engram_member_changed']:raise RuntimeError('exact controlled native variant link differs')
variants=[r for r in q['variants'] if r['profile']==@VARIANT@]
if len(variants)!=1 or variants[0]['invalid_modes_refused']!=8:raise RuntimeError('exact closed native variant differs')
binary=variants[0];p=Path(binary['path'])
if p.is_symlink() or p.stat().st_uid!=10250 or p.stat().st_size!=binary['bytes'] or hashlib.sha256(p.read_bytes()).hexdigest()!=binary['sha256']:raise RuntimeError('exact checked native variant binary differs')
''' + source[end:]
change("print(json.dumps({'binary':q['binary'],'config':json.loads(p.read_text())}))", "print(json.dumps({'binary':binary,'config':json.loads(p.read_text())}))")
change(".replace('@API@',repr(adapter))", ".replace('@API@',repr(adapter)).replace('@VARIANT@',repr('trace_simd' if profile=='simd' else 'trace_control'))")
change("selected_qualified_baseline='Controlled pool-seed9a684/API f062/S512/GPFS/cache2M; admissionseed1, pool0/1/0repeat; no adoption'", "selected_qualified_baseline='Same GPU099e, trace-control / SIMD / trace-control; API f062/S512/1M/GPFS/cache2M/admission1/pool1. No adoption.'")
change("ASTER_XQ_POOL_DRAFT_SEED='1')", "ASTER_XQ_POOL_DRAFT_SEED='1',ASTER_XQ_PREFILL_PHASE_TRACE='1')")
change("sealed_owned(code,'code-service-v118r1')", "sealed_owned(code,'phase-simd-v139-'+profile)")
source = source.replace('xolotl-code-service-v118-', 'xolotl-phase-simd-v139-').replace('serve-code-v118-', 'serve-phase-simd-v139-')
source = source.replace('aster-xolotl-isolation-plan-v89', 'aster-xolotl-phase-simd-plan-v139')
files = {'launch_phase_simd_service_v139.frozen.py': source}
for original, name in [('manage_code_basic_v118.frozen.py', 'manage_phase_simd_basic_v139.frozen.py'),
                       ('manage_code_extended_v118.frozen.py', 'manage_phase_simd_extended_v139.frozen.py')]:
    text = (BASE / original).read_text()
    if text.count("choices=['full1m_code']") != 1:
        raise RuntimeError('exact standard acceptance profile list differs')
    text = text.replace("choices=['full1m_code']", 'choices=' + profiles)
    text = text.replace('xolotl-code-service-v118-', 'xolotl-phase-simd-v139-')
    text = text.replace('xolotl-code-extended-v118-', 'xolotl-phase-simd-extended-v139-')
    text = text.replace('qualification-basic-v118', 'qualification-basic-v139')
    text = text.replace('qualification-extended-v118', 'qualification-extended-v139')
    files[name] = text
sealed = (BASE / 'xolotl_sealed_bootstrap_v125.py').read_text()
before = "LABELS={'native-profile-v125'}"
if sealed.count(before) != 1:
    raise RuntimeError('exact bounded sealed label set differs')
files['xolotl_sealed_bootstrap_v139.py'] = sealed.replace(before,
    "LABELS={'phase-simd-v139-control_a','phase-simd-v139-simd','phase-simd-v139-control_b'}")
rows = []
for name, text in files.items():
    ast.parse(text)
    raw = text.encode()
    with (BASE / name).open('xb') as output:
        output.write(raw)
    rows.append(dict(file=name, bytes=len(raw), sha256=hashlib.sha256(raw).hexdigest()))
with (BASE / 'xolotl-phase-simd-v139-source-manifest.json').open('x') as output:
    output.write(json.dumps(dict(schema='aster-phase-simd-service-matrix-v139', files=rows,
        acceptance_counts_per_profile=dict(basic=28, native_tool_and_1024=12), acceptance_changed=False,
        profiles=['control_a','simd','control_b'], api_source_sha256='f062d9d39131959da9c981b6704e2ec27f89d8e2806730062dcabc3eb27682ba',
        trace_enabled=True, full1m_gpu_qualified=False, performance_claimed=False), indent=2) + '\n')
print(json.dumps(dict(phase='prepared', files=len(rows), acceptance_changed=False, gpu_executed=False)))
