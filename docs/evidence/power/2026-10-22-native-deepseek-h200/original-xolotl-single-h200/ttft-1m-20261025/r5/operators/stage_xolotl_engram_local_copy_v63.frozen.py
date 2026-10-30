"""Launch one low-priority CPU/disk staging task on the isolated optimization resource."""
import base64
import hashlib
import json
import time
from xolotl_trial_common import BASE, lease, observe, owned

intent = BASE / 'xolotl-engram-local-copy-v63-intent.json'
if intent.exists():
    raise RuntimeError('Engram local copy has an exact handle; observe it without resubmission')
resource = observe()
if resource['job_uid'] != '08538623-2284-45f9-b98a-7733bb70f32d' or resource['phase'] != 'Running':
    raise RuntimeError('exact isolated optimization resource differs')
catalog = json.loads((BASE / 'native-engram-blob-catalog-1792792024.json').read_text())
root = lease()['remote'] + '/engram-local-copy-v63'
plan = dict(allocation_uid=resource['job_uid'], local_root='/var/tmp/aster-h200-engram-v63-08538623',
            source_index='/mnt/shared-storage-user/yangchao/Workspace/aster-h200-xq/aster-xolotl-pair-1fab10648af3/xolotl-b345/engram/model.safetensors.index.json',
            blobs=[{key: row[key] for key in ['physical_file', 'bytes', 'expected_sha256']} for row in catalog['blobs']],
            copy_bytes_per_second_per_file=256 * 1024**2, verify_bytes_per_second_per_file=512 * 1024**2)
files = {'copy-plan.json': (json.dumps(plan, indent=2) + '\n').encode(),
         'copy-worker.py': (BASE / 'xolotl_engram_local_copy_worker_v63.frozen.py').read_bytes()}
record = dict(unix=time.time(), phase='prepared', root=root, allocation_uid=resource['job_uid'],
              replica=resource['replicas'][0]['name'], replica_uid=resource['replicas'][0]['uid'],
              gpu_executed=False, live_service_changed=False, plan=plan,
              files=[dict(file=n, bytes=len(v), sha256=hashlib.sha256(v).hexdigest()) for n, v in files.items()])
code = '''import base64,hashlib,json,os,subprocess,time
from pathlib import Path
r=@R@;root=Path(r['root']);data=@DATA@
if os.getuid()!=10250 or root.parent.resolve()!=root.parent or root.parent.stat().st_uid!=10250 or root.exists():raise RuntimeError('exclusive owned Engram staging root differs')
os.umask(0o077);root.mkdir(mode=0o700)
for row in r['files']:
 raw=base64.b64decode(data[row['file']])
 if len(raw)!=row['bytes'] or hashlib.sha256(raw).hexdigest()!=row['sha256']:raise RuntimeError('exact low-priority copy worker input differs')
 with (root/row['file']).open('xb') as output:output.write(raw)
with (root/'copy-worker.log').open('xb') as log:
 p=subprocess.Popen(['timeout','--signal=TERM','--kill-after=15','1800','python3','-u',str(root/'copy-worker.py')],cwd=root,stdin=subprocess.DEVNULL,stdout=log,stderr=log,start_new_session=True)
process=dict(pid=p.pid,start_ticks=int(Path('/proc/'+str(p.pid)+'/stat').read_text().split(') ',1)[1].split()[19]),uid=os.getuid(),unix=time.time(),budget_seconds=1800,namespace='optimization pod')
(root/'copy-process.json').write_text(json.dumps(process,indent=2)+'\\n');print(json.dumps(process))
'''.replace('@R@', repr(record)).replace('@DATA@', repr({n: base64.b64encode(v).decode() for n, v in files.items()}))
compile(code, 'low-priority-owned-Engram-copy-v63', 'exec')
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
print(json.dumps(dict(phase=record['phase'], process=record['process'], gpu_executed=False,
                      live_service_changed=False, total_bytes=203073077576)))
