"""Launch once or observe exactly seven isolated natural-text correctness/timing requests."""
import argparse
import base64
import hashlib
import json
import time
from xolotl_trial_common import BASE, jump, observe

parser = argparse.ArgumentParser()
parser.add_argument('profile', choices=['pool_off','pool_on','pool_off_repeat'])
parser.add_argument('action', choices=['launch', 'observe'])
args = parser.parse_args()
intent = BASE / ('xolotl-pool-long-v111-' + args.profile + '-intent.json')
if args.action == 'launch':
    if intent.exists():
        raise RuntimeError('natural client has an exact handle; observe it')
    service = json.loads((BASE / ('xolotl-pool-service-v108-' + args.profile + '-intent.json')).read_text())
    prepared = json.loads((BASE / 'xolotl-natural-long-fixtures-v109-latest.json').read_text())
    fixture = json.loads((BASE / 'xolotl-natural-long-fixtures-v109-intent.json').read_text())
    if prepared['result']['phase'] != 'passed' or prepared['exact_cpu_process_alive']:
        raise RuntimeError('exact CPU tokenizer fixtures must pass and finish')
    resource = observe()
    reservation=json.loads((BASE/'xolotl-active-serving-reservation.json').read_text())
    if resource['phase'] != 'Running' or resource['job_uid'] != service['allocation_uid'] or service['allocation_uid']==reservation['allocation_uid']:
        raise RuntimeError('exact independent natural-text resource differs')
    reference = json.loads((BASE / 'xolotl-pool-long-v111-pool_off-intent.json').read_text()) if args.profile != 'pool_off' else None
    request_source = (BASE / 'xolotl_fullwindow_long_worker_v54.frozen.py').read_bytes()
    root = service['root'] + '/qualification-pool-long-v111'
    plan = dict(profile=args.profile, service_root=service['root'],
        base_url='http://' + service['pod_ip'] + ':30003/v1', fixture_root=fixture['root'],
        fixtures=prepared['result']['fixtures'], reference_root=reference['root'] if reference else None,
        pool_seed_expected=service['plan']['engine_env']['ASTER_XQ_POOL_DRAFT_SEED']=='1', request_source_sha256=hashlib.sha256(request_source).hexdigest())
    files = {'request-worker.py': (BASE / 'xolotl_pool_long_worker_v110.frozen.py').read_bytes(),
             'request-plan.json': (json.dumps(plan, indent=2) + '\n').encode(),
             'request-only-source.py': request_source,
             'xolotl_fullwindow_long_fixture_v54.py': (BASE / 'xolotl_fullwindow_long_fixture_v54.py').read_bytes()}
    record = dict(unix=time.time(), phase='prepared', root=root, allocation_uid=service['allocation_uid'],
                  gpu_uuid=service['gpu_uuid'], public_fixture_only=True,
                  files=[dict(file=n, bytes=len(v), sha256=hashlib.sha256(v).hexdigest()) for n, v in files.items()])
    code = '''import base64,hashlib,json,os,subprocess,time,urllib.request
from pathlib import Path
r=@R@;root=Path(r['root']);fixture=@FIXTURE@;reference=@REFERENCE@;data=@DATA@
if os.getuid()!=10250 or root.parent.resolve()!=root.parent or root.parent.stat().st_uid!=10250 or root.exists():raise RuntimeError('exclusive natural-text client owner differs')
q=json.loads((root.parent/'service-longctx-v1-result.json').read_text())
if q['phase']!='serving' or q['gpu_uuid']!=r['gpu_uuid'] or q['context_tokens']!=1048576 or time.time()+1020>q['lease_ends_unix']:raise RuntimeError('qualified exact1M service/lease differs')
proof=fixture['process'];p=Path('/proc/'+str(proof['pid'])+'/stat')
alive=p.exists() and p.stat().st_uid==10250 and int(p.read_text().split(') ',1)[1].split()[19])==proof['start_ticks'] and p.read_text().split(') ',1)[1].split()[0]!='Z'
if alive or json.loads((Path(fixture['root'])/'prepare-result.json').read_text())['phase']!='passed':raise RuntimeError('exact CPU natural fixture must pass and finish')
if reference is not None:
 proof=reference['process'];p=Path('/proc/'+str(proof['pid'])+'/stat')
 alive=p.exists() and p.stat().st_uid==10250 and int(p.read_text().split(') ',1)[1].split()[19])==proof['start_ticks'] and p.read_text().split(') ',1)[1].split()[0]!='Z'
 refq=json.loads((Path(reference['root'])/'request-result.json').read_text())
 if alive or not refq.get('diagnostic_completed') or len(refq['tests'])!=13 or not refq.get('cold_reference_qualified'):raise RuntimeError('completed structurally qualified cold reference required; failed replay gate is retained')
for folder,count in [('qualification-basic-v108',28),('qualification-extended-v108',12)]:
 prior=json.loads((root.parent/folder/'request-result.json').read_text())
 if prior['phase']!='passed' or len(prior['tests'])!=count:raise RuntimeError('fresh basic28 andnative-tool/long-output12 must pass first')
key=(root.parent/'credentials/api-key.txt').read_text().strip();req=urllib.request.Request(@SLOTS@,headers={'Authorization':'Bearer '+key})
with urllib.request.build_opener(urllib.request.ProxyHandler({})).open(req,timeout=5) as response:slots=json.load(response)
if not isinstance(slots,list) or not slots or any(slot['is_processing'] for slot in slots):raise RuntimeError('natural-text slots must be idle')
os.umask(0o077);root.mkdir(mode=0o700)
for row in r['files']:
 raw=base64.b64decode(data[row['file']])
 if len(raw)!=row['bytes'] or hashlib.sha256(raw).hexdigest()!=row['sha256']:raise RuntimeError('exact natural-text request input differs')
 with (root/row['file']).open('xb') as output:output.write(raw)
with (root/'request-worker.log').open('xb') as log:
 p=subprocess.Popen(['timeout','--signal=TERM','--kill-after=15','900','python3','-u',str(root/'request-worker.py')],cwd=root,stdin=subprocess.DEVNULL,stdout=log,stderr=log,start_new_session=True)
process=dict(pid=p.pid,start_ticks=int(Path('/proc/'+str(p.pid)+'/stat').read_text().split(') ',1)[1].split()[19]),uid=os.getuid(),unix=time.time(),budget_seconds=900,namespace='jump host')
(root/'request-process.json').write_text(json.dumps(process,indent=2)+chr(10));print(json.dumps(process))
'''.replace('@R@', repr(record)).replace('@FIXTURE@', repr(fixture)).replace('@REFERENCE@', repr(reference)).replace('@SLOTS@', repr(plan['base_url'].removesuffix('/v1') + '/slots')).replace('@DATA@', repr({n: base64.b64encode(v).decode() for n, v in files.items()}))
    compile(code, 'exact-natural-text-client-v69', 'exec')
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
    print(json.dumps(dict(phase=record['phase'], process=record['process'])))
