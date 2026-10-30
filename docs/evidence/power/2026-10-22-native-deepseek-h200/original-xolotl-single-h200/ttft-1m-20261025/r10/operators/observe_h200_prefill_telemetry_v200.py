"""Observe the current exact optimization GPU without starting inference or changing policy."""
import json
import time
from xolotl_trial_common import BASE, owned

intent = json.loads((BASE / 'xolotl-ncu-actual1m-v172r2-intent.json').read_text())
code = '''import csv,io,json,subprocess,time
from pathlib import Path
root=Path(@ROOT@)
if root.resolve()!=root or root.stat().st_uid!=10250:raise RuntimeError('exact active profile root differs')
fields=['uuid','utilization.gpu','utilization.memory','memory.used','memory.total','power.draw','power.limit','temperature.gpu','clocks.current.sm','clocks.current.memory','clocks.max.sm']
samples=[]
for ordinal in range(3):
 raw=subprocess.run(['nvidia-smi','--query-gpu='+','.join(fields),'--format=csv,noheader,nounits'],capture_output=True,text=True,check=True,timeout=8).stdout
 if len(raw)>16384:raise RuntimeError('bounded GPU telemetry exceeded')
 rows=list(csv.reader(io.StringIO(raw)))
 if len(rows)!=1 or len(rows[0])!=len(fields) or rows[0][0].strip()!='GPU-099e8da4-8df0-c431-9e67-eb721e59beee':raise RuntimeError('one exact independent optimization H200 required')
 sample=dict(unix=time.time(),**{key:value.strip() for key,value in zip(fields,rows[0])})
 result=root/'result.json'
 if result.exists():
  if result.is_symlink() or result.stat().st_size>2*1024**2:raise RuntimeError('bounded active profile result exceeded')
  current=json.loads(result.read_text());sample['profile_state']={key:current.get(key) for key in ['phase','current_request','current_state']}
 samples.append(sample)
 if ordinal<2:time.sleep(2)
print(json.dumps(dict(samples=samples,GPU_work_started=False,GPU_policy_changed=False,inference_sent=False,interpretation='Three instantaneous cold-prefill observations; not sustained utilization, Tensor Core counters, or an API speed result.')))
'''.replace('@ROOT@', repr(intent['root']))
result = owned(code)
path = BASE / ('xolotl-h200-prefill-telemetry-v200-' + str(time.time_ns()) + '.json')
with path.open('x') as output:
    output.write(json.dumps(result, indent=2) + '\n')
print(json.dumps(result, indent=2))
