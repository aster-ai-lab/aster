"""Run one bounded read-only readiness observer on the jump host, independent of Windows."""
import base64
import hashlib
import json
import time
from xolotl_serving_common import BASE, jump

intent = BASE / 'xolotl-readiness-watch-v65-intent.json'
if intent.exists():
    raise RuntimeError('readiness observer has an exact handle; observe it')
service = json.loads((BASE / 'xolotl-stable-service-v58-intent.json').read_text())
promotion = json.loads((BASE / 'xolotl-stable-recovery-v58-promotion.json').read_text())
if promotion['phase'] != 'promoted' or not promotion['backend_verified_by_unique_nonce_and_kv_reuse']:
    raise RuntimeError('owned same-VIP Windows promotion must pass first')
root = service['root'] + '/readiness-watch-v65'
plan = {key: service[key] for key in ['allocation_job', 'allocation_uid']}
plan.update(service_root=service['root'], lease_ends_unix=service['plan']['lease_ends_unix'],
            service_name='aster-xolotl-api-v3pool-20261024', service_uid='1f968a8d-4ddc-4cee-b32a-390cd44210c7',
            health_url='http://10.102.93.68/health')
files = {'watch-plan.json': (json.dumps(plan, indent=2) + '\n').encode(),
         'watch-worker.py': (BASE / 'xolotl_readiness_watch_worker_v65.frozen.py').read_bytes()}
record = dict(unix=time.time(), phase='prepared', root=root, plan=plan,
              observation_only=True, namespace='jump host',
              files=[dict(file=n, bytes=len(v), sha256=hashlib.sha256(v).hexdigest()) for n, v in files.items()])
code = '''import base64,hashlib,json,os,subprocess,time
from pathlib import Path
r=@R@;root=Path(r['root']);data=@DATA@
if os.getuid()!=10250 or root.parent.resolve()!=root.parent or root.parent.stat().st_uid!=10250 or root.exists():raise RuntimeError('exclusive readiness artifact owner differs')
os.umask(0o077);root.mkdir(mode=0o700)
for row in r['files']:
 raw=base64.b64decode(data[row['file']])
 if len(raw)!=row['bytes'] or hashlib.sha256(raw).hexdigest()!=row['sha256']:raise RuntimeError('exact readiness worker input differs')
 with (root/row['file']).open('xb') as output:output.write(raw)
budget=int(r['plan']['lease_ends_unix']-time.time()+120)
if budget<120:raise RuntimeError('finite readiness observation lease insufficient')
with (root/'watch-worker.log').open('xb') as log:
 p=subprocess.Popen(['timeout','--signal=TERM','--kill-after=15',str(budget),'python3','-u',str(root/'watch-worker.py')],cwd=root,stdin=subprocess.DEVNULL,stdout=log,stderr=log,start_new_session=True)
process=dict(pid=p.pid,start_ticks=int(Path('/proc/'+str(p.pid)+'/stat').read_text().split(') ',1)[1].split()[19]),uid=os.getuid(),unix=time.time(),budget_seconds=budget,namespace='jump host')
(root/'watch-process.json').write_text(json.dumps(process,indent=2)+'\\n');print(json.dumps(process))
'''.replace('@R@', repr(record)).replace('@DATA@', repr({n: base64.b64encode(v).decode() for n, v in files.items()}))
compile(code, 'finite-readonly-readiness-watch-v65', 'exec')
with intent.open('x') as output:
    output.write(json.dumps(record, indent=2) + '\n')
record['phase'] = 'launch_requested'
intent.write_text(json.dumps(record, indent=2) + '\n')
try:
    record['process'] = jump(code)
    record['phase'] = 'launched'
except BaseException:
    record['phase'] = 'launch_outcome_requires_inspection'
    intent.write_text(json.dumps(record, indent=2) + '\n')
    raise
intent.write_text(json.dumps(record, indent=2) + '\n')
print(json.dumps(dict(phase=record['phase'], process=record['process'], observation_only=True)))
