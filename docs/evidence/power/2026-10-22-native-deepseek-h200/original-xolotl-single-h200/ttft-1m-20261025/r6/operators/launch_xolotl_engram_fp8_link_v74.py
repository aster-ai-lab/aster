"""Link once, only after the exact complete A/B/A has been drained on the optimization GPU."""
import base64
import hashlib
import json
import time
from xolotl_trial_common import BASE, jump, lease, observe, owned

intent=BASE/'xolotl-engram-fp8-link-v74-intent.json'
if intent.exists():
    raise RuntimeError('exact native link handle exists; inspect it')
controller=json.loads((BASE/'xolotl-engram-comparison-v71r1-controller.json').read_text())
if controller['phase']!='passed' or [x['profile'] for x in controller['profiles']]!=['gpfs_a','local','gpfs_b'] or any(x['natural_tests']!=7 or x['extended_tests']!=12 for x in controller['profiles']):
    raise RuntimeError('all57 exact Engram directory comparison requests must pass first')
cpu=json.loads((BASE/'xolotl-engram-fp8-build-v73-intent.json').read_text())
service=json.loads((BASE/'xolotl-engram-disk-v68-gpfs_b-intent.json').read_text())
resource=observe()
if resource['job_uid']!=service['allocation_uid']:
    raise RuntimeError('exact optimization link allocation differs')
code='''import json,time
from pathlib import Path
r=@R@;p=Path('/proc/'+str(r['process']['pid'])+'/stat')
alive=p.exists() and p.stat().st_uid==10250 and int(p.read_text().split(') ',1)[1].split()[19])==r['process']['start_ticks'] and p.read_text().split(') ',1)[1].split()[0]!='Z'
q=json.loads((Path(r['root'])/'build-result.json').read_text())
if alive or q['phase']!='compiled' or not q['original_object_byte_equal'] or not q['default0_object_byte_equal']:raise RuntimeError('exact host Engram build must pass and finish')
print(json.dumps({'phase':'passed','cpu_process_gone':True}))
'''.replace('@R@',repr(cpu))
jump(code)
owned('''import json,subprocess
from pathlib import Path
r=@R@;q=json.loads((Path(r['root'])/'service-longctx-v1-result.json').read_text())
if q['phase']!='stopped' or not q.get('api_reaped'):raise RuntimeError('exact completed GPFS control must be stopped and reaped')
active=subprocess.run(['nvidia-smi','--query-compute-apps=pid','--format=csv,noheader,nounits'],capture_output=True,text=True,check=True,timeout=5).stdout.strip()
if active:raise RuntimeError('private optimization GPU must be idle before native relink')
print(json.dumps({'gpu_idle':True,'private_control_reaped':True}))
'''.replace('@R@',repr(service)))
plan=dict(cpu_root=cpu['root'],link_cwd=json.loads((BASE/'xolotl-draft-cumulative-v49-generator-parent.json').read_text())['plan']['parent']+'/build',
          control_binary_sha256='34f3c963304445e50c4ae398194f4b4745a0b1868a6b4a1d6668068aebcf96e6')
files={'native_link_worker.py':(BASE/'engram-fp8-lut-v72/native_link_worker_v74.py').read_bytes(),
       'native-link-plan.json':(json.dumps(plan,indent=2)+'\n').encode()}
record=dict(unix=time.time(),phase='prepared',root=lease()['remote']+'/build-engram-fp8-v74',allocation_uid=service['allocation_uid'],gpu_uuid=service['gpu_uuid'],
            gpu_qualified=False,files=[dict(file=name,bytes=len(raw),sha256=hashlib.sha256(raw).hexdigest()) for name,raw in files.items()])
code='''import base64,hashlib,json,os,subprocess,time
from pathlib import Path
r=@R@;data=@DATA@;root=Path(r['root'])
if root.parent.resolve()!=root.parent or root.parent.stat().st_uid!=10250 or root.exists():raise RuntimeError('exclusive native FP8 link root differs')
os.umask(0o077);root.mkdir(mode=0o700)
for row in r['files']:
 raw=base64.b64decode(data[row['file']])
 if len(raw)!=row['bytes'] or hashlib.sha256(raw).hexdigest()!=row['sha256']:raise RuntimeError('exact native FP8 link input differs')
 with (root/row['file']).open('xb') as output:output.write(raw)
with (root/'native-link-worker.log').open('xb') as log:
 p=subprocess.Popen(['timeout','--signal=TERM','--kill-after=15','170','python3','-u',str(root/'native_link_worker.py')],cwd=root,env=dict(os.environ,CUDA_VISIBLE_DEVICES=''),stdin=subprocess.DEVNULL,stdout=log,stderr=log,start_new_session=True)
process=dict(pid=p.pid,start_ticks=int(Path('/proc/'+str(p.pid)+'/stat').read_text().split(') ',1)[1].split()[19]),uid=os.getuid(),unix=time.time(),budget_seconds=170,namespace='optimization pod')
(root/'native-link-process.json').write_text(json.dumps(process,indent=2)+chr(10));print(json.dumps(process))
'''.replace('@R@',repr(record)).replace('@DATA@',repr({name:base64.b64encode(raw).decode() for name,raw in files.items()}))
compile(code,'native-engram-FP8-link-v74','exec')
with intent.open('x') as output:output.write(json.dumps(record,indent=2)+'\n')
record['phase']='launch_requested';intent.write_text(json.dumps(record,indent=2)+'\n')
try:
    record.update(process=owned(code),phase='launched')
except BaseException:
    record['phase']='launch_outcome_requires_inspection';intent.write_text(json.dumps(record,indent=2)+'\n');raise
intent.write_text(json.dumps(record,indent=2)+'\n')
print(json.dumps(dict(phase=record['phase'],process=record['process']),indent=2))
