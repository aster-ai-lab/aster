"""One replay-free diagnostic after the exact same-GPU model comparison is complete."""
import argparse
import ast
import base64
import hashlib
import json
import time
import urllib.request
from xolotl_trial_common import BASE,jump,lease,observe,owned
from xolotl_serving_common import observe as observe_formal

parser=argparse.ArgumentParser();parser.add_argument('action',choices=['prepare','launch','observe']);action=parser.parse_args().action
pp=BASE/'xolotl-manual-nsys-actual1m-v391-plan.json';ip=BASE/'xolotl-manual-nsys-actual1m-v391-intent.json'
if action=='prepare':
    lifecycle=json.loads((BASE/'xolotl-nsys-cpu-lifecycle-v303.json').read_text())
    if lifecycle['phase']!='passed' or not lifecycle['no_sessions_after_primary_exit'] or lifecycle['model_requests']!=0:raise RuntimeError('actual installed CPU-only lifecycle proof required')
    if pp.exists():raise RuntimeError('exact replay-free profiler plan already exists')
    plan=json.loads((BASE/'xolotl-ncu-actual1m-v172r2-plan.json').read_text())
    link=json.loads((BASE/'xolotl-topk-nsys-link-v250-latest.json').read_text())
    q=link['result.json']
    if link['exact_worker_alive'] or q['phase']!='linked' or not q['control_byte_equal'] or q['invalid_profiler_modes_refused']!=8:
        raise RuntimeError('controlled fourth-GEN profiling binary link must verify')
    resource=observe()
    if resource['job_uid']!='5bc306b2-e9e1-4e25-bd05-d78eca2b4833' or resource['phase']!='Running':raise RuntimeError('exact current optimization role must be observed before preparing its finite plan')
    plan.update(schema='aster-nsys-actual1m-v246r4',binary=q['binary']['path'],binary_sha256=q['binary']['sha256'],
        profile_output_tokens=1024,allocation_uid='5bc306b2-e9e1-4e25-bd05-d78eca2b4833',
        lease_ends_unix=resource['resource-result.json']['leaseEndsUnix'],
        expected_gpu_uuid='GPU-da0c6213-da08-a516-298a-002a7a2899b7',
        prior_service_root=lease()['remote']+'/serve-graph-native-v373-control_b',
        context_tokens=1048576,prompt_tokens=1047544,diagnostic_only=True,GPU_kernel_replay_used=False,
        full1024_completion_gate_substituted=False,performance_300_claimed=False,
        model_math='native2bc83 candidate with TopK35=1 and exact same GPU archives; bounded fourth-GEN profiling generator only, no formal promotion')
    plan['engine_env']['ASTER_XQ_TOPK35']='1'
    inventory=json.loads((BASE/'xolotl-explicit-nsys-device-v389.json').read_text());gpu=inventory['NVML_visible_GPU']
    if gpu['index']!=0 or gpu['uuid']!=plan['expected_gpu_uuid']:raise RuntimeError('explicit C15 profiler mapping differs')
    plan.update(nsys_gpu_index=0,NVML_inventory_line='0, '+gpu['uuid']+', '+gpu['pci_bus_id'],physical_counter_values_qualified=False,
        manual_capture=True,model_math='Existing controlled77e860 profiler for native2bc83 TopK1 baseline; graph source-type fix not included; diagnostic only')
    plan['engine_env']['ASTER_XQ_GRAPH_SOURCE_TYPES']='0'

    required={'--ctx':'1048576','--prefill-chunk':'512','--batch':'2'}
    if any(plan['native_args'].count(key)!=1 or plan['native_args'][plan['native_args'].index(key)+1]!=value for key,value in required.items()):
        raise RuntimeError('actual1M/S512/batch2 native diagnostic arguments differ')
    with pp.open('x') as out:out.write(json.dumps(plan,indent=2)+'\n')
    print(json.dumps(dict(phase='prepared',actual_input_tokens=1047544,output_tokens=1024,GEN_requests=4,GPU_executed=False)))
    raise SystemExit
