"""Prepare same-binary flag0/flag1/flag0 model API tests after exact38 GPU gates."""
import ast
import hashlib
import json
from xolotl_trial_common import BASE

operator=json.loads((BASE/'xolotl-topk-harness-v203-gpu-latest.json').read_text())
q=operator['gpu-result.json']
if operator['alive'] or q['phase']!='passed' or len(q['cases'])!=38 or not q['engine_reaped'] or not q['gpu_idle_after'] or any(not row['all_IDs_equal'] for row in q['cases']):raise RuntimeError('all38 actual original-reference/CUDA-graph GPU gates required')
source=(BASE/'launch_phase_simd_service_v139.frozen.py').read_text()
def change(before,after):
    global source
    if source.count(before)!=1:raise RuntimeError('closed native service source anchor differs: '+before[:90])
    source=source.replace(before,after)
change("choices=['control_a','simd','control_b']", "choices=['control_a','topk','control_b']")
change("BASE/'xolotl-native-phase-simd-v138-intent.json'", "BASE/'xolotl-topk-native-bridge-v202-intent.json'")
change("resource['job_uid']!='7455d989-8255-4f7c-ae8f-3cc0aa5928e4'", "resource['job_uid']!='b37b7201-2c9c-45a2-ae1f-e522319324ca'")
start=source.index("root=Path(build['root']);q=json.loads((root/'native-link-result.json').read_text())")
end=source.index("active=subprocess.run(['nvidia-smi'",start)
source=source[:start]+'''root=Path(build['root']);q=json.loads((root/'result.json').read_text())
if q['phase']!='compiled' or q['control_sha256']!='99f3eee8c7eeee54c304120db50e8e632a7c001f9ac90fadf0983040fef92c25' or len(q['invalid_mode_tests'])!=7:raise RuntimeError('exact parent reproduction/closed native bridge build differs')
binary=q['candidate'];p=Path(binary['path'])
if binary['sha256']!='2bc83dcc3d3993ae7473b4e289454672f04190f21924e672de8dd53e2aff0195' or p.is_symlink() or p.stat().st_uid!=10250 or p.stat().st_size!=binary['bytes'] or hashlib.sha256(p.read_bytes()).hexdigest()!=binary['sha256']:raise RuntimeError('exact default-off Top-K native binary differs')
'''+source[end:]
change(".replace('@VARIANT@',repr('trace_simd' if profile=='simd' else 'trace_control'))", '')
change("BASE/'xolotl-stable-service-v97-intent.json'", "BASE/'xolotl-stable-service-v157-intent.json'")
change("ASTER_XQ_PREFILL_PHASE_TRACE='1')", "ASTER_XQ_PREFILL_PHASE_TRACE='1',ASTER_XQ_TOPK35=('1' if profile=='topk' else '0'))")
change("selected_qualified_baseline='Same GPU099e, trace-control / SIMD / trace-control; API f062/S512/1M/GPFS/cache2M/admission1/pool1. No adoption.'", "selected_qualified_baseline='Same GPU099e and native2bc83 flag0 / flag1 / flag0; exact API f062/S512/1M/GPFS/cache2M/admission1/pool1. Operator38 passed; model gates pending, no adoption.'")
source=source.replace('v139','v212').replace('phase-simd','topk-native').replace('phase_simd','topk_native')
files={'launch_topk_native_service_v212.frozen.py':source}
for old,new in [('manage_phase_simd_basic_v139.frozen.py','manage_topk_native_basic_v212.frozen.py'),('manage_phase_simd_extended_v139.frozen.py','manage_topk_native_extended_v212.frozen.py')]:
    raw=(BASE/old).read_text()
    if raw.count("choices=['control_a','simd','control_b']")!=1:raise RuntimeError('exact unchanged40 model gate profiles differ')
    raw=raw.replace("choices=['control_a','simd','control_b']", "choices=['control_a','topk','control_b']")
    files[new]=raw.replace('v139','v212').replace('phase-simd','topk-native').replace('phase_simd','topk_native')
sealed=(BASE/'xolotl_sealed_bootstrap_v139.py').read_text()
before="LABELS={'phase-simd-v139-control_a','phase-simd-v139-simd','phase-simd-v139-control_b'}"
if sealed.count(before)!=1:raise RuntimeError('exact bounded private bootstrap labels differ')
files['xolotl_sealed_bootstrap_v212.py']=sealed.replace(before,"LABELS={'topk-native-v212-control_a','topk-native-v212-topk','topk-native-v212-control_b'}")
rows=[]
for name,text in files.items():
    ast.parse(text)
    with (BASE/name).open('x') as output:output.write(text)
    data=(BASE/name).read_bytes();rows.append(dict(file=name,bytes=len(data),sha256=hashlib.sha256(data).hexdigest()))
with (BASE/'xolotl-topk-native-v212-source-manifest.json').open('x') as output:
    output.write(json.dumps(dict(files=rows,profiles=['control_a','topk','control_b'],native_binary_sha256='2bc83dcc3d3993ae7473b4e289454672f04190f21924e672de8dd53e2aff0195',modes=['0','1','0'],basic_requests=28,tool_and_1024_requests=12,acceptance_changed=False,model_forward_executed=False,formal_changed=False),indent=2)+'\n')
print(json.dumps(dict(prepared=True,files=len(rows),exact40_model_acceptance_unchanged=True,model_forward_executed=False)))
