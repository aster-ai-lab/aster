"""Profile only after all exact pool-matrix clients and models finish on the optimization GPU."""
import base64
import hashlib
import json
import time
from xolotl_trial_common import BASE, lease, observe, owned
from xolotl_sealed_bootstrap_v125 import sealed_owned

intent=BASE/'xolotl-native-profile-v125-intent.json'
if intent.exists():raise RuntimeError('exact profile handle exists; observe it')
matrix=json.loads((BASE/'xolotl-pool-seed-v112r2-controller.json').read_text())
if matrix['phase']!='comparison_completed' or len(matrix['profiles'])!=3:
    raise RuntimeError('exact finite matrix must finish and drain first')
cpu=json.loads((BASE/'xolotl-nsys-cpu-protocol-v115-latest.json').read_text())
if cpu['exact_cpu_process_alive'] or not cpu['result']['passed']:raise RuntimeError('fresh completed CPU profiler control required')
r=json.loads((BASE/'xolotl-pool-service-v108-pool_on-intent.json').read_text())
o=observe()
if o['job_uid']!=r['allocation_uid'] or o['phase']!='Running':raise RuntimeError('exact optimization profile resource differs')
completed=json.loads((BASE/'xolotl-native-profile-v117-latest.json').read_text())
if completed['exact_worker_alive'] or completed['result']['phase']!='complete' or not completed['result']['engine_reaped'] or not completed['result']['native_target_gone']:raise RuntimeError('exact prior standalone trace must finish/reap first')
inventory=json.loads((BASE/'xolotl-pool-runtime-v116.json').read_text())
if inventory['config']['exe']!=r['plan']['binary']:raise RuntimeError('exact current profiled executable differs')
root=lease()['remote']+'/native-critical-path-v125'
baseline=json.loads((BASE/'qualified-xolotl-single-h200-baseline.json').read_text())['plan']
plan={k:baseline[k] for k in ['model','rank','sidecar','engram','visible_devices','cases']}
plan.update(binary=r['plan']['binary'],binary_sha256=r['plan']['binary_sha256'],
    dspark=True,lease_ends_unix=r['plan']['lease_ends_unix'],engine_env=r['plan']['engine_env'],
    native_args=inventory['config']['args'],context_tokens=1048576,prefill_chunk_tokens=512,
    expected_gpu_uuid=r['gpu_uuid'],diagnostic_only=True)
# Match the API f062's explicit ds41_parallel_args, rather than assuming config args are final.
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
files={'worker.py':(BASE/'xolotl_native_profile_worker_v125.frozen.py').read_bytes(),
       'plan.json':(json.dumps(plan,indent=2)+'\n').encode()}
key=(BASE/'access-private-xolotl-9c996c5a434b/api-key.txt').read_bytes().strip()
if any(key in data for data in files.values()):raise RuntimeError('actual credential negative control failed')
record=dict(unix=time.time(),phase='prepared',root=root,allocation_uid=r['allocation_uid'],gpu_uuid=r['gpu_uuid'],
    binary_sha256=r['plan']['binary_sha256'],credentials_copied=False,api_started=False,
    diagnostic_only=True,formal_service_unchanged=True,context_tokens=1048576,
    files=[dict(file=n,bytes=len(data),sha256=hashlib.sha256(data).hexdigest()) for n,data in files.items()])
handles=[json.loads((BASE/('xolotl-pool-service-v108-'+p+'-intent.json')).read_text()) for p in ['pool_off','pool_on','pool_off_repeat']]
code='''import base64,hashlib,json,os,subprocess,time
from pathlib import Path
r=@R@;root=Path(r['root']);data=@DATA@;prior=@PRIOR@
if root.parent.resolve()!=root.parent or root.parent.stat().st_uid!=10250 or root.exists():raise RuntimeError('exclusive native profile owner differs')
for old in prior:
 q=json.loads((Path(old['root'])/'service-longctx-v1-result.json').read_text());p=Path('/proc/'+str(old['process']['pid'])+'/stat')
 alive=p.exists() and p.stat().st_uid==10250 and int(p.read_text().split(') ',1)[1].split()[19])==old['process']['start_ticks'] and p.read_text().split(') ',1)[1].split()[0]!='Z'
 if alive or q['phase']!='stopped' or not q['api_reaped']:raise RuntimeError('all exact prior comparison services must drain')
active=subprocess.run(['nvidia-smi','--query-compute-apps=pid','--format=csv,noheader,nounits'],capture_output=True,text=True,check=True,timeout=5).stdout.strip()
if active:raise RuntimeError('exact optimization GPU must be idle before profiler')
gpu=subprocess.run(['nvidia-smi','--query-gpu=uuid','--format=csv,noheader,nounits'],capture_output=True,text=True,check=True,timeout=5).stdout.strip()
if gpu!=r['gpu_uuid']:raise RuntimeError('exact isolated profiler GPU differs')
proof=json.loads((root.parent/'resource-result.json').read_text())
if proof['phase']!='passed' or proof['gpus'][0]['uuid']!=gpu or time.time()+1500>proof['leaseEndsUnix']:raise RuntimeError('finite qualified profiler lease insufficient')
root.mkdir(mode=0o700)
for row in r['files']:
 raw=base64.b64decode(data[row['file']])
 if len(raw)!=row['bytes'] or hashlib.sha256(raw).hexdigest()!=row['sha256']:raise RuntimeError('exact profiler input differs')
 with (root/row['file']).open('xb') as output:output.write(raw)
with (root/'worker.log').open('xb') as log:
 p=subprocess.Popen(['timeout','--signal=TERM','--kill-after=30','1200','python3','-u',str(root/'worker.py')],cwd=root,stdin=subprocess.DEVNULL,stdout=log,stderr=log,start_new_session=True)
process=dict(pid=p.pid,start_ticks=int(Path('/proc/'+str(p.pid)+'/stat').read_text().split(') ',1)[1].split()[19]),uid=os.getuid(),unix=time.time(),budget_seconds=1200,namespace='optimization pod')
(root/'process.json').write_text(json.dumps(process,indent=2)+chr(10));print(json.dumps(process))
'''.replace('@R@',repr(record)).replace('@PRIOR@',repr(handles)).replace('@DATA@',repr({n:base64.b64encode(data).decode() for n,data in files.items()}))
compile(code,'current-native-profile-stage-v117','exec')
with intent.open('x') as output:output.write(json.dumps(record,indent=2)+'\n')
record['phase']='launch_requested';intent.write_text(json.dumps(record,indent=2)+'\n')
try:record['process']=sealed_owned(code,'native-profile-v125');record['phase']='launched'
except BaseException:
    record['phase']='launch_outcome_requires_inspection';intent.write_text(json.dumps(record,indent=2)+'\n');raise
intent.write_text(json.dumps(record,indent=2)+'\n')
print(json.dumps(dict(phase=record['phase'],process=record['process'],gpu_uuid=record['gpu_uuid'],diagnostic_only=True)))
