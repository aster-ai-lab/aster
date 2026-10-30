"""Launch or observe only the exact completed native trace's finite CPU-only aggregation."""
import argparse
import base64
import hashlib
import json
import time
from xolotl_trial_common import BASE, observe, owned

action=argparse.ArgumentParser();action.add_argument('action',choices=['launch','observe']);a=action.parse_args().action
intent=BASE/'xolotl-native-statistics-v123-intent.json'
r=json.loads((BASE/'xolotl-native-profile-v117-intent.json').read_text());o=observe()
if o['job_uid']!=r['allocation_uid']:raise RuntimeError('exact native statistics resource differs')
if a=='launch':
    if intent.exists():raise RuntimeError('statistics has an exact handle; observe it')
    raw=(BASE/'xolotl_profile_statistics_worker_v123.frozen.py').read_bytes()
    record=dict(unix=time.time(),phase='prepared',root=r['root'],allocation_uid=r['allocation_uid'],gpu_executed=False,
                bytes=len(raw),sha256=hashlib.sha256(raw).hexdigest(),raw_profiler_artifacts_published=False)
    code='''import base64,hashlib,json,os,subprocess,time
from pathlib import Path
r=@R@;root=Path(r['root']);raw=base64.b64decode(@DATA@)
if root.resolve()!=root or root.stat().st_uid!=10250 or (root/'statistics-process.json').exists():raise RuntimeError('exclusive exact statistics root differs')
q=json.loads((root/'result.json').read_text())
if q['phase']!='complete' or not q['engine_reaped'] or not q['native_target_gone']:raise RuntimeError('exact profiled native workload must finish/reap')
if len(raw)!=r['bytes'] or hashlib.sha256(raw).hexdigest()!=r['sha256']:raise RuntimeError('exact CPU statistics source differs')
with (root/'statistics-worker.py').open('xb') as output:output.write(raw)
with (root/'statistics-worker.log').open('xb') as log:
 p=subprocess.Popen(['timeout','--signal=TERM','--kill-after=15','360','python3','-u',str(root/'statistics-worker.py')],cwd=root,stdin=subprocess.DEVNULL,stdout=log,stderr=log,start_new_session=True,env=dict(os.environ,CUDA_VISIBLE_DEVICES=''))
process=dict(pid=p.pid,start_ticks=int(Path('/proc/'+str(p.pid)+'/stat').read_text().split(') ',1)[1].split()[19]),uid=os.getuid(),unix=time.time(),budget_seconds=360,namespace='optimization pod')
(root/'statistics-process.json').write_text(json.dumps(process,indent=2)+chr(10));print(json.dumps(process))
'''.replace('@R@',repr(record)).replace('@DATA@',repr(base64.b64encode(raw).decode()))
    compile(code,'current-native-statistics-stage-v123','exec')
    with intent.open('x') as output:output.write(json.dumps(record,indent=2)+'\n')
    record['phase']='launch_requested';intent.write_text(json.dumps(record,indent=2)+'\n')
    try:record['process']=owned(code);record['phase']='launched'
    except BaseException:
        record['phase']='launch_outcome_requires_inspection';intent.write_text(json.dumps(record,indent=2)+'\n');raise
    intent.write_text(json.dumps(record,indent=2)+'\n');print(json.dumps(dict(phase=record['phase'],process=record['process'],gpu_executed=False)))
else:
    record=json.loads(intent.read_text())
    code='''import json,time
from pathlib import Path
r=@R@;root=Path(r['root']);out={'unix':time.time()}
if root.resolve()!=root or root.stat().st_uid!=10250:raise RuntimeError('exact statistics root differs')
p=Path('/proc/'+str(r['process']['pid'])+'/stat')
out['exact_cpu_process_alive']=p.exists() and p.stat().st_uid==10250 and int(p.read_text().split(') ',1)[1].split()[19])==r['process']['start_ticks'] and p.read_text().split(') ',1)[1].split()[0]!='Z'
for name in ['statistics-result.json','numeric-summary.json']:
 p=root/name
 if p.exists():
  if p.is_symlink() or p.stat().st_size>262144:raise RuntimeError('bounded numeric statistics receipt differs')
  out[name]=json.loads(p.read_text())
if out.get('statistics-result.json',{}).get('phase')=='failed':
 for name in ['statistics-worker.log','statistics.stderr.log']:
  p=root/name
  if p.exists() and p.stat().st_size<65536:out[name]=p.read_text()[-4000:]
print(json.dumps(out))
'''.replace('@R@',repr(record))
    q=owned(code)
    key=(BASE/'access-private-xolotl-9c996c5a434b/api-key.txt').read_bytes().strip()
    if key in json.dumps(q).encode():raise RuntimeError('actual credential negative control failed')
    (BASE/'xolotl-native-statistics-v123-latest.json').write_text(json.dumps(q,indent=2)+'\n')
    summary=q.get('numeric-summary.json',{})
    print(json.dumps(dict(phase=q.get('statistics-result.json',{}).get('phase'),alive=q['exact_cpu_process_alive'],
        kernel_count=summary.get('kernel_count'),memcpy_count=summary.get('memcpy_count'),
        recorded_gpu_event_busy_union_fraction=summary.get('recorded_gpu_event_busy_union_fraction'),
        top_kernels=summary.get('top_kernels',[])[:6],top_runtime_calls=summary.get('top_runtime_calls',[])[:6],
        error=q.get('statistics-result.json',{}).get('error'),failure_log=q.get('statistics-worker.log')),indent=2))
