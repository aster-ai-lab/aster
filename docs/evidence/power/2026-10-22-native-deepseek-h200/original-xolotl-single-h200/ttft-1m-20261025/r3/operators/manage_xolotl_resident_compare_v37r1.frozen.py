"""Observe, qualify or safely drain one exact isolated comparison profile."""
import argparse
import ast
import base64
import hashlib
import json
import time
from xolotl_trial_common import BASE, observe, owned, jump

parser=argparse.ArgumentParser()
parser.add_argument('profile',choices=['v15_control','v35_keep','v35_packed'])
parser.add_argument('action',choices=['observe','launch_client','observe_client','drain'])
a=parser.parse_args()
r=json.loads((BASE/('xolotl-resident-compare-v37-'+a.profile+'-intent.json')).read_text())
o=observe()
if o['job_uid']!=r['allocation_uid']:raise RuntimeError('comparison allocation differs')
client_ip=BASE/('xolotl-resident-compare-v37-'+a.profile+'-qualification-intent.json')

if a.action=='observe':
    code='''import json,time
from pathlib import Path
r=@R@;root=Path(r['root']);out={'unix':time.time()}
if root.resolve()!=root or root.stat().st_uid!=10250:raise RuntimeError('exact comparison root differs')
for name in ['service-longctx-v1-result.json','service-process.json']:
 p=root/name
 if p.exists():
  if p.is_symlink() or p.stat().st_size>65536:raise RuntimeError('comparison receipt bound exceeded')
  out[name]=json.loads(p.read_text())
p=Path('/proc/'+str(r['process']['pid'])+'/stat')
out['exact_supervisor_alive']=p.exists() and p.stat().st_uid==10250 and int(p.read_text().split(') ',1)[1].split()[19])==r['process']['start_ticks'] and p.read_text().split(') ',1)[1].split()[0]!='Z'
p=root/'engine.log'
if p.exists():
 if p.is_symlink() or p.stat().st_size>8*1024**2:raise RuntimeError('comparison public numeric log bound exceeded')
 prefixes=('serve: keep-cache:','serve: cache kept for ','serve: cache regrown:','[keep-chk]','dense prof ','serve: prompt phases: ','serve: prompt ','chunk prof','resident MMQ stage:')
 out['numeric_diagnostics']=[line for line in p.read_text(errors='replace').splitlines() if line.startswith(prefixes)][-24:]
if out.get('service-longctx-v1-result.json',{}).get('phase')=='failed':
 p=root/'service-worker.log'
 if p.exists() and p.stat().st_size<65536:out['failure_log']=p.read_text()[-4000:]
print(json.dumps(out))
'''.replace('@R@',repr(r))
    v=owned(code)
    (BASE/('xolotl-resident-compare-v37-'+a.profile+'-latest.json')).write_text(json.dumps(v,indent=2)+'\n')
    print(json.dumps(v,indent=2))
elif a.action=='launch_client':
    if client_ip.exists():raise RuntimeError('client has an exact handle; observe it')
    old=json.loads((BASE/'xolotl-longctx-request-v1-intent.json').read_text())
    root=r['root']+'/qualification-v37'
    plan=dict(base_url='http://'+r['pod_ip']+':30003/v1',service_root=r['root'],context_tokens=1048576,public_fixture=old['root']+'/public-fixture.txt',fixture_sha256='8612db6d8a2201190388ee524d88dcb04dada6721f9e983f6389aaf722fbc808')
    payload={'request-worker.py':(BASE/'xolotl_ttft_qualification_worker_v15.frozen.py').read_bytes(),'request-plan.json':(json.dumps(plan,indent=2)+'\n').encode(),'prompt-id-controls.json':(BASE/'xolotl-trial-prompt-ids.json').read_bytes()}
    c=dict(unix=time.time(),phase='prepared',root=root,allocation_uid=r['allocation_uid'],profile=a.profile,public_fixture_only=True,files=[dict(file=n,bytes=len(b),sha256=hashlib.sha256(b).hexdigest()) for n,b in payload.items()])
    tree=ast.parse((BASE/'launch_xolotl_context_1m_over128k_v8.frozen.py').read_text())
    bodies=[x.value for x in ast.walk(tree) if isinstance(x,ast.Constant) and isinstance(x.value,str) and "q=json.loads((service/'service-longctx-v1-result.json').read_text())" in x.value]
    if len(bodies)!=1:raise RuntimeError('exact bounded client bootstrap differs')
    code=bodies[0].replace('@R@',repr(c)).replace('@DATA@',repr({n:base64.b64encode(b).decode() for n,b in payload.items()}))
    with client_ip.open('x') as f:f.write(json.dumps(c,indent=2)+'\n')
    c['phase']='launch_requested';client_ip.write_text(json.dumps(c,indent=2)+'\n')
    try:
        c['process']=jump(code)
        c['phase']='launched'
    except BaseException:
        c['phase']='launch_outcome_requires_inspection';client_ip.write_text(json.dumps(c,indent=2)+'\n');raise
    client_ip.write_text(json.dumps(c,indent=2)+'\n')
    print(json.dumps({'phase':c['phase'],'profile':a.profile,'process':c['process']}))
