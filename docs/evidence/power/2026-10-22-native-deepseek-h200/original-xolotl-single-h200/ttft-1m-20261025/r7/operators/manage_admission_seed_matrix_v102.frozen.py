"""Observe the exact matrix service, launch one counterfactual client, or drain a completed diagnostic safely."""
import argparse
import base64
import hashlib
import json
import time
from xolotl_trial_common import BASE, jump, observe, owned

p=argparse.ArgumentParser();p.add_argument('profile',choices=['off','on','off_repeat']);p.add_argument('action',choices=['observe','launch_client','observe_client','drain']);a=p.parse_args()
service=json.loads((BASE/('xolotl-admission-service-v101-'+a.profile+'-intent.json')).read_text())
resource=observe()
if resource['job_uid']!=service['allocation_uid']:raise RuntimeError('exact matrix allocation differs')
client_path=BASE/('xolotl-admission-client-v102-'+a.profile+'-intent.json')
if a.action=='observe':
    code='''import json,time
from pathlib import Path
r=@R@;root=Path(r['root']);out={'unix':time.time()}
if root.resolve()!=root or root.stat().st_uid!=10250:raise RuntimeError('exact matrix service directory differs')
for name in ['service-longctx-v1-result.json','service-process.json']:
 p=root/name
 if p.exists():
  if p.is_symlink() or p.stat().st_size>65536:raise RuntimeError('bounded model receipt exceeded')
  out[name]=json.loads(p.read_text())
p=Path('/proc/'+str(r['process']['pid'])+'/stat')
out['exact_supervisor_alive']=p.exists() and p.stat().st_uid==10250 and int(p.read_text().split(') ',1)[1].split()[19])==r['process']['start_ticks'] and p.read_text().split(') ',1)[1].split()[0]!='Z'
print(json.dumps(out))
'''.replace('@R@',repr(service))
    v=owned(code);(BASE/('xolotl-admission-service-v101-'+a.profile+'-latest.json')).write_text(json.dumps(v,indent=2)+'\n')
    q=v.get('service-longctx-v1-result.json',{});print(json.dumps(dict(phase=q.get('phase'),error=q.get('error'),api_reaped=q.get('api_reaped'),supervisor_alive=v['exact_supervisor_alive']),indent=2))
elif a.action=='launch_client':
    if client_path.exists():raise RuntimeError('exact diagnostic client handle exists; observe it')
    reference=json.loads((BASE/'xolotl-draft-extended-v53-off-intent.json').read_text())
    source=(BASE/'xolotl_draft_long_output_worker_v50.frozen.py').read_bytes()
    plan=dict(profile=a.profile,service_root=service['root'],base_url='http://'+service['pod_ip']+':30003/v1',reference_root=reference['root'],request_source_sha256=hashlib.sha256(source).hexdigest())
    files={'probe-worker.py':(BASE/'xolotl_isolation_worker_v90.frozen.py').read_bytes(),'qualified-request-source.py':source,'probe-plan.json':(json.dumps(plan,indent=2)+'\n').encode()}
    record=dict(unix=time.time(),phase='prepared',profile=a.profile,root=service['root']+'/qualification-admission-v102',allocation_uid=service['allocation_uid'],gpu_uuid=service['gpu_uuid'],
        formal_service_unchanged=True,diagnostic_only=True,files=[dict(file=name,bytes=len(raw),sha256=hashlib.sha256(raw).hexdigest()) for name,raw in files.items()])
    code='''import base64,hashlib,json,os,subprocess,time,urllib.request
from pathlib import Path
r=@R@;ref=@REF@;data=@DATA@;root=Path(r['root']);service=root.parent
if os.getuid()!=10250 or service.resolve()!=service or service.stat().st_uid!=10250 or root.exists():raise RuntimeError('exclusive exact matrix client owner differs')
q=json.loads((service/'service-longctx-v1-result.json').read_text())
if q['phase']!='serving' or q['gpu_uuid']!=r['gpu_uuid'] or q['context_tokens']!=1048576 or time.time()+540>q['lease_ends_unix']:raise RuntimeError('exact matrix model/finite lease differs')
p=Path('/proc/'+str(ref['process']['pid'])+'/stat')
alive=p.exists() and p.stat().st_uid==10250 and int(p.read_text().split(') ',1)[1].split()[19])==ref['process']['start_ticks'] and p.read_text().split(') ',1)[1].split()[0]!='Z'
if alive or json.loads((Path(ref['root'])/'request-result.json').read_text())['phase']!='passed':raise RuntimeError('exact original qualified reference must be terminal and passed')
key=(service/'credentials/api-key.txt').read_text().strip();req=urllib.request.Request(@SLOTS@,headers={'Authorization':'Bearer '+key})
with urllib.request.build_opener(urllib.request.ProxyHandler({})).open(req,timeout=5) as response:slots=json.load(response)
if not isinstance(slots,list) or not slots or any(row['is_processing'] for row in slots):raise RuntimeError('matrix slots must be idle before one diagnostic client')
os.umask(0o077);root.mkdir(mode=0o700)
for row in r['files']:
 raw=base64.b64decode(data[row['file']])
 if len(raw)!=row['bytes'] or hashlib.sha256(raw).hexdigest()!=row['sha256']:raise RuntimeError('exact frozen matrix input differs')
 with (root/row['file']).open('xb') as output:output.write(raw)
with (root/'probe-worker.log').open('xb') as log:
 p=subprocess.Popen(['timeout','--signal=TERM','--kill-after=15','450','python3','-u',str(root/'probe-worker.py')],cwd=root,stdin=subprocess.DEVNULL,stdout=log,stderr=log,start_new_session=True)
process=dict(pid=p.pid,start_ticks=int(Path('/proc/'+str(p.pid)+'/stat').read_text().split(') ',1)[1].split()[19]),uid=os.getuid(),unix=time.time(),budget_seconds=450,namespace='jump host')
(root/'probe-process.json').write_text(json.dumps(process,indent=2)+chr(10));print(json.dumps(process))
'''.replace('@R@',repr(record)).replace('@REF@',repr(reference)).replace('@SLOTS@',repr(plan['base_url'].removesuffix('/v1')+'/slots')).replace('@DATA@',repr({name:base64.b64encode(raw).decode() for name,raw in files.items()}))
    compile(code,'exact-isolation-client-bootstrap-v90','exec')
    with client_path.open('x') as output:output.write(json.dumps(record,indent=2)+'\n')
    record['phase']='launch_requested';client_path.write_text(json.dumps(record,indent=2)+'\n')
    try:record.update(process=jump(code),phase='launched')
    except BaseException:
        record['phase']='launch_outcome_requires_inspection';client_path.write_text(json.dumps(record,indent=2)+'\n');raise
    client_path.write_text(json.dumps(record,indent=2)+'\n');print(json.dumps(dict(phase=record['phase'],process=record['process']),indent=2))
