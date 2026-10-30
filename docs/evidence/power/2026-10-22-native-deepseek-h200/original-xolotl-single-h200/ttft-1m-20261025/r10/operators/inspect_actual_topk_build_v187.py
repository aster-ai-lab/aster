"""Read the actual linked Top-K object, compiler recipe and installed library version without GPU work."""
import json
from xolotl_trial_common import BASE,owned
ip=BASE/'xolotl-actual-topk-build-v187.json'
if ip.exists():raise RuntimeError('exact Top-K build inventory already retained')
plan=json.loads((BASE/'xolotl-native-serve-profile-v168-plan.json').read_text())
code='''import hashlib,json,subprocess,time
from pathlib import Path
build=Path(@BUILD@);source=build.parent/'source';out=dict(unix=time.time(),GPU_executed=False,model_changed=False)
path=build/'compile_commands.json'
if path.is_symlink() or path.stat().st_uid!=10250 or path.stat().st_size>16*1024**2:raise RuntimeError('bounded exact compile database differs')
rows=json.loads(path.read_text());selected=[row for row in rows if Path(row['file']).name in ['top-k.cu','argsort.cu']]
if len(selected)!=2:raise RuntimeError('exact Top-K/argsort compiler recipes must each exist once')
out['recipes']=selected;out['source_files']=[]
for row in selected:
 p=Path(row['file'])
 if p.is_symlink() or p.stat().st_uid!=10250 or p.stat().st_size>262144:raise RuntimeError('bounded actual Top-K source differs')
 raw=p.read_bytes();out['source_files'].append(dict(path=str(p),bytes=len(raw),sha256=hashlib.sha256(raw).hexdigest(),body=raw.decode()))
for name in ['/usr/local/cuda/include/cub/version.cuh','/usr/local/cuda/include/cuda/version','/usr/local/cuda/version.json']:
 p=Path(name)
 if p.is_file():
  if p.stat().st_size>65536:raise RuntimeError('bounded installed CUDA/CUB version exceeded')
  out.setdefault('installed_versions',[]).append(dict(path=name,body=p.read_text()))
p=subprocess.run(['/usr/local/cuda/bin/nvcc','--version'],capture_output=True,text=True,check=True,timeout=8)
out['nvcc_version']=p.stdout+p.stderr
object=build/'ggml/src/ggml-cuda/CMakeFiles/ggml-cuda.dir/top-k.cu.o'
if object.is_symlink() or object.stat().st_uid!=10250 or object.stat().st_size>64*1024**2:raise RuntimeError('bounded actual linked Top-K object differs')
q=subprocess.run(['nm','-C','--defined-only',str(object)],capture_output=True,text=True,check=True,timeout=15)
if len(q.stdout)>8*1024**2:raise RuntimeError('bounded actual Top-K symbol observation exceeded')
out['linked_topk_object']=dict(path=str(object),bytes=object.stat().st_size,sha256=hashlib.sha256(object.read_bytes()).hexdigest())
out['declared_symbol_counts']={name:q.stdout.count(name) for name in ['DeviceTopK','DeviceRadixSort','DeviceSegmentedSort','DeviceSegmentedRadixSort','top_k_cub','DeviceTopKKernel','DeviceRadixSortKernel']}
print(json.dumps(out))
'''.replace('@BUILD@',repr(plan['link_cwd']))
r=owned(code)
with ip.open('x') as output:output.write(json.dumps(r,indent=2)+'\n')
print(json.dumps({key:value for key,value in r.items() if key not in ['source_files','recipes','installed_versions']},indent=2))
for row in r['installed_versions']:
    print(json.dumps(dict(path=row['path'],version_lines=[line for line in row['body'].splitlines() if line.startswith(('#define CUB_VERSION','#define CUB_MAJOR_VERSION','#define CUB_MINOR_VERSION'))])))
