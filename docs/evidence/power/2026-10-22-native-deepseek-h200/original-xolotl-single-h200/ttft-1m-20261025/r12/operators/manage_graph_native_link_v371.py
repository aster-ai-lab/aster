"""CPU-only native link on the current exact C15 optimization allocation."""
import argparse,base64,hashlib,json,time
from xolotl_trial_common import BASE,jump,lease,observe,owned
def read(n):return json.loads((BASE/n).read_text())
action=argparse.ArgumentParser();action.add_argument('action',choices=['launch','observe']);a=action.parse_args().action
ip=BASE/'xolotl-graph-native-link-v371-intent.json';outp=BASE/'xolotl-graph-native-link-v371-latest.json'
o=observe()
if o['job_uid']!='5bc306b2-e9e1-4e25-bd05-d78eca2b4833' or o['phase']!='Running' or o.get('resource-result.json',{}).get('phase')!='passed':raise RuntimeError('fresh exact C15 proof required')
if a=='observe':
 r=read(ip.name)
 code='''import json,time
from pathlib import Path
r=@R@;root=Path(r['root']);h=r.get('process') or json.loads((root/'process.json').read_text());p=Path('/proc/'+str(h['pid'])+'/stat')
if root.resolve()!=root or root.stat().st_uid!=10250:raise RuntimeError('exact C15 native link root differs')
out=dict(unix=time.time(),exact_worker_alive=p.exists() and p.stat().st_uid==10250 and int(p.read_text().split(') ',1)[1].split()[19])==h['start_ticks'] and p.read_text().split(') ',1)[1].split()[0]!='Z')
for name in ['result.json','worker.log','link-control.log','link-candidate.log','mode-compile.log']:
 p=root/name
 if p.exists():
  if p.is_symlink() or p.stat().st_uid!=10250 or p.stat().st_size>1048576:raise RuntimeError('bounded native link output differs')
  out[name]=json.loads(p.read_text()) if name.endswith('.json') else p.read_text(errors='replace')[-3000:]
print(json.dumps(out))
'''.replace('@R@',repr(r))
 out=owned(code);outp.write_text(json.dumps(out,indent=2)+'\n');print(json.dumps(out,indent=2));raise SystemExit
if ip.exists():raise RuntimeError('C15 native link already has an exact handle; observe it')
qualified=read('xolotl-graph-guard-gpu-v342r2-latest.json')['result']
if qualified['phase']!='passed' or not qualified['full_score_byte_identity_passed'] or qualified['candidate_core_sha256']!='786ea84ef9bcddb0ee2c93378448cb78a5680b7cb0af4eb0eb1411c8b4addfd2':raise RuntimeError('exact completed CUDA core qualification required')
parent=read('xolotl-topk-nsys-link-v250-plan.json');donor=read('xolotl-graph-guard-gpu-v342r2-intent.json')
requested=[dict(path=donor['root']+'/core-candidate.o',sha256=qualified['candidate_core_sha256'])]
for name in ['graph_source_type_mode_v338.cpp','aster_graph_source_type_v338.hpp']:
 row=next(x for x in donor['files'] if x['file']==name);requested.append(dict(path=donor['root']+'/'+name,bytes=row['bytes'],sha256=row['sha256']))
code='''import hashlib,json
from pathlib import Path
rows=@ROWS@;out=[]
for row in rows:
 p=Path(row['path'])
 if p.is_symlink() or p.stat().st_uid!=10250 or p.stat().st_size>4*1024**2 or hashlib.sha256(p.read_bytes()).hexdigest()!=row['sha256'] or ('bytes' in row and p.stat().st_size!=row['bytes']):raise RuntimeError('exact qualified graph core donor differs')
 out.append(dict(path=str(p),bytes=p.stat().st_size,sha256=row['sha256']))
print(json.dumps(out))
'''.replace('@ROWS@',repr(requested))
inputs=jump(code)
p=dict(schema='aster-graph-native-link-v371',native=parent['native'],bridge=parent['bridge'],prototype=parent['prototype'],control_sha256=parent['control_sha256'],candidate_core=inputs[0],mode_source=inputs[1]['path'],
 inputs=parent['native']['inputs']+[parent['bridge'],parent['prototype']]+inputs,gpu_uuid=o['resource-result.json']['gpus'][0]['uuid'],allocation_uid=o['job_uid'],donor_GPU_qualification='xolotl-graph-guard-gpu-v342r2-latest.json',GPU_executed=False,formal_service_changed=False)
root=lease()['remote']+'/graph-native-link-v371-cpu'
files={'worker.py':(BASE/'graph_native_link_worker_v370.py').read_bytes(),'plan.json':(json.dumps(p,indent=2)+'\n').encode()}
r=dict(unix=time.time(),phase='staging',root=root,allocation_uid=o['job_uid'],gpu_uuid=p['gpu_uuid'],files=[dict(file=n,bytes=len(b),sha256=hashlib.sha256(b).hexdigest()) for n,b in files.items()])
with ip.open('x') as f:f.write(json.dumps(r,indent=2)+'\n')
code='''import base64,json,os
from pathlib import Path
r=@R@;root=Path(r['root']);data=@DATA@
if os.getuid()!=10250 or root.exists() or root.parent.resolve()!=root.parent or root.parent.stat().st_uid!=10250:raise RuntimeError('exclusive C15 CPU stage differs')
os.umask(0o077);root.mkdir(mode=0o700)
for name,b in data.items():
 with (root/name).open('xb') as f:f.write(base64.b64decode(b))
print(json.dumps(dict(staged=True)))
'''.replace('@R@',repr(r)).replace('@DATA@',repr({n:base64.b64encode(b).decode() for n,b in files.items()}))
r['stage']=jump(code);r['phase']='launch_requested';ip.write_text(json.dumps(r,indent=2)+'\n')
code='''import hashlib,json,os,subprocess,time
from pathlib import Path
r=@R@;root=Path(r['root'])
if root.resolve()!=root or root.stat().st_uid!=10250 or (root/'process.json').exists():raise RuntimeError('exclusive C15 native activation differs')
for row in r['files']:
 p=root/row['file']
 if p.is_symlink() or p.stat().st_uid!=10250 or p.stat().st_size!=row['bytes'] or hashlib.sha256(p.read_bytes()).hexdigest()!=row['sha256']:raise RuntimeError('sealed C15 native link payload differs')
with (root/'worker.log').open('xb') as log:q=subprocess.Popen(['timeout','--signal=TERM','--kill-after=15','290','python3','-u',str(root/'worker.py')],cwd=root,env=dict(os.environ,CUDA_VISIBLE_DEVICES=''),stdin=subprocess.DEVNULL,stdout=log,stderr=log,start_new_session=True)
h=dict(pid=q.pid,start_ticks=int(Path('/proc/'+str(q.pid)+'/stat').read_text().split(') ',1)[1].split()[19]),uid=os.getuid(),unix=time.time(),namespace='C15 optimization pod',budget_seconds=290)
(root/'process.json').write_text(json.dumps(h,indent=2)+'\\n');print(json.dumps(h))
'''.replace('@R@',repr(r))
try:r.update(process=owned(code),phase='launched')
except BaseException:r['phase']='launch_outcome_requires_inspection';ip.write_text(json.dumps(r,indent=2)+'\n');raise
ip.write_text(json.dumps(r,indent=2)+'\n');print(json.dumps(dict(phase=r['phase'],process=r['process'],GPU_executed=False)))
