"""Stage an opt-in checked-cache candidate; compile only, no model execution."""
import base64, hashlib, json, time
from xolotl_trial_common import BASE, lease, observe, jump, owned

intent = BASE / 'xolotl-mixed-keep-build-v18-intent.json'
if intent.exists():
    raise RuntimeError('cache candidate intent exists; observe its exact process')
observation = observe()
candidate = json.loads((BASE / 'xolotl-ttft-shadow-v15-intent.json').read_text())
reservation = json.loads((BASE / 'xolotl-active-serving-reservation.json').read_text())
if observation['job_uid'] != candidate['allocation_uid'] or observation['job_uid'] == reservation['allocation_uid']:
    raise RuntimeError('compile requires the current isolated optimization allocation')
recipe = json.loads((BASE / 'xolotl-mixed-keep-v18-build-recipe.json').read_text())
original_link = json.loads((BASE / 'xolotl-ttft-v11-build-recipe.json').read_text())
v15_build = json.loads((BASE / 'xolotl-ttft-build-v15-intent.json').read_text())
local = BASE / 'ttft-v18'
files = {name: (local / name).read_bytes() for name in ['ds41_moe.cpp', 'aster_cache_restore_policy.hpp', 'cache_policy_test.cpp', 'build_worker.py', 'mixed-cache-checked-v18.patch']}
plan = dict(recipe, link_command=original_link['link_command'], v15_build_root=v15_build['root'],
            v15_binary_sha256=candidate['plan']['binary_sha256'],
            sources=[dict(file=name, bytes=len(raw), sha256=hashlib.sha256(raw).hexdigest()) for name, raw in files.items()])
files['plan.json'] = (json.dumps(plan, indent=2) + '\n').encode()
record = dict(unix=time.time(), phase='prepared', root=lease()['remote'] + '/build-mixed-keep-v18',
              allocation_uid=observation['job_uid'], gpu_free=True, public_fixture_only=True,
              files=[dict(file=name, bytes=len(raw), sha256=hashlib.sha256(raw).hexdigest()) for name, raw in files.items()])
intent.write_text(json.dumps(record, indent=2) + '\n')
code = '''import base64,hashlib,json,os
from pathlib import Path
r=@R@;data=@DATA@;root=Path(r['root'])
if os.getuid()!=10250 or root.parent.resolve()!=root.parent or root.parent.stat().st_uid!=10250 or root.exists():raise RuntimeError('exclusive owned cache build root differs')
root.mkdir(mode=0o700)
for row in r['files']:
 raw=base64.b64decode(data[row['file']])
 if len(raw)!=row['bytes'] or hashlib.sha256(raw).hexdigest()!=row['sha256']:raise RuntimeError('frozen checked cache source changed')
 with (root/row['file']).open('xb') as f:f.write(raw)
print(json.dumps(dict(staged=True,files=len(r['files']))))
'''.replace('@R@', repr(record)).replace('@DATA@', repr({name: base64.b64encode(raw).decode() for name, raw in files.items()}))
record['stage'] = jump(code)
record['phase'] = 'staged'
intent.write_text(json.dumps(record, indent=2) + '\n')
code = '''import json,os,subprocess,time
from pathlib import Path
r=@R@;root=Path(r['root'])
if root.stat().st_uid!=10250 or root.is_symlink():raise RuntimeError('cache build root owner differs')
with (root/'worker.log').open('xb') as log:
 p=subprocess.Popen(['timeout','--signal=TERM','--kill-after=15','210','python3','-u',str(root/'build_worker.py')],cwd=root,stdin=subprocess.DEVNULL,stdout=log,stderr=log,start_new_session=True)
process=dict(pid=p.pid,start_ticks=int(Path('/proc/'+str(p.pid)+'/stat').read_text().split(') ',1)[1].split()[19]),uid=os.getuid(),unix=time.time(),budget_seconds=210)
(root/'process.json').write_text(json.dumps(process,indent=2)+'\\n');print(json.dumps(process))
'''.replace('@R@', repr(record))
record['phase'] = 'launch_requested'
intent.write_text(json.dumps(record, indent=2) + '\n')
record['process'] = owned(code)
record['phase'] = 'launched'
intent.write_text(json.dumps(record, indent=2) + '\n')
print(json.dumps(dict(phase='launched', gpu_free=True, root=record['root'])))