else:
    c=json.loads(client_ip.read_text())
    code='''import json,time
from pathlib import Path
c=@C@;root=Path(c['root']);out={'unix':time.time()}
if root.resolve()!=root or root.stat().st_uid!=10250:raise RuntimeError('exact client root differs')
p=root/'request-result.json'
if p.exists():
 if p.is_symlink() or p.stat().st_size>262144:raise RuntimeError('client receipt bound exceeded')
 out['result']=json.loads(p.read_text())
p=Path('/proc/'+str(c['process']['pid'])+'/stat')
out['exact_client_alive']=p.exists() and p.stat().st_uid==10250 and int(p.read_text().split(') ',1)[1].split()[19])==c['process']['start_ticks'] and p.read_text().split(') ',1)[1].split()[0]!='Z'
print(json.dumps(out))
'''.replace('@C@',repr(c))
    v=jump(code)
    (BASE/('xolotl-resident-compare-v37-'+a.profile+'-qualification-latest.json')).write_text(json.dumps(v,indent=2)+'\n')
    q=v.get('result',{})
    if a.action=='observe_client':
        print(json.dumps({'profile':a.profile,'phase':q.get('phase'),'exact_client_alive':v['exact_client_alive'],'tests':len(q.get('tests',[])),'error':q.get('error'),'short_summary':q.get('short_summary'),'long_tests':[{'name':t['name'],'ttft_seconds':t['ttft_seconds'],'seconds':t['seconds'],'usage':t['usage'],'content':t['content']} for t in q.get('tests',[])[20:]]},indent=2))
    else:
        if q.get('phase')!='passed' or v['exact_client_alive']:raise RuntimeError('comparison must pass and finish before planned drain')
        code='''import json,os,signal,time,urllib.request
from pathlib import Path
r=@R@;root=Path(r['root']);state=json.loads((root/'service-longctx-v1-result.json').read_text())
if state['phase']!='serving' or state['gpu_uuid']!=r['gpu_uuid']:raise RuntimeError('exact serving comparison required')
key=(root/'credentials/api-key.txt').read_text().strip()
req=urllib.request.Request('http://127.0.0.1:30003/health',headers={'Authorization':'Bearer '+key})
with urllib.request.urlopen(req,timeout=4) as response:health=json.load(response)
if any(slot.get('busy') for slot in health.get('slots',[])):raise RuntimeError('comparison has a busy request')
p=Path('/proc/'+str(r['process']['pid'])+'/stat')
if not p.exists() or p.stat().st_uid!=10250 or int(p.read_text().split(') ',1)[1].split()[19])!=r['process']['start_ticks']:raise RuntimeError('exact comparison supervisor differs')
os.kill(r['process']['pid'],signal.SIGTERM)
print(json.dumps({'signal_sent':True,'pid':r['process']['pid'],'unix':time.time(),'health_before_stop':health}))
'''.replace('@R@',repr(r))
        stop=owned(code)
        (BASE/('xolotl-resident-compare-v37-'+a.profile+'-drain.json')).write_text(json.dumps(stop,indent=2)+'\n')
        print(json.dumps({'profile':a.profile,'drain_signal_sent':True,'exact_supervisor_pid':stop['pid']}))
