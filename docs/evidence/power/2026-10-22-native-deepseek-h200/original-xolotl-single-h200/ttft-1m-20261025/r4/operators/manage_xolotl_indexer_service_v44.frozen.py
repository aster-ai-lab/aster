"""Read or qualify the exact isolated indexer service; no formal requests."""
import argparse
import ast
import base64
import hashlib
import json
import time
from xolotl_trial_common import BASE,observe,owned,jump
p=argparse.ArgumentParser();p.add_argument('action',choices=['observe','launch_client','observe_client']);a=p.parse_args()
r=json.loads((BASE/'xolotl-indexer-service-v44-intent.json').read_text());o=observe()
if o['job_uid']!=r['allocation_uid']:raise RuntimeError('exact indexer allocation differs')
ip=BASE/'xolotl-indexer-basic-v44-intent.json'
if a.action=='observe':
    code='''import json,time
from pathlib import Path
r=@R@;root=Path(r['root']);out={'unix':time.time()}
if root.resolve()!=root or root.stat().st_uid!=10250:raise RuntimeError('exact indexer service root differs')
for name in ['service-longctx-v1-result.json','service-process.json']:
 p=root/name
 if p.exists():
  if p.is_symlink() or p.stat().st_size>65536:raise RuntimeError('bounded indexer service receipt exceeded')
  out[name]=json.loads(p.read_text())
p=Path('/proc/'+str(r['process']['pid'])+'/stat')
out['exact_supervisor_alive']=p.exists() and p.stat().st_uid==10250 and int(p.read_text().split(') ',1)[1].split()[19])==r['process']['start_ticks'] and p.read_text().split(') ',1)[1].split()[0]!='Z'
p=root/'engine.log'
if p.exists():
 if p.is_symlink() or p.stat().st_size>8*1024**2:raise RuntimeError('bounded numeric indexer log exceeded')
 prefixes=('serve: prompt phases: ','serve: prompt ','dense prof ','resident MMQ stage:','serve: cache kept for ','serve: cache regrown:')
 out['numeric_tail']=[line for line in p.read_text(errors='replace').splitlines() if line.startswith(prefixes)][-15:]
if out.get('service-longctx-v1-result.json',{}).get('phase')=='failed':
 for name in ['service-worker.log','api-longctx-v1-attempt1.stderr.log']:
  p=root/name
  if p.exists() and p.stat().st_size<262144:out[name]=p.read_text(errors='replace')[-3000:]
print(json.dumps(out))
'''.replace('@R@',repr(r))
    v=owned(code);(BASE/'xolotl-indexer-service-v44-latest.json').write_text(json.dumps(v,indent=2)+'\n');print(json.dumps(v,indent=2))
elif a.action=='launch_client':
    if ip.exists():raise RuntimeError('V44 basic client already has an exact handle')
    old=json.loads((BASE/'xolotl-longctx-request-v1-intent.json').read_text())
    plan=dict(base_url='http://'+r['pod_ip']+':30003/v1',service_root=r['root'],context_tokens=1048576,public_fixture=old['root']+'/public-fixture.txt',fixture_sha256='8612db6d8a2201190388ee524d88dcb04dada6721f9e983f6389aaf722fbc808')
    files={'request-worker.py':(BASE/'xolotl_ttft_qualification_worker_v15.frozen.py').read_bytes(),'request-plan.json':(json.dumps(plan,indent=2)+'\n').encode(),'prompt-id-controls.json':(BASE/'xolotl-trial-prompt-ids.json').read_bytes()}
    c=dict(unix=time.time(),phase='prepared',root=r['root']+'/qualification-basic-v44',allocation_uid=r['allocation_uid'],public_fixture_only=True,files=[dict(file=n,bytes=len(b),sha256=hashlib.sha256(b).hexdigest()) for n,b in files.items()])
    tree=ast.parse((BASE/'launch_xolotl_context_1m_over128k_v8.frozen.py').read_text());bodies=[x.value for x in ast.walk(tree) if isinstance(x,ast.Constant) and isinstance(x.value,str) and "q=json.loads((service/'service-longctx-v1-result.json').read_text())" in x.value]
    if len(bodies)!=1:raise RuntimeError('exact bounded public client bootstrap differs')
    code=bodies[0].replace('@R@',repr(c)).replace('@DATA@',repr({n:base64.b64encode(b).decode() for n,b in files.items()}));compile(code,'indexer-basic-v44','exec')
    with ip.open('x') as f:f.write(json.dumps(c,indent=2)+'\n')
    c['phase']='launch_requested';ip.write_text(json.dumps(c,indent=2)+'\n')
    try:
        c['process']=jump(code);c['phase']='launched'
    except BaseException:
        c['phase']='launch_outcome_requires_inspection';ip.write_text(json.dumps(c,indent=2)+'\n');raise
    ip.write_text(json.dumps(c,indent=2)+'\n');print(json.dumps({'phase':c['phase'],'process':c['process']}))
else:
    c=json.loads(ip.read_text())
    code='''import json,time
from pathlib import Path
c=@C@;root=Path(c['root']);out={'unix':time.time()}
p=root/'request-result.json'
if p.exists():
 if p.is_symlink() or p.stat().st_size>262144:raise RuntimeError('bounded public basic receipt exceeded')
 out['result']=json.loads(p.read_text())
p=Path('/proc/'+str(c['process']['pid'])+'/stat')
out['exact_client_alive']=p.exists() and p.stat().st_uid==10250 and int(p.read_text().split(') ',1)[1].split()[19])==c['process']['start_ticks'] and p.read_text().split(') ',1)[1].split()[0]!='Z'
print(json.dumps(out))
'''.replace('@C@',repr(c))
    v=jump(code);(BASE/'xolotl-indexer-basic-v44-latest.json').write_text(json.dumps(v,indent=2)+'\n');q=v.get('result',{})
    print(json.dumps({'phase':q.get('phase'),'exact_client_alive':v['exact_client_alive'],'tests':len(q.get('tests',[])),'error':q.get('error'),'short':q.get('short_summary'),'long_tests':[{'name':x['name'],'ttft_seconds':x['ttft_seconds'],'seconds':x['seconds'],'content':x['content']} for x in q.get('tests',[])[20:]]},indent=2))
