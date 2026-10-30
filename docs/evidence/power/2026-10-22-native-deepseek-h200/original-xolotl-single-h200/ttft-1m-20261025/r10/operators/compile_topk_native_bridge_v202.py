"""Build a separate default-off candidate; never replace or launch a model service."""
import argparse
import base64
import hashlib
import json
import time
from xolotl_trial_common import BASE, jump, lease, observe, owned

parser = argparse.ArgumentParser()
parser.add_argument('action', choices=['launch', 'observe'])
action = parser.parse_args().action
ip = BASE / 'xolotl-topk-native-bridge-v202-intent.json'
if action == 'observe':
    record = json.loads(ip.read_text())
    code = '''import json,time
from pathlib import Path
root=Path(@ROOT@);h=json.loads((root/'process.json').read_text());p=Path('/proc/'+str(h['pid'])+'/stat')
out=dict(unix=time.time(),alive=p.exists() and p.stat().st_uid==10250 and int(p.read_text().split(') ',1)[1].split()[19])==h['start_ticks'] and p.read_text().split(') ',1)[1].split()[0]!='Z')
for name in ['result.json','compile.log','worker.log','link-candidate.log','link-control.log']:
 p=root/name
 if p.exists():
  if p.is_symlink() or p.stat().st_size>4*1024**2:raise RuntimeError('bounded private compiler observation exceeded')
  out[name]=json.loads(p.read_text()) if name.endswith('.json') else p.read_text()[-3000:]
print(json.dumps(out))
'''.replace('@ROOT@', repr(record['root']))
    result = owned(code)
    (BASE / 'xolotl-topk-native-bridge-v202-latest.json').write_text(json.dumps(result, indent=2) + '\n')
    print(json.dumps(dict(alive=result['alive'], result=result.get('result.json', {})), indent=2))
    raise SystemExit

if ip.exists():
    raise RuntimeError('exact native bridge has a handle; observe it')
current = observe()
if current['job_uid'] != '7455d989-8255-4f7c-ae8f-3cc0aa5928e4' or current['phase'] != 'Running':
    raise RuntimeError('exact independent optimization allocation differs')
audit = json.loads((BASE / 'xolotl-actual-topk-build-v187.json').read_text())
original = next(row for row in audit['source_files'] if row['path'].endswith('/top-k.cu'))
recipe = next(row for row in audit['recipes'] if row['file'] == original['path'])
source = original['body'].encode()
if len(source) != original['bytes'] or hashlib.sha256(source).hexdigest() != original['sha256']:
    raise RuntimeError('retained original Top-K source bytes differ')
prior = json.loads((BASE / 'xolotl-topk-compile-v193-latest.json').read_text())
if prior['alive'] or prior['result.json']['phase'] != 'compiled':
    raise RuntimeError('isolated packed-key prototype must compile first')
native = json.loads((BASE / 'xolotl-native-serve-profile-v168-plan.json').read_text())
files = {'top-k.original.cu': source}
for name in ['aster_topk_native_bridge_v201.cu', 'aster_topk35_api_v192.hpp']:
    files[name] = (BASE / name).read_bytes()
record = dict(root=lease()['remote'] + '/topk-native-bridge-v202-cpu', phase='staging',
    allocation_uid=current['job_uid'], GPU_executed=False, model_forward_executed=False,
    model_service_changed=False, experimental_flag='ASTER_XQ_TOPK35', default_mode=0,
    parent_topk=original['sha256'], recipe=recipe, native=native,
    prototype=prior['result.json']['object'],
    files=[dict(file=name, bytes=len(raw), sha256=hashlib.sha256(raw).hexdigest()) for name, raw in files.items()])
