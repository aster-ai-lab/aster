"""Launch once on the proven idle C10 GPU; a timeout requires observation, never resubmission."""
import base64
import hashlib
import json
import time
from pathlib import Path
from xolotl_trial_common import BASE, jump, lease, observe, owned

ip = BASE / 'xolotl-pool-seed-native-v106-intent.json'
if ip.exists():
    raise RuntimeError('exact native-link handle already exists; observe it')
o = observe()
if (o['job_uid'] != '7455d989-8255-4f7c-ae8f-3cc0aa5928e4' or o['phase'] != 'Running'
        or o.get('resource-result.json', {}).get('phase') != 'passed'):
    raise RuntimeError('fresh exact C10 H200/CUDA ownership proof required')
formal = json.loads((BASE / 'xolotl-active-serving-reservation.json').read_text())
gpu = o['resource-result.json']['gpus'][0]['uuid']
if o['job_uid'] == formal['allocation_uid'] or gpu == formal['gpu_uuid']:
    raise RuntimeError('formal serving allocation must stay independent')
cpu = json.loads((BASE / 'xolotl-pool-seed-cpu-v105r2-intent.json').read_text())
old = json.loads((BASE / 'xolotl-pool-seed-v106-link-parent.json').read_text())
code = '''import hashlib,json
from pathlib import Path
c=@CPU@;p=Path('/proc/'+str(c['process']['pid'])+'/stat')
alive=p.exists() and p.stat().st_uid==10250 and int(p.read_text().split(') ',1)[1].split()[19])==c['process']['start_ticks'] and p.read_text().split(') ',1)[1].split()[0]!='Z'
q=json.loads((Path(c['root'])/'build-result.json').read_text())
if alive or q['phase']!='compiled' or not q['control_object_byte_equal']:raise RuntimeError('exact V94 CPU compilation unfinished')
p=Path(q['candidate_object']['path'])
if p.is_symlink() or p.stat().st_uid!=10250 or p.stat().st_size!=q['candidate_object']['bytes'] or hashlib.sha256(p.read_bytes()).hexdigest()!=q['candidate_object']['sha256']:raise RuntimeError('exact candidate generator object differs')
print(json.dumps({'candidate':q['candidate_object'],'cpu_result_path':str(Path(c['root'])/'build-result.json'),'cpu_result_sha256':hashlib.sha256((Path(c['root'])/'build-result.json').read_bytes()).hexdigest(),'cpu_result_bytes':(Path(c['root'])/'build-result.json').stat().st_size}))
'''.replace('@CPU@', repr(cpu))
checked = jump(code)
control = list(old['control_link'])
if control.count(old['original_host_object']) != 1:
    raise RuntimeError('exact original generator link position differs')
control[control.index(old['original_host_object'])] = old['candidate_host_object']
root = lease()['remote'] + '/build-pool-seed-v106'
plan = dict(link_cwd=old['link_cwd'], control_link=control,
            control_binary_sha256='c96e244dc7ffb1ebd56a2039c96506a81d702cd8c9e158c0f50c134edded2a6c',
            original_host_object=old['candidate_host_object'], candidate_host_object=checked['candidate']['path'],
            cpu_root=cpu['root'], inputs=old['inputs'] + [checked['candidate'],
                dict(path=checked['cpu_result_path'], bytes=checked['cpu_result_bytes'], sha256=checked['cpu_result_sha256'])],
            resource_directory=Path(lease()['remote']).name, expected_gpu_uuid=gpu,
            independent_formal_gpu_uuid=formal['gpu_uuid'])
preflight = '''import json,subprocess
p=subprocess.run(['nvidia-smi','--query-compute-apps=pid','--format=csv,noheader,nounits'],capture_output=True,text=True,check=True,timeout=5)
if p.stdout.strip():raise RuntimeError('exact optimization GPU must be idle before native link')
print(json.dumps({'gpu_idle':True}))
'''
owned(preflight)
files = {'native_link_worker.py': (BASE / 'pool_seed_native_worker_v106.frozen.py').read_bytes(),
         'native-link-plan.json': (json.dumps(plan, indent=2) + '\n').encode()}
r = dict(unix=time.time(), phase='prepared', root=root, allocation_uid=o['job_uid'],
         gpu_uuid=gpu, gpu_qualified=False,
         files=[dict(file=n, bytes=len(v), sha256=hashlib.sha256(v).hexdigest()) for n, v in files.items()])
code = '''import base64,hashlib,json,os,subprocess,time
from pathlib import Path
r=@R@;root=Path(r['root']);data=@DATA@
if root.parent.resolve()!=root.parent or root.parent.stat().st_uid!=10250 or root.exists():raise RuntimeError('exclusive native-link owner/root differs')
os.umask(0o077);root.mkdir(mode=0o700)
for row in r['files']:
 raw=base64.b64decode(data[row['file']])
 if len(raw)!=row['bytes'] or hashlib.sha256(raw).hexdigest()!=row['sha256']:raise RuntimeError('frozen native-link input differs')
 with (root/row['file']).open('xb') as f:f.write(raw)
with (root/'native-link-worker.log').open('xb') as log:
 p=subprocess.Popen(['timeout','--signal=TERM','--kill-after=15','200','python3','-u',str(root/'native_link_worker.py')],cwd=root,env=dict(os.environ,CUDA_VISIBLE_DEVICES=''),stdin=subprocess.DEVNULL,stdout=log,stderr=log,start_new_session=True)
process=dict(pid=p.pid,start_ticks=int(Path('/proc/'+str(p.pid)+'/stat').read_text().split(') ',1)[1].split()[19]),uid=os.getuid(),unix=time.time(),budget_seconds=200)
(root/'native-link-process.json').write_text(json.dumps(process,indent=2)+'\\n');print(json.dumps(process))
'''.replace('@R@', repr(r)).replace('@DATA@', repr({n:base64.b64encode(v).decode() for n,v in files.items()}))
compile(code, 'admission-seed-native-link-v98', 'exec')
with ip.open('x') as f:
    f.write(json.dumps(r, indent=2) + '\n')
r['phase'] = 'launch_requested'
ip.write_text(json.dumps(r, indent=2) + '\n')
try:
    r['process'] = owned(code)
    r['phase'] = 'launched'
except BaseException:
    r['phase'] = 'launch_outcome_requires_inspection'
    ip.write_text(json.dumps(r, indent=2) + '\n')
    raise
ip.write_text(json.dumps(r, indent=2) + '\n')
print(json.dumps(dict(phase=r['phase'], process=r['process'], gpu_qualified=False)))
