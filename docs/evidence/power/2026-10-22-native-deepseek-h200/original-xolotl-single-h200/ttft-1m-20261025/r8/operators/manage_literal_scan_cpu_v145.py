"""Stage/launch once, or observe the exact finite CPU-only API encoder replay."""
import argparse
import base64
import hashlib
import json
import time
from xolotl_trial_common import BASE, jump, lease

action = argparse.ArgumentParser()
action.add_argument('action', choices=['launch', 'observe'])
action = action.parse_args().action
intent_path = BASE / 'xolotl-literal-scan-cpu-v145-intent.json'
if action == 'launch':
    if intent_path.exists():
        raise RuntimeError('exact CPU replay handle exists; observe it')
    fixture = json.loads((BASE / 'xolotl-code-fullwindow-fixture-v114-intent.json').read_text())
    plan = dict(source_public_root=fixture['source_public_root'], fixture_root=fixture['root'],
        api_sha256='f062d9d39131959da9c981b6704e2ec27f89d8e2806730062dcabc3eb27682ba',
        canonical_api_id_sha256='b442a593a4066e3e99b663b670db4d7d624c471f1c5f6ee906e315c99d1969c4')
    files = {name: (BASE / 'literal-scan-v144' / name).read_bytes() for name in ['aster_literal_scan.py', 'test_literal_scan.py']}
    files['worker.py'] = (BASE / 'literal_scan_cpu_worker_v145.frozen.py').read_bytes()
    files['plan.json'] = (json.dumps(plan, indent=2) + '\n').encode()
    keys = [p.read_bytes().strip() for p in BASE.glob('access-private-*/api-key.txt')]
    if not keys or any(not key or key in raw for key in keys for raw in files.values()):
        raise RuntimeError('actual credential negative control failed')
    record = dict(unix=time.time(), phase='prepared', root=lease()['remote'] + '/cpu-literal-scan-v145',
        gpu_executed=False, model_requests_sent=0, live_service_changed=False, actual_credential_negative_control_passed=True,
        files=[dict(file=name, bytes=len(raw), sha256=hashlib.sha256(raw).hexdigest()) for name, raw in files.items()])
    code = '''import base64,hashlib,json,os,subprocess,time
from pathlib import Path
r=@R@;data=@DATA@;root=Path(r['root'])
if os.getuid()!=10250 or root.parent.resolve()!=root.parent or root.parent.stat().st_uid!=10250 or root.exists():raise RuntimeError('exact exclusive CPU replay root differs')
os.umask(0o077);root.mkdir(mode=0o700)
for row in r['files']:
 raw=base64.b64decode(data[row['file']])
 if len(raw)!=row['bytes'] or hashlib.sha256(raw).hexdigest()!=row['sha256']:raise RuntimeError('frozen exact CPU replay input differs')
 with (root/row['file']).open('xb') as output:output.write(raw)
with (root/'worker.log').open('xb') as log:
 p=subprocess.Popen(['timeout','--signal=TERM','--kill-after=10','180','python3','-u',str(root/'worker.py')],cwd=root,stdin=subprocess.DEVNULL,stdout=log,stderr=log,start_new_session=True)
process=dict(pid=p.pid,start_ticks=int(Path('/proc/'+str(p.pid)+'/stat').read_text().split(') ',1)[1].split()[19]),uid=os.getuid(),unix=time.time(),budget_seconds=180,namespace='jump host')
(root/'process.json').write_text(json.dumps(process,indent=2)+'\\n');print(json.dumps(process))
'''.replace('@R@', repr(record)).replace('@DATA@', repr({name: base64.b64encode(raw).decode() for name, raw in files.items()}))
    compile(code, 'launch-literal-scan-cpu-v145', 'exec')
    with intent_path.open('x') as output:
        output.write(json.dumps(record, indent=2) + '\n')
    record['phase'] = 'launch_requested'
    intent_path.write_text(json.dumps(record, indent=2) + '\n')
    try:
        record.update(process=jump(code), phase='launched')
    except BaseException:
        record['phase'] = 'launch_outcome_requires_inspection'
        intent_path.write_text(json.dumps(record, indent=2) + '\n')
        raise
    intent_path.write_text(json.dumps(record, indent=2) + '\n')
    print(json.dumps(dict(phase=record['phase'], process=record['process']), indent=2))
else:
    record = json.loads(intent_path.read_text())
    code = '''import json,time
from pathlib import Path
r=@R@;root=Path(r['root']);out=dict(unix=time.time())
if root.resolve()!=root or root.stat().st_uid!=10250:raise RuntimeError('exact CPU replay root owner differs')
p=Path('/proc/'+str(r['process']['pid'])+'/stat')
out['exact_cpu_process_alive']=p.exists() and p.stat().st_uid==10250 and int(p.read_text().split(') ',1)[1].split()[19])==r['process']['start_ticks'] and p.read_text().split(') ',1)[1].split()[0]!='Z'
for name in ['cpu-result.json','worker.log','unit-tests.log','upstream-frontend-tests.log','fixture-metadata-canonical.json']:
 p=root/name
 if p.exists():
  if p.is_symlink() or p.stat().st_size>1048576:raise RuntimeError('CPU replay observation bound exceeded')
  out[name]=json.loads(p.read_text()) if name.endswith('.json') else p.read_text()[-8192:]
print(json.dumps(out))
'''.replace('@R@', repr(record))
    result = jump(code)
    (BASE / 'xolotl-literal-scan-cpu-v145-latest.json').write_text(json.dumps(result, indent=2) + '\n')
    q = result.get('cpu-result.json', {})
    print(json.dumps(dict(phase=q.get('phase'), current=q.get('current'), alive=result['exact_cpu_process_alive'],
        units=q.get('unit_methods_passed'), runs=q.get('runs'), all_ids_exact=q.get('all_A_B_A_ids_exact'), error=q.get('error')), indent=2))
