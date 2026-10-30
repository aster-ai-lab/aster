"""Launch once or observe exact finite jump-host compilation, without using GPUs."""
import argparse
import base64
import hashlib
import json
import time
from xolotl_trial_common import BASE, jump, lease

parser = argparse.ArgumentParser()
parser.add_argument('action', choices=['launch', 'observe'])
action = parser.parse_args().action
intent_path = BASE / 'xolotl-serve-profile-cpu-v167-intent.json'
if action == 'launch':
    if intent_path.exists():
        raise RuntimeError('exact CPU compile handle exists; observe it')
    source = BASE / 'serve-decode-profile-v166'
    manifest = json.loads((source / 'source-manifest.json').read_text())
    parent = json.loads((BASE / 'xolotl-prefill-phase-cpu-v134-intent.json').read_text())
    files = {row['file']: (source / row['file']).read_bytes()
             for row in manifest['files'] if row['file'].endswith(('.hpp', '.cpp')) and '.original.' not in row['file']}
    for row in manifest['files']:
        if row['file'] in files and hashlib.sha256(files[row['file']]).hexdigest() != row['sha256']:
            raise RuntimeError('exact prepared prefill source differs')
    inputs = [dict(file=name, bytes=len(raw), sha256=hashlib.sha256(raw).hexdigest()) for name, raw in files.items()]
    link = json.loads((BASE / 'xolotl-native-phase-simd-v138-plan.json').read_text())
    moe = [p for p in link['control_link'] if p.endswith('/build-resident-mmq-v35/ds41_moe.cpp.o')]
    if len(moe) != 1:raise RuntimeError('exact original MoE object provenance differs')
    plan = dict(parent_cpu_root=parent['root'], parent_source_sha256=manifest['parent_source_sha256'], inputs=inputs, moe_object=moe[0])
    files['build-plan.json'] = (json.dumps(plan, indent=2) + '\n').encode()
    files['worker.py'] = (BASE / 'serve_decode_profile_cpu_worker_v167.frozen.py').read_bytes()
    record = dict(unix=time.time(), phase='prepared', root=lease()['remote'] + '/cpu-serve-profile-v167',
                  gpu_executed=False, original_source_sha256=manifest['parent_source_sha256'],
                  files=[dict(file=name, bytes=len(raw), sha256=hashlib.sha256(raw).hexdigest()) for name, raw in files.items()])
    code = '''import base64,hashlib,json,os,subprocess,time
from pathlib import Path
r=@R@;data=@DATA@;root=Path(r['root'])
if os.getuid()!=10250 or root.parent.resolve()!=root.parent or root.parent.stat().st_uid!=10250 or root.exists():raise RuntimeError('exact exclusive CPU root differs')
os.umask(0o077);root.mkdir(mode=0o700)
for row in r['files']:
 raw=base64.b64decode(data[row['file']])
 if len(raw)!=row['bytes'] or hashlib.sha256(raw).hexdigest()!=row['sha256']:raise RuntimeError('frozen CPU input differs')
 with (root/row['file']).open('xb') as f:f.write(raw)
with (root/'worker.log').open('xb') as log:
 p=subprocess.Popen(['timeout','--signal=TERM','--kill-after=15','260','python3','-u',str(root/'worker.py')],cwd=root,stdin=subprocess.DEVNULL,stdout=log,stderr=log,start_new_session=True)
process=dict(pid=p.pid,start_ticks=int(Path('/proc/'+str(p.pid)+'/stat').read_text().split(') ',1)[1].split()[19]),uid=os.getuid(),unix=time.time(),budget_seconds=260,namespace='jump host')
(root/'process.json').write_text(json.dumps(process,indent=2)+'\\n');print(json.dumps(process))
'''.replace('@R@', repr(record)).replace('@DATA@', repr({name: base64.b64encode(raw).decode() for name, raw in files.items()}))
    compile(code, 'launch-prefill-phase-cpu-v134', 'exec')
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
if root.resolve()!=root or root.stat().st_uid!=10250:raise RuntimeError('exact CPU root owner differs')
p=Path('/proc/'+str(r['process']['pid'])+'/stat')
out['exact_cpu_process_alive']=p.exists() and p.stat().st_uid==10250 and int(p.read_text().split(') ',1)[1].split()[19])==r['process']['start_ticks'] and p.read_text().split(') ',1)[1].split()[0]!='Z'
for name in ['build-result.json','control-compile.log','candidate-compile.log','worker.log']:
 p=root/name
 if p.exists():
  if p.is_symlink() or p.stat().st_size>1048576:raise RuntimeError('CPU observation artifact bound exceeded')
  out[name]=json.loads(p.read_text()) if name.endswith('.json') else p.read_text()[-4096:]
print(json.dumps(out))
'''.replace('@R@', repr(record))
    result = jump(code)
    path = BASE / 'xolotl-serve-profile-cpu-v167-latest.json'
    path.write_text(json.dumps(result, indent=2) + '\n')
    q = result.get('build-result.json', {})
    print(json.dumps(dict(phase=q.get('phase'), current=q.get('current'), alive=result['exact_cpu_process_alive'],
                         control_byte_equal=q.get('control_object_byte_equal'), error=q.get('error')), indent=2))
