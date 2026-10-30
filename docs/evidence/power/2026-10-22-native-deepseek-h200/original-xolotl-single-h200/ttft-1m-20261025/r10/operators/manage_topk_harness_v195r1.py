"""Build a private operator harness, then run it only after the exact real1M profiler exits."""
import argparse
import base64
import hashlib
import json
from pathlib import Path
import time
from xolotl_trial_common import BASE,jump,lease,observe,owned

parser=argparse.ArgumentParser();parser.add_argument('action',choices=['launch_compile','observe_compile','launch_gpu','observe_gpu']);action=parser.parse_args().action
ip=BASE/'xolotl-topk-harness-v195r1-intent.json'
if action=='launch_compile':
    if ip.exists():raise RuntimeError('exact operator harness has a handle; observe it')
    prior=json.loads((BASE/'xolotl-topk-compile-v193-latest.json').read_text())
    if prior['alive'] or prior['result.json']['phase']!='compiled' or [r['result']['contracts'] for r in prior['result.json']['tests']]!=[14680074]*2:raise RuntimeError('exact isolated Top-K compiler/contracts must pass first')
    original=json.loads((BASE/'xolotl-native-serve-profile-v168-plan.json').read_text())['link_cwd']
    parent=json.loads((BASE/'xolotl-actual-topk-build-v187.json').read_text())['linked_topk_object']
    root=lease()['remote']+'/topk-harness-v195r1-cpu'
    files={name:(BASE/name).read_bytes() for name in ['bench_topk_cccl35_v194.cpp','aster_topk35_api_v192.hpp']}
    record=dict(root=root,phase='staging',build=original,object=prior['result.json']['object'],original_topk_object=parent,
        files=[dict(file=name,bytes=len(raw),sha256=hashlib.sha256(raw).hexdigest()) for name,raw in files.items()],GPU_executed=False,model_changed=False)
    worker='''import hashlib,json,os,subprocess,time
from pathlib import Path
root=Path(__file__).resolve().parent;p=json.loads((root/'plan.json').read_text());state=dict(unix=time.time(),phase='compiling',GPU_executed=False,model_changed=False)
def save():
 t=root/'compile-result.tmp';t.write_text(json.dumps(state,indent=2)+'\\n');t.replace(root/'compile-result.json')
def sha(path):return hashlib.sha256(path.read_bytes()).hexdigest()
try:
 os.umask(0o077);save()
 if root.resolve()!=root or root.stat().st_uid!=10250 or os.getuid()!=10250:raise RuntimeError('exact harness build owner differs')
 for row in p['files']+[p['object']]:
  path=root/row['file'] if 'file' in row else Path(row['path'])
  if path.is_symlink() or path.stat().st_uid!=10250 or path.stat().st_size!=row['bytes'] or sha(path)!=row['sha256']:raise RuntimeError('exact operator harness/compiler object differs')
 build=Path(p['build']);source=build.parent/'source';ggml=source/'third_party/llama.cpp/ggml'
 library=build/'ggml/src/ggml-cuda/libggml-cuda.a'
 member=subprocess.run(['ar','p',str(library),'top-k.cu.o'],capture_output=True,check=True,timeout=10).stdout
 if len(member)!=p['original_topk_object']['bytes'] or hashlib.sha256(member).hexdigest()!=p['original_topk_object']['sha256']:raise RuntimeError('actual linked reference Top-K member differs')
 command=['/usr/bin/c++','-O3','-DNDEBUG','-std=c++20','-I'+str(ggml/'include'),'-I/usr/local/cuda/include',str(root/'bench_topk_cccl35_v194.cpp'),p['object']['path'],'-o',str(root/'benchmark'),'-Wl,-rpath,/usr/local/cuda-12.6/targets/x86_64-linux/lib',str(build/'ggml/src/libggml.a'),str(library),str(build/'ggml/src/libggml-cpu.a'),str(build/'ggml/src/libggml-base.a'),'/usr/local/cuda-12.6/targets/x86_64-linux/lib/libcublas.so','/usr/local/cuda-12.6/targets/x86_64-linux/lib/libcublasLt.so','/usr/local/cuda-12.6/targets/x86_64-linux/lib/libcudart.so','/usr/lib/x86_64-linux-gnu/libnccl.so','-L/usr/local/cuda/targets/x86_64-linux/lib/stubs','-lcuda','-fopenmp','-lpthread','-ldl','-lrt']
 state.update(command=command,exact_reference_archive_member_sha256=hashlib.sha256(member).hexdigest());save()
 env={k:os.environ[k] for k in ['PATH','LD_LIBRARY_PATH'] if k in os.environ};env['CUDA_VISIBLE_DEVICES']=''
 with (root/'compile.log').open('xb') as output:q=subprocess.run(['nice','-n','19',*command],cwd=root,env=env,stdout=output,stderr=output,timeout=90)
 if q.returncode:raise RuntimeError('isolated actual-reference operator harness link failed')
 target=root/'benchmark';state.update(phase='compiled',binary=dict(path=str(target),bytes=target.stat().st_size,sha256=sha(target)),finished_unix=time.time());save()
except BaseException as error:state.update(phase='failed',error=type(error).__name__+': '+str(error)[:2000],finished_unix=time.time());save();raise
'''
    gpu_worker='''import hashlib,json,os,signal,subprocess,time
from pathlib import Path
root=Path(__file__).resolve().parent;state=dict(unix=time.time(),phase='preflight',GPU_executed=False,model_forward_executed=False,model_speed_claimed=False);child=None
def save():
 t=root/'gpu-result.tmp';t.write_text(json.dumps(state,indent=2)+'\\n');t.replace(root/'gpu-result.json')
def interrupted(signum,frame):raise InterruptedError('finite exact operator harness signal/deadline')
try:
 os.umask(0o077);signal.signal(signal.SIGTERM,interrupted);signal.signal(signal.SIGINT,interrupted);signal.signal(signal.SIGALRM,interrupted);signal.alarm(315);save()
 p=json.loads((root/'gpu-plan.json').read_text());q=json.loads((root/'compile-result.json').read_text());binary=Path(q['binary']['path'])
 if root.resolve()!=root or root.stat().st_uid!=10250 or os.getuid()!=10250 or q['phase']!='compiled' or binary.is_symlink() or hashlib.sha256(binary.read_bytes()).hexdigest()!=q['binary']['sha256']:raise RuntimeError('exact isolated operator benchmark binary differs')
 proof=json.loads((root.parent/'resource-result.json').read_text())
 if proof['phase']!='passed' or proof['gpus'][0]['uuid']!='GPU-099e8da4-8df0-c431-9e67-eb721e59beee' or time.time()+450>proof['leaseEndsUnix']:raise RuntimeError('finite exact isolated H200 benchmark lease differs')
 old=json.loads((Path(p['profiler_root'])/'result.json').read_text())
 if old['phase']!='complete' or not old['hardware_counters_collected'] or not old['engine_reaped'] or not old['native_target_gone'] or len(old['requests'])!=4:raise RuntimeError('exact actual1M hardware profile must finish/reap first')
 def idle():return not subprocess.run(['nvidia-smi','--query-compute-apps=pid','--format=csv,noheader,nounits'],capture_output=True,text=True,check=True,timeout=5).stdout.strip()
 if not idle():raise RuntimeError('exact optimization GPU must be idle before operator harness')
 env={k:os.environ[k] for k in ['PATH','LD_LIBRARY_PATH'] if k in os.environ};env.update(CUDA_VISIBLE_DEVICES='0',OMP_NUM_THREADS='1',OPENBLAS_NUM_THREADS='1',GGML_NO_BACKTRACE='1')
 with (root/'benchmark.stdout.log').open('xb') as out,(root/'benchmark.stderr.log').open('xb') as err:
  child=subprocess.Popen([str(binary)],cwd=root,env=env,stdin=subprocess.DEVNULL,stdout=out,stderr=err,start_new_session=True)
  handle=dict(pid=child.pid,start_ticks=int(Path('/proc/'+str(child.pid)+'/stat').read_text().split(') ',1)[1].split()[19]),uid=os.getuid(),namespace='optimization pod')
  (root/'benchmark-process.json').write_text(json.dumps(handle,indent=2)+'\\n');state.update(phase='running',GPU_executed=True,process=handle);save();child.wait(timeout=300)
 raw=(root/'benchmark.stdout.log').read_bytes()
 if len(raw)>2*1024**2:raise RuntimeError('bounded operator benchmark result exceeded')
 entries=[json.loads(line) for line in raw.decode().splitlines() if line.strip()];state['cases']=[row for row in entries if row['kind']=='case'];state['summary']=[row for row in entries if row['kind']=='summary'];save()
 if child.returncode or len(state['cases'])!=38 or len(state['summary'])!=1 or state['summary'][0]['phase']!='passed' or any(row['phase']!='passed' or not row['all_IDs_equal'] or not row['graph_changed_input_exact'] for row in state['cases']):raise RuntimeError('exact38 original-reference/CUDA-graph operator gates failed')
 state.update(phase='passed',finished_unix=time.time());save()
except BaseException as error:state.update(phase='failed',error=type(error).__name__+': '+str(error)[:2000],finished_unix=time.time());save()
finally:
 signal.alarm(0)
 if child is not None and child.poll() is None:
  child.terminate()
  try:child.wait(timeout=5)
  except subprocess.TimeoutExpired:child.kill();child.wait(timeout=5)
 state['engine_reaped']=child is None or child.poll() is not None
 try:state['gpu_idle_after']=not subprocess.run(['nvidia-smi','--query-compute-apps=pid','--format=csv,noheader,nounits'],capture_output=True,text=True,check=True,timeout=5).stdout.strip()
 except BaseException as error:state['gpu_idle_observation_error']=type(error).__name__+': '+str(error)[:500]
 save()
'''
    compile(worker,'exact operator harness compiler','exec');compile(gpu_worker,'finite operator GPU worker','exec')
    files.update({'worker.py':worker.encode(),'gpu-worker.py':gpu_worker.encode(),'plan.json':(json.dumps(record,indent=2)+'\n').encode()})
    with ip.open('x') as output:output.write(json.dumps(record,indent=2)+'\n')
    code='''import base64,hashlib,json,os
from pathlib import Path
r=@R@;root=Path(r['root']);data=@DATA@
if root.exists() or root.parent.resolve()!=root.parent or root.parent.stat().st_uid!=10250 or os.getuid()!=10250:raise RuntimeError('exclusive exact operator harness stage differs')
os.umask(0o077);root.mkdir(mode=0o700)
for name,encoded in data.items():
 with (root/name).open('xb') as output:output.write(base64.b64decode(encoded))
print(json.dumps(dict(worker_sha256=hashlib.sha256((root/'worker.py').read_bytes()).hexdigest(),gpu_worker_sha256=hashlib.sha256((root/'gpu-worker.py').read_bytes()).hexdigest())))
'''.replace('@R@',repr(record)).replace('@DATA@',repr({name:base64.b64encode(raw).decode() for name,raw in files.items()}))
    record['stage']=jump(code);record['phase']='launch_requested';ip.write_text(json.dumps(record,indent=2)+'\n')
    code='''import hashlib,json,os,subprocess,time
from pathlib import Path
r=@R@;root=Path(r['root']);p=root/'worker.py'
if root.resolve()!=root or root.stat().st_uid!=10250 or p.is_symlink() or hashlib.sha256(p.read_bytes()).hexdigest()!=r['stage']['worker_sha256'] or (root/'compile-process.json').exists():raise RuntimeError('exact sealed operator compiler activation differs')
with (root/'worker.log').open('xb') as log:q=subprocess.Popen(['timeout','--signal=TERM','--kill-after=15','120','python3','-u',str(p)],cwd=root,env=dict(os.environ,CUDA_VISIBLE_DEVICES=''),stdin=subprocess.DEVNULL,stdout=log,stderr=log,start_new_session=True)
h=dict(pid=q.pid,start_ticks=int(Path('/proc/'+str(q.pid)+'/stat').read_text().split(') ',1)[1].split()[19]),uid=os.getuid(),unix=time.time(),namespace='optimization pod',budget_seconds=120)
(root/'compile-process.json').write_text(json.dumps(h,indent=2)+'\\n');print(json.dumps(h))
'''.replace('@R@',repr(record))
    record.update(process=owned(code),phase='launched');ip.write_text(json.dumps(record,indent=2)+'\n');print(json.dumps(dict(phase=record['phase'],process=record['process'],GPU_executed=False)))
