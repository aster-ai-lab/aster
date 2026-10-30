"""After the same-card comparison, qualify the exact remainder then actual full1M."""
import argparse
import base64
import hashlib
import json
import time
from xolotl_trial_common import BASE, observe, jump

parser=argparse.ArgumentParser()
parser.add_argument('kind',choices=['remainder','fullwindow'])
kind=parser.parse_args().kind
ip=BASE/('xolotl-resident-v38-'+kind+'-intent.json')
if ip.exists():raise RuntimeError('V38 client has an exact handle; observe it')
r=json.loads((BASE/'xolotl-resident-compare-v37-v35_packed-intent.json').read_text())
o=observe()
if o['job_uid']!=r['allocation_uid'] or o['phase']!='Running' or len(o['replicas'])!=1 or o['replicas'][0]['uid']!=r['replica_uid']:
    raise RuntimeError('exact packed optimization allocation differs')
reservation=json.loads((BASE/'xolotl-active-serving-reservation.json').read_text())
if reservation['allocation_uid']==r['allocation_uid']:raise RuntimeError('full-window test refused on formal serving resource')
source=BASE/('xolotl_context_qos_remainder_worker_v15.frozen.py' if kind=='remainder' else 'xolotl_full_1m_qualification_worker_v17.frozen.py')
worker=source.read_text()
if kind=='remainder':
    anchor="done and finish and '42' in content"
    if worker.count(anchor)!=1:raise RuntimeError('exact one-token remainder gate differs')
    worker=worker.replace(anchor,"done and finish and content.strip()=='42' and usage is not None and usage.get('prompt_tokens')==132097")
    with (BASE/'xolotl_resident_qos_remainder_worker_v38.frozen.py').open('x') as f:f.write(worker)
root=r['root']+'/qualification-v38-'+kind
plan=dict(base_url='http://'+r['pod_ip']+':30003/v1',service_root=r['root'],context_tokens=1048576,context_safety_tokens=8)
payload={'request-worker.py':worker.encode(),'request-plan.json':(json.dumps(plan,indent=2)+'\n').encode()}
c=dict(unix=time.time(),phase='prepared',kind=kind,root=root,allocation_uid=r['allocation_uid'],gpu_uuid=r['gpu_uuid'],binary_sha256=r['plan']['binary_sha256'],public_fixture_only=True,files=[dict(file=n,bytes=len(b),sha256=hashlib.sha256(b).hexdigest()) for n,b in payload.items()])
qa=json.loads((BASE/'xolotl-resident-compare-v37-v35_packed-qualification-intent.json').read_text())
prior=json.loads((BASE/'xolotl-resident-v38-remainder-intent.json').read_text()) if kind=='fullwindow' else None
code='''import base64,hashlib,json,os,subprocess,time
from pathlib import Path
c=@C@;qa=@QA@;prior=@PRIOR@;root=Path(c['root']);service=root.parent
if service.resolve()!=service or service.stat().st_uid!=10250 or root.exists():raise RuntimeError('exclusive candidate capacity test root differs')
q=json.loads((service/'service-longctx-v1-result.json').read_text())
if q['phase']!='serving' or q['gpu_uuid']!=c['gpu_uuid'] or q['binary_sha256']!=c['binary_sha256'] or q['context_tokens']!=1048576:raise RuntimeError('exact packed service identity differs')
def finished(record,result_name):
 result=json.loads((Path(record['root'])/result_name).read_text());p=Path('/proc/'+str(record['process']['pid'])+'/stat')
 alive=p.exists() and p.stat().st_uid==10250 and int(p.read_text().split(') ',1)[1].split()[19])==record['process']['start_ticks'] and p.read_text().split(') ',1)[1].split()[0]!='Z'
 if result['phase']!='passed' or alive:raise RuntimeError('prior exact public request must finish and pass')
 return result
basic=finished(qa,'request-result.json')
if len(basic['tests'])!=28 or not basic.get('handoff_three_position_retrieval_passed'):raise RuntimeError('packed basic28 request gates incomplete')
if prior is not None:
 edge=finished(prior,'request-result.json')
 if edge['usage']['prompt_tokens']!=132097 or edge['content'].strip()!='42':raise RuntimeError('actual one-token remainder gate differs')
budget=1100 if c['kind']=='remainder' else 3400
if time.time()+budget+120>q['lease_ends_unix']:raise RuntimeError('finite full-window lease insufficient')
root.mkdir(mode=0o700);data=@DATA@
for row in c['files']:
 raw=base64.b64decode(data[row['file']])
 if len(raw)!=row['bytes'] or hashlib.sha256(raw).hexdigest()!=row['sha256']:raise RuntimeError('capacity test frozen input differs')
 with (root/row['file']).open('xb') as f:f.write(raw)
with (root/'request-worker.log').open('xb') as log:
 p=subprocess.Popen(['timeout','--signal=TERM','--kill-after=15',str(budget),'python3','-u',str(root/'request-worker.py')],cwd=root,stdin=subprocess.DEVNULL,stdout=log,stderr=log,start_new_session=True)
process=dict(pid=p.pid,start_ticks=int(Path('/proc/'+str(p.pid)+'/stat').read_text().split(') ',1)[1].split()[19]),uid=os.getuid(),unix=time.time(),budget_seconds=budget)
(root/'request-process.json').write_text(json.dumps(process,indent=2)+'\\n');print(json.dumps(process))
'''.replace('@C@',repr(c)).replace('@QA@',repr(qa)).replace('@PRIOR@',repr(prior)).replace('@DATA@',repr({n:base64.b64encode(b).decode() for n,b in payload.items()}))
compile(code,'packed-fullwindow-client-v38','exec')
with ip.open('x') as f:f.write(json.dumps(c,indent=2)+'\n')
c['phase']='launch_requested';ip.write_text(json.dumps(c,indent=2)+'\n')
try:
    c['process']=jump(code)
    c['phase']='launched'
except BaseException:
    c['phase']='launch_outcome_requires_inspection';ip.write_text(json.dumps(c,indent=2)+'\n');raise
ip.write_text(json.dumps(c,indent=2)+'\n')
print(json.dumps({'phase':c['phase'],'kind':kind,'process':c['process'],'formal_service_unchanged':True}))
