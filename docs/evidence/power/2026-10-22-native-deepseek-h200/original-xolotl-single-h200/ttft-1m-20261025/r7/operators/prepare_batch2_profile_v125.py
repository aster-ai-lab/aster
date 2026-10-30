"""Retain standalone trace; explicitly add the API's real --batch2 for a matched second trace."""
import hashlib
import json
from xolotl_trial_common import BASE

def replace(source,a,b):
    if source.count(a)!=1:raise RuntimeError('pinned matched-profile anchor differs: '+a[:75])
    return source.replace(a,b)

worker=(BASE/'xolotl_native_profile_worker_v117.frozen.py').read_text()
before="            if line.startswith('READY '):RESULT['ready_line']=line;break"
after='''            if line.startswith('INFO '):
                parts=line.split()[1:]
                if any('=' not in part for part in parts):raise RuntimeError('exact native INFO syntax differs')
                info=dict(part.split('=',1) for part in parts)
                if len(info)!=len(parts) or info.get('ctx')!='1048576' or info.get('batch_slots')!='2' or info.get('slot_cache')!='1':raise RuntimeError('actual native1M batch2 slots not established')
                RESULT['native_info']=info
            if line.startswith('READY '):
                if not RESULT.get('native_info'):raise RuntimeError('actual native batch2 INFO proof required before READY')
                RESULT['ready_line']=line;break'''
worker=replace(worker,before,after)
worker=replace(worker,"scope='Current same9a684/1M/S512 direct single-request native critical path; three pre-trace controls then one trace. Not API throughput or full1M-input validation.'",
    "scope='Current9a684/1M/S512 and actual API --batch2; one direct GEN request at a time. Three pre-trace controls then one trace. Not API throughput or actual1M-input validation.'")
with (BASE/'xolotl_native_profile_worker_v125.frozen.py').open('x',encoding='utf-8',newline='\n') as output:output.write(worker)
compile(worker,'xolotl_native_profile_worker_v125.frozen.py','exec')

helper=(BASE/'xolotl_sealed_bootstrap_v120.py').read_text()
helper=replace(helper,"LABELS={'native-profile-v117r1','code-service-v118r1'}","LABELS={'native-profile-v125'}")
with (BASE/'xolotl_sealed_bootstrap_v125.py').open('x',encoding='utf-8',newline='\n') as output:output.write(helper)

source=(BASE/'launch_native_profile_v117r1.frozen.py').read_text()
source=source.replace('xolotl_sealed_bootstrap_v120','xolotl_sealed_bootstrap_v125')
source=source.replace('xolotl-native-profile-v117-intent.json','xolotl-native-profile-v125-intent.json')
source=source.replace('native-critical-path-v117','native-critical-path-v125')
source=source.replace('xolotl_native_profile_worker_v117.frozen.py','xolotl_native_profile_worker_v125.frozen.py')
source=source.replace("'native-profile-v117r1'","'native-profile-v125'")
anchor="inventory=json.loads((BASE/'xolotl-pool-runtime-v116.json').read_text())"
guard="""completed=json.loads((BASE/'xolotl-native-profile-v117-latest.json').read_text())
if completed['exact_worker_alive'] or completed['result']['phase']!='complete' or not completed['result']['engine_reaped'] or not completed['result']['native_target_gone']:raise RuntimeError('exact prior standalone trace must finish/reap first')
"""
source=replace(source,anchor,guard+anchor)
anchor="files={'worker.py':(BASE/'xolotl_native_profile_worker_v125.frozen.py').read_bytes(),"
insert="""# Match the API f062's explicit ds41_parallel_args, rather than assuming config args are final.
import ast,os
api=(BASE/'api-adapter-v52/serve_ds41.py').read_bytes()
if hashlib.sha256(api).hexdigest()!='f062d9d39131959da9c981b6704e2ec27f89d8e2806730062dcabc3eb27682ba':raise RuntimeError('exact API parallel argument source differs')
tree=ast.parse(api);functions=[n for n in tree.body if isinstance(n,ast.FunctionDef) and n.name=='ds41_parallel_args']
maxima=[n for n in tree.body if isinstance(n,ast.Assign) and any(isinstance(t,ast.Name) and t.id=='DS41_PARALLEL_MAX' for t in n.targets)]
if len(functions)!=1 or len(maxima)!=1:raise RuntimeError('exact API slot argument parser differs')
namespace={'os':os,'DS41_PARALLEL_MAX':ast.literal_eval(maxima[0].value)}
exec(compile(ast.Module(body=[functions[0]],type_ignores=[]),'exact-api-parallel-parser','exec'),namespace)
extra=namespace['ds41_parallel_args'](inventory['config'],list(plan['native_args']))
if extra!=['--batch','2']:raise RuntimeError('exact current API must add two native batch slots')
plan['native_args']=list(plan['native_args'])+extra
plan['parallel_slots']=2
"""
source=replace(source,anchor,insert+anchor)
compile(source,'launch_native_profile_v125.frozen.py','exec')
with (BASE/'launch_native_profile_v125.frozen.py').open('x',encoding='utf-8',newline='\n') as output:output.write(source)

observer=(BASE/'observe_native_profile_v117.frozen.py').read_text().replace('xolotl-native-profile-v117-','xolotl-native-profile-v125-')
with (BASE/'observe_native_profile_v125.frozen.py').open('x',encoding='utf-8',newline='\n') as output:output.write(observer)
receipt=dict(standalone_profile_preserved=True,api_source_sha256='f062d9d39131959da9c981b6704e2ec27f89d8e2806730062dcabc3eb27682ba',
    gpu_launched=False,actual_batch2_INFO_required=True,acceptance_gates_unchanged=True)
with (BASE/'xolotl-batch2-profile-v125-prepared.json').open('x') as output:output.write(json.dumps(receipt,indent=2)+'\n')
print(json.dumps(receipt))
