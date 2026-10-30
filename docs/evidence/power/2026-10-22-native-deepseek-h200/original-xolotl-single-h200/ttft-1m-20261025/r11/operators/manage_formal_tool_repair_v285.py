"""Exact-owned formal hot replacement, with separate drain/launch receipts and no extra GPU."""
import argparse,json,time
from xolotl_recovery_common_v217 import BASE,observe,owned

p=argparse.ArgumentParser();p.add_argument('action',choices=['drain','observe_old','launch','observe_new']);a=p.parse_args()
r=json.loads((BASE/'xolotl-formal-tool-repair-v284-intent.json').read_text())
old=json.loads((BASE/'xolotl-stable-service-v279-intent.json').read_text())
o=observe()
if o['job_uid']!=r['allocation_uid'] or o['phase']!='Running' or r['phase']!='cpu_qualified':raise RuntimeError('exact owned CPU-qualified formal replacement required')
base='''import hashlib,json,os,signal,subprocess,time,urllib.request
from pathlib import Path
r=@R@;old=@OLD@;root=Path(r['root']);src=Path(r['old_root'])
if root.resolve()!=root or src.resolve()!=src or root.stat().st_uid!=10250:raise RuntimeError('exact owned formal stage differs')
def exact(handle):
 p=Path('/proc/'+str(handle['pid']))
 try:
  fields=(p/'stat').read_text().split(') ',1)[1].split()
  return p.stat().st_uid==10250 and int(fields[19])==handle['start_ticks'] and fields[0]!='Z'
 except FileNotFoundError:return False
def stop_exact(handle):
 if not exact(handle):raise RuntimeError('exact formal supervisor identity changed before planned drain')
 fd=os.pidfd_open(handle['pid'])
 try:
  if not exact(handle):raise RuntimeError('exact formal supervisor changed after pidfd open')
  signal.pidfd_send_signal(fd,signal.SIGTERM)
 finally:os.close(fd)
def gpu_apps():
 q=subprocess.run(['nvidia-smi','--query-compute-apps=pid','--format=csv,noheader,nounits'],capture_output=True,text=True,timeout=5,check=True)
 return bool(q.stdout.strip())
'''.replace('@R@',repr(r)).replace('@OLD@',repr(old))
receipt=BASE/('xolotl-formal-tool-repair-v285-'+a.action+'.json')
if a.action in ['drain','launch'] and receipt.exists():raise RuntimeError('mutation already has an exact receipt; observe instead')
if a.action=='drain':
 code=base+'''
pre=json.loads((root/'cpu-preflight.json').read_text())
if pre['phase']!='cpu_qualified':raise RuntimeError('CPU preflight incomplete')
key=(src/'credentials/api-key.txt').read_text().strip()
request=urllib.request.Request('http://127.0.0.1:30003/slots',headers={'Authorization':'Bearer '+key})
with urllib.request.urlopen(request,timeout=4) as response:slots=json.load(response)
if not slots or any(s.get('is_processing') for s in slots):raise RuntimeError('formal requests are active; do not interrupt them')
h=json.loads((src/'service-process.json').read_text());state=json.loads((src/'service-longctx-v1-result.json').read_text())
if state['phase']!='serving' or state['gpu_uuid']!=r['gpu_uuid'] or state['binary_sha256']!=r['native_sha256']:raise RuntimeError('old formal identity differs')
if h['pid']!=2850 or h['start_ticks']!=735215129:raise RuntimeError('known formal supervisor changed')
stop_exact(h)
print(json.dumps(dict(unix=time.time(),phase='drain_signal_sent',supervisor=h,active_requests_before=0,signal='SIGTERM',pod_namespace=True,extra_GPU_requested=False)))
'''
elif a.action=='observe_old':
 code=base+'''
h=json.loads((src/'service-process.json').read_text());s=json.loads((src/'service-longctx-v1-result.json').read_text())
print(json.dumps(dict(unix=time.time(),supervisor_alive=exact(h),phase=s['phase'],api_reaped=s.get('api_reaped'),gpu_workload_present=gpu_apps(),error=s.get('error'))))
'''
elif a.action=='launch':
 drain=json.loads((BASE/'xolotl-formal-tool-repair-v285-drain.json').read_text())
 if drain.get('phase')!='drain_signal_sent':raise RuntimeError('planned exact drain not recorded')
 code=base+'''
h=json.loads((src/'service-process.json').read_text());s=json.loads((src/'service-longctx-v1-result.json').read_text())
if exact(h) or s['phase']!='stopped' or not s.get('api_reaped') or gpu_apps():raise RuntimeError('old formal worker and native GPU workload must be gone before replacement')
if (root/'service-process.json').exists():raise RuntimeError('replacement already has an exact handle')
for row in r['files']:
 p=root/row['file']
 if p.is_symlink() or p.stat().st_size!=row['bytes'] or hashlib.sha256(p.read_bytes()).hexdigest()!=row['sha256']:raise RuntimeError('CPU-qualified replacement bytes changed')
deadline=int(r['lease_ends_unix']-time.time()-10)
if deadline<1800:raise RuntimeError('remaining formal lease too short')
with (root/'service-worker.log').open('xb') as log:
 child=subprocess.Popen(['timeout','--signal=TERM','--kill-after=30',str(deadline),'python3','-u',str(root/'service-worker.py')],cwd=root,stdin=subprocess.DEVNULL,stdout=log,stderr=log,start_new_session=True)
handle=dict(pid=child.pid,start_ticks=int(Path('/proc/'+str(child.pid)+'/stat').read_text().split(') ',1)[1].split()[19]),uid=os.getuid(),unix=time.time(),deadline_seconds=deadline)
with (root/'service-process.json').open('x') as f:f.write(json.dumps(handle,indent=2)+'\\n')
print(json.dumps(dict(unix=time.time(),phase='launched',process=handle,extra_GPU_requested=False,same_native_binary=True,same_vip=True,same_existing_key=True)))
'''
else:
 code=base+'''
out=dict(unix=time.time())
p=root/'service-process.json'
if p.exists():
 h=json.loads(p.read_text());out['exact_supervisor_alive']=exact(h);out['process']=h
p=root/'service-longctx-v1-result.json'
if p.exists():
 s=json.loads(p.read_text());out['state']={k:v for k,v in s.items() if k in ['phase','error','api_process','api_attempt','last_health_unix','context_tokens','binary_sha256','gpu_uuid','lease_ends_unix']}
if out.get('state',{}).get('phase')=='failed':
 p=root/'service-worker.log'
 if p.stat().st_size>65536:raise RuntimeError('bounded worker error exceeded')
 out['failure_log']=p.read_text()[-3000:]
print(json.dumps(out))
'''
compile(code,'exact-formal-tool-repair-v285-'+a.action,'exec')
if a.action in ['drain','launch']:
 with receipt.open('x') as f:f.write(json.dumps(dict(unix=time.time(),phase='mutation_requested',action=a.action,allocation_uid=r['allocation_uid']))+'\n')
result=owned(code)
receipt.write_text(json.dumps(result,indent=2)+'\n')
print(json.dumps(result,indent=2))
