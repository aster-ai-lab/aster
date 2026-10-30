"""Launch once or observe the exact finite off-repeat edge/full1M-plus1024-output/load suite."""
import argparse
import base64
import hashlib
import json
import time
from xolotl_trial_common import BASE, jump, observe

parser = argparse.ArgumentParser()
parser.add_argument('action', choices=['launch', 'observe'])
parser.add_argument('--profile', choices=['control_a','topk','control_b'], required=True)
args = parser.parse_args()
action = args.action
profile = args.profile
intent = BASE / ('xolotl-topk-fullwindow-v214-' + profile + '-intent.json')

if action == 'launch':
    if intent.exists():
        raise RuntimeError('exact V56 handle already exists; observe it')
    prefix = 'xolotl-topk-native-v212-' + profile
    extended_prefix = 'xolotl-topk-native-extended-v212-' + profile
    service = json.loads((BASE / (prefix + '-intent.json')).read_text())
    basic = json.loads((BASE / (prefix + '-qualification-intent.json')).read_text())
    extended = json.loads((BASE / (extended_prefix + '-intent.json')).read_text())
    fixture = json.loads((BASE / 'xolotl-canonical-code-fixture-v147-intent.json').read_text())
    if fixture['phase'] != 'staged' or fixture['body_changed']:
        raise RuntimeError('unchanged canonical actual-API public fixture must be staged')
    metadata = fixture['metadata']
    api_sha256 = 'f062d9d39131959da9c981b6704e2ec27f89d8e2806730062dcabc3eb27682ba'
    observation = observe()
    reservation = json.loads((BASE / 'xolotl-active-serving-reservation.json').read_text())
    if (observation['phase'] != 'Running' or observation['job_uid'] != service['allocation_uid']
            or service['allocation_uid'] == reservation['allocation_uid']
            or len(observation['replicas']) != 1 or observation['replicas'][0]['uid'] != service['replica_uid']):
        raise RuntimeError('exact independent optimization allocation required')
    root = service['root'] + '/qualification-canonical-topk-v214'
    common = dict(base_url='http://' + service['pod_ip'] + ':30003/v1', service_root=service['root'],
                  context_tokens=1048576, context_safety_tokens=8)
    suite = dict(service_root=service['root'], gpu_uuid=service['gpu_uuid'],
                 binary_sha256=service['plan']['binary_sha256'])
    sources = {'remainder': 'xolotl_resident_qos_remainder_worker_v38.frozen.py',
               'fullwindow': 'xolotl_code_fullwindow_long_worker_v119.frozen.py'}
    files = {'suite-worker.py': (BASE / 'xolotl_literal_fullwindow_suite_worker_v150.frozen.py').read_bytes(),
             'suite-plan.json': (json.dumps(suite, indent=2) + '\n').encode()}
    for stage, source in sources.items():
        plan = dict(common)
        if stage == 'fullwindow':
            plan.update(fixture_root=fixture['root'], fixture_sha256=metadata['prompt_sha256'],
                        fixture_id_sha256=metadata['prompt_id_sha256'], load_requests_per_level=8)
            files['fullwindow/xolotl_fullwindow_long_fixture_v54.py'] = (BASE / 'xolotl_fullwindow_long_fixture_v54.py').read_bytes()
        files[stage + '/request-worker.py'] = (BASE / source).read_bytes()
        files[stage + '/request-plan.json'] = (json.dumps(plan, indent=2) + '\n').encode()
    record = dict(unix=time.time(), phase='prepared', root=root, allocation_uid=service['allocation_uid'],
                  gpu_uuid=service['gpu_uuid'], binary_sha256=suite['binary_sha256'], public_fixture_only=True, api_sha256=api_sha256, canonical_api_metadata=True, acceptance_changed=False, profile=profile,
                  files=[dict(file=n, bytes=len(b), sha256=hashlib.sha256(b).hexdigest()) for n, b in files.items()])
    code = '''import base64,hashlib,json,os,subprocess,time
from pathlib import Path
c=@C@;basic=@BASIC@;extended=@EXTENDED@;root=Path(c['root']);service=root.parent
if os.getuid()!=10250 or service.resolve()!=service or service.stat().st_uid!=10250 or root.exists():raise RuntimeError('exact exclusive public suite owner differs')
q=json.loads((service/'service-longctx-v1-result.json').read_text())
if q['phase']!='serving' or q['gpu_uuid']!=c['gpu_uuid'] or q['binary_sha256']!=c['binary_sha256'] or q['context_tokens']!=1048576:raise RuntimeError('exact qualified off-repeat service identity differs')
p=Path('/proc/'+str(basic['process']['pid'])+'/stat')
alive=p.exists() and p.stat().st_uid==10250 and int(p.read_text().split(') ',1)[1].split()[19])==basic['process']['start_ticks'] and p.read_text().split(') ',1)[1].split()[0]!='Z'
qa=json.loads((Path(basic['root'])/'request-result.json').read_text())
if alive or qa['phase']!='passed' or len(qa['tests'])!=28 or not qa['handoff_three_position_retrieval_passed']:raise RuntimeError('exact qualified off-repeat basic28 must pass and finish first')
proof=extended['process'];p=Path('/proc/'+str(proof['pid'])+'/stat')
alive=p.exists() and p.stat().st_uid==10250 and int(p.read_text().split(') ',1)[1].split()[19])==proof['start_ticks'] and p.read_text().split(') ',1)[1].split()[0]!='Z'
qe=json.loads((Path(extended['root'])/'request-result.json').read_text())
if alive or qe['phase']!='passed' or not qe['native_tool_roundtrip_passed'] or not qe['exact_long_output_reference_matched'] or len(qe['tests'])!=12:raise RuntimeError('exact native tool and matched1024 off-repeat gate must pass and finish')
plan=json.loads((service/'service-plan.json').read_text())
if plan['engine_env']['ASTER_XQ_DSPARK_CUM']!='0' or plan['engine_env'].get('STRATA_DEBUG'):raise RuntimeError('only unchanged off control is qualified for actual1M')
api=service/'source/tools/v41/serve/serve_ds41.py'
if hashlib.sha256(api.read_bytes()).hexdigest()!=c['api_sha256']:raise RuntimeError('exact common strict API differs')
if time.time()+7200+120>q['lease_ends_unix']:raise RuntimeError('finite suite lease insufficient')
os.umask(0o077);root.mkdir(mode=0o700);data=@DATA@
for row in c['files']:
 raw=base64.b64decode(data[row['file']])
 if len(raw)!=row['bytes'] or hashlib.sha256(raw).hexdigest()!=row['sha256']:raise RuntimeError('frozen suite input differs')
 p=root/row['file'];p.parent.mkdir(mode=0o700,parents=True,exist_ok=True)
 with p.open('xb') as f:f.write(raw)
with (root/'suite-worker.log').open('xb') as log:
 p=subprocess.Popen(['timeout','--signal=TERM','--kill-after=15','7200','python3','-u',str(root/'suite-worker.py')],cwd=root,stdin=subprocess.DEVNULL,stdout=log,stderr=log,start_new_session=True)
process=dict(pid=p.pid,start_ticks=int(Path('/proc/'+str(p.pid)+'/stat').read_text().split(') ',1)[1].split()[19]),uid=os.getuid(),unix=time.time(),budget_seconds=7200)
(root/'suite-process.json').write_text(json.dumps(process,indent=2)+'\\n');print(json.dumps(process))
'''.replace('@C@', repr(record)).replace('@BASIC@', repr(basic)).replace('@EXTENDED@', repr(extended)).replace('@DATA@', repr({n: base64.b64encode(b).decode() for n, b in files.items()}))
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
for name in ['suite-result.json','remainder/request-result.json','fullwindow/request-result.json']:
 p=root/name
 if p.exists():
  if p.is_symlink() or p.stat().st_size>1024**2:raise RuntimeError('public suite receipt bound exceeded')
  out[name]=json.loads(p.read_text())
p=Path('/proc/'+str(c['process']['pid'])+'/stat')
out['exact_client_alive']=p.exists() and p.stat().st_uid==10250 and int(p.read_text().split(') ',1)[1].split()[19])==c['process']['start_ticks'] and p.read_text().split(') ',1)[1].split()[0]!='Z'
print(json.dumps(out))
'''.replace('@C@', repr(record))
    result = jump(code)
    (BASE / ('xolotl-topk-fullwindow-v214-' + profile + '-latest.json')).write_text(json.dumps(result, indent=2) + '\n')
    print(json.dumps(dict(phase=result.get('suite-result.json', {}).get('phase'),
                         current_stage=result.get('suite-result.json', {}).get('current_stage'),
                         exact_client_alive=result['exact_client_alive'],
                         error=result.get('suite-result.json', {}).get('error'),
                         stages=result.get('suite-result.json', {}).get('stages'),
                         children={k: dict(phase=v.get('phase'), tests=len(v.get('tests', [])),
                                          current_test=v.get('current_test'), error=v.get('error'))
                                   for k, v in result.items() if k.endswith('/request-result.json')}), indent=2))
