"""Inventory the installed profiler's CLI and permission setting without profiling any process."""
import json
from xolotl_trial_common import BASE, observe, owned

ip=BASE/'xolotl-ncu-inventory-v161.json'
if ip.exists():raise RuntimeError('installed profiler inventory already frozen')
o=observe()
if o['job_uid']!='7455d989-8255-4f7c-ae8f-3cc0aa5928e4':raise RuntimeError('exact optimization UID differs')
code='''import hashlib,json,subprocess,time
from pathlib import Path
out=dict(unix=time.time(),hardware_counters_collected=False,model_process_profiled=False,gpu_policy_changed=False)
for flag in ['--version','--help']:
 p=subprocess.run(['/usr/local/bin/ncu',flag],capture_output=True,text=True,check=True,timeout=8)
 raw=(p.stdout+p.stderr).encode()
 if len(raw)>262144:raise RuntimeError('bounded installed profiler documentation exceeded')
 if flag=='--version':out['version']=raw.decode()
 else:
  lines=raw.decode().splitlines();selected=[]
  for index,line in enumerate(lines):
   if any(value in line for value in ['--metrics ','--kernel-name ','--kernel-name-base ','--launch-count ','--launch-skip ','--replay-mode ','--cache-control ','--graph-profiling ','--target-processes ','--clock-control ']):selected.extend(lines[index:index+4])
  out['bounded_selected_help']=selected;out['full_help_sha256']=hashlib.sha256(raw).hexdigest()
p=Path('/proc/driver/nvidia/params')
if p.exists():
 if p.stat().st_size>65536:raise RuntimeError('bounded driver parameter text exceeded')
 text=p.read_text()
 if len(text)>65536:raise RuntimeError('bounded driver parameter text exceeded')
 out['profiling_access_setting']=[line for line in text.splitlines() if line.startswith('RmProfilingAdminOnly:')]
print(json.dumps(out))
'''
result=owned(code)
with ip.open('x') as output:output.write(json.dumps(result,indent=2)+'\n')
print(json.dumps(result,indent=2))