if action=='observe':
    record=json.loads(ip.read_text());resource=observe()
    if resource['job_uid']!=record['allocation_uid']:raise RuntimeError('exact diagnostic allocation differs')
    code='''import json,time
from pathlib import Path
r=@R@;root=Path(r['root']);out=dict(unix=time.time())
if root.resolve()!=root or root.stat().st_uid!=10250:raise RuntimeError('exact replay-free diagnostic root differs')
h=r.get('process') or json.loads((root/'process.json').read_text());p=Path('/proc/'+str(h['pid'])+'/stat')
out['exact_worker_alive']=p.exists() and p.stat().st_uid==10250 and int(p.read_text().split(') ',1)[1].split()[19])==h['start_ticks'] and p.read_text().split(') ',1)[1].split()[0]!='Z'
for name in ['result.json','worker.log']:
 p=root/name
 if p.exists():
  if p.is_symlink() or p.stat().st_uid!=10250 or p.stat().st_size>2*1024**2:raise RuntimeError('bounded exact diagnostic receipt exceeded')
  out[name]=json.loads(p.read_text()) if name.endswith('.json') else p.read_text(errors='replace')[-4096:]
if out.get('result.json',{}).get('phase')=='failed':
 for name in ['cpu-protocol.stdout.log','cpu-protocol.stderr.log','nsys-start.log','nsys-stop.log','engine.stderr.log']:
  p=root/name
  if p.exists():
   if p.is_symlink() or p.stat().st_uid!=10250 or p.stat().st_size>8*1024**2:raise RuntimeError('bounded diagnostic failure log exceeded')
   out[name]=p.read_text(errors='replace')[-4096:]
print(json.dumps(out))
'''.replace('@R@',repr(record))
    result=owned(code);(BASE/'xolotl-manual-nsys-actual1m-v391-latest.json').write_text(json.dumps(result,indent=2)+'\n')
    q=result.get('result.json',{})
    print(json.dumps(dict(phase=q.get('phase'),alive=result['exact_worker_alive'],current=q.get('current_request'),state=q.get('current_state'),requests=len(q.get('requests',[])),
        trace=q.get('report'),numeric_pending=q.get('numeric_interpretation_pending'),native_gone=q.get('native_target_gone'),engine_reaped=q.get('engine_reaped'),error=q.get('error')),indent=2))
    raise SystemExit
if ip.exists():raise RuntimeError('exact replay-free diagnostic has a handle; observe it')
smoke=json.loads((BASE/'xolotl-interactive-nsys-smoke-v390-latest.json').read_text())
if smoke['exact_worker_alive'] or smoke['result.json']['phase']!='passed' or not smoke['result.json']['GPU_idle_after'] or not smoke['result.json']['sessions_closed']:raise RuntimeError('current physical C15 manual start/stop/export preflight must finish and reap first')

comparison=json.loads((BASE/'xolotl-graph-native-model-v375-controller.json').read_text())
if comparison['phase']!='comparison_completed' or comparison['completed_profiles']!=['control_a','guard','control_b'] or not comparison['same_full_1024_output_across_profiles']:
    raise RuntimeError('all three complete model comparisons and their native drains must finish first')
formal=observe_formal();resource=observe();plan=json.loads(pp.read_text())
if formal['job_uid']!='bd638591-0ba6-4c60-9eb6-129a531714ee' or formal['phase']!='Running' or resource['phase']!='Running' or resource['job_uid']!=plan['allocation_uid'] or formal['job_uid']==resource['job_uid']:
    raise RuntimeError('independent exact stable C13/optimization C12 role verification required')
with urllib.request.build_opener(urllib.request.ProxyHandler({})).open('http://10.102.93.68/health',timeout=5) as response:
    raw=response.read(65537)
    if len(raw)>65536:raise RuntimeError('bounded formal Windows health exceeded')
    health=json.loads(raw)
if response.status!=200 or health.get('loaded') is not True or health.get('max_context')!=1048576:
    raise RuntimeError('formal service must remain ready before independent diagnostic')
for profile in comparison['profiles']:
    observation=json.loads((BASE/('xolotl-graph-native-v373-'+profile+'-latest.json')).read_text())
    q=observation['service-longctx-v1-result.json']
    if q['phase']!='stopped' or not q.get('api_reaped') or observation.get('exact_supervisor_alive'):
        raise RuntimeError('exact native profile remains live; no competing model launch')
files={name:(BASE/name).read_bytes() for name in ['aster_nsys_lifecycle_v302.py','test_nsys_lifecycle_v302.py','aster_native_resume_v197.py','test_native_resume_v197.py','test_full_output_protocol_v247.py','xolotl_fullwindow_long_fixture_v54.py','aster_nsys_decode_window_v321.py','test_nsys_decode_window_v321.py','aster_nsys_raw_decode_metrics_v386.py','test_nsys_raw_decode_metrics_v386.py']}
files.update({'worker.py':(BASE/'xolotl_nsys_manual_actual1m_worker_v387.frozen.py').read_bytes(),'plan.json':(json.dumps(plan,indent=2)+'\n').encode()})
for name,raw in files.items():
    if name.endswith('.py'):ast.parse(raw)
