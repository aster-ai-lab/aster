"""Freeze and compile a synthetic GPU cache gate; do not execute it yet."""
import base64, hashlib, json, shlex, time
from xolotl_trial_common import BASE, lease, observe, jump, owned

ip = BASE / 'xolotl-mixed-cache-gpu-gate-v18r2-intent.json'
if ip.exists():
    raise RuntimeError('GPU cache gate compile already has an intent; inspect it')
observation = observe()
formal = json.loads((BASE / 'xolotl-active-serving-reservation.json').read_text())
candidate = json.loads((BASE / 'xolotl-ttft-shadow-v15-intent.json').read_text())
if observation['job_uid'] != candidate['allocation_uid'] or observation['job_uid'] == formal['allocation_uid']:
    raise RuntimeError('GPU gate compilation requires isolated optimization allocation')
parent = json.loads((BASE / 'xolotl-mixed-keep-v18-build-recipe.json').read_text())['parent']
root = lease()['remote'] + '/mixed-cache-gpu-gate-v18r2'
local = BASE / 'ttft-v18'
files = {name: (local / name).read_bytes() for name in ['aster_cache_restore_policy.hpp', 'mixed_cache_gpu_test.cpp', 'gpu_gate_compile_worker_v18r2.py']}
link_command=json.loads((BASE/'xolotl-ttft-v11-build-recipe.json').read_text())['link_command']
if not link_command.startswith(': && ') or not link_command.endswith(' && :'):raise RuntimeError('exact original link wrapper differs')
link=shlex.split(link_command[5:-5])
if link.count('libds41_dense.a')!=1:raise RuntimeError('exact original archive list differs')
argv = ['/usr/bin/c++', '-O2', '-std=c++20', '-Wall', '-Wextra', '-Werror', '-I' + root,
        '-I' + parent + '/source/include', '-isystem', '/usr/local/cuda/targets/x86_64-linux/include',
        root + '/mixed_cache_gpu_test.cpp', '-o', root + '/mixed_cache_gpu_test',
        '-L/usr/local/cuda/targets/x86_64-linux/lib',
        '-Wl,-rpath,/usr/local/cuda-12.6/targets/x86_64-linux/lib'] + link[link.index('libds41_dense.a'):]

plan = dict(argv=argv, link_cwd=parent+'/build', gpu_uuid=candidate['gpu_uuid'], sources=[dict(file=name, bytes=len(raw), sha256=hashlib.sha256(raw).hexdigest()) for name, raw in files.items()])
files['compile-plan.json'] = (json.dumps(plan, indent=2) + '\n').encode()
r = dict(unix=time.time(), phase='prepared', root=root, allocation_uid=observation['job_uid'], gpu_uuid=candidate['gpu_uuid'],
         gpu_executed=False, public_fixture_only=True,
         files=[dict(file=name, bytes=len(raw), sha256=hashlib.sha256(raw).hexdigest()) for name, raw in files.items()])
ip.write_text(json.dumps(r, indent=2) + '\n')
code = '''import base64,hashlib,json,os
from pathlib import Path
r=@R@;root=Path(r['root']);data=@DATA@
if os.getuid()!=10250 or root.parent.resolve()!=root.parent or root.parent.stat().st_uid!=10250 or root.exists():raise RuntimeError('exclusive GPU gate compile root differs')
root.mkdir(mode=0o700)
for row in r['files']:
 raw=base64.b64decode(data[row['file']])
 if len(raw)!=row['bytes'] or hashlib.sha256(raw).hexdigest()!=row['sha256']:raise RuntimeError('GPU gate source changed')
 with (root/row['file']).open('xb') as f:f.write(raw)
print(json.dumps(dict(staged=True)))
'''.replace('@R@', repr(r)).replace('@DATA@', repr({name: base64.b64encode(raw).decode() for name, raw in files.items()}))
r['stage'] = jump(code)
r['phase'] = 'staged'
ip.write_text(json.dumps(r, indent=2) + '\n')
code = '''import json,os,subprocess,time
from pathlib import Path
r=@R@;root=Path(r['root'])
with (root/'worker.log').open('xb') as log:
 p=subprocess.Popen(['timeout','--signal=TERM','--kill-after=10','60','python3','-u',str(root/'gpu_gate_compile_worker_v18r2.py')],cwd=root,stdin=subprocess.DEVNULL,stdout=log,stderr=log,start_new_session=True)
process=dict(pid=p.pid,start_ticks=int(Path('/proc/'+str(p.pid)+'/stat').read_text().split(') ',1)[1].split()[19]),uid=os.getuid(),unix=time.time(),budget_seconds=60)
(root/'compile-process.json').write_text(json.dumps(process,indent=2)+'\\n');print(json.dumps(process))
'''.replace('@R@', repr(r))
r['phase'] = 'compile_requested'
ip.write_text(json.dumps(r, indent=2) + '\n')
r['process'] = owned(code)
r['phase'] = 'compiling'
ip.write_text(json.dumps(r, indent=2) + '\n')
print(json.dumps(dict(phase=r['phase'], gpu_executed=False, root=root)))
