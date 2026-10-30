"""Compile one isolated SM90 prototype with CUDA hidden; do not link or change serving code."""
import base64
import hashlib
import json
from xolotl_trial_common import BASE,jump,lease,observe,owned
ip=BASE/'xolotl-topk-compile-v193-intent.json'
if ip.exists():raise RuntimeError('exact Top-K CPU compile already has a handle; inspect it')
source=json.loads((BASE/'xolotl-cccl-headers-v191-latest.json').read_text())
if source['alive'] or source['result.json']['phase']!='prepared':raise RuntimeError('exact official header preparation must finish')
headers=source['result.json']['headers'];root=lease()['remote']+'/topk-cccl35-v193-cpu'
o=observe()
if o['phase']!='Running' or o['job_uid']!='7455d989-8255-4f7c-ae8f-3cc0aa5928e4':raise RuntimeError('exact independent optimization allocation differs')
files={name:(BASE/name).read_bytes() for name in ['aster_topk_order_v192.hpp','test_topk_order_v192.cpp','aster_topk35_api_v192.hpp','aster_topk_cccl35_v192.cu']}
worker='''import hashlib,json,os,subprocess,time
from pathlib import Path
root=Path(__file__).resolve().parent;p=json.loads((root/'plan.json').read_text());state=dict(unix=time.time(),phase='preflight',GPU_executed=False,model_changed=False,tests=[])
def save():
 t=root/'result.tmp';t.write_text(json.dumps(state,indent=2)+'\\n');t.replace(root/'result.json')
def run(command,label,budget,env):
 state.update(phase='executing',current=label);save()
 with (root/(label+'.stdout.log')).open('xb') as out,(root/(label+'.stderr.log')).open('xb') as err:q=subprocess.run(['nice','-n','19',*command],cwd=root,env=env,stdout=out,stderr=err,timeout=budget)
 if q.returncode:raise RuntimeError('bounded isolated Top-K CPU command failed: '+label)
 return (root/(label+'.stdout.log')).read_text()
try:
 os.umask(0o077);save()
 if root.resolve()!=root or root.stat().st_uid!=10250 or os.getuid()!=10250:raise RuntimeError('exact isolated compiler owner differs')
 for row in p['files']:
  f=root/row['file']
  if f.is_symlink() or f.stat().st_uid!=10250 or f.stat().st_size!=row['bytes'] or hashlib.sha256(f.read_bytes()).hexdigest()!=row['sha256']:raise RuntimeError('exact frozen Top-K prototype differs')
 headers=Path(p['headers']);manifest=json.loads((headers.parent/'header-manifest.json').read_text())
 if manifest['commit']!='28101b07413743e1dcca8ca8af1cfc63e8cff2d1' or len(manifest['files'])>20000:raise RuntimeError('exact official compiler header set differs')
 for row in manifest['files']:
  f=headers/row['file']
  if f.is_symlink() or f.stat().st_uid!=10250 or f.stat().st_size!=row['bytes'] or hashlib.sha256(f.read_bytes()).hexdigest()!=row['sha256']:raise RuntimeError('exact pinned official header bytes differ')
 env={key:os.environ[key] for key in ['PATH','LD_LIBRARY_PATH'] if key in os.environ};env.update(CUDA_VISIBLE_DEVICES='',OMP_NUM_THREADS='1',ASAN_OPTIONS='detect_leaks=1:halt_on_error=1',UBSAN_OPTIONS='halt_on_error=1:print_stacktrace=1')
 for label,flags in [('release',['-O3']),('sanitized',['-O1','-g','-fsanitize=address,undefined','-fno-omit-frame-pointer'])]:
  run(['/usr/bin/c++','-std=c++20','-Wall','-Wextra','-Werror',*flags,'test_topk_order_v192.cpp','-o','test-'+label],label+'-compile',25,env)
  text=run([str(root/('test-'+label))],label+'-test',20,env);q=json.loads(text)
  if q['phase']!='passed' or q['contracts']!=14680074 or q['GPU_executed']:raise RuntimeError('exact tie/zero/NaN radix contract proof differs')
  state['tests'].append(dict(mode=label,result=q));save()
 target=root/'topk-cccl35.o';command=['/usr/local/cuda/bin/nvcc','-O3','-DNDEBUG','-std=c++17','--generate-code=arch=compute_90,code=sm_90','-I'+str(headers/'cub'),'-I'+str(headers/'thrust'),'-I'+str(headers/'libcudacxx/include'),'-Xptxas=-v','-c',str(root/'aster_topk_cccl35_v192.cu'),'-o',str(target)]
 state['cuda_compile_command']=command;save();run(command,'SM90-prototype-compile',120,env)
 state.update(phase='compiled',object=dict(path=str(target),bytes=target.stat().st_size,sha256=hashlib.sha256(target.read_bytes()).hexdigest()),official_header_commit=manifest['commit'],all_original_GPU_archives_unchanged=True,finished_unix=time.time());save()
except BaseException as error:state.update(phase='failed',error=type(error).__name__+': '+str(error)[:2000],finished_unix=time.time());save();raise
'''
compile(worker,'isolated-Top-K-compiler-v193','exec')
record=dict(root=root,headers=headers,phase='staging',allocation_uid=o['job_uid'],GPU_executed=False,
    files=[dict(file=name,bytes=len(raw),sha256=hashlib.sha256(raw).hexdigest()) for name,raw in files.items()])
