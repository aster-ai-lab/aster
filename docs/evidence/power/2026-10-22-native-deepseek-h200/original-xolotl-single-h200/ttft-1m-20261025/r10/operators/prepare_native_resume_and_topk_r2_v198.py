"""Correct a measured protocol mismatch and a measured link path, preserving failed roots."""
import ast
import hashlib
import json
from xolotl_trial_common import BASE

failure=json.loads((BASE/'xolotl-profiler-prefix-failure-v196.json').read_text())
metadata=[row['line'] for row in failure['protocol_metadata']]
if failure['phase']!='failed' or failure['token_count']!=128 or any(line.startswith('RESUME') for line in metadata) or metadata[-1].split()[8]!='0' or not failure['engine_reaped'] or not failure['native_gone']:
    raise RuntimeError('exact first actual1M conditional-marker failure must be retained and terminal')
worker=(BASE/'xolotl_ncu_actual1m_worker_v171r1.frozen.py').read_text()
worker=worker.replace('from aster_ncu_query_v178 import confirmed_metrics','from aster_ncu_query_v178 import confirmed_metrics\nfrom aster_native_resume_v197 import validate_resume')
old="                resumes=[int(value.split()[1]) for value in metadata if value.startswith('RESUME ')]\n                if len(resumes)!=1 or resumes[0]!=(0 if index==0 else 1047544):raise RuntimeError('exact cold/three restored native prefixes differ')"
if worker.count(old)!=1:raise RuntimeError('unique retained conditional-marker mismatch anchor differs')
worker=worker.replace(old,'                validate_resume(metadata,line,index)')
worker=worker.replace('signal.alarm(4470)','signal.alarm(2570)').replace('time.time()+4350','time.time()+2450').replace('deadline=time.monotonic()+600','deadline=time.monotonic()+150').replace('(3600 if index==0','(2400 if index==0')
line=next(line for line in worker.splitlines() if line.startswith('    expression='))
worker=worker.replace(line,r'''    names=FAMILIES+('lightning_indexer_kernel_wmma','lightning_indexer_kernel_vec')
    expression='regex:(?:(?:^|::|\\s)(?:'+'|'.join(re.escape(name) for name in names)+')(?:<|\\()|cub::)' ''')
worker=worker.replace("'--launch-count','24'","'--launch-count','96'")
manager=(BASE/'manage_ncu_actual1m_v172r1.py').read_text().replace('v172r1','v172r2').replace('v171r1','v171r2')
anchor="        'test_ncu_query_v178.py':(BASE/'test_ncu_query_v178.py').read_bytes(),"
if manager.count(anchor)!=1:raise RuntimeError('unique staged parser-test anchor differs')
manager=manager.replace(anchor,anchor+"\n        'aster_native_resume_v197.py':(BASE/'aster_native_resume_v197.py').read_bytes(),\n        'test_native_resume_v197.py':(BASE/'test_native_resume_v197.py').read_bytes(),")
manager=manager.replace("'test_ncu_metrics_v171','test_ncu_query_v178']","'test_ncu_metrics_v171','test_ncu_query_v178','test_native_resume_v197']")
manager=manager.replace('time.time()+4500+120','time.time()+2600+120').replace("'4500','python3'","'2600','python3'").replace('budget_seconds=4500','budget_seconds=2600')
pipeline=(BASE/'run_fullwindow_then_hardware_profile_v173r1.frozen.py').read_text().replace('v173r1','v173r2').replace('v172r1','v172r2')
harness=(BASE/'manage_topk_harness_v195.py').read_text().replace('v195','v195r1').replace('v172r1','v172r2')
old="build/'ggml/src/ggml-cpu/libggml-cpu.a'"
if harness.count(old)!=1:raise RuntimeError('unique measured original CPU library path differs')
native=json.loads((BASE/'xolotl-native-serve-profile-v168-plan.json').read_text())['control_link']
if 'ggml/src/libggml-cpu.a' not in native:raise RuntimeError('exact original CPU library authority differs')
harness=harness.replace(old,"build/'ggml/src/libggml-cpu.a'")
files=[]
for name,text in [('xolotl_ncu_actual1m_worker_v171r2.frozen.py',worker),('manage_ncu_actual1m_v172r2.py',manager),('run_fullwindow_then_hardware_profile_v173r2.frozen.py',pipeline),('manage_topk_harness_v195r1.py',harness)]:
    raw=text.encode();ast.parse(raw,filename=name)
    with (BASE/name).open('xb') as output:output.write(raw)
    files.append(dict(file=name,bytes=len(raw),sha256=hashlib.sha256(raw).hexdigest()))
with (BASE/'xolotl-protocol-r2-v198-prepared.json').open('x') as output:output.write(json.dumps(dict(files=files,actual_input_tokens=1047544,diagnostic_output_tokens=128,
    cold_request_budget_seconds=2400,total_profile_budget_seconds=2600,unchanged_1024_output_API_gates=True,selected_kernel_limit=96,
    retained_failures=['xolotl-profiler-prefix-failure-v196.json','xolotl-topk-harness-v195-compile-latest.json'],GPU_executed=False),indent=2)+'\n')
print(json.dumps(dict(phase='prepared',files=len(files),context_reduced=False,API_gates_changed=False,GPU_executed=False)))
