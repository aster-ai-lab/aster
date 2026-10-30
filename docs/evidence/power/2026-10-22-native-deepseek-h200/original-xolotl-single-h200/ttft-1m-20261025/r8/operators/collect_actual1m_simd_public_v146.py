"""Read bounded closed public diagnostics; publish no binaries, raw profiler reports or credentials."""
import base64
import hashlib
import json
from pathlib import PurePosixPath
import time
from xolotl_trial_common import BASE, jump

def read(name):
    return json.loads((BASE/name).read_text())
requests = []
def add(root, names, target):
    for name in names:
        requests.append(dict(root=root, file=name, target=target+'/'+name))
old = read('xolotl-code-service-v118-full1m_code-intent.json')
suite = read('xolotl-code-fullwindow-suite-v119-intent.json')
add(old['root'], ['engine.log','service-longctx-v1-result.json'], 'actual-code1m-v119/service')
add(suite['root'], ['suite-result.json','suite-worker.log','remainder/request-result.json','remainder/request-worker.log',
    'fullwindow/request-result.json','fullwindow/request-worker.log'], 'actual-code1m-v119/suite')
for profile in ['control_a','simd']:
    service = read('xolotl-phase-simd-v139-'+profile+'-intent.json')
    add(service['root'], ['engine.log','service-longctx-v1-result.json'], 'simd-v139/'+profile)
for version in ['143','145','145r1']:
    cpu = read('xolotl-literal-scan-cpu-v'+version+'-intent.json')
    names = ['cpu-result.json','worker.log','unit-tests.log']
    if version in ['145','145r1']:
        names.append('upstream-frontend-tests.log')
    if version in ['143','145r1']:
        names.append('fixture-metadata-canonical.json')
    add(cpu['root'], names, 'literal-cpu-v'+version)
api = read('xolotl-api-literal-v148-intent.json')
cpu = read('xolotl-api-literal-v148-latest.json')['cpu-preflight-result.json']
if cpu['phase'] != 'passed':
    raise RuntimeError('exact closed API preflight required')
names = ['cpu-preflight-result.json','worker.log']
for row in cpu['tests']:
    names += ['cpu-'+row['mode']+'-'+row['name']+suffix for suffix in ['.stdout.log','.stderr.log']]
add(api['root'], names, 'api-literal-v148')
keys = [p.read_bytes().strip() for p in BASE.glob('access-private-*/api-key.txt')]
if not keys or any(not key for key in keys):
    raise RuntimeError('actual private credential negative controls missing')
dest = BASE/'actual1m-simd-public-v146'
manifest_path = BASE/'xolotl-actual1m-simd-public-v146.json'
if manifest_path.exists():
    raise RuntimeError('closed public collection is already frozen')
dest.mkdir(exist_ok=True)
rows = []
for start in range(0,len(requests),5):
    batch = requests[start:start+5]
    code = '''import base64,hashlib,json
from pathlib import Path,PurePosixPath
requests=@REQUESTS@;out=[];total=0
for r in requests:
 root=Path(r['root']);name=PurePosixPath(r['file']);p=root/r['file']
 if name.is_absolute() or '..' in name.parts or root.resolve()!=root or root.stat().st_uid!=10250 or p.is_symlink() or p.stat().st_uid!=10250 or p.stat().st_size>1048576:raise RuntimeError('bounded exact public diagnostic owner/path differs')
 raw=p.read_bytes();total+=len(raw)
 if total>1048576:raise RuntimeError('bounded public batch exceeded')
 out.append(dict(target=r['target'],bytes=len(raw),sha256=hashlib.sha256(raw).hexdigest(),data=base64.b64encode(raw).decode()))
print(json.dumps(out))
'''.replace('@REQUESTS@',repr(batch))
    result = jump(code)
    if len(result) != len(batch):
        raise RuntimeError('exact public diagnostic batch count differs')
    for item in result:
        raw = base64.b64decode(item.pop('data'))
        if len(raw) != item['bytes'] or hashlib.sha256(raw).hexdigest() != item['sha256'] or any(key in raw for key in keys):
            raise RuntimeError('actual credential/hash public gate refused: '+item['target'])
        target = PurePosixPath(item['target'])
        if target.is_absolute() or '..' in target.parts:
            raise RuntimeError('exact public destination differs')
        path = dest/item['target']
        path.parent.mkdir(parents=True,exist_ok=True)
        if path.exists():
            if path.read_bytes() != raw:
                raise RuntimeError('immutable partial public collection differs')
        else:
            with path.open('xb') as output:
                output.write(raw)
        rows.append(dict(file=item['target'],bytes=len(raw),sha256=item['sha256']))
    print(json.dumps(dict(collected=len(rows),total=len(requests))),flush=True)
old_log = next(row for row in rows if row['file']=='actual-code1m-v119/service/engine.log')
if old_log['sha256'] != '55dabb81b6da0f2d51c039ad7ee4d7a1f721e2dd87e0b3fd25e1f11d8179c2e0':
    raise RuntimeError('exact completed original actual1M native log differs')
record = dict(schema='aster-public-actual1m-simd-v146',unix=time.time(),files=rows,
    credentials_copied=False,raw_profilers_copied=False,private_user_state_copied=False,
    actual_credential_negative_controls_passed=True,active_control_b_log_excluded=True,
    actual1m_c2_identity_gate_passed=False,formal_service_changed=False)
with manifest_path.open('x') as output:
    output.write(json.dumps(record,indent=2)+'\n')
print(json.dumps(dict(phase='collected',files=len(rows),bytes=sum(row['bytes'] for row in rows))))
