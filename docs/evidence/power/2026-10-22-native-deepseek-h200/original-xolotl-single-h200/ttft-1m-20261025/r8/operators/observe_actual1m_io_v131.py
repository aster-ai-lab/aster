"""Read-only sampling of the exact running million-token native process.

No inference, attachment, signal, file payload read, or service mutation is used.
GPU host PIDs are deliberately not interpreted as pod namespace PIDs.
"""
import json
import time
from xolotl_trial_common import BASE, observe, owned

service = json.loads((BASE / 'xolotl-code-service-v118-full1m_code-intent.json').read_text())
previous = json.loads((BASE / 'xolotl-actual1m-native-readonly-progress-v130.json').read_text())
targets = previous['native_pod_namespace']
if len(targets) != 1:
    raise RuntimeError('exact prior native process is required')
target = dict(pid=targets[0]['pid'], start_ticks=int(targets[0]['stat'][19]),
              executable=service['plan']['binary'])
proof = observe()
if proof['job_uid'] != service['allocation_uid'] or proof['phase'] != 'Running':
    raise RuntimeError('exact running independent allocation differs')

code = '''import json,os,subprocess,time
from pathlib import Path
c=@C@;root=Path(@ROOT@);base=Path('/proc')/str(c['pid']);samples=[]
if root.resolve()!=root or root.stat().st_uid!=10250:raise RuntimeError('exact service owner differs')
def safe_text(p,maximum=65536):
 try:
  with p.open('rb') as f:raw=f.read(maximum+1)
  if len(raw)>maximum:raise RuntimeError('read-only proc bound exceeded')
  return raw.decode('utf-8','replace')
 except (PermissionError,ProcessLookupError,FileNotFoundError) as e:return type(e).__name__+': '+str(e)
def sample():
 if base.stat().st_uid!=10250 or os.readlink(base/'exe')!=c['executable']:raise RuntimeError('exact native owner/exe differs')
 stat=(base/'stat').read_text().split(') ',1)[1].split()
 if int(stat[19])!=c['start_ticks']:raise RuntimeError('exact native start time differs')
 threads=[]
 for t in (base/'task').iterdir():
  try:
   fields=(t/'stat').read_text().split(') ',1)[1].split()
   if fields[0] in ['D','R']:
    threads.append(dict(tid=int(t.name),state=fields[0],utime=int(fields[11]),stime=int(fields[12]),wchan=safe_text(t/'wchan',4096),syscall=safe_text(t/'syscall',4096),stack=safe_text(t/'stack',8192)))
  except (FileNotFoundError,ProcessLookupError):pass
 fdlinks=[]
 for f in (base/'fd').iterdir():
  try:
   path=os.readlink(f)
   if path.startswith('/mnt/shared-storage-user/') or path.startswith('/var/tmp/'):
    fdlinks.append(dict(fd=int(f.name),path=path,fdinfo=safe_text(base/'fdinfo'/f.name,4096)))
  except (FileNotFoundError,ProcessLookupError):pass
 log=root/'engine.log'
 return dict(unix=time.time(),state=stat[0],utime=int(stat[11]),stime=int(stat[12]),minor_faults=int(stat[7]),major_faults=int(stat[9]),wchan=safe_text(base/'wchan',4096),io=safe_text(base/'io',8192),active_threads=threads,open_files=fdlinks,log_bytes=log.stat().st_size,log_mtime=log.stat().st_mtime)
for i in range(3):
 samples.append(sample())
 if i<2:time.sleep(4)
gpu=subprocess.run(['nvidia-smi','--query-gpu=uuid,utilization.gpu,memory.used,power.draw','--format=csv,noheader,nounits'],capture_output=True,text=True,timeout=8,check=True).stdout.strip()
print(json.dumps(dict(unix=time.time(),native=c,samples=samples,gpu=gpu,no_inference=True,no_attach_or_signal=True)))
'''.replace('@C@', repr(target)).replace('@ROOT@', repr(service['root']))

result = owned(code)
result['previous_observation_unix'] = previous['unix']
before = targets[0]['stat']
after = result['samples'][-1]
result['delta_from_previous'] = dict(seconds=after['unix'] - previous['unix'],
    utime_ticks=after['utime'] - int(before[11]), stime_ticks=after['stime'] - int(before[12]),
    minor_faults=after['minor_faults'] - int(before[7]), major_faults=after['major_faults'] - int(before[9]))
path = BASE / 'xolotl-actual1m-io-v131-latest.json'
temporary = path.with_suffix('.tmp')
temporary.write_text(json.dumps(result, indent=2) + '\n')
temporary.replace(path)
print(json.dumps(dict(native=target, delta=result['delta_from_previous'], gpu=result['gpu'],
    samples=[{k: r[k] for k in ['unix','state','utime','stime','major_faults','wchan','log_bytes','active_threads']} for r in result['samples']]), indent=2))
