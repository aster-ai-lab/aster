"""Read installed capture/export/stdin options; never initialize a profiling target."""
import json
from xolotl_trial_common import BASE, observe, owned
ip=BASE/'xolotl-ncu-capture-cli-v170.json'
if ip.exists():raise RuntimeError('exact profiler CLI documentation already frozen')
o=observe()
if o['job_uid']!='7455d989-8255-4f7c-ae8f-3cc0aa5928e4':raise RuntimeError('exact optimization allocation differs')
code='''import hashlib,json,subprocess,time
q=subprocess.run(['/usr/local/bin/ncu','--help'],capture_output=True,text=True,check=True,timeout=8)
raw=(q.stdout+q.stderr).encode()
if len(raw)>262144:raise RuntimeError('bounded installed profiler help exceeded')
lines=raw.decode().splitlines();selected=[]
for index,line in enumerate(lines):
 if any(value in line for value in ['--profile-from-start','--log-file','--export','--null-stdin','--query-metrics','--csv','--page','--print-units','--print-fp','--filter-mode','--kill ']):selected.extend(lines[index:index+7])
print(json.dumps(dict(unix=time.time(),gpu_profiled=False,hardware_counters_collected=False,selected_help=selected,full_help_sha256=hashlib.sha256(raw).hexdigest())))
'''
result=owned(code)
with ip.open('x') as output:output.write(json.dumps(result,indent=2)+'\n')
print(json.dumps(result,indent=2))
