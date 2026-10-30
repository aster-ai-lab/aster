"""Fix only the gate's profile flush, retaining the failed first gate unchanged."""
import base64
import hashlib
import json
import shlex
import time
from xolotl_trial_common import BASE, lease, observe, owned

ip = BASE/'xolotl-resident-mmq-gpu-gate-v36r2-intent.json'
if ip.exists():
    raise RuntimeError('V36r2 has an exact handle; inspect it')
build = json.loads((BASE/'xolotl-resident-mmq-build-v35-intent.json').read_text())
old = json.loads((BASE/'xolotl-resident-mmq-gpu-gate-v36-intent.json').read_text())
o = observe()
if o['job_uid']!=build['allocation_uid']:
    raise RuntimeError('exact V35 build allocation differs')
source = (BASE/'resident-mmq-v35/test_resident_mmq.cpp').read_text()
if source.count('    tier.close();')!=1:
    raise RuntimeError('exact upstream gate cleanup differs')
source = source.replace('    tier.close();','    tier.release_chunk();   // flush existing per-prompt numeric counters before close\n    tier.close();')
local = BASE/'resident-mmq-v36r2'
local.mkdir(exist_ok=False)
(local/'test_resident_mmq.cpp').write_text(source,newline='\n')
worker = (BASE/'resident-mmq-v35/gpu_gate_worker.py').read_bytes()
(local/'gpu_gate_worker.py').write_bytes(worker)
plan = json.loads((BASE/'xolotl-mixed-keep-v19-build-recipe.json').read_text())
link = json.loads((BASE/'xolotl-ttft-v11-build-recipe.json').read_text())
root = lease()['remote']+'/gate-resident-mmq-v36r2'
args = shlex.split(link['link_command'][5:-5])
args[args.index('CMakeFiles/ds41_generate.dir/tools/v41/engine/ds41_generate.cpp.o')] = root+'/test_resident_mmq.cpp'
args[args.index('-o')+1] = root+'/test_resident_mmq'
args.insert(args.index('libds41_moe_cuda.a'),build['root']+'/ds41_moe.cpp.o')
parent = plan['parent']
args[1:1] = ['-std=c++20','-I'+parent+'/source/include','-I'+parent+'/source/tools/v41/engine','-I'+parent+'/source/third_party/llama.cpp/ggml/include']
oldplan = json.loads((BASE/'xolotl-resident-mmq-gpu-gate-v36-intent.json').read_text())
payload = {'test_resident_mmq.cpp':source.encode(),'gpu_gate_worker.py':worker}
r = dict(unix=time.time(),phase='prepared',root=root,allocation_uid=o['job_uid'],gpu_uuid=build['gpu_uuid'],prior_gate_root=old['root'],prior_gate_process=old['process'],build_root=build['root'],compile_argv=args,compile_cwd=parent+'/build',files=[dict(file=n,bytes=len(b),sha256=hashlib.sha256(b).hexdigest()) for n,b in payload.items()])
with ip.open('x') as f:f.write(json.dumps(r,indent=2)+'\n')
code = '''import base64,hashlib,json,os,subprocess,time
from pathlib import Path
r=@R@;data=@DATA@;root=Path(r['root']);prior=Path(r['prior_gate_root'])
q=json.loads((prior/'gpu-result.json').read_text())
p=Path('/proc/'+str(r['prior_gate_process']['pid'])+'/stat')
alive=p.exists() and p.stat().st_uid==10250 and int(p.read_text().split(') ',1)[1].split()[19])==r['prior_gate_process']['start_ticks'] and p.read_text().split(') ',1)[1].split()[0]!='Z'
if q['phase']!='failed' or alive:raise RuntimeError('prior exact gate must be terminal before corrected gate')
if root.parent.resolve()!=root.parent or root.parent.stat().st_uid!=10250 or root.exists():raise RuntimeError('corrected gate exclusive root differs')
apps=subprocess.run(['nvidia-smi','--query-compute-apps=pid','--format=csv,noheader,nounits'],capture_output=True,text=True,check=True,timeout=4).stdout.strip()
if apps:raise RuntimeError('gate requires idle test GPU')
root.mkdir(mode=0o700)
for row in r['files']:
 raw=base64.b64decode(data[row['file']])
 if len(raw)!=row['bytes'] or hashlib.sha256(raw).hexdigest()!=row['sha256']:raise RuntimeError('corrected gate source digest differs')
 with (root/row['file']).open('xb') as f:f.write(raw)
with (root/'gate-compile.log').open('xb') as log:
 q=subprocess.run(r['compile_argv'],cwd=r['compile_cwd'],env=dict(os.environ,CUDA_VISIBLE_DEVICES=''),stdout=log,stderr=log,timeout=18)
if q.returncode:raise RuntimeError('corrected gate compile exit '+str(q.returncode))
built=json.loads((Path(r['build_root'])/'result.json').read_text())
built.update(real_weight_gate_binary={'path':str(root/'test_resident_mmq'),'sha256':hashlib.sha256((root/'test_resident_mmq').read_bytes()).hexdigest()},gate_only_profile_flush_corrected=True,inherited_controlled_build=r['build_root'])
(root/'result.json').write_text(json.dumps(built,indent=2)+'\\n')
plan=json.loads((prior/'gpu-plan.json').read_text())
(root/'gpu-plan.json').write_text(json.dumps(plan,indent=2)+'\\n')
with (root/'gpu-worker.log').open('xb') as log:
 p=subprocess.Popen(['timeout','--signal=TERM','--kill-after=15','520','python3','-u',str(root/'gpu_gate_worker.py')],cwd=root,stdin=subprocess.DEVNULL,stdout=log,stderr=log,start_new_session=True)
process=dict(pid=p.pid,start_ticks=int(Path('/proc/'+str(p.pid)+'/stat').read_text().split(') ',1)[1].split()[19]),uid=os.getuid(),unix=time.time(),budget_seconds=520)
(root/'gpu-process.json').write_text(json.dumps(process,indent=2)+'\\n');print(json.dumps(process))
'''.replace('@R@',repr(r)).replace('@DATA@',repr({n:base64.b64encode(b).decode() for n,b in payload.items()}))
r['phase'] = 'launch_requested'
ip.write_text(json.dumps(r,indent=2)+'\n')
try:
    r['process'] = owned(code)
    r['phase'] = 'launched'
except BaseException:
    r['phase'] = 'launch_outcome_requires_inspection'
    ip.write_text(json.dumps(r,indent=2)+'\n')
    raise
ip.write_text(json.dumps(r,indent=2)+'\n')
print(json.dumps({'phase':r['phase'],'process':r['process'],'unchanged_candidate_binary':True,'test_fix':'flush per-prompt numeric counters before upstream close'}))
