"""Prepare, launch after prior GPU drain, or observe controlled native variants."""
import argparse
import base64
import hashlib
import json
import time
from pathlib import Path
from xolotl_trial_common import BASE, jump, lease, observe, owned

parser = argparse.ArgumentParser()
parser.add_argument('action', choices=['prepare', 'launch', 'observe'])
action = parser.parse_args().action
plan_path = BASE / 'xolotl-native-phase-simd-v138-plan.json'
intent_path = BASE / 'xolotl-native-phase-simd-v138-intent.json'

if action == 'prepare':
    if plan_path.exists():
        raise RuntimeError('exact prepared variant plan exists; launch or observe it')
    previous = json.loads((BASE / 'xolotl-pool-seed-native-v106-intent.json').read_text())
    expected = next(row for row in previous['files'] if row['file'] == 'native-link-plan.json')
    code = '''import hashlib,json
from pathlib import Path
p=Path(@ROOT@)/'native-link-plan.json';e=@EXPECTED@
if p.is_symlink() or p.stat().st_uid!=10250 or p.stat().st_size!=e['bytes']:raise RuntimeError('exact current native link plan owner/size differs')
raw=p.read_bytes()
if hashlib.sha256(raw).hexdigest()!=e['sha256']:raise RuntimeError('exact current native link plan SHA differs')
print(raw.decode(),end='')
'''.replace('@ROOT@', repr(previous['root'])).replace('@EXPECTED@', repr(expected))
    old = jump(code)
    trace = json.loads((BASE / 'xolotl-prefill-phase-cpu-v134-latest.json').read_text())['build-result.json']
    simd = json.loads((BASE / 'xolotl-engram-simd-cpu-v137-latest.json').read_text())['cpu-result.json']
    if trace['phase'] != 'compiled' or simd['phase'] != 'compiled':
        raise RuntimeError('both exact CPU builds must pass first')
    link = list(old['control_link'])
    if link.count(old['original_host_object']) != 1:
        raise RuntimeError('exact native9a generator substitution differs')
    link[link.index(old['original_host_object'])] = old['candidate_host_object']
    dense = [x for x in link if Path(x).name == 'libds41_dense.a']
    if len(dense) != 1:
        raise RuntimeError('exact one model dense archive required')
    service = json.loads((BASE / 'xolotl-code-service-v118-full1m_code-intent.json').read_text())
    formal = json.loads((BASE / 'xolotl-active-serving-reservation.json').read_text())
    if previous['allocation_uid'] != service['allocation_uid'] or service['gpu_uuid'] == formal['gpu_uuid']:
        raise RuntimeError('exact independent optimization role differs')
    plan = dict(schema='aster-native-phase-simd-link-v138', link_cwd=old['link_cwd'], control_link=link,
                control_binary_sha256=service['plan']['binary_sha256'], original_generator=old['candidate_host_object'],
                dense_archive=dense[0], prior_service_root=service['root'],
                trace_cpu_root=json.loads((BASE / 'xolotl-prefill-phase-cpu-v134-intent.json').read_text())['root'],
                simd_cpu_root=json.loads((BASE / 'xolotl-engram-simd-cpu-v137-intent.json').read_text())['root'],
                inputs=old['inputs'] + [trace['candidate_object'], simd['candidate_object']],
                allocation_uid=service['allocation_uid'], gpu_uuid=service['gpu_uuid'], formal_gpu_uuid=formal['gpu_uuid'])
    with plan_path.open('x') as output:
        output.write(json.dumps(plan, indent=2) + '\n')
    # The same bounded sealed-file transport as V125, with a single new closed label.
    source = (BASE / 'xolotl_sealed_bootstrap_v125.py').read_text()
    before = "LABELS={'native-profile-v125'}"
    if source.count(before) != 1:
        raise RuntimeError('exact sealed transport label set differs')
    with (BASE / 'xolotl_sealed_bootstrap_v138.py').open('x') as output:
        output.write(source.replace(before, "LABELS={'native-variants-v138'}"))
    print(json.dumps(dict(phase='prepared', gpu_executed=False, prior_service_must_be_stopped=True)))
