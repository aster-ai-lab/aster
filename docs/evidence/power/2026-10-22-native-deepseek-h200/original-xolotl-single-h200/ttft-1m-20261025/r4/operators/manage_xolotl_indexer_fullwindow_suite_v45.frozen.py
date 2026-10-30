"""Launch once or observe the exact finite V44 edge/full1M/cache-load suite."""
import argparse
import base64
import hashlib
import json
import time
from xolotl_trial_common import BASE, jump, observe

parser = argparse.ArgumentParser()
parser.add_argument('action', choices=['launch', 'observe'])
action = parser.parse_args().action
intent = BASE / 'xolotl-indexer-fullwindow-suite-v45-intent.json'

if action == 'launch':
    if intent.exists():
        raise RuntimeError('exact V45 handle already exists; observe it')
    service = json.loads((BASE / 'xolotl-indexer-service-v44-intent.json').read_text())
    basic = json.loads((BASE / 'xolotl-indexer-basic-v44-intent.json').read_text())
    observation = observe()
    reservation = json.loads((BASE / 'xolotl-active-serving-reservation.json').read_text())
    if (observation['phase'] != 'Running' or observation['job_uid'] != service['allocation_uid']
            or service['allocation_uid'] == reservation['allocation_uid']
            or len(observation['replicas']) != 1 or observation['replicas'][0]['uid'] != service['replica_uid']):
        raise RuntimeError('exact independent optimization allocation required')
    root = service['root'] + '/qualification-suite-v45'
    common = dict(base_url='http://' + service['pod_ip'] + ':30003/v1', service_root=service['root'],
                  context_tokens=1048576, context_safety_tokens=8)
    suite = dict(service_root=service['root'], gpu_uuid=service['gpu_uuid'],
                 binary_sha256=service['plan']['binary_sha256'])
    sources = {'remainder': 'xolotl_resident_qos_remainder_worker_v38.frozen.py',
               'fullwindow': 'xolotl_full_1m_qualification_worker_v17.frozen.py',
               'cache-load': 'xolotl_fullwindow_reuse_load_worker_v20.frozen.py'}
    files = {'suite-worker.py': (BASE / 'xolotl_indexer_fullwindow_suite_worker_v45.frozen.py').read_bytes(),
             'suite-plan.json': (json.dumps(suite, indent=2) + '\n').encode()}
    for stage, source in sources.items():
        plan = dict(common)
        if stage == 'cache-load':
            plan['full_qualification_root'] = root + '/fullwindow'
        files[stage + '/request-worker.py'] = (BASE / source).read_bytes()
        files[stage + '/request-plan.json'] = (json.dumps(plan, indent=2) + '\n').encode()
    record = dict(unix=time.time(), phase='prepared', root=root, allocation_uid=service['allocation_uid'],
                  gpu_uuid=service['gpu_uuid'], binary_sha256=suite['binary_sha256'], public_fixture_only=True,
                  files=[dict(file=n, bytes=len(b), sha256=hashlib.sha256(b).hexdigest()) for n, b in files.items()])
    code = '''import base64,hashlib,json,os,subprocess,time
from pathlib import Path
c=@C@;basic=@BASIC@;root=Path(c['root']);service=root.parent
if os.getuid()!=10250 or service.resolve()!=service or service.stat().st_uid!=10250 or root.exists():raise RuntimeError('exact exclusive public suite owner differs')
q=json.loads((service/'service-longctx-v1-result.json').read_text())
if q['phase']!='serving' or q['gpu_uuid']!=c['gpu_uuid'] or q['binary_sha256']!=c['binary_sha256'] or q['context_tokens']!=1048576:raise RuntimeError('exact V44 service identity differs')
p=Path('/proc/'+str(basic['process']['pid'])+'/stat')
alive=p.exists() and p.stat().st_uid==10250 and int(p.read_text().split(') ',1)[1].split()[19])==basic['process']['start_ticks'] and p.read_text().split(') ',1)[1].split()[0]!='Z'
qa=json.loads((Path(basic['root'])/'request-result.json').read_text())
if alive or qa['phase']!='passed' or len(qa['tests'])!=28 or not qa['handoff_three_position_retrieval_passed']:raise RuntimeError('exact V44 basic28 must pass and finish first')
if time.time()+5900+120>q['lease_ends_unix']:raise RuntimeError('finite suite lease insufficient')
os.umask(0o077);root.mkdir(mode=0o700);data=@DATA@
for row in c['files']:
 raw=base64.b64decode(data[row['file']])
 if len(raw)!=row['bytes'] or hashlib.sha256(raw).hexdigest()!=row['sha256']:raise RuntimeError('frozen suite input differs')
 p=root/row['file'];p.parent.mkdir(mode=0o700,parents=True,exist_ok=True)
 with p.open('xb') as f:f.write(raw)
with (root/'suite-worker.log').open('xb') as log:
 p=subprocess.Popen(['timeout','--signal=TERM','--kill-after=15','5900','python3','-u',str(root/'suite-worker.py')],cwd=root,stdin=subprocess.DEVNULL,stdout=log,stderr=log,start_new_session=True)
process=dict(pid=p.pid,start_ticks=int(Path('/proc/'+str(p.pid)+'/stat').read_text().split(') ',1)[1].split()[19]),uid=os.getuid(),unix=time.time(),budget_seconds=5900)
(root/'suite-process.json').write_text(json.dumps(process,indent=2)+'\\n');print(json.dumps(process))
'''.replace('@C@', repr(record)).replace('@BASIC@', repr(basic)).replace('@DATA@', repr({n: base64.b64encode(b).decode() for n, b in files.items()}))
    compile(code, 'launch-indexer-fullwindow-suite-v45', 'exec')
    with intent.open('x') as f:
        f.write(json.dumps(record, indent=2) + '\n')
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
    print(json.dumps(dict(phase=record['phase'], process=record['process'], formal_promoted=False)))
else:
    record = json.loads(intent.read_text())
    code = '''import json,time
from pathlib import Path
c=@C@;root=Path(c['root']);out={'unix':time.time()}
if root.resolve()!=root or root.stat().st_uid!=10250:raise RuntimeError('exact suite owner differs')
for name in ['suite-result.json','remainder/request-result.json','fullwindow/request-result.json','cache-load/request-result.json']:
 p=root/name
 if p.exists():
  if p.is_symlink() or p.stat().st_size>262144:raise RuntimeError('public suite receipt bound exceeded')
  out[name]=json.loads(p.read_text())
p=Path('/proc/'+str(c['process']['pid'])+'/stat')
out['exact_client_alive']=p.exists() and p.stat().st_uid==10250 and int(p.read_text().split(') ',1)[1].split()[19])==c['process']['start_ticks'] and p.read_text().split(') ',1)[1].split()[0]!='Z'
print(json.dumps(out))
'''.replace('@C@', repr(record))
    result = jump(code)
    (BASE / 'xolotl-indexer-fullwindow-suite-v45-latest.json').write_text(json.dumps(result, indent=2) + '\n')
    print(json.dumps(dict(phase=result.get('suite-result.json', {}).get('phase'),
                         current_stage=result.get('suite-result.json', {}).get('current_stage'),
                         exact_client_alive=result['exact_client_alive'],
                         error=result.get('suite-result.json', {}).get('error'),
                         stages=result.get('suite-result.json', {}).get('stages'),
                         children={k: dict(phase=v.get('phase'), tests=len(v.get('tests', [])),
                                          current_test=v.get('current_test'), error=v.get('error'))
                                   for k, v in result.items() if k.endswith('/request-result.json')}), indent=2))
