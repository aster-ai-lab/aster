"""Launch actual full-window cache-hit load only after the cold full-window test passes."""
import base64, hashlib, json, time
from xolotl_trial_common import BASE, observe, jump

ip = BASE / 'xolotl-fullwindow-reuse-load-v20-intent.json'
if ip.exists():
    raise RuntimeError('full-window load already has an intent; inspect its exact client')
r = json.loads((BASE / 'xolotl-ttft-shadow-v15-intent.json').read_text())
full = json.loads((BASE / 'xolotl-full-1m-qualification-v17-intent.json').read_text())
reservation = json.loads((BASE / 'xolotl-active-serving-reservation.json').read_text())
observation = observe()
if observation['job_uid'] != r['allocation_uid'] or observation['phase'] != 'Running' or r['allocation_uid'] == reservation['allocation_uid']:
    raise RuntimeError('actual full-window load requires the isolated optimization service')
root = r['root'] + '/qualification-fullwindow-reuse-v20'
plan = dict(base_url='http://' + r['pod_ip'] + ':30003/v1', service_root=r['root'], full_qualification_root=full['root'])
files = {'request-worker.py': (BASE / 'xolotl_fullwindow_reuse_load_worker_v20.frozen.py').read_bytes(),
         'request-plan.json': (json.dumps(plan, indent=2) + '\n').encode()}
record = dict(unix=time.time(), phase='prepared', root=root, allocation_uid=r['allocation_uid'], public_fixture_only=True,
              files=[dict(file=name, bytes=len(raw), sha256=hashlib.sha256(raw).hexdigest()) for name, raw in files.items()])
code = '''import base64,hashlib,json,os,subprocess,time
from pathlib import Path
r=@R@;service=@SERVICE@;prior=@PRIOR@;root=Path(r['root']);full=Path(prior['root'])
result=json.loads((full/'request-result.json').read_text());p=prior['process'];stat=Path('/proc/'+str(p['pid'])+'/stat')
alive=stat.exists() and stat.stat().st_uid==10250 and int(stat.read_text().split(') ',1)[1].split()[19])==p['start_ticks']
if result['phase']!='passed' or not result.get('full1m_cache_restore_passed') or alive:raise RuntimeError('full-window cold/retrieval/restore must finish and client must be gone')
q=json.loads((Path(service['root'])/'service-longctx-v1-result.json').read_text())
if q['phase']!='serving' or q['gpu_uuid']!=service['gpu_uuid'] or q['binary_sha256']!=service['plan']['binary_sha256']:raise RuntimeError('exact qualified V15 service differs')
if time.time()+1200>q['lease_ends_unix']:raise RuntimeError('full-window load lease deadline insufficient')
if root.parent.resolve()!=root.parent or root.parent.stat().st_uid!=10250 or root.exists():raise RuntimeError('exclusive public full-window load root differs')
root.mkdir(mode=0o700);data=@DATA@
for row in r['files']:
 raw=base64.b64decode(data[row['file']])
 if len(raw)!=row['bytes'] or hashlib.sha256(raw).hexdigest()!=row['sha256']:raise RuntimeError('full-window load frozen input differs')
 with (root/row['file']).open('xb') as f:f.write(raw)
with (root/'request-worker.log').open('xb') as log:
 child=subprocess.Popen(['timeout','--signal=TERM','--kill-after=15','1100','python3','-u',str(root/'request-worker.py')],cwd=root,stdin=subprocess.DEVNULL,stdout=log,stderr=log,start_new_session=True)
process=dict(pid=child.pid,start_ticks=int(Path('/proc/'+str(child.pid)+'/stat').read_text().split(') ',1)[1].split()[19]),uid=os.getuid(),unix=time.time(),budget_seconds=1100)
(root/'request-process.json').write_text(json.dumps(process,indent=2)+'\\n');print(json.dumps(process))
'''.replace('@R@', repr(record)).replace('@SERVICE@', repr(r)).replace('@PRIOR@', repr(full)).replace('@DATA@', repr({name: base64.b64encode(raw).decode() for name, raw in files.items()}))
compile(code, 'actual-full-window-reuse-load-bootstrap', 'exec')
record['phase'] = 'launch_requested'
with ip.open('x') as stream:
    stream.write(json.dumps(record, indent=2) + '\n')
try:
    record['process'] = jump(code)
    record['phase'] = 'launched'
except BaseException:
    record['phase'] = 'launch_outcome_requires_inspection'
    ip.write_text(json.dumps(record, indent=2) + '\n')
    raise
ip.write_text(json.dumps(record, indent=2) + '\n')
print(json.dumps(dict(phase=record['phase'], root=root, actual_prompt_tokens=1048504, scope='cache-hit concurrency1/2, max64, fixed public JSON retrieval')))
