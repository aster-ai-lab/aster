"""Read installed Nsight Systems GPU-metric options; never start a profile here."""
import json
from xolotl_trial_common import BASE,owned
path=BASE/'xolotl-nsys-metrics-cli-v226.json'
if path.exists():raise RuntimeError('exact Nsight Systems CLI discovery already retained')
code='''import json,shutil,subprocess,time
binary=shutil.which('nsys')
if not binary:raise RuntimeError('installed Nsight Systems CLI absent')
out=dict(unix=time.time(),binary=binary,profiling_started=False)
for label,args in [('version',['--version']),('profile_help',['profile','--help']),('gpu_metric_devices',['profile','--gpu-metrics-device=help'])]:
 q=subprocess.run([binary,*args],capture_output=True,text=True,timeout=8)
 if len(q.stdout)>262144 or len(q.stderr)>65536:raise RuntimeError('bounded installed Nsight Systems CLI discovery exceeded')
 out[label]=dict(returncode=q.returncode,stdout=q.stdout,stderr=q.stderr)
print(json.dumps(out))
'''
result=owned(code)
with path.open('x') as out:out.write(json.dumps(result,indent=2)+'\n')
print(json.dumps({k:v for k,v in result.items() if k!='profile_help'},indent=2))
print(json.dumps(dict(metric_options=[line for line in result['profile_help']['stdout'].splitlines() if any(word in line.lower() for word in ['gpu-metrics','--trace=','--sample=','--cpuctxsw=','--duration='])]),indent=2))
