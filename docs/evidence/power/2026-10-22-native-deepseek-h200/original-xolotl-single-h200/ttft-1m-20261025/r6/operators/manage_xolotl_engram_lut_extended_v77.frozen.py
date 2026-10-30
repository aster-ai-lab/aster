"""Launch once or observe exact native-tool and matched long-output comparison."""
import argparse
import base64
import hashlib
import json
import time
from xolotl_trial_common import BASE, jump, observe

parser = argparse.ArgumentParser()
parser.add_argument('profile', choices=['lut_local'])
parser.add_argument('action', choices=['launch', 'observe'])
args = parser.parse_args()
intent = BASE / ('xolotl-engram-lut-extended-v77-' + args.profile + '-intent.json')
if args.action == 'launch':
    if intent.exists():
        raise RuntimeError('extended comparison has an exact handle; observe it')
    service = json.loads((BASE / ('xolotl-engram-lut-v75-' + args.profile + '-intent.json')).read_text())
    basic = json.loads((BASE / ('xolotl-engram-lut-natural-v76-' + args.profile + '-intent.json')).read_text())
    resource = observe()
    reservation = json.loads((BASE / 'xolotl-active-serving-reservation.json').read_text())
    if resource['phase'] != 'Running' or resource['job_uid'] != service['allocation_uid'] or resource['job_uid'] == reservation['allocation_uid']:
        raise RuntimeError('exact isolated comparison allocation differs')
    reference = json.loads((BASE / 'xolotl-draft-extended-v53-off-intent.json').read_text()) if args.profile != 'off' else None
    plan = dict(service_root=service['root'], base_url='http://' + service['pod_ip'] + ':30003/v1',
                profile=args.profile, reference_root=reference['root'] if reference else None)
    files = {'request-worker.py': (BASE / 'xolotl_draft_long_output_worker_v50.frozen.py').read_bytes(),
             'request-plan.json': (json.dumps(plan, indent=2) + '\n').encode()}
    record = dict(unix=time.time(), phase='prepared', root=service['root'] + '/qualification-extended-v77',
        allocation_uid=service['allocation_uid'], gpu_uuid=service['gpu_uuid'], binary_sha256=service['plan']['binary_sha256'],
        public_fixture_only=True, files=[dict(file=n, bytes=len(v), sha256=hashlib.sha256(v).hexdigest()) for n, v in files.items()])
    code = '''import base64,hashlib,json,os,subprocess,time,urllib.request
from pathlib import Path
r=@R@;basic=@BASIC@;reference=@REFERENCE@;root=Path(r['root']);service=root.parent;data=@DATA@
if os.getuid()!=10250 or service.resolve()!=service or service.stat().st_uid!=10250 or root.exists():raise RuntimeError('exact exclusive extended client root differs')
q=json.loads((service/'service-longctx-v1-result.json').read_text())
if q['phase']!='serving' or q['gpu_uuid']!=r['gpu_uuid'] or q['binary_sha256']!=r['binary_sha256'] or q['context_tokens']!=1048576:raise RuntimeError('exact combined native/API service differs')
def finished(record,name):
 p=Path('/proc/'+str(record['process']['pid'])+'/stat')
 alive=p.exists() and p.stat().st_uid==10250 and int(p.read_text().split(') ',1)[1].split()[19])==record['process']['start_ticks'] and p.read_text().split(') ',1)[1].split()[0]!='Z'
 result=json.loads((Path(record['root'])/name).read_text())
 if alive or result['phase']!='passed':raise RuntimeError('exact prior comparison must pass and finish')
 return result
qa=finished(basic,'request-result.json')
if len(qa['tests'])!=7 or not qa['handoff_three_position_retrieval_passed']:raise RuntimeError('native basic28 correctness gates incomplete')
if reference is not None:finished(reference,'request-result.json')
key=(service/'credentials/api-key.txt').read_text().strip()
request=urllib.request.Request(@BASE_URL@.removesuffix('/v1')+'/slots',headers={'Authorization':'Bearer '+key})
with urllib.request.build_opener(urllib.request.ProxyHandler({})).open(request,timeout=5) as response:
 raw=response.read(65537)
 if len(raw)>65536:raise RuntimeError('bounded private slot status exceeded')
 slots=json.loads(raw)
if not isinstance(slots,list) or not slots or any(slot['is_processing'] for slot in slots):raise RuntimeError('private comparison slots must be idle')
if time.time()+720>q['lease_ends_unix']:raise RuntimeError('finite extended comparison lease insufficient')
os.umask(0o077);root.mkdir(mode=0o700)
for row in r['files']:
 raw=base64.b64decode(data[row['file']])
 if len(raw)!=row['bytes'] or hashlib.sha256(raw).hexdigest()!=row['sha256']:raise RuntimeError('frozen extended client input differs')
 with (root/row['file']).open('xb') as f:f.write(raw)
with (root/'request-worker.log').open('xb') as log:
 p=subprocess.Popen(['timeout','--signal=TERM','--kill-after=15','600','python3','-u',str(root/'request-worker.py')],cwd=root,stdin=subprocess.DEVNULL,stdout=log,stderr=log,start_new_session=True)
process=dict(pid=p.pid,start_ticks=int(Path('/proc/'+str(p.pid)+'/stat').read_text().split(') ',1)[1].split()[19]),uid=os.getuid(),unix=time.time(),budget_seconds=600)
(root/'request-process.json').write_text(json.dumps(process,indent=2)+chr(10));print(json.dumps(process))
'''.replace('@R@', repr(record)).replace('@BASIC@', repr(basic)).replace('@REFERENCE@', repr(reference)).replace('@BASE_URL@', repr(plan['base_url'])).replace('@DATA@', repr({n: base64.b64encode(v).decode() for n, v in files.items()}))
    compile(code, 'combined-native-extended-v50-launch', 'exec')
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
    print(json.dumps(dict(phase=record['phase'], profile=args.profile, process=record['process'])))
else:
    record = json.loads(intent.read_text())
    code = '''import json,time
from pathlib import Path
r=@R@;root=Path(r['root']);out={'unix':time.time()}
if root.resolve()!=root or root.stat().st_uid!=10250:raise RuntimeError('exact extended client owner differs')
p=root/'request-result.json'
if p.exists():
 if p.is_symlink() or p.stat().st_size>1024**2:raise RuntimeError('bounded extended receipt exceeded')
 out['result']=json.loads(p.read_text())
p=Path('/proc/'+str(r['process']['pid'])+'/stat')
out['exact_client_alive']=p.exists() and p.stat().st_uid==10250 and int(p.read_text().split(') ',1)[1].split()[19])==r['process']['start_ticks'] and p.read_text().split(') ',1)[1].split()[0]!='Z'
if out.get('result',{}).get('phase')=='failed':
 p=root/'request-worker.log'
 if p.exists() and p.stat().st_size<65536:out['failure_log']=p.read_text()[-3000:]
print(json.dumps(out))
'''.replace('@R@', repr(record))
    result = jump(code)
    (BASE / ('xolotl-engram-lut-extended-v77-' + args.profile + '-latest.json')).write_text(json.dumps(result, indent=2) + '\n')
    q = result.get('result', {})
    print(json.dumps(dict(profile=args.profile, phase=q.get('phase'), tests=len(q.get('tests', [])),
        exact_client_alive=result['exact_client_alive'], error=q.get('error'),
        native_tool_roundtrip_passed=q.get('native_tool_roundtrip_passed'),
        long_summary=q.get('long_summary'), reference_matched=q.get('exact_long_output_reference_matched'),
        failure_log=result.get('failure_log')), indent=2))
