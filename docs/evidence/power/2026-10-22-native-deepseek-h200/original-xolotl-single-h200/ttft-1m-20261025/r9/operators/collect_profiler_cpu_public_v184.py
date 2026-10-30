"""Retain only closed completed CPU logs; exclude active engine/raw profiler/client state."""
import hashlib
import json
from xolotl_trial_common import BASE,jump
ip=BASE/'xolotl-profiler-cpu-public-v184.json';directory=BASE/'profiler-cpu-public-v184'
if ip.exists() or directory.exists():raise RuntimeError('exact bounded CPU public collection already exists')
cpu=json.loads((BASE/'xolotl-serve-profile-cpu-v167-intent.json').read_text())
native=json.loads((BASE/'xolotl-native-serve-profile-v168-intent.json').read_text())
profile=json.loads((BASE/'xolotl-ncu-actual1m-v172r1-intent.json').read_text())
rows=[dict(root=cpu['root'],file=name,out='lifecycle/'+name) for name in ['release-unit-run.log','asan_ubsan-unit-run.log']]
rows += [dict(root=native['root'],file='native-link-result.json',out='controlled-link-result.json'),
    dict(root=profile['root'],file='numeric-parser-tests.log',out='numeric-query-parser-tests.log')]
code='''import base64,hashlib,json,time
from pathlib import Path
rows=@ROWS@;output=[]
for row in rows:
 root=Path(row['root']);p=root/row['file']
 if root.resolve()!=root or root.stat().st_uid!=10250 or p.is_symlink() or p.stat().st_uid!=10250 or p.stat().st_size>262144:raise RuntimeError('bounded closed completed CPU diagnostic differs')
 raw=p.read_bytes();output.append(dict(file=row['out'],bytes=len(raw),sha256=hashlib.sha256(raw).hexdigest(),base64=base64.b64encode(raw).decode()))
print(json.dumps(dict(unix=time.time(),files=output,active_engine_logs_included=False,raw_profiler_report_included=False)))
'''.replace('@ROWS@',repr(rows))
result=jump(code)
keys=list({p.read_bytes().strip() for p in BASE.glob('access-private-*/api-key.txt') if len(p.read_bytes().strip())>=24})
directory.mkdir();output=[]
import base64
for row in result['files']:
    raw=base64.b64decode(row.pop('base64'))
    if len(raw)!=row['bytes'] or hashlib.sha256(raw).hexdigest()!=row['sha256'] or any(key in raw for key in keys):raise RuntimeError('exact bounded public diagnostic/key gate differs')
    p=directory/row['file'];p.parent.mkdir(parents=True,exist_ok=True)
    with p.open('xb') as stream:stream.write(raw)
    output.append(row)
result['files']=output
with ip.open('x') as stream:stream.write(json.dumps(result,indent=2)+'\n')
print(json.dumps(dict(phase='collected',files=len(output),active_model_log_copied=False,hardware_report_copied=False)))
