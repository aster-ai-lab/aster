"""One finite tiny-kernel GPU counter preflight after the exact operator worker reaps."""
import argparse
import base64
import hashlib
import json
from xolotl_trial_common import BASE, jump, lease, observe, owned

parser=argparse.ArgumentParser();parser.add_argument('action',choices=['launch','observe']);action=parser.parse_args().action
ip=BASE/'xolotl-ncu-smoke-v205-intent.json'
if action=='observe':
    record=json.loads(ip.read_text())
    code='''import json,time
from pathlib import Path
root=Path(@ROOT@);h=json.loads((root/'process.json').read_text());p=Path('/proc/'+str(h['pid'])+'/stat');result=dict(unix=time.time(),alive=p.exists() and p.stat().st_uid==10250 and int(p.read_text().split(') ',1)[1].split()[19])==h['start_ticks'] and p.read_text().split(') ',1)[1].split()[0]!='Z')
for name in ['result.json','worker.log','compile.log','private-ncu.csv','smoke.stdout.log','smoke.stderr.log']:
 p=root/name
 if p.exists():
  if p.is_symlink() or p.stat().st_size>2*1024**2:raise RuntimeError('bounded exact counter-smoke observation exceeded')
  result[name]=json.loads(p.read_text()) if name.endswith('.json') else p.read_text()[-5000:]
print(json.dumps(result))
'''.replace('@ROOT@',repr(record['root']))
    result=owned(code);(BASE/'xolotl-ncu-smoke-v205-latest.json').write_text(json.dumps(result,indent=2)+'\n')
    print(json.dumps(dict(alive=result['alive'],result=result.get('result.json',{})),indent=2));raise SystemExit
if ip.exists():raise RuntimeError('exact tiny-kernel profile has a handle; observe it')
current=observe();prior=json.loads((BASE/'xolotl-topk-harness-v203-gpu-latest.json').read_text());q=prior['gpu-result.json']
if current['job_uid']!='7455d989-8255-4f7c-ae8f-3cc0aa5928e4' or prior['alive'] or q['phase']!='passed' or not q['engine_reaped'] or not q['gpu_idle_after'] or len(q['cases'])!=38:raise RuntimeError('exact38 original-reference operator gates must finish/reap first')
files={name:(BASE/name).read_bytes() for name in ['h200_ncu_smoke_v204.cu','aster_ncu_metrics_v171.py']}
record=dict(root=lease()['remote']+'/ncu-smoke-v205',phase='staging',allocation_uid=current['job_uid'],
    prior_operator_root=json.loads((BASE/'xolotl-topk-harness-v203-intent.json').read_text())['root'],
    model_forward_executed=False,service_changed=False,
    files=[dict(file=name,bytes=len(raw),sha256=hashlib.sha256(raw).hexdigest()) for name,raw in files.items()])
