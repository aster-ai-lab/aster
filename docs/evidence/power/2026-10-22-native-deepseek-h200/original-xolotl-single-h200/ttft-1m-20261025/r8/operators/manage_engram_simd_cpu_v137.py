"""Launch once or observe the finite SIMD proof on jump-host CPUs, with GPUs hidden."""
import argparse
import base64
import hashlib
import json
import time
from xolotl_trial_common import BASE, jump, lease

parser = argparse.ArgumentParser()
parser.add_argument('action', choices=['launch', 'observe'])
action = parser.parse_args().action
intent_path = BASE / 'xolotl-engram-simd-cpu-v137-intent.json'
if action == 'launch':
    if intent_path.exists():
        raise RuntimeError('exact SIMD CPU handle exists; observe it')
    root = BASE / 'engram-fp8-simd-v136'
    manifest = json.loads((root / 'source-manifest.json').read_text())
    names = ['ds41_engram.cpp', 'aster_engram_fp8_simd.hpp', 'fp8_simd_exact_test.cpp']
    files = {name: (root / name).read_bytes() for name in names}
    for name, raw in files.items():
        expected = [row for row in manifest['files'] if row['file'] == name]
        if len(expected) != 1 or expected[0]['bytes'] != len(raw) or expected[0]['sha256'] != hashlib.sha256(raw).hexdigest():
            raise RuntimeError('frozen exact SIMD CPU input differs')
    original = json.loads((BASE / 'xolotl-engram-fp8-build-v73-intent.json').read_text())
    inputs = [dict(file=name, bytes=len(raw), sha256=hashlib.sha256(raw).hexdigest()) for name, raw in files.items()]
    plan = dict(original_engram_cpu_root=original['root'], original_source_sha256=manifest['original_source_sha256'], inputs=inputs)
    files['build-plan.json'] = (json.dumps(plan, indent=2) + '\n').encode()
    files['worker.py'] = (BASE / 'engram_simd_cpu_worker_v137.frozen.py').read_bytes()
    record = dict(unix=time.time(), phase='prepared', root=lease()['remote'] + '/cpu-engram-simd-v137',
                  gpu_executed=False, files=[dict(file=name, bytes=len(raw), sha256=hashlib.sha256(raw).hexdigest()) for name, raw in files.items()])
    code = '''import base64,hashlib,json,os,subprocess,time
from pathlib import Path
r=@R@;data=@DATA@;root=Path(r['root'])
if os.getuid()!=10250 or root.parent.resolve()!=root.parent or root.parent.stat().st_uid!=10250 or root.exists():raise RuntimeError('exact exclusive SIMD CPU root differs')
os.umask(0o077);root.mkdir(mode=0o700)
for row in r['files']:
 raw=base64.b64decode(data[row['file']])
 if len(raw)!=row['bytes'] or hashlib.sha256(raw).hexdigest()!=row['sha256']:raise RuntimeError('frozen SIMD CPU input differs')
 with (root/row['file']).open('xb') as f:f.write(raw)
with (root/'worker.log').open('xb') as log:
 p=subprocess.Popen(['timeout','--signal=TERM','--kill-after=15','260','python3','-u',str(root/'worker.py')],cwd=root,stdin=subprocess.DEVNULL,stdout=log,stderr=log,start_new_session=True)
process=dict(pid=p.pid,start_ticks=int(Path('/proc/'+str(p.pid)+'/stat').read_text().split(') ',1)[1].split()[19]),uid=os.getuid(),unix=time.time(),budget_seconds=260,namespace='jump host')
(root/'process.json').write_text(json.dumps(process,indent=2)+'\\n');print(json.dumps(process))
'''.replace('@R@', repr(record)).replace('@DATA@', repr({name: base64.b64encode(raw).decode() for name, raw in files.items()}))
    compile(code, 'launch-engram-simd-cpu-v137', 'exec')
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
r=@R@;root=Path(r['root']);out={'unix':time.time()}
if root.resolve()!=root or root.stat().st_uid!=10250:raise RuntimeError('exact SIMD CPU root owner differs')
p=Path('/proc/'+str(r['process']['pid'])+'/stat')
out['exact_cpu_process_alive']=p.exists() and p.stat().st_uid==10250 and int(p.read_text().split(') ',1)[1].split()[19])==r['process']['start_ticks'] and p.read_text().split(') ',1)[1].split()[0]!='Z'
for name in ['cpu-result.json','worker.log']:
 p=root/name
 if p.exists():
  if p.is_symlink() or p.stat().st_size>1048576:raise RuntimeError('SIMD CPU observation bound exceeded')
  out[name]=json.loads(p.read_text()) if name.endswith('.json') else p.read_text()[-4096:]
print(json.dumps(out))
'''.replace('@R@', repr(record))
    result = jump(code)
    path = BASE / 'xolotl-engram-simd-cpu-v137-latest.json'
    path.write_text(json.dumps(result, indent=2) + '\n')
    q = result.get('cpu-result.json', {})
    print(json.dumps(dict(phase=q.get('phase'), current=q.get('current'), alive=result['exact_cpu_process_alive'],
                         tests=[dict(mode=r['mode'], values=r['result']['scaled_value_cases'], avx2=r['result']['avx2_enabled']) for r in q.get('tests', [])],
                         control_byte_equal=q.get('original_object_byte_equal'), default0_byte_equal=q.get('default0_object_byte_equal'),
                         cpu_only_ratio=q.get('cpu_only_microbenchmark_ratio'), model_speed_claimed=False, error=q.get('error')), indent=2))
