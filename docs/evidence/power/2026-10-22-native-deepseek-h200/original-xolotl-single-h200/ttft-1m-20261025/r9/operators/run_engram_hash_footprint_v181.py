"""Launch one bounded CPU-only canonical input footprint measurement on the jump host."""
import base64
import hashlib
import json
from xolotl_trial_common import BASE,jump,lease

ip=BASE/'xolotl-engram-hash-footprint-v181-intent.json'
if ip.exists():raise RuntimeError('exact CPU hash footprint has a handle; inspect it')
inventory=json.loads((BASE/'xolotl-engram-hash-inputs-v179.json').read_text())
prior=json.loads((BASE/'xolotl-engram-simd-cpu-v137-latest.json').read_text())['cpu-result.json']
original=next(row['object'] for row in prior['runs'] if row['mode']=='original')
build=json.loads((BASE/'xolotl-native-serve-profile-v168-plan.json').read_text())['link_cwd']
source=build.rsplit('/',1)[0]+'/source'
ids=json.loads((BASE/'xolotl-native-actual1m-input-v169-intent.json').read_text())['root']+'/prompt.u32le'
model=json.loads((BASE/'xolotl-ncu-actual1m-v172r1-plan.json').read_text())['model']
raw=(BASE/'engram_hash_footprint_v180.cpp').read_bytes()
record=dict(root=lease()['remote']+'/engram-hash-footprint-v181-cpu',source=source,build=build,
    model=model,input_ids=ids,original_object=original,source_bytes=len(raw),source_sha256=hashlib.sha256(raw).hexdigest(),
    source_headers=[row for row in inventory['paths'] if row.get('sha256') and row['path'].endswith('ds41_engram.hpp')],
    GPU_executed=False,model_forward_executed=False,namespace='jump host')
worker='''import hashlib,json,os,subprocess,time
from pathlib import Path
root=Path(__file__).resolve().parent;p=json.loads((root/'plan.json').read_text());state=dict(unix=time.time(),phase='preflight',GPU_executed=False,model_forward_executed=False)
def save():
 t=root/'result.tmp';t.write_text(json.dumps(state,indent=2)+'\\n');t.replace(root/'result.json')
def sha(path):return hashlib.sha256(path.read_bytes()).hexdigest()
try:
 save()
 if os.getuid()!=10250 or root.resolve()!=root or root.stat().st_uid!=10250:raise RuntimeError('exact CPU hash root owner differs')
 for path,size,digest in [(root/'footprint.cpp',p['source_bytes'],p['source_sha256']),(Path(p['input_ids']),4190176,'b442a593a4066e3e99b663b670db4d7d624c471f1c5f6ee906e315c99d1969c4'),(Path(p['original_object']['path']),p['original_object']['bytes'],p['original_object']['sha256'])]+[(Path(r['path']),r['bytes'],r['sha256']) for r in p['source_headers']]:
  if path.is_symlink() or path.stat().st_uid!=10250 or path.stat().st_size!=size or sha(path)!=digest:raise RuntimeError('exact CPU input/object/header lineage differs')
 if Path(p['model']).stat().st_size!=162954903552:raise RuntimeError('exact GGUF metadata source size differs')
 env={k:os.environ[k] for k in ['PATH','LD_LIBRARY_PATH'] if k in os.environ};env.update(CUDA_VISIBLE_DEVICES='',OMP_NUM_THREADS='1',LC_ALL='C')
 command=['nice','-n','19','/usr/bin/c++','-O3','-DNDEBUG','-std=c++20','-I'+p['source']+'/include','-I'+p['source']+'/third_party/ggml','-I'+p['source']+'/tools/v41/engine',str(root/'footprint.cpp'),p['original_object']['path'],p['build']+'/libstrata_core.a','-lpthread','-ldl','-o',str(root/'footprint')]
 state.update(phase='compiling',command=command);save()
 with (root/'compile.log').open('xb') as output:q=subprocess.run(command,cwd=root,env=env,stdout=output,stderr=output,timeout=40)
 if q.returncode:raise RuntimeError('exact CPU hash-tool link failed')
 state.update(phase='hashing');save()
 with (root/'hash.stdout.log').open('xb') as stdout,(root/'hash.stderr.log').open('xb') as stderr:q=subprocess.run(['nice','-n','19',str(root/'footprint'),p['model'],p['input_ids']],cwd=root,env=env,stdout=stdout,stderr=stderr,timeout=100)
 output=root/'hash.stdout.log'
 if q.returncode or output.stat().st_size>65536:raise RuntimeError('bounded CPU hash footprint command failed')
 result=json.loads(output.read_text())
 if result['phase']!='passed' or result['prompt_tokens']!=1047544 or result['hash_references']!=1047544*48 or not 0<result['unique_row_keys']<=1047544*48 or sum(result['layer_unique_rows'])!=result['unique_row_keys'] or result['GPU_executed'] or result['row_tables_opened']:raise RuntimeError('exact CPU hash footprint proof differs')
 state.update(phase='passed',result=result,finished_unix=time.time());save()
except BaseException as error:state.update(phase='failed',error=type(error).__name__+': '+str(error)[:2000],finished_unix=time.time());save();raise
'''
compile(worker,'engram-hash-footprint-v181-worker','exec')
files={'footprint.cpp':raw,'worker.py':worker.encode(),'plan.json':(json.dumps(record,indent=2)+'\n').encode()}
code='''import base64,hashlib,json,os,subprocess,time
from pathlib import Path
r=@R@;data=@DATA@;root=Path(r['root'])
if root.exists() or os.getuid()!=10250 or root.parent.resolve()!=root.parent or root.parent.stat().st_uid!=10250:raise RuntimeError('exclusive CPU footprint stage differs')
os.umask(0o077);root.mkdir(mode=0o700)
for name,encoded in data.items():
 with (root/name).open('xb') as output:output.write(base64.b64decode(encoded))
with (root/'worker.log').open('xb') as log:q=subprocess.Popen(['timeout','--signal=TERM','--kill-after=10','160','python3','-u',str(root/'worker.py')],cwd=root,env=dict(os.environ,CUDA_VISIBLE_DEVICES=''),stdin=subprocess.DEVNULL,stdout=log,stderr=log,start_new_session=True)
handle=dict(pid=q.pid,start_ticks=int(Path('/proc/'+str(q.pid)+'/stat').read_text().split(') ',1)[1].split()[19]),uid=os.getuid(),unix=time.time(),namespace='jump host',budget_seconds=160)
(root/'process.json').write_text(json.dumps(handle,indent=2)+'\\n');print(json.dumps(handle))
'''.replace('@R@',repr(record)).replace('@DATA@',repr({name:base64.b64encode(raw).decode() for name,raw in files.items()}))
compile(code,'engram-hash-footprint-v181-launch','exec')
with ip.open('x') as output:output.write(json.dumps(dict(record,phase='launch_requested'),indent=2)+'\n')
record.update(process=jump(code),phase='launched')
ip.write_text(json.dumps(record,indent=2)+'\n')
print(json.dumps(dict(phase=record['phase'],process=record['process'])))