files.update({'worker.py':worker.encode(),'plan.json':(json.dumps(record,indent=2)+'\n').encode()})
with ip.open('x') as output:output.write(json.dumps(record,indent=2)+'\n')
code='''import base64,hashlib,json,os
from pathlib import Path
r=@R@;root=Path(r['root']);data=@DATA@
if root.exists() or root.parent.resolve()!=root.parent or root.parent.stat().st_uid!=10250 or os.getuid()!=10250:raise RuntimeError('exclusive isolated compiler stage differs')
os.umask(0o077);root.mkdir(mode=0o700)
for name,encoded in data.items():
 with (root/name).open('xb') as output:output.write(base64.b64decode(encoded))
print(json.dumps(dict(staged=True,worker_sha256=hashlib.sha256((root/'worker.py').read_bytes()).hexdigest())))
'''.replace('@R@',repr(record)).replace('@DATA@',repr({name:base64.b64encode(raw).decode() for name,raw in files.items()}))
record['stage']=jump(code);record['phase']='launch_requested';ip.write_text(json.dumps(record,indent=2)+'\n')
code='''import hashlib,json,os,subprocess,time
from pathlib import Path
r=@R@;root=Path(r['root']);worker=root/'worker.py'
if root.resolve()!=root or root.stat().st_uid!=10250 or worker.is_symlink() or hashlib.sha256(worker.read_bytes()).hexdigest()!=r['stage']['worker_sha256'] or (root/'process.json').exists():raise RuntimeError('exact sealed compiler activation differs')
with (root/'worker.log').open('xb') as log:q=subprocess.Popen(['timeout','--signal=TERM','--kill-after=15','240','python3','-u',str(worker)],cwd=root,env=dict(os.environ,CUDA_VISIBLE_DEVICES=''),stdin=subprocess.DEVNULL,stdout=log,stderr=log,start_new_session=True)
h=dict(pid=q.pid,start_ticks=int(Path('/proc/'+str(q.pid)+'/stat').read_text().split(') ',1)[1].split()[19]),uid=os.getuid(),unix=time.time(),namespace='optimization pod',budget_seconds=240)
(root/'process.json').write_text(json.dumps(h,indent=2)+'\\n');print(json.dumps(h))
'''.replace('@R@',repr(record))
record.update(process=owned(code),phase='launched');ip.write_text(json.dumps(record,indent=2)+'\n')
print(json.dumps(dict(phase=record['phase'],process=record['process'],GPU_executed=False,model_changed=False)))