else:
    record=json.loads(client_path.read_text())
    code='''import json,time
from pathlib import Path
r=@R@;root=Path(r['root']);out={'unix':time.time()}
if root.resolve()!=root or root.stat().st_uid!=10250:raise RuntimeError('exact matrix result owner differs')
p=root/'probe-result.json'
if p.exists():
 if p.is_symlink() or p.stat().st_size>1048576:raise RuntimeError('bounded exact matrix result exceeded')
 out['result']=json.loads(p.read_text())
p=Path('/proc/'+str(r['process']['pid'])+'/stat')
out['exact_client_alive']=p.exists() and p.stat().st_uid==10250 and int(p.read_text().split(') ',1)[1].split()[19])==r['process']['start_ticks'] and p.read_text().split(') ',1)[1].split()[0]!='Z'
print(json.dumps(out))
'''.replace('@R@',repr(record))
    v=jump(code);(BASE/('xolotl-admission-client-v102-'+a.profile+'-latest.json')).write_text(json.dumps(v,indent=2)+'\n');q=v.get('result',{})
    if a.action=='observe_client':
        print(json.dumps(dict(phase=q.get('phase'),completed=q.get('diagnostic_completed'),tests=len(q.get('tests',[])),alive=v['exact_client_alive'],qualified_gate=q.get('qualified_reference_gate_passed'),profile_serial=q.get('profile_serial_repeat_passed'),profile_concurrent=q.get('profile_concurrent_reference_passed'),profile_post=q.get('profile_post_serial_passed'),error=q.get('error')),indent=2))
    else:
        if v['exact_client_alive'] or not q.get('diagnostic_completed') or len(q['tests'])!=10:raise RuntimeError('all10 diagnostic calls must finish before planned drain; failed qualified gate remains failed')
        code='''import json,os,signal,time,urllib.request
from pathlib import Path
r=@R@;root=Path(r['root']);q=json.loads((root/'service-longctx-v1-result.json').read_text())
if q['phase']!='serving' or q['gpu_uuid']!=r['gpu_uuid']:raise RuntimeError('exact private model differs')
key=(root/'credentials/api-key.txt').read_text().strip();req=urllib.request.Request('http://127.0.0.1:30003/slots',headers={'Authorization':'Bearer '+key})
with urllib.request.urlopen(req,timeout=5) as response:slots=json.load(response)
if not isinstance(slots,list) or not slots or any(row['is_processing'] for row in slots):raise RuntimeError('private diagnostic model must be idle')
p=Path('/proc/'+str(r['process']['pid'])+'/stat')
if not p.exists() or p.stat().st_uid!=10250 or int(p.read_text().split(') ',1)[1].split()[19])!=r['process']['start_ticks']:raise RuntimeError('exact diagnostic supervisor differs')
os.kill(r['process']['pid'],signal.SIGTERM);print(json.dumps({'unix':time.time(),'signal_sent':True,'pid':r['process']['pid']}))
'''.replace('@R@',repr(service))
        stop=owned(code);(BASE/('xolotl-admission-service-v101-'+a.profile+'-drain.json')).write_text(json.dumps(stop,indent=2)+'\n');print(json.dumps(stop,indent=2))
