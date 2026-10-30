"""Reparse existing successful tiny-kernel CSV; preserve the failed old parser receipt."""
import base64
import hashlib
import json
import time
from xolotl_trial_common import BASE, jump, lease, owned

path=BASE/'xolotl-ncu-smoke-recovery-v207.json'
if path.exists():raise RuntimeError('exact CSV recovery already recorded')
prior=json.loads((BASE/'xolotl-ncu-smoke-v205-latest.json').read_text())
q=prior['result.json']
if prior['alive'] or q['phase']!='failed' or q['ncu_returncode']!=0 or q['error']!='ValueError: profiler numeric CSV header/data absent' or not q['engine_reaped'] or not q['gpu_idle_after']:
    raise RuntimeError('exact successful profiler/failed old-format parser must be retained')
intent=json.loads((BASE/'xolotl-ncu-smoke-v205-intent.json').read_text())
root=lease()['remote']+'/ncu-smoke-v207-recovery'
files={name:(BASE/name).read_bytes() for name in ['aster_ncu_metrics_v171.py','aster_ncu_wide_metrics_v206.py','test_ncu_wide_metrics_v206.py']}
record=dict(root=root,source_root=intent['root'],phase='staging',GPU_work_started=False,model_forward_executed=False,
    files=[dict(file=name,bytes=len(raw),sha256=hashlib.sha256(raw).hexdigest()) for name,raw in files.items()])
code='''import base64,json,os
from pathlib import Path
root=Path(@ROOT@);data=@DATA@
if root.exists() or root.parent.resolve()!=root.parent or root.parent.stat().st_uid!=10250 or os.getuid()!=10250:raise RuntimeError('exclusive exact parser recovery stage differs')
os.umask(0o077);root.mkdir(mode=0o700)
for name,encoded in data.items():
 with (root/name).open('xb') as output:output.write(base64.b64decode(encoded))
print(json.dumps(dict(staged=True)))
'''.replace('@ROOT@',repr(root)).replace('@DATA@',repr({name:base64.b64encode(raw).decode() for name,raw in files.items()}))
jump(code)
code='''import hashlib,json,os,subprocess,sys,time
from pathlib import Path
r=@R@;root=Path(r['root']);source=Path(r['source_root'])
if root.resolve()!=root or root.stat().st_uid!=10250 or source.resolve()!=source or source.stat().st_uid!=10250:raise RuntimeError('exact parser recovery roots differ')
for row in r['files']:
 p=root/row['file']
 if p.is_symlink() or p.stat().st_size!=row['bytes'] or hashlib.sha256(p.read_bytes()).hexdigest()!=row['sha256']:raise RuntimeError('exact revised parser bytes differ')
env={k:os.environ[k] for k in ['PATH','LD_LIBRARY_PATH'] if k in os.environ};env['CUDA_VISIBLE_DEVICES']=''
test=subprocess.run(['python3','-B','-m','unittest','-v','test_ncu_wide_metrics_v206.py'],cwd=root,env=env,capture_output=True,text=True,timeout=15)
if test.returncode or len(test.stderr)>32768:raise RuntimeError('observed wide-CSV parser tests failed')
(root/'parser-tests.log').write_text(test.stderr)
sys.path.insert(0,str(root));from aster_ncu_wide_metrics_v206 import numeric_wide_rows
p=source/'private-ncu.csv'
if p.is_symlink() or p.stat().st_size>2*1024**2:raise RuntimeError('bounded existing successful profiler CSV exceeded')
raw=p.read_bytes();rows=numeric_wide_rows(raw.decode());prior=json.loads((source/'result.json').read_text());out=json.loads((source/'smoke.stdout.log').read_text())
if prior['phase']!='failed' or prior['ncu_returncode']!=0 or not prior['engine_reaped'] or not prior['gpu_idle_after'] or len(rows)!=8 or len({row['kernel_id'] for row in rows})!=1 or {row['symbol'] for row in rows}!={'aster_h200_counter_smoke(unsigned int *, int)'} or out['phase']!='passed' or out['checked_output_words']!=65536:raise RuntimeError('exact existing tiny-kernel proof differs')
memory={}
for name in ['memory.max','memory.current','memory.peak','memory.events','memory.swap.current','memory/memory.limit_in_bytes','memory/memory.usage_in_bytes','memory/memory.max_usage_in_bytes','memory/memory.failcnt','memory/memory.oom_control']:
 p=Path('/sys/fs/cgroup')/name
 if p.exists():
  raw_memory=p.read_bytes()
  if len(raw_memory)>4096:raise RuntimeError('bounded own-cgroup memory observation exceeded')
  memory[name]=raw_memory.decode()
result=dict(unix=time.time(),phase='passed',existing_CSV_reparsed=True,new_GPU_work_started=False,prior_parser_phase='failed',prior_parser_receipt_unchanged=True,hardware_counters_collected=True,model_hardware_counters_collected=False,numeric_metrics=rows,raw_CSV_bytes=len(raw),raw_CSV_sha256=hashlib.sha256(raw).hexdigest(),raw_CSV_published=False,parser_tests_passed=7,output=out,own_cgroup_memory=memory,model_profile_failure_cause_proven=False)
(root/'result.json').write_text(json.dumps(result,indent=2)+'\\n');print(json.dumps(result))
'''.replace('@R@',repr(record))
record.update(phase='complete',result=owned(code),unix=time.time())
with path.open('x') as output:output.write(json.dumps(record,indent=2)+'\n')
print(json.dumps(record['result'],indent=2))
