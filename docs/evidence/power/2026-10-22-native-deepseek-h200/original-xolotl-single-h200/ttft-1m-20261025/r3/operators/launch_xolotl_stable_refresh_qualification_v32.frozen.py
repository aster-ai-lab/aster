"""Freeze and launch a bounded public client; no formal serving requests."""
import ast,base64,hashlib,json,time
from xolotl_trial_common import BASE,observe,jump
ip=BASE/'xolotl-stable-refresh-v32-qualification-intent.json'
if ip.exists():raise RuntimeError('V15 client exists; observe exact process')
r=json.loads((BASE/'xolotl-stable-refresh-v31-intent.json').read_text());o=observe()
if r['allocation_uid']!=o['job_uid'] or o['phase']!='Running':raise RuntimeError('exact optimization allocation changed')
old=json.loads((BASE/'xolotl-longctx-request-v1-intent.json').read_text());root=r['root']+'/qualification-stable-refresh-v32'
plan=dict(base_url='http://'+r['pod_ip']+':30003/v1',service_root=r['root'],context_tokens=1048576,public_fixture=old['root']+'/public-fixture.txt',fixture_sha256='8612db6d8a2201190388ee524d88dcb04dada6721f9e983f6389aaf722fbc808')
payload={'request-worker.py':(BASE/'xolotl_ttft_qualification_worker_v15.frozen.py').read_bytes(),'request-plan.json':(json.dumps(plan,indent=2)+'\n').encode(),'prompt-id-controls.json':(BASE/'xolotl-trial-prompt-ids.json').read_bytes()}
rec=dict(unix=time.time(),phase='prepared',root=root,allocation_uid=r['allocation_uid'],public_fixture_only=True,files=[dict(file=n,bytes=len(b),sha256=hashlib.sha256(b).hexdigest()) for n,b in payload.items()]);ip.write_text(json.dumps(rec,indent=2)+'\n')
tree=ast.parse((BASE/'launch_xolotl_context_1m_over128k_v8.frozen.py').read_text());body=[x.value for x in ast.walk(tree) if isinstance(x,ast.Constant) and isinstance(x.value,str) and "q=json.loads((service/'service-longctx-v1-result.json').read_text())" in x.value]
if len(body)!=1:raise RuntimeError('bounded client bootstrap differs')
code=body[0].replace('@R@',repr(rec)).replace('@DATA@',repr({n:base64.b64encode(b).decode() for n,b in payload.items()}));compile(code,'public-v15-ttft-client','exec')
rec['phase']='launch_requested';ip.write_text(json.dumps(rec,indent=2)+'\n');rec['process']=jump(code);rec['phase']='launched';ip.write_text(json.dumps(rec,indent=2)+'\n');print(json.dumps(dict(phase='launched',root=root)))
