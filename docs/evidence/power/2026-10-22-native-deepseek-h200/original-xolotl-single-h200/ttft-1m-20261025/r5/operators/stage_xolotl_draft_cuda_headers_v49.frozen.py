"""Copy only the exact CUDA host header dependency closure for jump-host CPU compilation."""
import json
import time
from xolotl_trial_common import BASE, observe, owned

intent = BASE / 'xolotl-draft-cumulative-v49-cuda-header-intent.json'
if intent.exists():
    raise RuntimeError('CUDA header stage has an exact handle; inspect its receipt')
parent = json.loads((BASE / 'xolotl-draft-cumulative-v49-generator-parent.json').read_text())
cpu = json.loads((BASE / 'xolotl-draft-cumulative-v49-cpu-intent.json').read_text())
service = json.loads((BASE / 'xolotl-indexer-service-v44-intent.json').read_text())
observation = observe()
if observation['job_uid'] != service['allocation_uid']:
    raise RuntimeError('exact separate optimization allocation differs')
record = dict(unix=time.time(), phase='prepared', root=cpu['root'], allocation_uid=service['allocation_uid'],
              gpu_uuid=service['gpu_uuid'], cuda_code_executed=False, original_files_mutated=False)
code = '''import hashlib,json,os,shlex,subprocess,time
from pathlib import Path,PurePosixPath
r=@R@;parent=@PARENT@;root=Path(r['root']);target=root/'cuda-include'
if root.resolve()!=root or root.stat().st_uid!=10250 or target.exists():raise RuntimeError('exclusive CPU header stage differs')
original=Path(parent['object']['path']).parent/'ds41_generate.cpp'
if original.is_symlink() or original.stat().st_uid!=10250 or hashlib.sha256(original.read_bytes()).hexdigest()!=parent['source_sha256']:raise RuntimeError('original generator identity differs')
source=shlex.split(parent['plan']['compile_command']);args=[];i=0
while i<len(source):
 if source[i] in ['-MT','-MF','-o','-c']:i+=2;continue
 if source[i]=='-MD':i+=1;continue
 args.append(source[i]);i+=1
if args[0]!='/usr/bin/c++' or args.count('-isystem')!=1:raise RuntimeError('exact host compiler/header flags differ')
include=Path(args[args.index('-isystem')+1])
if str(include)!='/usr/local/cuda/targets/x86_64-linux/include':raise RuntimeError('exact CUDA host include root differs')
dep=root/'generator-cuda-deps.d'
if dep.exists():raise RuntimeError('dependency preflight already exists; inspect instead')
command=args+['-M','-MT','aster-generator-deps','-MF',str(dep),str(original)]
q=subprocess.run(command,cwd=root,capture_output=True,text=True,timeout=15)
(root/'cuda-header-preprocessor.log').write_text(q.stdout+q.stderr)
if q.returncode:raise RuntimeError('CUDA host dependency preflight failed')
if dep.stat().st_size>131072:raise RuntimeError('dependency list bound exceeded')
text=dep.read_text().replace(chr(92)+chr(10),' ');names=shlex.split(text.split(':',1)[1])
headers=sorted(set(name for name in names if name.startswith(str(include)+'/')))
if not headers or len(headers)>128:raise RuntimeError('CUDA host header count differs')
total=0;rows=[];include_real=include.resolve();target.mkdir(mode=0o700)
for name in headers:
 p=Path(name);relative=p.relative_to(include);pure=PurePosixPath(relative.as_posix())
 if '..' in pure.parts or not p.resolve().is_relative_to(include_real) or not p.is_file():raise RuntimeError('CUDA host include escaped declared root')
 raw=p.read_bytes();total+=len(raw)
 if len(raw)>4*1024**2 or total>24*1024**2:raise RuntimeError('CUDA host header byte bound exceeded')
 dest=target/relative;dest.parent.mkdir(mode=0o700,parents=True,exist_ok=True)
 with dest.open('xb') as f:f.write(raw)
 rows.append(dict(file=relative.as_posix(),bytes=len(raw),sha256=hashlib.sha256(raw).hexdigest()))
result=dict(unix=time.time(),phase='staged',cuda_code_executed=False,original_files_mutated=False,
 original_include_root=str(include),header_root=str(target),headers=rows,total_bytes=total,compiler_command=command)
(root/'cuda-header-result.json').write_text(json.dumps(result,indent=2)+'\\n');print(json.dumps(result))
'''.replace('@R@', repr(record)).replace('@PARENT@', repr(parent))
compile(code, 'stage-exact-CUDA-host-headers-v49', 'exec')
with intent.open('x') as stream:
    stream.write(json.dumps(record, indent=2) + '\n')
record['phase'] = 'stage_requested'
intent.write_text(json.dumps(record, indent=2) + '\n')
try:
    result = owned(code)
except BaseException:
    record['phase'] = 'stage_outcome_requires_inspection'
    intent.write_text(json.dumps(record, indent=2) + '\n')
    raise
(BASE / 'xolotl-draft-cumulative-v49-cuda-header-latest.json').write_text(json.dumps(result, indent=2) + '\n')
record['phase'] = 'staged'
intent.write_text(json.dumps(record, indent=2) + '\n')
print(json.dumps(dict(phase='staged', headers=len(result['headers']), total_bytes=result['total_bytes'],
                      cuda_code_executed=False, original_files_mutated=False)))
