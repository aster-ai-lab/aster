"""Compile CUDA only after completed full-window tests and private API reaping."""
import base64
import hashlib
import json
import shlex
import time
from xolotl_trial_common import BASE,lease,observe,owned,jump

ip=BASE/'xolotl-indexer-gpu-build-v40r2-intent.json'
if ip.exists():raise RuntimeError('V40 CUDA build has an exact handle; inspect it')
o=observe();r=json.loads((BASE/'xolotl-resident-compare-v37-v35_packed-intent.json').read_text())
cpu=json.loads((BASE/'xolotl-indexer-cpu-preflight-v40-intent.json').read_text())
if o['job_uid']!=r['allocation_uid'] or o['job_uid']!=cpu['allocation_uid']:raise RuntimeError('exact optimization allocation differs')
stage=BASE/'indexer-v40r2';root=lease()['remote']+'/build-indexer-mask-v40r2'
names=['aster_indexer_mask_policy.hpp','aster_indexer_mask_skip.cuh','lightning-indexer.cu','indexer_mask_probe.cu','indexer_graph_gate.cpp','gpu_build_worker.py']
files={n:(stage/n).read_bytes() for n in names}
meta=json.loads((BASE/'xolotl-indexer-v40-parent-provenance.json').read_text())['parents'][0]
recipe=json.loads((BASE/'xolotl-indexer-v40-original-build-recipes.json').read_text())['recipes'][0]
cuda=shlex.split(recipe['command'])
cuda[cuda.index('-c')+1]=root+'/lightning-indexer.cu'
cuda[cuda.index('-o')+1]=root+'/lightning-indexer.cu.o'
include=meta['root']+'/source/third_party/llama.cpp/ggml'
cuda[1:1]=['-I'+root,'-I'+include+'/src/ggml-cuda']
probe=list(cuda);probe[probe.index('-c')+1]=root+'/indexer_mask_probe.cu';probe[probe.index('-o')+1]=root+'/indexer_mask_probe.cu.o'
graph=['/usr/bin/c++','-O3','-DNDEBUG','-std=c++20','-I'+include+'/include','-I'+include+'/src','-I'+include+'/src/ggml-cuda','-c',root+'/indexer_graph_gate.cpp','-o',root+'/indexer_graph_gate.cpp.o']
v35=json.loads((BASE/'xolotl-resident-mmq-build-v35-intent.json').read_text())
v15=json.loads((BASE/'xolotl-ttft-build-v15-intent.json').read_text())
link=shlex.split(json.loads((BASE/'xolotl-ttft-v11-build-recipe.json').read_text())['link_command'][5:-5])
host='CMakeFiles/ds41_generate.dir/tools/v41/engine/ds41_generate.cpp.o'
link[link.index(host)]=v15['root']+'/ds41_generate.cpp.o'
link.insert(link.index('libds41_moe_cuda.a'),v35['root']+'/ds41_moe.cpp.o')
plan=dict(parent=meta['root'],cpu_root=cpu['root'],dense_archive_sha256=meta['archive_sha256'],cuda_original_source_sha256='957dea8b4e66a1c9c63058a546f0eeb716a64c388f81e8aca7cf2bcf1b661aac',v35_control_link=link,v35_binary_sha256=r['plan']['binary_sha256'],generate_host_object=v15['root']+'/ds41_generate.cpp.o',cuda_compile=cuda,probe_compile=probe,graph_compile=graph,sources=[dict(file=n,bytes=len(b),sha256=hashlib.sha256(b).hexdigest()) for n,b in files.items()])
files['gpu-build-plan.json']=(json.dumps(plan,indent=2)+'\n').encode()
record=dict(unix=time.time(),phase='prepared',root=root,allocation_uid=o['job_uid'],gpu_uuid=r['gpu_uuid'],gpu_executed=False,files=[dict(file=n,bytes=len(b),sha256=hashlib.sha256(b).hexdigest()) for n,b in files.items()])
code='''import json,subprocess
from pathlib import Path
r=@SERVICE@;cpu=@CPU@
q=json.loads((Path(r['root'])/'service-longctx-v1-result.json').read_text())
if q['phase']!='stopped' or not q.get('api_reaped'):raise RuntimeError('packed private API must be stopped/reaped after completed full-window tests')
if json.loads((Path(cpu['root'])/'cpu-result.json').read_text())['phase']!='passed':raise RuntimeError('exact CPU preflight must pass')
apps=subprocess.run(['nvidia-smi','--query-compute-apps=pid','--format=csv,noheader,nounits'],capture_output=True,text=True,check=True,timeout=5).stdout.strip()
if apps:raise RuntimeError('model GPU must be idle before controlled CUDA build')
print(json.dumps({'private_api_reaped':True,'gpu_idle':True}))
'''.replace('@SERVICE@',repr(r)).replace('@CPU@',repr(cpu))
record['preflight']=owned(code)
with ip.open('x') as f:f.write(json.dumps(record,indent=2)+'\n')
code='''import base64,hashlib,json,os
from pathlib import Path
r=@R@;root=Path(r['root']);data=@DATA@
if os.getuid()!=10250 or root.parent.resolve()!=root.parent or root.parent.stat().st_uid!=10250 or root.exists():raise RuntimeError('exclusive CUDA build root differs')
root.mkdir(mode=0o700)
for row in r['files']:
 raw=base64.b64decode(data[row['file']])
 if len(raw)!=row['bytes'] or hashlib.sha256(raw).hexdigest()!=row['sha256']:raise RuntimeError('controlled CUDA source digest differs')
 with (root/row['file']).open('xb') as f:f.write(raw)
print(json.dumps({'staged':True,'files':len(r['files'])}))
'''.replace('@R@',repr(record)).replace('@DATA@',repr({n:base64.b64encode(b).decode() for n,b in files.items()}))
record['stage']=jump(code);record['phase']='staged';ip.write_text(json.dumps(record,indent=2)+'\n')
code='''import json,os,subprocess,time
from pathlib import Path
r=@R@;root=Path(r['root'])
if root.resolve()!=root or root.stat().st_uid!=10250:raise RuntimeError('controlled CUDA build owner differs')
with (root/'gpu-build-worker.log').open('xb') as log:
 p=subprocess.Popen(['timeout','--signal=TERM','--kill-after=15','590','python3','-u',str(root/'gpu_build_worker.py')],cwd=root,env=dict(os.environ,CUDA_VISIBLE_DEVICES=''),stdin=subprocess.DEVNULL,stdout=log,stderr=log,start_new_session=True)
process=dict(pid=p.pid,start_ticks=int(Path('/proc/'+str(p.pid)+'/stat').read_text().split(') ',1)[1].split()[19]),uid=os.getuid(),unix=time.time(),budget_seconds=590)
(root/'gpu-build-process.json').write_text(json.dumps(process,indent=2)+'\\n');print(json.dumps(process))
'''.replace('@R@',repr(record))
record['phase']='launch_requested';ip.write_text(json.dumps(record,indent=2)+'\n')
try:
    record['process']=owned(code);record['phase']='launched'
except BaseException:
    record['phase']='launch_outcome_requires_inspection';ip.write_text(json.dumps(record,indent=2)+'\n');raise
ip.write_text(json.dumps(record,indent=2)+'\n');print(json.dumps({'phase':record['phase'],'process':record['process'],'gpu_executed':False}))
