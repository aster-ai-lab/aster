"""Load exact-mask candidate only after full-score identity and zero-error memcheck."""
import ast
import base64
import hashlib
import json
import time
from xolotl_trial_common import BASE,lease,observe,owned
ip=BASE/'xolotl-indexer-service-v44-intent.json'
if ip.exists():raise RuntimeError('V44 service has an exact handle; inspect it')
baseline=json.loads((BASE/'xolotl-resident-compare-v37-v35_packed-intent.json').read_text())
build=json.loads((BASE/'xolotl-indexer-gpu-build-v40r2-intent.json').read_text())
mem=json.loads((BASE/'xolotl-indexer-memcheck-v43-intent.json').read_text())
num=json.loads((BASE/'xolotl-indexer-gpu-numerical-v40r2-intent.json').read_text())
o=observe()
if o['job_uid']!=baseline['allocation_uid'] or o['job_uid']!=build['allocation_uid']:raise RuntimeError('exact independent model comparison allocation differs')
code='''import json,subprocess
from pathlib import Path
baseline=@BASELINE@;build=@BUILD@;mem=@MEM@;num=@NUM@
root=Path(build['root']);b=json.loads((root/'gpu-build-result.json').read_text());n=json.loads((root/'gpu-numerical-result.json').read_text());m=json.loads((root/'memcheck-result.json').read_text())
def alive(record):
 p=Path('/proc/'+str(record['process']['pid'])+'/stat')
 return p.exists() and p.stat().st_uid==10250 and int(p.read_text().split(') ',1)[1].split()[19])==record['process']['start_ticks'] and p.read_text().split(') ',1)[1].split()[0]!='Z'
if b['phase']!='built' or n['phase']!='passed' or not n.get('full_score_byte_identity_passed') or m['phase']!='passed' or not m.get('zero_errors') or alive(build) or alive(mem) or alive(num):raise RuntimeError('exact CUDA build/numerical/memory gates must pass and finish')
old=json.loads((Path(baseline['root'])/'service-longctx-v1-result.json').read_text())
if old['phase']!='stopped' or not old.get('api_reaped'):raise RuntimeError('qualified packed baseline must be stopped/reaped')
apps=subprocess.run(['nvidia-smi','--query-compute-apps=pid','--format=csv,noheader,nounits'],capture_output=True,text=True,check=True,timeout=5).stdout.strip()
if apps:raise RuntimeError('model comparison GPU must be idle')
print(json.dumps({'built':b,'config':json.loads((Path(baseline['root'])/'server-config.json').read_text())}))
'''.replace('@BASELINE@',repr(baseline)).replace('@BUILD@',repr(build)).replace('@MEM@',repr(mem)).replace('@NUM@',repr(num))
q=owned(code);root=lease()['remote']+'/serve-indexer-mask-v44'
plan=dict(baseline['plan']);plan.update(schema='aster-xolotl-exact-indexer-mask-v44',binary=q['built']['candidate']['path'],binary_sha256=q['built']['candidate']['sha256'])
plan['engine_env']=dict(plan['engine_env'],ASTER_XQ_INDEXER_MASK_SKIP='1')
config=q['config'];config.update(exe=plan['binary'],cwd=root,tokenizer=root+'/tokenizer',log=root+'/engine.log')
worker=(BASE/'xolotl_context_1m_worker_v8.frozen.py').read_text()
anchor=" env.update(PLAN['engine_env']);env['STRATA_API_KEY']=key"
if worker.count(anchor)!=1:raise RuntimeError('exact model environment handoff differs')
worker=worker.replace(anchor," env.pop('DS41_CHECK_ARENA',None)\n"+anchor)
payload={'service-worker.py':worker.encode(),'service-plan.json':(json.dumps(plan,indent=2)+'\n').encode(),'server-config.json':(json.dumps(config,indent=2)+'\n').encode(),'prompt-id-controls.json':(BASE/'xolotl-trial-prompt-ids.json').read_bytes()}
r=dict(unix=time.time(),phase='prepared',root=root,source_public_root=baseline['root'],allocation_job=o['job'],allocation_uid=o['job_uid'],replica=o['replicas'][0]['name'],replica_uid=o['replicas'][0]['uid'],gpu_uuid=baseline['gpu_uuid'],pod_ip=o['replicas'][0]['status']['podIP'],host_ip=o['replicas'][0]['status']['hostIP'],plan=plan,credential_file=baseline['credential_file'],same_gpu_full1m_baseline='xolotl-resident-v38-fullwindow-latest.json',formal_service_unchanged=True,full1m_qualified=False,files=[dict(file=n,bytes=len(b),sha256=hashlib.sha256(b).hexdigest()) for n,b in payload.items()])
tree=ast.parse((BASE/'launch_xolotl_longctx_shadow_v1.py').read_text());bodies=[x.value for x in ast.walk(tree) if isinstance(x,ast.Constant) and isinstance(x.value,str) and "root=Path(r['root']);src=Path(r['source_public_root'])" in x.value]
if len(bodies)!=1:raise RuntimeError('exact owned source bootstrap differs')
code=bodies[0].replace('@R@',repr(r)).replace('@DATA@',repr({n:base64.b64encode(b).decode() for n,b in payload.items()}));compile(code,'exact-indexer-model-v44','exec')
with ip.open('x') as f:f.write(json.dumps(r,indent=2)+'\n')
r['phase']='launch_requested';ip.write_text(json.dumps(r,indent=2)+'\n')
try:
    r['process']=owned(code);r['phase']='launched'
except BaseException:
    r['phase']='launch_outcome_requires_inspection';ip.write_text(json.dumps(r,indent=2)+'\n');raise
ip.write_text(json.dumps(r,indent=2)+'\n');print(json.dumps({'phase':r['phase'],'binary_sha256':plan['binary_sha256'],'context_tokens':1048576,'formal_service_unchanged':True}))
