"""Run the frozen synthetic byte gate only on the idle optimization GPU."""
import json, time
from xolotl_trial_common import BASE, observe, owned

ip = BASE / 'xolotl-mixed-cache-gpu-gate-v22-run-intent.json'
if ip.exists():
    raise RuntimeError('synthetic GPU gate run intent exists; inspect the recorded outcome')
r = json.loads((BASE / 'xolotl-mixed-cache-gpu-gate-v22-intent.json').read_text())
baseline = json.loads((BASE / 'xolotl-stable-service-v8-intent.json').read_text())
observation = observe()
if observation['job_uid'] != r['allocation_uid'] or observation['resource-result.json']['gpus'][0]['uuid'] != r['gpu_uuid']:
    raise RuntimeError('exact optimization GPU gate allocation differs')
record = dict(unix=time.time(), phase='prepared', root=r['root'], allocation_uid=r['allocation_uid'], gpu_uuid=r['gpu_uuid'])
with ip.open('x') as stream:
    stream.write(json.dumps(record, indent=2) + '\n')
code = '''import hashlib,json,os,subprocess,time
from pathlib import Path
r=@R@;baseline=@BASELINE@;root=Path(r['root']);q=json.loads((Path(baseline['root'])/'service-longctx-v1-result.json').read_text())
if q['phase']!='stopped' or not q.get('api_reaped'):raise RuntimeError('V15 must be stopped and reaped before synthetic GPU byte gate')
if root.resolve()!=root or root.stat().st_uid!=10250:raise RuntimeError('synthetic GPU gate root differs')
compiled=json.loads((root/'compile-result.json').read_text());binary=root/'mixed_cache_gpu_test'
if compiled['phase']!='built' or binary.is_symlink() or binary.stat().st_uid!=10250 or hashlib.sha256(binary.read_bytes()).hexdigest()!=compiled['binary_sha256']:raise RuntimeError('compiled GPU byte gate differs')
gpu=subprocess.run(['nvidia-smi','--query-gpu=uuid','--format=csv,noheader,nounits'],capture_output=True,text=True,check=True,timeout=4).stdout.strip()
active=subprocess.run(['nvidia-smi','--query-compute-apps=pid','--format=csv,noheader,nounits'],capture_output=True,text=True,check=True,timeout=4).stdout.strip()
if gpu!=r['gpu_uuid'] or active:raise RuntimeError('synthetic byte gate requires its single idle GPU')
state=dict(unix=time.time(),phase='running',gpu_uuid=gpu,binary_sha256=compiled['binary_sha256'])
(root/'gpu-result.json').write_text(json.dumps(state,indent=2)+'\\n')
env=dict(os.environ,CUDA_VISIBLE_DEVICES='0')
with (root/'gpu-test.stdout.log').open('xb') as out,(root/'gpu-test.stderr.log').open('xb') as err:
 p=subprocess.run([str(binary)],cwd=root,env=env,stdin=subprocess.DEVNULL,stdout=out,stderr=err,timeout=12)
stdout=(root/'gpu-test.stdout.log').read_text();stderr=(root/'gpu-test.stderr.log').read_text()
if len(stdout)>65536 or len(stderr)>65536:raise RuntimeError('bounded synthetic GPU gate log exceeded')
state.update(phase='passed' if p.returncode==0 and stdout.startswith('PASS mixed cache: 3 shrink/grow cycles, ') and not stderr else 'failed',returncode=p.returncode,stdout=stdout,stderr=stderr,finished_unix=time.time())
(root/'gpu-result.json').write_text(json.dumps(state,indent=2)+'\\n');print(json.dumps(state))
'''.replace('@R@', repr(r)).replace('@BASELINE@', repr(baseline))
record['phase'] = 'run_requested'
ip.write_text(json.dumps(record, indent=2) + '\n')
try:
    record['result'] = owned(code)
    record['phase'] = record['result']['phase']
except BaseException:
    record['phase'] = 'outcome_requires_inspection'
    ip.write_text(json.dumps(record, indent=2) + '\n')
    raise
ip.write_text(json.dumps(record, indent=2) + '\n')
print(json.dumps(record, indent=2))