elif action=='launch_gpu':
    r=json.loads(ip.read_text());gp=BASE/'xolotl-topk-harness-v195r1-gpu-intent.json'
    if gp.exists():raise RuntimeError('exact operator GPU benchmark has a handle; observe it')
    current=observe();ncu=json.loads((BASE/'xolotl-ncu-actual1m-v172r2-intent.json').read_text())
    latest=json.loads((BASE/'xolotl-ncu-actual1m-v172r2-latest.json').read_text());q=latest['result.json']
    if current['job_uid']!='7455d989-8255-4f7c-ae8f-3cc0aa5928e4' or latest['exact_worker_alive'] or q['phase']!='complete' or not q.get('engine_reaped') or not q.get('native_target_gone') or not q.get('hardware_counters_collected'):raise RuntimeError('exact actual1M profile must finish and reap before GPU benchmark')
    gpu=dict(root=r['root'],phase='launch_requested',profiler_root=ncu['root'],allocation_uid=current['job_uid'],stage=r['stage'],model_forward_executed=False)
    with gp.open('x') as output:output.write(json.dumps(gpu,indent=2)+'\n')
    code='''import hashlib,json,os,subprocess,time
from pathlib import Path
r=@R@;root=Path(r['root']);p=root/'gpu-worker.py';q=json.loads((root/'compile-result.json').read_text())
if root.resolve()!=root or root.stat().st_uid!=10250 or q['phase']!='compiled' or p.is_symlink() or hashlib.sha256(p.read_bytes()).hexdigest()!=r['stage']['gpu_worker_sha256'] or (root/'gpu-process.json').exists():raise RuntimeError('exact sealed finite operator GPU activation differs')
with (root/'gpu-plan.json').open('x') as output:output.write(json.dumps(r,indent=2)+'\\n')
with (root/'gpu-worker.log').open('xb') as log:q=subprocess.Popen(['timeout','--signal=TERM','--kill-after=15','330','python3','-u',str(p)],cwd=root,env=dict(os.environ,CUDA_VISIBLE_DEVICES=''),stdin=subprocess.DEVNULL,stdout=log,stderr=log,start_new_session=True)
h=dict(pid=q.pid,start_ticks=int(Path('/proc/'+str(q.pid)+'/stat').read_text().split(') ',1)[1].split()[19]),uid=os.getuid(),unix=time.time(),namespace='optimization pod',budget_seconds=330)
(root/'gpu-process.json').write_text(json.dumps(h,indent=2)+'\\n');print(json.dumps(h))
'''.replace('@R@',repr(gpu))
    gpu.update(process=owned(code),phase='launched');gp.write_text(json.dumps(gpu,indent=2)+'\n');print(json.dumps(dict(phase=gpu['phase'],process=gpu['process'])))
