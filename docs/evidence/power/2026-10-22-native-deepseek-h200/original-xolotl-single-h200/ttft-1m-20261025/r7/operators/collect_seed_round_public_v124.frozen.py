"""Collect only closed comparison logs and public CPU fixture files; scan actual credentials."""
import base64
import concurrent.futures
import gzip
import hashlib
import json
import time
from pathlib import Path
from xolotl_trial_common import BASE, jump

matrix=json.loads((BASE/'xolotl-pool-seed-v112r2-controller.json').read_text())
if matrix['phase']!='comparison_completed' or len(matrix['profiles'])!=3:
    raise RuntimeError('exact finite pool matrix must finish/drain before log collection')
destination=BASE/'seed-round-public-v124'
if destination.exists():raise RuntimeError('public collection has an exact destination; inspect existing files')
keys=[]
for p in BASE.glob('access-private-*/api-key.txt'):
    value=p.read_bytes().strip()
    if len(value)>=24 and value not in keys:keys.append(value)
if not keys:raise RuntimeError('actual credential negative controls are missing')
requests=[]
for profile in ['pool_off','pool_on','pool_off_repeat']:
    r=json.loads((BASE/('xolotl-pool-service-v108-'+profile+'-intent.json')).read_text())
    requests.append(dict(root=r['root'],file='engine.log',target='logs/'+profile+'-engine.log'))
fixture=json.loads((BASE/'xolotl-code-fullwindow-fixture-v114-intent.json').read_text())
for name in ['public-fixture.txt.gz','fixture-metadata.json','corpus-source-manifest.json','unit-tests.log']:
    requests.append(dict(root=fixture['root'],file=name,target='code-fixture/'+name))
destination.mkdir()

def fetch(r):
    code='''import base64,hashlib,json
from pathlib import Path
r=@R@;root=Path(r['root']);p=root/r['file']
if root.resolve()!=root or root.stat().st_uid!=10250 or p.is_symlink() or p.stat().st_uid!=10250 or not 0<p.stat().st_size<=1400000:raise RuntimeError('bounded owner-only public artifact differs')
data=p.read_bytes()
if len(data)!=p.stat().st_size:raise RuntimeError('public artifact changed during read')
print(json.dumps(dict(bytes=len(data),sha256=hashlib.sha256(data).hexdigest(),base64=base64.b64encode(data).decode())))
'''.replace('@R@',repr(r))
    q=jump(code,80);raw=base64.b64decode(q['base64'],validate=True)
    if len(raw)!=q['bytes'] or hashlib.sha256(raw).hexdigest()!=q['sha256'] or any(k in raw for k in keys):
        raise RuntimeError('exact public artifact hash/credential scan refused')
    if r['file'].endswith('.gz'):
        plain=gzip.decompress(raw)
        if len(plain)>8*1024**2 or any(k in plain for k in keys):raise RuntimeError('decompressed public input bound/credential scan refused')
    p=destination/r['target'];p.parent.mkdir(parents=True,exist_ok=True)
    with p.open('xb') as output:output.write(raw)
    return dict(file=r['target'],bytes=len(raw),sha256=q['sha256'],source_root=r['root'],source_file=r['file'])

with concurrent.futures.ThreadPoolExecutor(max_workers=3) as executor:
    rows=list(executor.map(fetch,requests))
summary=dict(unix=time.time(),files=rows,credential_negative_controls=len(keys),credential_occurrences=0,
    model_weights_copied=False,binaries_copied=False,raw_profiler_artifacts_copied=False,actual1m_gpu_executed=False)
with (BASE/'xolotl-seed-round-public-v124.json').open('x') as output:output.write(json.dumps(summary,indent=2)+'\n')
print(json.dumps(dict(collected_files=len(rows),credential_occurrences=0,actual1m_gpu_executed=False)))
