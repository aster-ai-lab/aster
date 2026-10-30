"""Run one bounded CPU/disk component probe on the existing optimization H200 allocation."""
import base64
import hashlib
import json
import time
from xolotl_trial_common import BASE, lease, observe, owned

intent = BASE / 'xolotl-engram-storage-probe-v64-intent.json'
if intent.exists():
    raise RuntimeError('Engram storage probe has an exact handle; observe it')
resource = observe()
if resource['job_uid'] != '08538623-2284-45f9-b98a-7733bb70f32d' or resource['phase'] != 'Running':
    raise RuntimeError('exact independent storage probe allocation differs')
copy = json.loads((BASE / 'xolotl-engram-local-copy-v63-latest.json').read_text())
if copy['result']['phase'] != 'passed' or copy['exact_copy_process_alive']:
    raise RuntimeError('exact two full local Engram readbacks must pass and finish')
catalog = json.loads((BASE / 'native-engram-blob-catalog-1792792024.json').read_text())
tables = []
for index, layer in enumerate([1, 14]):
    blob = catalog['blobs'][index]
    row = {key: blob[key] for key in ['physical_file', 'expected_sha256', 'bytes']}
    row.update(weight_name='layers.' + str(layer) + '.engram.embed.weight',
               scale_name='layers.' + str(layer) + '.engram.embed.scale')
    tables.append(row)
root = lease()['remote'] + '/engram-storage-probe-v64'
plan = dict(local_root=copy['result']['local_root'], tables=tables)
files = {'probe-plan.json': (json.dumps(plan, indent=2) + '\n').encode(),
         'probe-worker.py': (BASE / 'xolotl_engram_storage_probe_worker_v64.frozen.py').read_bytes()}
record = dict(unix=time.time(), phase='prepared', root=root, allocation_uid=resource['job_uid'],
              gpu_executed=False, live_service_changed=False, source_cache_evicted=False,
              files=[dict(file=n, bytes=len(v), sha256=hashlib.sha256(v).hexdigest()) for n, v in files.items()])
code = '''import base64,hashlib,json,os,subprocess,time
from pathlib import Path
r=@R@;root=Path(r['root']);data=@DATA@
if os.getuid()!=10250 or root.parent.resolve()!=root.parent or root.parent.stat().st_uid!=10250 or root.exists():raise RuntimeError('exclusive storage probe artifact owner differs')
os.umask(0o077);root.mkdir(mode=0o700)
for row in r['files']:
 raw=base64.b64decode(data[row['file']])
 if len(raw)!=row['bytes'] or hashlib.sha256(raw).hexdigest()!=row['sha256']:raise RuntimeError('exact storage probe input differs')
 with (root/row['file']).open('xb') as output:output.write(raw)
with (root/'probe-worker.log').open('xb') as log:
 p=subprocess.Popen(['timeout','--signal=TERM','--kill-after=15','180','python3','-u',str(root/'probe-worker.py')],cwd=root,stdin=subprocess.DEVNULL,stdout=log,stderr=log,start_new_session=True)
process=dict(pid=p.pid,start_ticks=int(Path('/proc/'+str(p.pid)+'/stat').read_text().split(') ',1)[1].split()[19]),uid=os.getuid(),unix=time.time(),budget_seconds=180,namespace='optimization pod')
(root/'probe-process.json').write_text(json.dumps(process,indent=2)+'\\n');print(json.dumps(process))
'''.replace('@R@', repr(record)).replace('@DATA@', repr({n: base64.b64encode(v).decode() for n, v in files.items()}))
compile(code, 'owned-storage-component-probe-v64', 'exec')
with intent.open('x') as output:
    output.write(json.dumps(record, indent=2) + '\n')
record['phase'] = 'launch_requested'
intent.write_text(json.dumps(record, indent=2) + '\n')
try:
    record['process'] = owned(code)
    record['phase'] = 'launched'
except BaseException:
    record['phase'] = 'launch_outcome_requires_inspection'
    intent.write_text(json.dumps(record, indent=2) + '\n')
    raise
intent.write_text(json.dumps(record, indent=2) + '\n')
print(json.dumps(dict(phase=record['phase'], process=record['process'], gpu_executed=False)))