keys=[p.read_bytes().strip() for p in BASE.glob('access-private-*/api-key.txt')]
if not keys or any(not key or key in raw for key in keys for raw in files.values()):raise RuntimeError('actual credential negative control refused diagnostic payload')
record=dict(unix=time.time(),phase='staging',root=lease()['remote']+'/manual-hardware-actual1m-v391',allocation_uid=resource['job_uid'],gpu_uuid=plan['expected_gpu_uuid'],
    binary_sha256=plan['binary_sha256'],actual_input_tokens=1047544,output_tokens=1024,GEN_requests=4,profiled_GEN=4,diagnostic_only=True,
    GPU_kernel_replay_used=False,credentials_copied=False,API_started=False,formal_service_changed=False,
    files=[dict(file=name,bytes=len(raw),sha256=hashlib.sha256(raw).hexdigest()) for name,raw in files.items()])
with ip.open('x') as out:out.write(json.dumps(record,indent=2)+'\n')
code='''import base64,hashlib,json,os
from pathlib import Path
r=@R@;root=Path(r['root']);data=@DATA@
if os.getuid()!=10250 or root.exists() or root.parent.resolve()!=root.parent or root.parent.stat().st_uid!=10250:raise RuntimeError('exclusive replay-free diagnostic stage differs')
os.umask(0o077);root.mkdir(mode=0o700)
for row in r['files']:
 raw=base64.b64decode(data[row['file']])
 if len(raw)!=row['bytes'] or hashlib.sha256(raw).hexdigest()!=row['sha256']:raise RuntimeError('exact diagnostic stage bytes differ')
 with (root/row['file']).open('xb') as out:out.write(raw)
print(json.dumps(dict(staged=True)))
'''.replace('@R@',repr(record)).replace('@DATA@',repr({name:base64.b64encode(raw).decode() for name,raw in files.items()}))
record['stage']=jump(code);record['phase']='launch_requested';ip.write_text(json.dumps(record,indent=2)+'\n')
code='''import hashlib,json,os,subprocess,time
from pathlib import Path
r=@R@;root=Path(r['root'])
if root.resolve()!=root or root.stat().st_uid!=10250 or (root/'process.json').exists():raise RuntimeError('exclusive exact diagnostic activation differs')
for row in r['files']:
 p=root/row['file']
 if p.is_symlink() or p.stat().st_uid!=10250 or p.stat().st_size!=row['bytes'] or hashlib.sha256(p.read_bytes()).hexdigest()!=row['sha256']:raise RuntimeError('sealed exact diagnostic bytes differ')
proof=json.loads((root.parent/'resource-result.json').read_text())
if proof['phase']!='passed' or len(proof['gpus'])!=1 or proof['gpus'][0]['uuid']!=r['gpu_uuid'] or time.time()+2720>proof['leaseEndsUnix']:raise RuntimeError('exact independent finite H200 lease insufficient')
active=subprocess.run(['nvidia-smi','--query-compute-apps=pid','--format=csv,noheader,nounits'],capture_output=True,text=True,check=True,timeout=5)
if active.stdout.strip():raise RuntimeError('optimization GPU must be idle before replay-free diagnostic')
with (root/'native-protocol-tests.log').open('xb') as log:
 q=subprocess.run(['python3','-B','-m','unittest','-v','test_native_resume_v197','test_full_output_protocol_v247','test_nsys_decode_window_v321','test_nsys_raw_decode_metrics_v386'],cwd=root,env=dict(os.environ,CUDA_VISIBLE_DEVICES=''),stdout=log,stderr=log,timeout=15)
if q.returncode:raise RuntimeError('exact native reuse protocol contracts failed before GPU launch')
with (root/'worker.log').open('xb') as log:
 q=subprocess.Popen(['timeout','--signal=TERM','--kill-after=30','2600','python3','-u',str(root/'worker.py')],cwd=root,env=dict(os.environ,CUDA_VISIBLE_DEVICES=''),stdin=subprocess.DEVNULL,stdout=log,stderr=log,start_new_session=True)
h=dict(pid=q.pid,start_ticks=int(Path('/proc/'+str(q.pid)+'/stat').read_text().split(') ',1)[1].split()[19]),uid=os.getuid(),unix=time.time(),budget_seconds=2600,namespace='optimization pod')
(root/'process.json').write_text(json.dumps(h,indent=2)+'\\n');print(json.dumps(h))
'''.replace('@R@',repr(record))
try:record.update(process=owned(code),phase='launched')
except BaseException:
    record['phase']='launch_outcome_requires_inspection';ip.write_text(json.dumps(record,indent=2)+'\n');raise
ip.write_text(json.dumps(record,indent=2)+'\n')
print(json.dumps(dict(phase=record['phase'],process=record['process'],actual_input_tokens=1047544,output_tokens=1024)))
