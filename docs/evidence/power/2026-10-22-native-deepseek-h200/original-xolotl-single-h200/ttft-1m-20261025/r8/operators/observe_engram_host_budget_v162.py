"""Read exact native RSS/cgroup limits before proposing a larger bounded Engram cache."""
import json
from xolotl_trial_common import BASE, observe, owned

ip=BASE/'xolotl-engram-host-budget-v162.json'
if ip.exists():raise RuntimeError('exact host budget observation already frozen')
r=json.loads((BASE/'xolotl-phase-simd-v139-control_b-intent.json').read_text())
o=observe()
if o['job_uid']!=r['allocation_uid']:raise RuntimeError('exact current C10 resource differs')
code='''import json,time
from pathlib import Path
r=@R@;root=Path(r['root']);q=json.loads((root/'service-longctx-v1-result.json').read_text());api=q['api_process']
p=Path('/proc/'+str(api['pid']))
if p.stat().st_uid!=10250 or int((p/'stat').read_text().split(') ',1)[1].split()[19])!=api['start_ticks']:raise RuntimeError('exact API PID identity differs')
children=(p/'task'/str(api['pid'])/'children').read_text().split();native=[]
for child in children:
 p=Path('/proc/'+child)
 if p.stat().st_uid==10250 and str((p/'exe').resolve())==r['plan']['binary']:
  stat=(p/'stat').read_text().split(') ',1)[1].split()
  row=dict(pid=int(child),start_ticks=int(stat[19]),namespace='pod',uid=10250)
  status=(p/'status').read_text()
  row['bounded_status']=[line for line in status.splitlines() if line.startswith(('VmPeak:','VmSize:','VmRSS:','RssAnon:','RssFile:','VmSwap:','Threads:'))]
  row['cgroup']=(p/'cgroup').read_text()
  native.append(row)
if len(native)!=1:raise RuntimeError('exact current native child is not uniquely bound')
out=dict(unix=time.time(),native=native[0],model_requests_sent=0,cache_policy_changed=False)
for path in ['/sys/fs/cgroup/memory.max','/sys/fs/cgroup/memory.current','/sys/fs/cgroup/memory.events','/sys/fs/cgroup/memory/memory.limit_in_bytes','/sys/fs/cgroup/memory/memory.usage_in_bytes','/sys/fs/cgroup/memory/memory.failcnt']:
 p=Path(path)
 if p.exists():
  text=p.read_text()
  if len(text)>4096:raise RuntimeError('bounded cgroup numeric observation exceeded')
  out[path]=text.strip()
out['cache_rows_current']=r['plan']['engine_env']['STRATA_ENGRAM_CACHE_ROWS']
out['actual_weight_row_bytes_observed_previously']=256;out['actual_scale_row_bytes_observed_previously']=8
print(json.dumps(out))
'''.replace('@R@',repr(r))
result=owned(code)
with ip.open('x') as output:output.write(json.dumps(result,indent=2)+'\n')
print(json.dumps(result,indent=2))
