"""Stage or observe one finite tiny CUDA interactive-profiler prerequisite on the idle owned H200."""
import argparse,base64,hashlib,json,time
from xolotl_trial_common import BASE,observe,owned,lease
p=argparse.ArgumentParser();p.add_argument('action',choices=['launch','observe']);a=p.parse_args();ip=BASE/'xolotl-interactive-nsys-smoke-v390-intent.json'
o=observe()
if o['phase']!='Running' or o['job_uid']!='5bc306b2-e9e1-4e25-bd05-d78eca2b4833':raise RuntimeError('exact single optimization H200 required')
if a.action=='observe':
 r=json.loads(ip.read_text())
 q=owned('''import json,time
from pathlib import Path
r=@R@;root=Path(r['root']);h=r['process'];p=Path('/proc/'+str(h['pid'])+'/stat')
out=dict(unix=time.time(),exact_worker_alive=p.exists() and p.stat().st_uid==10250 and int(p.read_text().split(') ',1)[1].split()[19])==h['start_ticks'] and p.read_text().split(') ',1)[1].split()[0]!='Z')
for name in ['result.json','worker.log','compile.log','start.log','stop.log','export.log','target.stderr.log']:
 p=root/name
 if p.exists():
  if p.is_symlink() or p.stat().st_size>262144:raise RuntimeError('bounded exact interactive smoke artifact exceeded')
  out[name]=json.loads(p.read_text()) if name.endswith('.json') else p.read_text()[-2000:]
print(json.dumps(out))
'''.replace('@R@',repr(r)))
 (BASE/'xolotl-interactive-nsys-smoke-v390-latest.json').write_text(json.dumps(q,indent=2)+'\n');s=q.get('result.json',{})
 print(json.dumps(dict(alive=q['exact_worker_alive'],phase=s.get('phase'),error=s.get('error'),counts=s.get('counts'),target_reaped=s.get('target_reaped'),GPU_idle_after=s.get('GPU_idle_after')),indent=2));raise SystemExit
if ip.exists():raise RuntimeError('tiny interactive prerequisite already has a handle; observe it')
comparison=json.loads((BASE/'xolotl-graph-native-model-v375-controller.json').read_text())
if comparison['phase']!='comparison_completed' or comparison['completed_profiles']!=['control_a','guard','control_b']:raise RuntimeError('all three full-model profiles must finish before tiny GPU profiling')
for profile in comparison['profiles']:
 observation=json.loads((BASE/('xolotl-graph-native-v373-'+profile+'-latest.json')).read_text());q=observation['service-longctx-v1-result.json']
 if q['phase']!='stopped' or not q.get('api_reaped') or observation['exact_supervisor_alive']:raise RuntimeError('all exact model supervisors must be drained')
inventory=json.loads((BASE/'xolotl-explicit-nsys-device-v389.json').read_text());gpu=inventory['NVML_visible_GPU']
parts=gpu['pci_bus_id'].lower().split(':')
if len(parts)!=3 or parts[0]!='00000000':raise RuntimeError('observed PCI domain form differs')
expected='\t0: Hopper GH100 | NVIDIA H200 PCI[0000:'+parts[1]+':'+parts[2]+']'
if gpu['index']!=0 or gpu['uuid']!='GPU-da0c6213-da08-a516-298a-002a7a2899b7' or inventory['nsys_GPU_inventory']['returncode']!=0 or expected not in inventory['nsys_GPU_inventory']['stdout']:raise RuntimeError('exact literal explicit GPU inventory differs')
files={n:(BASE/n).read_bytes() for n in ['h200_interactive_nsys_smoke_v314.cu','aster_nsys_lifecycle_v302.py']};files['worker.py']=(BASE/'h200_interactive_nsys_worker_v390.py').read_bytes()
plan=dict(gpu_uuid=gpu['uuid'],nsys_gpu_index=0,NVML_inventory_line='0, '+gpu['uuid']+', '+gpu['pci_bus_id'],files=[dict(file=n,bytes=len(b),sha256=hashlib.sha256(b).hexdigest()) for n,b in files.items()])
files['plan.json']=(json.dumps(plan,indent=2)+'\n').encode();r=dict(unix=time.time(),phase='launch_requested',root=lease()['remote']+'/interactive-nsys-smoke-v390',allocation_uid=o['job_uid'],files=[dict(file=n,bytes=len(b),sha256=hashlib.sha256(b).hexdigest()) for n,b in files.items()])
with ip.open('x') as f:f.write(json.dumps(r,indent=2)+'\n')
code='''import base64,hashlib,json,os,subprocess,time
from pathlib import Path
r=@R@;root=Path(r['root']);data=@DATA@
if root.exists() or root.parent.resolve()!=root.parent or root.parent.stat().st_uid!=10250:raise RuntimeError('exclusive owned interactive-smoke stage differs')
root.mkdir(mode=0o700)
for row in r['files']:
 raw=base64.b64decode(data[row['file']])
 if len(raw)!=row['bytes'] or hashlib.sha256(raw).hexdigest()!=row['sha256']:raise RuntimeError('exact interactive prerequisite bytes differ')
 with (root/row['file']).open('xb') as f:f.write(raw)
with (root/'worker.log').open('xb') as log:p=subprocess.Popen(['timeout','--signal=TERM','--kill-after=20','190','python3','-u',str(root/'worker.py')],cwd=root,stdin=subprocess.DEVNULL,stdout=log,stderr=log,start_new_session=True)
h=dict(pid=p.pid,start_ticks=int(Path('/proc/'+str(p.pid)+'/stat').read_text().split(') ',1)[1].split()[19]),uid=os.getuid(),namespace='optimization pod',budget_seconds=190)
with (root/'process.json').open('x') as f:f.write(json.dumps(h)+'\\n')
print(json.dumps(h))
'''.replace('@R@',repr(r)).replace('@DATA@',repr({n:base64.b64encode(b).decode() for n,b in files.items()}))
compile(code,'interactive-nsys-prerequisite-v314','exec');r['process']=owned(code);r['phase']='launched';ip.write_text(json.dumps(r,indent=2)+'\n');print(json.dumps(r['process']))
