"""Prepare exact actual1M-plus1024 public input on CPU while the GPU screen runs."""
import base64
import hashlib
import json
import time
from xolotl_trial_common import BASE, jump, lease

intent = BASE / 'xolotl-fullwindow-long-fixture-v54-intent.json'
if intent.exists():
    raise RuntimeError('full-window long fixture has an exact handle; observe it')
source = json.loads((BASE / 'xolotl-api-hybrid-v52-intent.json').read_text())
prior = json.loads((BASE / 'xolotl-api-hybrid-v52-latest.json').read_text())
if prior['cpu-preflight-result.json']['phase'] != 'passed' or prior['exact_cpu_process_alive']:
    raise RuntimeError('exact completed public tokenizer CPU source required')
root = lease()['remote'] + '/fullwindow-long-fixture-v54-cpu'
plan = dict(source_public_root=source['root'],
            api_sha256='f062d9d39131959da9c981b6704e2ec27f89d8e2806730062dcabc3eb27682ba')
files = {'prepare-plan.json': (json.dumps(plan, indent=2) + '\n').encode()}
for name in ['xolotl_fullwindow_long_fixture_v54.py', 'test_xolotl_fullwindow_long_fixture_v54.py',
             'xolotl_fullwindow_long_prepare_v54.frozen.py']:
    files[name] = (BASE / name).read_bytes()
record = dict(unix=time.time(), phase='prepared', root=root, source_public_root=source['root'],
              gpu_executed=False, credentials_copied=False,
              files=[dict(file=n, bytes=len(v), sha256=hashlib.sha256(v).hexdigest()) for n, v in files.items()])
code = '''import base64,hashlib,json,os,subprocess,time
from pathlib import Path
r=@R@;root=Path(r['root']);data=@DATA@
if os.getuid()!=10250 or root.parent.resolve()!=root.parent or root.parent.stat().st_uid!=10250 or root.exists():raise RuntimeError('exclusive CPU fixture root owner differs')
os.umask(0o077);root.mkdir(mode=0o700)
for row in r['files']:
 raw=base64.b64decode(data[row['file']])
 if len(raw)!=row['bytes'] or hashlib.sha256(raw).hexdigest()!=row['sha256']:raise RuntimeError('exact frozen CPU fixture input differs')
 with (root/row['file']).open('xb') as output:output.write(raw)
with (root/'prepare-worker.log').open('xb') as log:
 p=subprocess.Popen(['timeout','--signal=TERM','--kill-after=15','120','python3','-u',str(root/'xolotl_fullwindow_long_prepare_v54.frozen.py')],cwd=root,stdin=subprocess.DEVNULL,stdout=log,stderr=log,start_new_session=True)
process=dict(pid=p.pid,start_ticks=int(Path('/proc/'+str(p.pid)+'/stat').read_text().split(') ',1)[1].split()[19]),uid=os.getuid(),unix=time.time(),budget_seconds=120)
(root/'prepare-process.json').write_text(json.dumps(process,indent=2)+'\\n');print(json.dumps(process))
'''.replace('@R@', repr(record)).replace('@DATA@', repr({n: base64.b64encode(v).decode() for n, v in files.items()}))
compile(code, 'fullwindow-long-fixture-cpu-v54', 'exec')
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