else:
    r=json.loads(ip.read_text());gpu=action=='observe_gpu';name='gpu' if gpu else 'compile'
    code='''import json,time
from pathlib import Path
r=@R@;root=Path(r['root']);kind=@KIND@;h=json.loads((root/(kind+'-process.json')).read_text());p=Path('/proc/'+str(h['pid'])+'/stat');out=dict(unix=time.time(),alive=p.exists() and p.stat().st_uid==10250 and int(p.read_text().split(') ',1)[1].split()[19])==h['start_ticks'] and p.read_text().split(') ',1)[1].split()[0]!='Z')
for name in [kind+'-result.json','worker.log' if kind=='compile' else 'gpu-worker.log','compile.log' if kind=='compile' else 'benchmark.stderr.log','compile.log' if kind=='compile' else 'benchmark.stdout.log']:
 p=root/name
 if p.exists():
  if p.is_symlink() or p.stat().st_size>4*1024**2:raise RuntimeError('bounded exact operator observation exceeded')
  out[name]=json.loads(p.read_text()) if name.endswith('.json') else p.read_text()[-5000:]
print(json.dumps(out))
'''.replace('@R@',repr(r)).replace('@KIND@',repr(name))
    result=owned(code);(BASE/('xolotl-topk-harness-v195r1-'+name+'-latest.json')).write_text(json.dumps(result,indent=2)+'\n')
    q=result.get(name+'-result.json',{});print(json.dumps(dict(phase=q.get('phase'),alive=result['alive'],cases=len(q.get('cases',[])),error=q.get('error'),engine_reaped=q.get('engine_reaped'),gpu_idle=q.get('gpu_idle_after')),indent=2))
