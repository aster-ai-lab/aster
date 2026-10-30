"""Retain the failed all-metric query and prepare a suffix-only exact support proof revision."""
import ast
import hashlib
import json
from xolotl_trial_common import BASE
from aster_ncu_query_v178 import confirmed_metrics

failed=json.loads((BASE/'xolotl-ncu-preflight-v176.json').read_text())
query=json.loads((BASE/'xolotl-ncu-suffix-query-v177.json').read_text())
if failed['phase']!='failed' or failed['error']!='RuntimeError: bounded installed H200 metric query exceeded' or query['query_returncode']:
    raise RuntimeError('exact retained query refusal/suffix revision provenance differs')
confirmed=confirmed_metrics(query['stdout'])
worker=(BASE/'xolotl_ncu_actual1m_worker_v171.frozen.py').read_text()
anchor='from aster_ncu_metrics_v171 import METRICS, FAMILIES, numeric_rows, family_counts'
if worker.count(anchor)!=1:raise RuntimeError('unique profiler numeric helper anchor differs')
worker=worker.replace(anchor,anchor+'\nfrom aster_ncu_query_v178 import confirmed_metrics')
old="[NCU,'--query-metrics-mode','all','--metrics',bases]"
if worker.count(old)!=1:raise RuntimeError('unique retained overbroad query anchor differs')
worker=worker.replace(old,"[NCU,'--query-metrics-mode','suffix','--metrics',bases,'--csv']")
old="    if query.returncode or len(query.stdout)>1048576 or any(metric not in query.stdout for metric in METRICS):\n        raise RuntimeError('installed H200 profiler query did not confirm all exact requested metrics')\n    STATE['exact_metrics_confirmed']=list(METRICS);save()"
new="    if query.returncode or len(query.stdout.encode())>262144 or len(query.stderr.encode())>65536:\n        raise RuntimeError('installed H200 suffix query failed/exceeded exact bounds')\n    STATE['exact_metrics_confirmed']=confirmed_metrics(query.stdout);save()"
if worker.count(old)!=1:raise RuntimeError('unique profiler metric proof anchor differs')
worker=worker.replace(old,new)
manager=(BASE/'manage_ncu_actual1m_v172.py').read_text().replace('v172','v172r1')
manager=manager.replace('xolotl_ncu_actual1m_worker_v171.frozen.py','xolotl_ncu_actual1m_worker_v171r1.frozen.py')
old="        'test_ncu_metrics_v171.py':(BASE/'test_ncu_metrics_v171.py').read_bytes(),"
if manager.count(old)!=1:raise RuntimeError('unique staged numeric parser test anchor differs')
manager=manager.replace(old,old+"\n        'aster_ncu_query_v178.py':(BASE/'aster_ncu_query_v178.py').read_bytes(),\n        'test_ncu_query_v178.py':(BASE/'test_ncu_query_v178.py').read_bytes(),")
old="['python3',str(root/'test_ncu_metrics_v171.py'),'-v']"
if manager.count(old)!=1:raise RuntimeError('unique remote profiler parser gate anchor differs')
manager=manager.replace(old,"['python3','-m','unittest','-v','test_ncu_metrics_v171','test_ncu_query_v178']")
pipeline=(BASE/'run_fullwindow_then_hardware_profile_v173.frozen.py').read_text().replace('v173','v173r1').replace('v172','v172r1')
rows=[]
for name,text in [('xolotl_ncu_actual1m_worker_v171r1.frozen.py',worker),('manage_ncu_actual1m_v172r1.py',manager),('run_fullwindow_then_hardware_profile_v173r1.frozen.py',pipeline)]:
    raw=text.encode();ast.parse(raw,filename=name)
    with (BASE/name).open('xb') as output:output.write(raw)
    rows.append(dict(file=name,bytes=len(raw),sha256=hashlib.sha256(raw).hexdigest()))
result=dict(phase='prepared',retained_failure='xolotl-ncu-preflight-v176.json',installed_suffix_query='xolotl-ncu-suffix-query-v177.json',
    metric_support_confirmed=confirmed,files=rows,model_math_changed=False,profiling_executed=False,hardware_counters_collected=False)
with (BASE/'xolotl-bounded-ncu-query-v178-prepared.json').open('x') as output:output.write(json.dumps(result,indent=2)+'\n')
print(json.dumps(dict(phase='prepared',exact_H200_metrics=len(confirmed),files=len(rows),GPU_executed=False)))
