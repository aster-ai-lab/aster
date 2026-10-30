"""Read only explicit CPU hash-tool paths; never read weight payloads or row tables."""
import json
from xolotl_trial_common import BASE,jump
ip=BASE/'xolotl-engram-hash-inputs-v179.json'
if ip.exists():raise RuntimeError('exact hash-tool inventory already retained')
p=json.loads((BASE/'xolotl-native-serve-profile-v168-plan.json').read_text())
code='''import hashlib,json,time
from pathlib import Path
build=Path(@BUILD@);source=build.parent/'source';result=dict(unix=time.time(),GPU_executed=False,model_executed=False,paths=[])
for p in [build/'ds41_engram_hot',build/'CMakeFiles/ds41_engram_hot.dir/link.txt',build/'CMakeFiles/bench_ds41_engram.dir/link.txt',source/'tools/v41/engine/ds41_engram_hot.cpp',source/'tools/v41/engine/ds41_engram.hpp',source/'tools/v41/engine/ds41_engram.cpp',build/'libstrata_core.a']:
 row=dict(path=str(p),exists=p.exists())
 if p.exists():
  if p.is_symlink() or p.stat().st_uid!=10250:raise RuntimeError('exact declared hash inventory owner differs')
  row.update(bytes=p.stat().st_size)
  if p.suffix in ['.txt','.cpp','.hpp']:
   if p.stat().st_size>262144:raise RuntimeError('bounded exact hash source inventory exceeded')
   raw=p.read_bytes();row['sha256']=hashlib.sha256(raw).hexdigest()
   if p.suffix=='.txt':row['text']=raw.decode()
 result['paths'].append(row)
print(json.dumps(result))
'''.replace('@BUILD@',repr(p['link_cwd']))
result=jump(code)
with ip.open('x') as output:output.write(json.dumps(result,indent=2)+'\n')
print(json.dumps(result,indent=2))
