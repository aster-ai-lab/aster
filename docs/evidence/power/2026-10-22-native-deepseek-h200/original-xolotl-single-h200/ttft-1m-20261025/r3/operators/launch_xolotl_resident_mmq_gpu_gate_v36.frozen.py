"""Run bounded real-weight correctness and timing gates on the idle test GPU."""
import base64
import hashlib
import json
import time
from xolotl_trial_common import BASE, observe, owned

ip = BASE/'xolotl-resident-mmq-gpu-gate-v36-intent.json'
if ip.exists():
    raise RuntimeError('V36 has an exact handle; inspect it')
r = json.loads((BASE/'xolotl-resident-mmq-build-v35-intent.json').read_text())
o = observe()
if o['job_uid']!=r['allocation_uid'] or o['resource-result.json']['gpus'][0]['uuid']!=r['gpu_uuid']:
    raise RuntimeError('exact isolated GPU changed')
slot = json.loads((BASE/'xolotl-mixed-cache-gpu-gate-v18r2-intent.json').read_text())
built = json.loads((BASE/'xolotl-mixed-cache-gpu-gate-v18r2-latest.json').read_text())
if built.get('compile-result.json',{}).get('phase')!='built':
    raise RuntimeError('slot byte gate binary not built')
plan = dict(gpu_uuid=r['gpu_uuid'],weights='/mnt/shared-storage-user/yangchao/Workspace/aster-h200-xq/native-payload-73df1934302f/V41-XQ-1017-v3.gguf',slot_gate=dict(path=slot['root']+'/mixed_cache_gpu_test',sha256=built['compile-result.json']['binary_sha256']))
payload = {'gpu_gate_worker.py':(BASE/'resident-mmq-v35/gpu_gate_worker.py').read_bytes(),'gpu-plan.json':(json.dumps(plan,indent=2)+'\n').encode()}
rec = dict(unix=time.time(),phase='prepared',root=r['root'],allocation_uid=r['allocation_uid'],gpu_uuid=r['gpu_uuid'],files=[dict(file=n,bytes=len(b),sha256=hashlib.sha256(b).hexdigest()) for n,b in payload.items()])
with ip.open('x') as f:
    f.write(json.dumps(rec,indent=2)+'\n')
code = '''import base64,hashlib,json,os,subprocess,time
from pathlib import Path
r=@R@;data=@DATA@;root=Path(r['root'])
if root.resolve()!=root or root.stat().st_uid!=10250:raise RuntimeError('GPU gate root differs')
if json.loads((root/'result.json').read_text())['phase']!='built':raise RuntimeError('candidate must be built first')
apps=subprocess.run(['nvidia-smi','--query-compute-apps=pid','--format=csv,noheader,nounits'],capture_output=True,text=True,check=True,timeout=4).stdout.strip()
if apps:raise RuntimeError('GPU must be idle before numerical gate')
for row in r['files']:
 raw=base64.b64decode(data[row['file']])
 if len(raw)!=row['bytes'] or hashlib.sha256(raw).hexdigest()!=row['sha256']:raise RuntimeError('exact GPU gate input differs')
 with (root/row['file']).open('xb') as f:f.write(raw)
with (root/'gpu-worker.log').open('xb') as log:
 p=subprocess.Popen(['timeout','--signal=TERM','--kill-after=15','520','python3','-u',str(root/'gpu_gate_worker.py')],cwd=root,stdin=subprocess.DEVNULL,stdout=log,stderr=log,start_new_session=True)
process=dict(pid=p.pid,start_ticks=int(Path('/proc/'+str(p.pid)+'/stat').read_text().split(') ',1)[1].split()[19]),uid=os.getuid(),unix=time.time(),budget_seconds=520)
(root/'gpu-process.json').write_text(json.dumps(process,indent=2)+'\\n');print(json.dumps(process))
'''.replace('@R@',repr(rec)).replace('@DATA@',repr({n:base64.b64encode(b).decode() for n,b in payload.items()}))
rec['phase'] = 'launch_requested'
ip.write_text(json.dumps(rec,indent=2)+'\n')
try:
    rec['process'] = owned(code)
    rec['phase'] = 'launched'
except BaseException:
    rec['phase'] = 'launch_outcome_requires_inspection'
    ip.write_text(json.dumps(rec,indent=2)+'\n')
    raise
ip.write_text(json.dumps(rec,indent=2)+'\n')
print(json.dumps({'phase':rec['phase'],'process':rec['process'],'formal_service_unchanged':True}))
