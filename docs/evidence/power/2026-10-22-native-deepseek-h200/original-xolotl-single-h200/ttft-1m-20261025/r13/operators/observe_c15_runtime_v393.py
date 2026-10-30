"""Read-only GPU and pod process evidence; never interpret host PIDs as pod PIDs."""
import json,time
from xolotl_trial_common import BASE,observe,owned
o=observe();c=json.loads((BASE/'xolotl-graph-native-model-v375-controller.json').read_text());r=json.loads((BASE/('xolotl-graph-native-v373-'+c['profile']+'-intent.json')).read_text())
if o['job_uid']!=r['allocation_uid'] or o['job_uid']!='5bc306b2-e9e1-4e25-bd05-d78eca2b4833':raise RuntimeError('exact active C15 role differs')
q=owned('''import json,os,subprocess,time
from pathlib import Path
r=@R@;root=Path(r['root']);s=json.loads((root/'service-longctx-v1-result.json').read_text());api=s['api_process'];p=Path('/proc/'+str(api['pid']));f=(p/'stat').read_text().split(') ',1)[1].split()
if s['phase']!='serving' or int(f[19])!=api['start_ticks'] or p.stat().st_uid!=10250:raise RuntimeError('exact active C15 API differs')
found=[]
for p in Path('/proc').iterdir():
 if not p.name.isdecimal():continue
 try:
  if p.stat().st_uid!=10250 or os.readlink(p/'exe')!=r['plan']['binary']:continue
  f=(p/'stat').read_text().split(') ',1)[1].split()
  if f[0]=='Z':continue
  found.append(dict(pid=int(p.name),start_ticks=int(f[19]),parent_pid=int(f[1]),threads=int(f[17]),namespace='C15 pod',resident_pages=int(f[21])))
 except (FileNotFoundError,PermissionError,ProcessLookupError):continue
if len(found)!=1 or found[0]['parent_pid']!=api['pid']:raise RuntimeError('unique active C15 native child differs')
gpu=subprocess.run(['nvidia-smi','--query-gpu=uuid,name,utilization.gpu,utilization.memory,memory.used,memory.total,power.draw,clocks.sm,clocks.mem','--format=csv,noheader,nounits'],capture_output=True,text=True,check=True,timeout=5).stdout.strip()
if not gpu.startswith(r['gpu_uuid']+','):raise RuntimeError('exact active physical GPU differs')
p=root/'engine.log'
if p.is_symlink() or p.stat().st_uid!=10250:raise RuntimeError('owned public numeric engine log differs')
with p.open('rb') as f:f.seek(max(0,p.stat().st_size-65536));tail=f.read(65536).decode(errors='replace')
prefixes=('serve: prompt phases: ','serve: prompt ','chunk prof','dense prof ','resident MMQ stage:')
lines=[line for line in tail.splitlines() if line.startswith(prefixes)][-6:]
print(json.dumps(dict(unix=time.time(),profile=r['profile'],native=found[0],API=api,GPU_csv_columns=['uuid','name','utilization.gpu percent','utilization.memory percent','memory.used MiB','memory.total MiB','power.draw W','clocks.sm MHz','clocks.mem MHz'],GPU_csv=gpu,last_public_numeric_diagnostics=lines,new_inference_sent=False,formal_changed=False)))
'''.replace('@R@',repr(r)))
name='xolotl-C15-runtime-v393-'+str(time.time_ns())+'.json'
with (BASE/name).open('x') as f:f.write(json.dumps(q,indent=2)+'\n')
print(json.dumps(q,indent=2))