worker = '''import hashlib,json,os,shlex,subprocess,time
from pathlib import Path
root=Path(__file__).resolve().parent;p=json.loads((root/'plan.json').read_text())
state=dict(unix=time.time(),phase='preflight',GPU_executed=False,model_forward_executed=False,service_changed=False,default_mode=0)
def save():
 t=root/'result.tmp';t.write_text(json.dumps(state,indent=2)+'\\n');t.replace(root/'result.json')
def sha(path):return hashlib.sha256(path.read_bytes()).hexdigest()
def run(command,label,budget,env,cwd):
 state.update(phase=label);save()
 with (root/(label+'.log')).open('xb') as output:q=subprocess.run(['nice','-n','19',*command],cwd=cwd,env=env,stdout=output,stderr=output,timeout=budget)
 if q.returncode:raise RuntimeError('bounded isolated native-bridge command failed: '+label)
try:
 os.umask(0o077);save()
 if root.resolve()!=root or root.stat().st_uid!=10250 or os.getuid()!=10250:raise RuntimeError('exact private native bridge owner differs')
 for row in p['files']+[p['prototype']]:
  path=root/row['file'] if 'file' in row else Path(row['path'])
  if path.is_symlink() or path.stat().st_uid!=10250 or path.stat().st_size!=row['bytes'] or sha(path)!=row['sha256']:raise RuntimeError('frozen native bridge/compiler object differs')
 native=p['native'];build=Path(native['link_cwd']);recipe=p['recipe'];prototype=p['prototype']
 for row in native['inputs']:
  path=Path(row['path'])
  if path.is_symlink() or path.stat().st_uid!=10250 or path.stat().st_size!=row['bytes'] or sha(path)!=row['sha256']:raise RuntimeError('exact pinned native parent input differs')
 env={k:os.environ[k] for k in ['PATH','LD_LIBRARY_PATH'] if k in os.environ};env.update(CUDA_VISIBLE_DEVICES='',OMP_NUM_THREADS='1')
 command=shlex.split(recipe['command']);command[command.index(recipe['file'])]=str(root/'aster_topk_native_bridge_v201.cu');command[command.index('-o')+1]=str(root/'top-k.bridge.o')
 command += ['-I'+str(Path(recipe['file']).parent),'-I'+str(root)]
 state['compile_command']=command;save();run(command,'compile',100,env,build)
 original_link=native['control_link'];control=list(original_link);control[control.index('-o')+1]=str(root/'ds41_generate.control')
 run(control,'link-control',55,env,build)
 if sha(root/'ds41_generate.control')!=native['control_binary_sha256']:raise RuntimeError('exact parent control binary did not reproduce')
 candidate=list(original_link);candidate[candidate.index('-o')+1]=str(root/'ds41_generate.topk35');candidate[3:3]=[str(root/'top-k.bridge.o'),prototype['path']]
 run(candidate,'link-candidate',55,env,build)
 tests=[]
 for invalid in ['', '2','-1','true','01','1 ',' 1']:
  q=subprocess.run([str(root/'ds41_generate.topk35'),'--help'],cwd=root,env=dict(env,ASTER_XQ_TOPK35=invalid),capture_output=True,text=True,timeout=8)
  if q.returncode!=2 or q.stdout or q.stderr!='ASTER_XQ_TOPK35 requires exactly 0 or 1\\n':raise RuntimeError('unknown Top-K mode must refuse before main/model/device work')
  tests.append(dict(mode=invalid,exit=q.returncode,before_main=True))
 state.update(phase='compiled',control_sha256=sha(root/'ds41_generate.control'),candidate=dict(path=str(root/'ds41_generate.topk35'),bytes=(root/'ds41_generate.topk35').stat().st_size,sha256=sha(root/'ds41_generate.topk35')),object=dict(path=str(root/'top-k.bridge.o'),bytes=(root/'top-k.bridge.o').stat().st_size,sha256=sha(root/'top-k.bridge.o')),invalid_mode_tests=tests,existing_GPU_archives_unchanged=True,GPU_operator_gates_pending=True,model_1024_output_gates_pending=True,finished_unix=time.time());save()
except BaseException as error:state.update(phase='failed',error=type(error).__name__+': '+str(error)[:2000],finished_unix=time.time());save();raise
'''
compile(worker, 'private-native-bridge-v202', 'exec')
files.update({'worker.py': worker.encode(), 'plan.json': (json.dumps(record, indent=2) + '\n').encode()})
with ip.open('x') as output:
    output.write(json.dumps(record, indent=2) + '\n')
code = '''import base64,hashlib,json,os
from pathlib import Path
r=@R@;root=Path(r['root']);data=@DATA@
if root.exists() or root.parent.resolve()!=root.parent or root.parent.stat().st_uid!=10250 or os.getuid()!=10250:raise RuntimeError('exclusive native-bridge staging owner differs')
os.umask(0o077);root.mkdir(mode=0o700)
for name,encoded in data.items():
 with (root/name).open('xb') as output:output.write(base64.b64decode(encoded))
print(json.dumps(dict(worker_sha256=hashlib.sha256((root/'worker.py').read_bytes()).hexdigest())))
'''.replace('@R@', repr(record)).replace('@DATA@', repr({name: base64.b64encode(raw).decode() for name, raw in files.items()}))
record['stage'] = jump(code)
record['phase'] = 'launch_requested'
ip.write_text(json.dumps(record, indent=2) + '\n')
code = '''import hashlib,json,os,subprocess,time
from pathlib import Path
r=@R@;root=Path(r['root']);worker=root/'worker.py'
if root.resolve()!=root or root.stat().st_uid!=10250 or worker.is_symlink() or hashlib.sha256(worker.read_bytes()).hexdigest()!=r['stage']['worker_sha256'] or (root/'process.json').exists():raise RuntimeError('exact sealed native-bridge activation differs')
with (root/'worker.log').open('xb') as log:q=subprocess.Popen(['timeout','--signal=TERM','--kill-after=15','260','python3','-u',str(worker)],cwd=root,env=dict(os.environ,CUDA_VISIBLE_DEVICES=''),stdin=subprocess.DEVNULL,stdout=log,stderr=log,start_new_session=True)
h=dict(pid=q.pid,start_ticks=int(Path('/proc/'+str(q.pid)+'/stat').read_text().split(') ',1)[1].split()[19]),uid=os.getuid(),unix=time.time(),namespace='optimization pod',budget_seconds=260)
(root/'process.json').write_text(json.dumps(h,indent=2)+'\\n');print(json.dumps(h))
'''.replace('@R@', repr(record))
record.update(process=owned(code), phase='launched')
ip.write_text(json.dumps(record, indent=2) + '\n')
print(json.dumps(dict(phase=record['phase'], process=record['process'], GPU_executed=False, service_changed=False)))
