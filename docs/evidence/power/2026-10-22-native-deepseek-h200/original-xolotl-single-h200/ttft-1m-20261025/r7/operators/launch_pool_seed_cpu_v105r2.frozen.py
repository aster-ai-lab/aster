"""Launch exactly one bounded CPU-only generator build on jump CPUs."""
import base64
import hashlib
import json
import time
from xolotl_trial_common import BASE, jump, lease

intent=BASE/'xolotl-pool-seed-cpu-v105r2-intent.json'
if intent.exists():raise RuntimeError('exact CPU build handle exists; inspect it')
old=json.loads((BASE/'xolotl-admission-seed-cpu-v94-intent.json').read_text())
source=BASE/'pool-seed-v104r2';manifest=json.loads((source/'source-manifest.json').read_text())
names=['ds41_generate.cpp','ds41_conv_reuse.hpp','aster_dspark_seed.hpp','aster_prefill_policy.hpp','aster_draft_cumulative.hpp','generator_cpu_worker_v105.py','test_pool_seed.cpp','ds41_conv_pool.cpp','ds41_conv_pool.hpp','ds41_conv_pool_dense.hpp','test_ds41_conv_pool.cpp']
files={name:(source/name).read_bytes() for name in names}
rows=[dict(file=name,bytes=len(raw),sha256=hashlib.sha256(raw).hexdigest()) for name,raw in files.items()]
parent=json.loads((BASE/'xolotl-admission-seed-v98-link-parent.json').read_text())
plan=dict(old_cpu_root=old['root'],original_source_sha256=hashlib.sha256((BASE/'admission-seed-v93/ds41_generate.cpp').read_bytes()).hexdigest(),original_engine_headers=parent['link_cwd'].removesuffix('/build')+'/source/tools/v41/engine',inputs=rows)
files['build-plan.json']=(json.dumps(plan,indent=2)+'\n').encode()
record=dict(unix=time.time(),phase='prepared',root=lease()['remote']+'/pool-seed-cpu-v105r2',gpu_executed=False,
            files=[dict(file=name,bytes=len(raw),sha256=hashlib.sha256(raw).hexdigest()) for name,raw in files.items()])
code='''import base64,hashlib,json,os,subprocess,time
from pathlib import Path
r=@R@;data=@DATA@;root=Path(r['root'])
if os.getuid()!=10250 or root.parent.resolve()!=root.parent or root.parent.stat().st_uid!=10250 or root.exists():raise RuntimeError('exclusive exact CPU compilation root differs')
os.umask(0o077);root.mkdir(mode=0o700)
for row in r['files']:
 raw=base64.b64decode(data[row['file']])
 if len(raw)!=row['bytes'] or hashlib.sha256(raw).hexdigest()!=row['sha256']:raise RuntimeError('frozen CPU compiler input differs')
 with (root/row['file']).open('xb') as output:output.write(raw)
with (root/'build-worker.log').open('xb') as log:
 p=subprocess.Popen(['timeout','--signal=TERM','--kill-after=15','480','python3','-u',str(root/'generator_cpu_worker_v105.py')],cwd=root,stdin=subprocess.DEVNULL,stdout=log,stderr=log,start_new_session=True)
process=dict(pid=p.pid,start_ticks=int(Path('/proc/'+str(p.pid)+'/stat').read_text().split(') ',1)[1].split()[19]),uid=os.getuid(),unix=time.time(),budget_seconds=480,namespace='jump host')
(root/'build-process.json').write_text(json.dumps(process,indent=2)+chr(10));print(json.dumps(process))
'''.replace('@R@',repr(record)).replace('@DATA@',repr({name:base64.b64encode(raw).decode() for name,raw in files.items()}))
compile(code,'exact-admission-seed-CPU-launch-v94','exec')
with intent.open('x') as output:output.write(json.dumps(record,indent=2)+'\n')
record['phase']='launch_requested';intent.write_text(json.dumps(record,indent=2)+'\n')
try:record.update(process=jump(code),phase='launched')
except BaseException:
    record['phase']='launch_outcome_requires_inspection';intent.write_text(json.dumps(record,indent=2)+'\n');raise
intent.write_text(json.dumps(record,indent=2)+'\n');print(json.dumps(dict(phase=record['phase'],process=record['process']),indent=2))
