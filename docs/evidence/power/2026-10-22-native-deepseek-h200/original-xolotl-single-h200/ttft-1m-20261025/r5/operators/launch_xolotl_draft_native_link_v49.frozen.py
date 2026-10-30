"""Link one opt-in generator candidate only on the exact idle optimization resource."""
import base64
import hashlib
import json
import time
from xolotl_trial_common import BASE, jump, lease, observe, owned

intent = BASE / 'xolotl-draft-cumulative-v49-native-link-intent.json'
if intent.exists():
    raise RuntimeError('native link has an exact handle; inspect it')
service = json.loads((BASE / 'xolotl-dense-subbatch-v47-dense128_repeat-intent.json').read_text())
controller = json.loads((BASE / 'xolotl-dense-subbatch-v47r1-controller.json').read_text())
if controller['phase'] != 'passed' or len(controller['profiles']) != 3 or any(p['tests'] != 28 or p['phase'] != 'passed' for p in controller['profiles']):
    raise RuntimeError('all84 exact same-GPU sub-batch gates must finish and pass first')
cpu = json.loads((BASE / 'xolotl-draft-cumulative-v49-generator-cpu-intent.json').read_text())
old = json.loads((BASE / 'xolotl-indexer-gpu-build-v40r2-intent.json').read_text())
observation = observe()
if observation['job_uid'] != service['allocation_uid'] or observation['job_uid'] != old['allocation_uid']:
    raise RuntimeError('exact optimization link allocation differs')
code = '''import hashlib,json
from pathlib import Path
old=@OLD@;cpu=@CPU@
p=Path('/proc/'+str(cpu['process']['pid'])+'/stat')
alive=p.exists() and p.stat().st_uid==10250 and int(p.read_text().split(') ',1)[1].split()[19])==cpu['process']['start_ticks'] and p.read_text().split(') ',1)[1].split()[0]!='Z'
c=json.loads((Path(cpu['root'])/'generator-cpu-build-result.json').read_text())
if alive or c['phase']!='compiled' or not c['original_generator_object_byte_equal']:raise RuntimeError('exact original and candidate CPU compilation must finish')
row=next(v for v in old['files'] if v['file']=='gpu-build-plan.json');p=Path(old['root'])/'gpu-build-plan.json'
if p.is_symlink() or p.stat().st_uid!=10250 or p.stat().st_size!=row['bytes'] or hashlib.sha256(p.read_bytes()).hexdigest()!=row['sha256']:raise RuntimeError('exact baseline link recipe differs')
plan=json.loads(p.read_text());paths=[Path(plan['generate_host_object']),Path(old['root'])/'ds41_generate',Path(old['root'])/'libds41_dense.a',Path(old['root'])/'lightning-indexer.cu.o',Path(c['candidate_object']['path'])]
print(json.dumps(dict(plan=plan,cpu=c,inputs=[dict(path=str(p),bytes=p.stat().st_size,sha256=hashlib.sha256(p.read_bytes()).hexdigest()) for p in paths])))
'''.replace('@OLD@', repr(old)).replace('@CPU@', repr(cpu))
result = jump(code)
control = list(result['plan']['v35_control_link'])
control[control.index('libds41_dense.a')] = old['root'] + '/libds41_dense.a'
control.insert(control.index('ggml/src/ggml-cuda/libggml-cuda.a'), old['root'] + '/lightning-indexer.cu.o')
root = lease()['remote'] + '/build-draft-cumulative-v49'
plan = dict(link_cwd=result['plan']['parent'] + '/build', control_link=control,
            control_binary_sha256=service['plan']['binary_sha256'], original_host_object=result['plan']['generate_host_object'],
            candidate_host_object=result['cpu']['candidate_object']['path'], cpu_root=cpu['root'], inputs=result['inputs'])
if plan['control_binary_sha256'] != 'fa0195a9b64f5e18136eec6852997cd0637f3197e9b6b97e4f512c5f50d5f373':
    raise RuntimeError('exact qualified V44 baseline differs')
preflight = '''import json,subprocess
from pathlib import Path
r=@SERVICE@;q=json.loads((Path(r['root'])/'service-longctx-v1-result.json').read_text())
if q['phase']!='stopped' or not q.get('api_reaped'):raise RuntimeError('exact last private control must be stopped and reaped first')
active=subprocess.run(['nvidia-smi','--query-compute-apps=pid','--format=csv,noheader,nounits'],capture_output=True,text=True,check=True,timeout=5).stdout.strip()
if active:raise RuntimeError('private optimization GPU must be idle before native relink')
print(json.dumps({'gpu_idle':True,'private_control_reaped':True}))
'''.replace('@SERVICE@', repr(service))
owned(preflight)
files = {'native_link_worker.py': (BASE / 'draft-cumulative-v49/native_link_worker.py').read_bytes(),
         'native-link-plan.json': (json.dumps(plan, indent=2) + '\n').encode()}
record = dict(unix=time.time(), phase='prepared', root=root, allocation_uid=service['allocation_uid'],
              gpu_uuid=service['gpu_uuid'], gpu_qualified=False,
              files=[dict(file=n, bytes=len(v), sha256=hashlib.sha256(v).hexdigest()) for n, v in files.items()])
code = '''import base64,hashlib,json,os,subprocess,time
from pathlib import Path
r=@R@;root=Path(r['root']);data=@DATA@
if root.parent.resolve()!=root.parent or root.parent.stat().st_uid!=10250 or root.exists():raise RuntimeError('exclusive native link root differs')
os.umask(0o077);root.mkdir(mode=0o700)
for row in r['files']:
 raw=base64.b64decode(data[row['file']])
 if len(raw)!=row['bytes'] or hashlib.sha256(raw).hexdigest()!=row['sha256']:raise RuntimeError('frozen native link input differs')
 with (root/row['file']).open('xb') as f:f.write(raw)
with (root/'native-link-worker.log').open('xb') as log:
 p=subprocess.Popen(['timeout','--signal=TERM','--kill-after=15','160','python3','-u',str(root/'native_link_worker.py')],cwd=root,env=dict(os.environ,CUDA_VISIBLE_DEVICES=''),stdin=subprocess.DEVNULL,stdout=log,stderr=log,start_new_session=True)
process=dict(pid=p.pid,start_ticks=int(Path('/proc/'+str(p.pid)+'/stat').read_text().split(') ',1)[1].split()[19]),uid=os.getuid(),unix=time.time(),budget_seconds=160)
(root/'native-link-process.json').write_text(json.dumps(process,indent=2)+'\\n');print(json.dumps(process))
'''.replace('@R@', repr(record)).replace('@DATA@', repr({n: base64.b64encode(v).decode() for n, v in files.items()}))
compile(code, 'controlled-cumulative-native-link-v49', 'exec')
with intent.open('x') as output:
    output.write(json.dumps(record, indent=2) + '\n')
record['phase'] = 'launch_requested'
intent.write_text(json.dumps(record, indent=2) + '\n')
try:
    record['process'] = owned(code)
    record['phase'] = 'launched'
except BaseException:
    record['phase'] = 'launch_outcome_requires_inspection'
    intent.write_text(json.dumps(record, indent=2) + '\n')
    raise
intent.write_text(json.dumps(record, indent=2) + '\n')
print(json.dumps(dict(phase=record['phase'], process=record['process'], gpu_qualified=False)))
