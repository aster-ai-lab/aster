"""Launch once or observe a bounded exact post-concurrency single-request replay."""
import argparse
import base64
import hashlib
import json
import time
from xolotl_trial_common import BASE, jump, observe

parser = argparse.ArgumentParser()
parser.add_argument('action', choices=['launch', 'observe'])
action = parser.parse_args().action
intent = BASE / 'xolotl-post-concurrency-v66-intent.json'
if action == 'launch':
    if intent.exists():
        raise RuntimeError('post-concurrency replay has an exact handle; observe it')
    old = json.loads((BASE / 'xolotl-fullwindow-long-suite-v56-intent.json').read_text())
    service = json.loads((BASE / 'xolotl-draft-service-v53-off_repeat-intent.json').read_text())
    resource = observe()
    if resource['job_uid'] != service['allocation_uid'] or resource['phase'] != 'Running':
        raise RuntimeError('exact independent post-concurrency resource differs')
    root = service['root'] + '/post-concurrency-single-v66'
    parent_worker = next(row for row in old['files'] if row['file'] == 'fullwindow/request-worker.py')
    fixture = json.loads((BASE / 'xolotl-fullwindow-long-fixture-v54r1-intent.json').read_text())
    plan = dict(service_root=service['root'], base_url='http://' + service['pod_ip'] + ':30003/v1',
                fixture_root=fixture['root'], source_worker=old['root'] + '/fullwindow/request-worker.py',
                source_worker_sha256=parent_worker['sha256'],
                reference_result=old['root'] + '/fullwindow/request-result.json')
    files = {'request-plan.json': (json.dumps(plan, indent=2) + '\n').encode(),
             'request-worker.py': (BASE / 'xolotl_post_concurrency_worker_v66.frozen.py').read_bytes(),
             'xolotl_fullwindow_long_fixture_v54.py': (BASE / 'xolotl_fullwindow_long_fixture_v54.py').read_bytes()}
    record = dict(unix=time.time(), phase='prepared', root=root, allocation_uid=service['allocation_uid'],
                  gpu_uuid=service['gpu_uuid'], original_failure_retained=True,
                  files=[dict(file=n, bytes=len(v), sha256=hashlib.sha256(v).hexdigest()) for n, v in files.items()])
    code = '''import base64,hashlib,json,os,subprocess,time,urllib.request
from pathlib import Path
r=@R@;prior=@PRIOR@;root=Path(r['root']);data=@DATA@
if os.getuid()!=10250 or root.parent.resolve()!=root.parent or root.parent.stat().st_uid!=10250 or root.exists():raise RuntimeError('exclusive post-concurrency root owner differs')
p=Path('/proc/'+str(prior['process']['pid'])+'/stat')
alive=p.exists() and p.stat().st_uid==10250 and int(p.read_text().split(') ',1)[1].split()[19])==prior['process']['start_ticks'] and p.read_text().split(') ',1)[1].split()[0]!='Z'
q=json.loads((Path(prior['root'])/'suite-result.json').read_text())
if alive or q['phase']!='failed':raise RuntimeError('exact original finite suite must fail and finish first')
key=(root.parent/'credentials/api-key.txt').read_text().strip();req=urllib.request.Request(@SLOTS@,headers={'Authorization':'Bearer '+key})
with urllib.request.build_opener(urllib.request.ProxyHandler({})).open(req,timeout=5) as response:slots=json.load(response)
if not isinstance(slots,list) or not slots or any(slot['is_processing'] for slot in slots):raise RuntimeError('post-concurrency slots must be idle')
os.umask(0o077);root.mkdir(mode=0o700)
for row in r['files']:
 raw=base64.b64decode(data[row['file']])
 if len(raw)!=row['bytes'] or hashlib.sha256(raw).hexdigest()!=row['sha256']:raise RuntimeError('exact post-concurrency client input differs')
 with (root/row['file']).open('xb') as output:output.write(raw)
with (root/'request-worker.log').open('xb') as log:
 p=subprocess.Popen(['timeout','--signal=TERM','--kill-after=15','320','python3','-u',str(root/'request-worker.py')],cwd=root,stdin=subprocess.DEVNULL,stdout=log,stderr=log,start_new_session=True)
process=dict(pid=p.pid,start_ticks=int(Path('/proc/'+str(p.pid)+'/stat').read_text().split(') ',1)[1].split()[19]),uid=os.getuid(),unix=time.time(),budget_seconds=320,namespace='jump host')
(root/'request-process.json').write_text(json.dumps(process,indent=2)+'\\n');print(json.dumps(process))
'''.replace('@R@', repr(record)).replace('@PRIOR@', repr(old)).replace('@SLOTS@', repr(plan['base_url'].removesuffix('/v1') + '/slots')).replace('@DATA@', repr({n: base64.b64encode(v).decode() for n, v in files.items()}))
    compile(code, 'post-concurrency-single-launch-v66', 'exec')
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
if root.resolve()!=root or root.stat().st_uid!=10250:raise RuntimeError('exact post-concurrency root differs')
p=root/'request-result.json'
if p.exists():
 if p.is_symlink() or p.stat().st_size>262144:raise RuntimeError('bounded post-concurrency receipt exceeded')
 out['result']=json.loads(p.read_text())
p=Path('/proc/'+str(r['process']['pid'])+'/stat')
out['exact_client_alive']=p.exists() and p.stat().st_uid==10250 and int(p.read_text().split(') ',1)[1].split()[19])==r['process']['start_ticks'] and p.read_text().split(') ',1)[1].split()[0]!='Z'
if out.get('result',{}).get('phase')=='failed':
 p=root/'request-worker.log'
 if p.exists() and p.stat().st_size<65536:out['failure_log']=p.read_text()[-3000:]
print(json.dumps(out))
'''.replace('@R@', repr(record))
    result = jump(code)
    (BASE / 'xolotl-post-concurrency-v66-latest.json').write_text(json.dumps(result, indent=2) + '\n')
    q = result.get('result', {})
    print(json.dumps(dict(phase=q.get('phase'), exact_client_alive=result['exact_client_alive'],
                          matched=q.get('post_concurrent_single_matches_cold'), error=q.get('error'),
                          failure_log=result.get('failure_log')), indent=2))
