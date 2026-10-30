"""Install bounded same-pod availability guard, referencing the existing protected key."""
import base64,hashlib,json,time
from xolotl_serving_common import BASE,observe,owned
ip=BASE/'xolotl-availability-guard-v415-intent.json'
if ip.exists():raise RuntimeError('Guard intent exists; inspect rather than launch twice')
r=json.loads((BASE/'xolotl-stable-service-v368-intent.json').read_text());o=observe()
if o['job_uid']!=r['allocation_uid'] or o['phase']!='Running':raise RuntimeError('Exact formal C14 required')
root=r['root'].rsplit('/',1)[0]+'/availability-guard-v415'
plan=dict(service_root=r['root'],binary=r['plan']['binary'],gpu_uuid=r['gpu_uuid'],lease_ends_unix=r['plan']['lease_ends_unix'],credential_file=r['root']+'/credentials/api-key.txt')
payload={n:(BASE/n).read_bytes() for n in ['aster_stall_window_v415.py','test_stall_window_v415.py','h200_availability_guard_v415.py']}
payload['guard-plan.json']=(json.dumps(plan,indent=2)+'\n').encode()
intent=dict(unix=time.time(),phase='launch_requested',root=root,allocation_uid=r['allocation_uid'],same_GPU=True,new_GPU_allocated=False,
 model_configuration_changed=False,credential_copied=False,optimization_resumed=False,
 files=[dict(file=n,bytes=len(b),sha256=hashlib.sha256(b).hexdigest()) for n,b in payload.items()])
with ip.open('x') as f:f.write(json.dumps(intent,indent=2)+'\n')
code='''import base64,hashlib,json,os,subprocess,sys,time
from pathlib import Path
r=@R@;intent=@I@;data=@DATA@;root=Path(intent['root']);service=Path(r['root'])
if root.parent!=service.parent or root.parent.stat().st_uid!=10250 or root.exists() or time.time()+600>r['plan']['lease_ends_unix']:raise RuntimeError('Exact fresh owned guard root/lease required')
if hashlib.sha256((service/'credentials/api-key.txt').read_bytes()).hexdigest()!=r['plan']['credential_file_sha256']:raise RuntimeError('Existing protected credential differs')
state=json.loads((service/'service-longctx-v1-result.json').read_text())
if state['phase']!='serving':raise RuntimeError('Existing service required')
root.mkdir(mode=0o700)
for row in intent['files']:
 b=base64.b64decode(data[row['file']])
 if len(b)!=row['bytes'] or hashlib.sha256(b).hexdigest()!=row['sha256']:raise RuntimeError('Guard payload differs')
 with (root/row['file']).open('xb') as f:f.write(b)
env=dict(os.environ,PYTHONDONTWRITEBYTECODE='1')
q=subprocess.run([sys.executable,'-B','-m','unittest','test_stall_window_v415'],cwd=root,env=env,capture_output=True,text=True,timeout=15)
if q.returncode:raise RuntimeError('Linux guard unit tests failed')
with (root/'cpu-test-result.json').open('x') as f:f.write(json.dumps(dict(exit=0,tests=10,stderr=q.stderr))+'\\n')
with (root/'guard.log').open('xb') as log:
 p=subprocess.Popen([sys.executable,'-B','-u',str(root/'h200_availability_guard_v415.py')],cwd=root,env=env,stdin=subprocess.DEVNULL,stdout=log,stderr=log,start_new_session=True)
ticks=int(Path('/proc/'+str(p.pid)+'/stat').read_text().split(') ',1)[1].split()[19])
out=dict(unix=time.time(),pid=p.pid,start_ticks=ticks,uid=os.getuid(),namespace='C14 pod',root=str(root),tests=10)
with (root/'guard-process.json').open('x') as f:f.write(json.dumps(out)+'\\n')
print(json.dumps(out))
'''.replace('@R@',repr(r)).replace('@I@',repr(intent)).replace('@DATA@',repr({n:base64.b64encode(b).decode() for n,b in payload.items()}))
try:intent.update(process=owned(code,70),phase='launched')
except BaseException as e:
 intent.update(phase='launch_outcome_requires_inspection',error_type=type(e).__name__);ip.write_text(json.dumps(intent,indent=2)+'\n');raise
ip.write_text(json.dumps(intent,indent=2)+'\n');print(json.dumps(dict(phase=intent['phase'],process=intent['process'],no_new_GPU=True,model_configuration_changed=False),indent=2))
