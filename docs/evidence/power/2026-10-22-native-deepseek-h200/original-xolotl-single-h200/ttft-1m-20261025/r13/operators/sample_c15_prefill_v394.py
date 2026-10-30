"""Bounded read-only same-pod CPU/I/O/GPU samples during the existing request."""
import json,time
from xolotl_trial_common import BASE,observe,owned
o=observe();r=json.loads((BASE/'xolotl-graph-native-v373-control_a-intent.json').read_text())
if o['job_uid']!=r['allocation_uid']:raise RuntimeError('exact active optimization role differs')
out=owned('''import json,subprocess,time
from pathlib import Path
r=@R@;root=Path(r['root']);api=json.loads((root/'service-longctx-v1-result.json').read_text())['api_process'];p=Path('/proc/437');samples=[]
for i in range(5):
 s=(p/'stat').read_text().split(') ',1)[1].split()
 if p.stat().st_uid!=10250 or int(s[19])!=563751141 or int(s[1])!=api['pid']:raise RuntimeError('exact C15 pod-native437/start563751141 differs')
 io={}
 for line in (p/'io').read_text().splitlines():
  k,v=line.split(':',1)
  if k in ['read_bytes','write_bytes','rchar','wchar']:io[k]=int(v)
 gpu=subprocess.run(['nvidia-smi','--query-gpu=uuid,utilization.gpu,utilization.memory,power.draw','--format=csv,noheader,nounits'],capture_output=True,text=True,check=True,timeout=5).stdout.strip()
 if not gpu.startswith(r['gpu_uuid']+','):raise RuntimeError('exact sampled GPU differs')
 samples.append(dict(unix=time.time(),state=s[0],utime_ticks=int(s[11]),stime_ticks=int(s[12]),io=io,GPU_csv=gpu))
 if i<4:time.sleep(1)
print(json.dumps(dict(samples=samples,new_inference_sent=False,model_interrupted=False,physical_GPU_counter_interpretation_claimed=False)))
'''.replace('@R@',repr(r)))
name='xolotl-C15-prefill-samples-v394-'+str(time.time_ns())+'.json'
with (BASE/name).open('x') as f:f.write(json.dumps(out,indent=2)+'\n')
print(json.dumps(out,indent=2))
