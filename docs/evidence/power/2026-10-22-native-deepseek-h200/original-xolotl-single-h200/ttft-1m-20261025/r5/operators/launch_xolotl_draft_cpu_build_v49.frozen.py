"""Launch one bounded CPU-only controlled generator compilation."""
import base64
import hashlib
import json
import time
from xolotl_trial_common import BASE, jump

intent = BASE / 'xolotl-draft-cumulative-v49-generator-cpu-intent.json'
if intent.exists():
    raise RuntimeError('generator CPU build has an exact handle; observe it')
cpu = json.loads((BASE / 'xolotl-draft-cumulative-v49-cpu-intent.json').read_text())
parent = json.loads((BASE / 'xolotl-draft-cumulative-v49-generator-parent.json').read_text())
header = json.loads((BASE / 'xolotl-draft-cumulative-v49-cuda-header-latest.json').read_text())
if header['phase'] != 'staged' or header['cuda_code_executed']:
    raise RuntimeError('exact host CUDA header closure required')
root = cpu['root']
sources = {'generator_cpu_build_worker.py': (BASE / 'draft-cumulative-v49/generator_cpu_build_worker.py').read_bytes(),
           'aster_prefill_policy.hpp': (BASE / 'ttft-v15/aster_prefill_policy.hpp').read_bytes()}
policy = json.loads((BASE / 'draft-cumulative-v49/source-manifest.json').read_text())
plan = dict(compile_command=parent['plan']['compile_command'],
            original_source=parent['object']['path'].rsplit('/', 1)[0] + '/ds41_generate.cpp',
            original_source_sha256=parent['source_sha256'], original_object_sha256=parent['object']['sha256'],
            inputs=[dict(path=root + '/' + name, bytes=len(raw), sha256=hashlib.sha256(raw).hexdigest())
                    for name, raw in sources.items()])
for name in ['ds41_generate.cpp', 'aster_draft_cumulative.hpp']:
    raw = (BASE / 'draft-cumulative-v49' / name).read_bytes()
    plan['inputs'].append(dict(path=root + '/' + name, bytes=len(raw), sha256=hashlib.sha256(raw).hexdigest()))
if plan['inputs'][-2]['sha256'] != policy['patched_generator_sha256']:
    raise RuntimeError('exact patched generator differs')
sources['generator-build-plan.json'] = (json.dumps(plan, indent=2) + '\n').encode()
record = dict(unix=time.time(), phase='prepared', root=root, gpu_executed=False,
              files=[dict(file=name, bytes=len(raw), sha256=hashlib.sha256(raw).hexdigest()) for name, raw in sources.items()])
code = '''import base64,hashlib,json,os,subprocess,time
from pathlib import Path
c=@C@;root=Path(c['root']);data=@DATA@
if root.resolve()!=root or root.stat().st_uid!=10250 or os.getuid()!=10250 or (root/'generator-build-plan.json').exists():raise RuntimeError('exclusive CPU generator build differs')
os.umask(0o077)
for row in c['files']:
 raw=base64.b64decode(data[row['file']])
 if len(raw)!=row['bytes'] or hashlib.sha256(raw).hexdigest()!=row['sha256']:raise RuntimeError('frozen CPU compiler input differs')
 with (root/row['file']).open('xb') as f:f.write(raw)
with (root/'generator-cpu-worker.log').open('xb') as log:
 p=subprocess.Popen(['timeout','--signal=TERM','--kill-after=15','220','python3','-u',str(root/'generator_cpu_build_worker.py')],cwd=root,stdin=subprocess.DEVNULL,stdout=log,stderr=log,start_new_session=True)
process=dict(pid=p.pid,start_ticks=int(Path('/proc/'+str(p.pid)+'/stat').read_text().split(') ',1)[1].split()[19]),uid=os.getuid(),unix=time.time(),budget_seconds=220)
(root/'generator-cpu-process.json').write_text(json.dumps(process,indent=2)+'\\n');print(json.dumps(process))
'''.replace('@C@', repr(record)).replace('@DATA@', repr({name: base64.b64encode(raw).decode() for name, raw in sources.items()}))
compile(code, 'generator-cpu-build-v49', 'exec')
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
print(json.dumps(dict(phase=record['phase'], process=record['process'], gpu_executed=False)))