elif action == 'launch':
    if intent_path.exists():
        raise RuntimeError('native variants have an exact handle; observe it')
    plan = json.loads(plan_path.read_text())
    o = observe()
    if o['job_uid'] != plan['allocation_uid'] or o['phase'] != 'Running':
        raise RuntimeError('exact current independent GPU allocation differs')
    check = '''import json,subprocess
from pathlib import Path
root=Path(@ROOT@);q=json.loads((root/'service-longctx-v1-result.json').read_text())
if q['phase']!='stopped' or not q.get('api_reaped'):raise RuntimeError('exact previous model must stop and reap before variant link')
r=subprocess.run(['nvidia-smi','--query-compute-apps=pid','--format=csv,noheader,nounits'],capture_output=True,text=True,timeout=5,check=True)
if r.stdout.strip():raise RuntimeError('optimization GPU must be idle')
print(json.dumps({'prior_model_stopped':True,'gpu_idle':True}))
'''.replace('@ROOT@', repr(plan['prior_service_root']))
    idle = owned(check)
    files = {'native-link-plan.json': (json.dumps(plan, indent=2) + '\n').encode(),
             'worker.py': (BASE / 'native_phase_simd_link_worker_v138.frozen.py').read_bytes()}
    record = dict(unix=time.time(), phase='prepared', root=lease()['remote'] + '/build-native-phase-simd-v138',
                  allocation_uid=o['job_uid'], gpu_uuid=plan['gpu_uuid'], idle=idle, gpu_qualified=False,
                  files=[dict(file=name, bytes=len(raw), sha256=hashlib.sha256(raw).hexdigest()) for name, raw in files.items()])
    code = '''import base64,hashlib,json,os,subprocess,time
from pathlib import Path
r=@R@;data=@DATA@;root=Path(r['root'])
if root.parent.resolve()!=root.parent or root.parent.stat().st_uid!=10250 or root.exists():raise RuntimeError('exclusive native variant root differs')
os.umask(0o077);root.mkdir(mode=0o700)
for row in r['files']:
 raw=base64.b64decode(data[row['file']])
 if len(raw)!=row['bytes'] or hashlib.sha256(raw).hexdigest()!=row['sha256']:raise RuntimeError('frozen native variant input differs')
 with (root/row['file']).open('xb') as f:f.write(raw)
with (root/'worker.log').open('xb') as log:
 p=subprocess.Popen(['timeout','--signal=TERM','--kill-after=15','220','python3','-u',str(root/'worker.py')],cwd=root,env=dict(os.environ,CUDA_VISIBLE_DEVICES=''),stdin=subprocess.DEVNULL,stdout=log,stderr=log,start_new_session=True)
process=dict(pid=p.pid,start_ticks=int(Path('/proc/'+str(p.pid)+'/stat').read_text().split(') ',1)[1].split()[19]),uid=os.getuid(),unix=time.time(),budget_seconds=220,namespace='optimization pod')
(root/'process.json').write_text(json.dumps(process,indent=2)+'\\n');print(json.dumps(process))
'''.replace('@R@', repr(record)).replace('@DATA@', repr({name: base64.b64encode(raw).decode() for name, raw in files.items()}))
    compile(code, 'launch-native-phase-simd-v138', 'exec')
    with intent_path.open('x') as output:
        output.write(json.dumps(record, indent=2) + '\n')
    record['phase'] = 'launch_requested'
    intent_path.write_text(json.dumps(record, indent=2) + '\n')
    try:
        from xolotl_sealed_bootstrap_v138 import sealed_owned
        record.update(process=sealed_owned(code, 'native-variants-v138'), phase='launched')
    except BaseException:
        record['phase'] = 'launch_outcome_requires_inspection'
        intent_path.write_text(json.dumps(record, indent=2) + '\n')
        raise
    intent_path.write_text(json.dumps(record, indent=2) + '\n')
    print(json.dumps(dict(phase=record['phase'], process=record['process'], gpu_qualified=False)))
else:
    record = json.loads(intent_path.read_text())
    o = observe()
    if o['job_uid'] != record['allocation_uid']:
        raise RuntimeError('exact variant observation allocation differs')
    code = '''import json,time
from pathlib import Path
r=@R@;root=Path(r['root']);out={'unix':time.time()}
if root.resolve()!=root or root.stat().st_uid!=10250:raise RuntimeError('exact native variant root owner differs')
process=r.get('process') or json.loads((root/'process.json').read_text());p=Path('/proc/'+str(process['pid'])+'/stat')
out['exact_link_process_alive']=p.exists() and p.stat().st_uid==10250 and int(p.read_text().split(') ',1)[1].split()[19])==process['start_ticks'] and p.read_text().split(') ',1)[1].split()[0]!='Z'
for name in ['native-link-result.json','worker.log']:
 p=root/name
 if p.exists():
  if p.is_symlink() or p.stat().st_size>1048576:raise RuntimeError('bounded native variant observation exceeded')
  out[name]=json.loads(p.read_text()) if name.endswith('.json') else p.read_text()[-4096:]
print(json.dumps(out))
'''.replace('@R@', repr(record))
    result = owned(code)
    (BASE / 'xolotl-native-phase-simd-v138-latest.json').write_text(json.dumps(result, indent=2) + '\n')
    q = result.get('native-link-result.json', {})
    print(json.dumps(dict(phase=q.get('phase'), current=q.get('current'), alive=result['exact_link_process_alive'],
                         control_byte_equal=q.get('control_binary_byte_equal'), variants=q.get('variants'), error=q.get('error')), indent=2))