worker='''import hashlib,json,os,signal,subprocess,time
from pathlib import Path
from aster_ncu_metrics_v171 import METRICS,numeric_rows
root=Path(__file__).resolve().parent;p=json.loads((root/'plan.json').read_text());state=dict(unix=time.time(),phase='preflight',model_forward_executed=False,service_changed=False,hardware_counters_collected=False);child=None
def save():
 t=root/'result.tmp';t.write_text(json.dumps(state,indent=2)+'\\n');t.replace(root/'result.json')
def interrupted(signum,frame):raise InterruptedError('finite tiny-kernel profiler deadline/signal')
def idle():return not subprocess.run(['nvidia-smi','--query-compute-apps=pid','--format=csv,noheader,nounits'],capture_output=True,text=True,check=True,timeout=5).stdout.strip()
try:
 os.umask(0o077);signal.signal(signal.SIGALRM,interrupted);signal.signal(signal.SIGTERM,interrupted);signal.signal(signal.SIGINT,interrupted);signal.alarm(145);save()
 if root.resolve()!=root or root.stat().st_uid!=10250 or os.getuid()!=10250:raise RuntimeError('exact owned smoke root differs')
 proof=json.loads((root.parent/'resource-result.json').read_text());old=json.loads((Path(p['prior_operator_root'])/'gpu-result.json').read_text())
 if proof['phase']!='passed' or proof['gpus'][0]['uuid']!='GPU-099e8da4-8df0-c431-9e67-eb721e59beee' or time.time()+180>proof['leaseEndsUnix'] or old['phase']!='passed' or not old['engine_reaped'] or not old['gpu_idle_after']:raise RuntimeError('exact idle finite optimization lease/previous worker differs')
 for row in p['files']:
  path=root/row['file']
  if path.is_symlink() or path.stat().st_size!=row['bytes'] or hashlib.sha256(path.read_bytes()).hexdigest()!=row['sha256']:raise RuntimeError('frozen counter smoke inputs differ')
 env={k:os.environ[k] for k in ['PATH','LD_LIBRARY_PATH'] if k in os.environ};env.update(CUDA_VISIBLE_DEVICES='',LC_ALL='C')
 command=['/usr/local/cuda/bin/nvcc','-O3','--generate-code=arch=compute_90,code=sm_90',str(root/'h200_ncu_smoke_v204.cu'),'-o',str(root/'smoke')]
 state.update(phase='compiling',compile_command=command);save()
 with (root/'compile.log').open('xb') as log:q=subprocess.run(['nice','-n','19',*command],cwd=root,env=env,stdout=log,stderr=log,timeout=35)
 if q.returncode:raise RuntimeError('tiny SM90 counter prerequisite compile failed')
 if not idle():raise RuntimeError('exact optimization GPU not idle before tiny-kernel profiler')
 command=['/usr/local/bin/ncu','--target-processes','all','--kernel-name-base','function','--kernel-name','aster_h200_counter_smoke','--launch-count','1','--clock-control','none','--cache-control','none','--metrics',','.join(METRICS),'--csv','--page','raw','--log-file',str(root/'private-ncu.csv'),str(root/'smoke')]
 state.update(phase='profiling',command=command);save();env['CUDA_VISIBLE_DEVICES']='0'
 with (root/'smoke.stdout.log').open('xb') as out,(root/'smoke.stderr.log').open('xb') as err:
  child=subprocess.Popen(command,cwd=root,env=env,stdin=subprocess.DEVNULL,stdout=out,stderr=err,start_new_session=True)
  state['process']=dict(pid=child.pid,start_ticks=int(Path('/proc/'+str(child.pid)+'/stat').read_text().split(') ',1)[1].split()[19]),uid=os.getuid(),namespace='optimization pod');save();child.wait(timeout=90)
 state['ncu_returncode']=child.returncode;save()
 path=root/'private-ncu.csv'
 if child.returncode or path.stat().st_size>2*1024**2:raise RuntimeError('tiny-kernel profiler did not complete successfully')
 rows=numeric_rows(path.read_text())
 if len(rows)!=8 or len({row['kernel_id'] for row in rows})!=1 or {row['symbol'] for row in rows}!={'aster_h200_counter_smoke(uint32_t *, int)'}:raise RuntimeError('exact one tiny kernel/eight numeric metrics differ')
 output=json.loads((root/'smoke.stdout.log').read_text())
 if output['phase']!='passed' or output['checked_output_words']!=65536 or output['model_forward_executed']:raise RuntimeError('tiny-kernel output correctness proof differs')
 state.update(phase='passed',hardware_counters_collected=True,model_hardware_counters_collected=False,numeric_metrics=rows,output=output,finished_unix=time.time());save()
except BaseException as error:state.update(phase='failed',error=type(error).__name__+': '+str(error)[:2000],finished_unix=time.time());save()
finally:
 signal.alarm(0)
 if child is not None and child.poll() is None:
  child.terminate()
  try:child.wait(timeout=5)
  except subprocess.TimeoutExpired:child.kill();child.wait(timeout=5)
 state['engine_reaped']=child is None or child.poll() is not None
 try:state['gpu_idle_after']=idle()
 except BaseException as error:state['idle_error']=type(error).__name__+': '+str(error)[:500]
 save()
'''
compile(worker,'bounded tiny-kernel counter preflight','exec')
files.update({'worker.py':worker.encode(),'plan.json':(json.dumps(record,indent=2)+'\n').encode()})
with ip.open('x') as output:output.write(json.dumps(record,indent=2)+'\n')
code='''import base64,hashlib,json,os
from pathlib import Path
r=@R@;root=Path(r['root']);data=@DATA@
if root.exists() or root.parent.resolve()!=root.parent or root.parent.stat().st_uid!=10250 or os.getuid()!=10250:raise RuntimeError('exclusive tiny-kernel stage differs')
os.umask(0o077);root.mkdir(mode=0o700)
for name,encoded in data.items():
 with (root/name).open('xb') as output:output.write(base64.b64decode(encoded))
print(json.dumps(dict(worker_sha256=hashlib.sha256((root/'worker.py').read_bytes()).hexdigest())))
'''.replace('@R@',repr(record)).replace('@DATA@',repr({name:base64.b64encode(raw).decode() for name,raw in files.items()}))
record['stage']=jump(code);record['phase']='launch_requested';ip.write_text(json.dumps(record,indent=2)+'\n')
code='''import hashlib,json,os,subprocess,time
from pathlib import Path
r=@R@;root=Path(r['root']);worker=root/'worker.py'
if root.resolve()!=root or root.stat().st_uid!=10250 or worker.is_symlink() or hashlib.sha256(worker.read_bytes()).hexdigest()!=r['stage']['worker_sha256'] or (root/'process.json').exists():raise RuntimeError('exact sealed tiny-kernel activation differs')
with (root/'worker.log').open('xb') as log:q=subprocess.Popen(['timeout','--signal=TERM','--kill-after=15','160','python3','-u',str(worker)],cwd=root,env=dict(os.environ,CUDA_VISIBLE_DEVICES=''),stdin=subprocess.DEVNULL,stdout=log,stderr=log,start_new_session=True)
h=dict(pid=q.pid,start_ticks=int(Path('/proc/'+str(q.pid)+'/stat').read_text().split(') ',1)[1].split()[19]),uid=os.getuid(),unix=time.time(),namespace='optimization pod',budget_seconds=160)
(root/'process.json').write_text(json.dumps(h,indent=2)+'\\n');print(json.dumps(h))
'''.replace('@R@',repr(record))
record.update(process=owned(code),phase='launched');ip.write_text(json.dumps(record,indent=2)+'\n');print(json.dumps(dict(phase=record['phase'],process=record['process'])))
