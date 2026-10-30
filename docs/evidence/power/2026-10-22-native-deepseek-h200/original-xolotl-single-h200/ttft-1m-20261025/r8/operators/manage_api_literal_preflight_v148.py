"""Public-source-only finite API preflight, no GPU/credential/service operations."""
import argparse
import ast
import base64
import hashlib
import json
import time
from xolotl_trial_common import BASE, jump, lease

parser = argparse.ArgumentParser()
parser.add_argument('action', choices=['launch','observe'])
action = parser.parse_args().action
ip = BASE / 'xolotl-api-literal-v148-intent.json'
if action == 'launch':
    if ip.exists():
        raise RuntimeError('exact API literal CPU handle exists; observe it')
    parent = json.loads((BASE / 'xolotl-api-hybrid-v52-intent.json').read_text())
    fixture = json.loads((BASE / 'xolotl-canonical-code-fixture-v147-intent.json').read_text())
    root = BASE / 'api-adapter-v148'
    files = {name: (root/name).read_bytes() for name in ['overlay-plan.json','test_exact_token_cache.py','test_strict_native_json.py','test_combined_adapter.py','test_hybrid_native_json.py','actual-tool-output.json']}
    for name in ['serve_ds41.py','aster_literal_scan.py']:
        files['overlay/'+name] = (root/name).read_bytes()
    files['literal-preflight-plan.json'] = (json.dumps(dict(fixture_root=fixture['root']), indent=2)+'\n').encode()
    files['cpu-worker.py'] = (BASE/'api_literal_preflight_worker_v148.frozen.py').read_bytes()
    files['canonical-probe.py'] = (BASE/'api_literal_canonical_probe_v148.frozen.py').read_bytes()
    keys = [p.read_bytes().strip() for p in BASE.glob('access-private-*/api-key.txt')]
    if not keys or any(not key or key in raw for key in keys for raw in files.values()):
        raise RuntimeError('actual credential negative control refused')
    record = dict(unix=time.time(), phase='prepared', root=lease()['remote']+'/api-literal-preflight-v148',
        source_public_root=parent['root'], public_fixture_only=True, gpu_executed=False, credentials_copied=False,
        actual_credential_negative_control_passed=True,
        files=[dict(file=name,bytes=len(raw),sha256=hashlib.sha256(raw).hexdigest()) for name,raw in files.items()])
    tree = ast.parse((BASE/'launch_xolotl_longctx_shadow_v1.py').read_text())
    bodies = [node.value for node in ast.walk(tree) if isinstance(node,ast.Constant) and isinstance(node.value,str) and "root=Path(r['root']);src=Path(r['source_public_root'])" in node.value]
    if len(bodies) != 1:
        raise RuntimeError('exact public source bootstrap differs')
    template = bodies[0].split('# Carry the existing authorized service credential',1)[0]
    anchor = " with (root/row['file']).open('xb') as f:f.write(raw)"
    if template.count(anchor) != 1:
        raise RuntimeError('unique public preflight writer differs')
    template = template.replace(anchor, " (root/row['file']).parent.mkdir(mode=0o700,parents=True,exist_ok=True)\n"+anchor)
    template += '''
with (root/'worker.log').open('xb') as log:
 p=subprocess.Popen(['timeout','--signal=TERM','--kill-after=10','300','python3','-u',str(root/'cpu-worker.py')],cwd=root,stdin=subprocess.DEVNULL,stdout=log,stderr=log,start_new_session=True)
process=dict(pid=p.pid,start_ticks=int(Path('/proc/'+str(p.pid)+'/stat').read_text().split(') ',1)[1].split()[19]),uid=os.getuid(),unix=time.time(),budget_seconds=300,namespace='jump host')
(root/'process.json').write_text(json.dumps(process,indent=2)+'\\n');print(json.dumps(process))
'''
    code = template.replace('@R@',repr(record)).replace('@DATA@',repr({name:base64.b64encode(raw).decode() for name,raw in files.items()}))
    compile(code, 'stage-api-literal-preflight-v148', 'exec')
    with ip.open('x') as output:
        output.write(json.dumps(record,indent=2)+'\n')
    record['phase'] = 'launch_requested'
    ip.write_text(json.dumps(record,indent=2)+'\n')
    try:
        record.update(process=jump(code),phase='cpu_preflight_running')
    except BaseException:
        record['phase'] = 'launch_outcome_requires_inspection'
        ip.write_text(json.dumps(record,indent=2)+'\n')
        raise
    ip.write_text(json.dumps(record,indent=2)+'\n')
    print(json.dumps(dict(phase=record['phase'],process=record['process'])))
else:
    r = json.loads(ip.read_text())
    code = '''import json,time
from pathlib import Path
r=@R@;root=Path(r['root']);out=dict(unix=time.time())
if root.resolve()!=root or root.stat().st_uid!=10250:raise RuntimeError('exact public CPU stage owner differs')
p=Path('/proc/'+str(r['process']['pid'])+'/stat')
out['exact_cpu_process_alive']=p.exists() and p.stat().st_uid==10250 and int(p.read_text().split(') ',1)[1].split()[19])==r['process']['start_ticks'] and p.read_text().split(') ',1)[1].split()[0]!='Z'
for name in ['cpu-preflight-result.json','worker.log']:
 p=root/name
 if p.exists():
  if p.is_symlink() or p.stat().st_size>1048576:raise RuntimeError('bounded public CPU preflight observation exceeded')
  out[name]=json.loads(p.read_text()) if name.endswith('.json') else p.read_text()[-4096:]
q=out.get('cpu-preflight-result.json',{})
if q.get('phase')=='failed':
 for test in q.get('tests',[]):
  if test['exit_code']:
   p=root/('cpu-'+test['mode']+'-'+test['name']+'.stderr.log')
   if p.stat().st_size>1048576:raise RuntimeError('bounded CPU test failure log exceeded')
   out['failure_log']=p.read_text()[-8192:]
for mode in ['0','1']:
 p=root/('cpu-'+mode+'-actual-api-canonical1m.stdout.log')
 if p.exists() and p.stat().st_size and p.stat().st_size<16384:out['canonical_mode_'+mode]=json.loads(p.read_text())
print(json.dumps(out))
'''.replace('@R@',repr(r))
    result = jump(code)
    (BASE/'xolotl-api-literal-v148-latest.json').write_text(json.dumps(result,indent=2)+'\n')
    q = result.get('cpu-preflight-result.json',{})
    print(json.dumps(dict(phase=q.get('phase'),alive=result['exact_cpu_process_alive'],tests=len(q.get('tests',[])),
        canonical0=result.get('canonical_mode_0'),canonical1=result.get('canonical_mode_1'),error=q.get('error'),failure=result.get('failure_log')),indent=2))