else:
    record = json.loads(intent.read_text())
    code = '''import json,time
from pathlib import Path
r=@R@;root=Path(r['root']);out={'unix':time.time()}
if root.resolve()!=root or root.stat().st_uid!=10250:raise RuntimeError('exact natural-text result owner differs')
p=root/'request-result.json'
if p.exists():
 if p.is_symlink() or p.stat().st_size>262144:raise RuntimeError('bounded natural-text receipt exceeded')
 out['result']=json.loads(p.read_text())
p=Path('/proc/'+str(r['process']['pid'])+'/stat')
out['exact_client_alive']=p.exists() and p.stat().st_uid==10250 and int(p.read_text().split(') ',1)[1].split()[19])==r['process']['start_ticks'] and p.read_text().split(') ',1)[1].split()[0]!='Z'
if out.get('result',{}).get('phase')=='failed':
 p=root/'request-worker.log'
 if p.exists() and p.stat().st_size<65536:out['failure_log']=p.read_text()[-3000:]
print(json.dumps(out))
'''.replace('@R@', repr(record))
    result = jump(code)
    (BASE / ('xolotl-pool-long-v111-' + args.profile + '-latest.json')).write_text(json.dumps(result, indent=2) + '\n')
    q = result.get('result', {})
    print(json.dumps(dict(profile=args.profile, phase=q.get('phase'), tests=len(q.get('tests', [])),
        exact_client_alive=result['exact_client_alive'], completed=q.get('diagnostic_completed'), cold_reference_qualified=q.get('cold_reference_qualified'), cases=q.get('cases'), error=q.get('error'),
        failure_log=result.get('failure_log')), indent=2))
