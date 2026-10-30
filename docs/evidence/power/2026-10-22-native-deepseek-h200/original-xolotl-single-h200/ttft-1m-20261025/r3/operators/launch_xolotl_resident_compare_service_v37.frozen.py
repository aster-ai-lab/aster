"""One explicit comparison profile at a time; never targets the formal service."""
import argparse
import ast
import base64
import hashlib
import json
import time
from xolotl_trial_common import BASE, lease, observe, owned

parser = argparse.ArgumentParser()
parser.add_argument('profile',choices=['v15_control','v35_keep','v35_packed'])
profile = parser.parse_args().profile
ip = BASE/('xolotl-resident-compare-v37-'+profile+'-intent.json')
if ip.exists():raise RuntimeError('comparison has an exact handle; inspect it')
stable = json.loads((BASE/'xolotl-stable-service-v31-intent.json').read_text())
build = json.loads((BASE/'xolotl-resident-mmq-build-v35-intent.json').read_text())
gate = json.loads((BASE/'xolotl-resident-mmq-gpu-gate-v36r2-intent.json').read_text())
o = observe()
if o['job_uid']!=build['allocation_uid'] or o['job_uid']==stable['allocation_uid'] or o['resource-result.json']['gpus'][0]['uuid']==stable['gpu_uuid']:
    raise RuntimeError('comparison requires its separate exact GPU')
code = '''import json,subprocess
from pathlib import Path
gate=@GATE@;stable=@STABLE@;build=@BUILD@
q=json.loads((Path(gate['root'])/'gpu-result.json').read_text())
p=Path('/proc/'+str(gate['process']['pid'])+'/stat')
alive=p.exists() and p.stat().st_uid==10250 and int(p.read_text().split(') ',1)[1].split()[19])==gate['process']['start_ticks'] and p.read_text().split(') ',1)[1].split()[0]!='Z'
if q['phase']!='passed' or alive:raise RuntimeError('real numerical/timing gate must pass and exit first')
apps=subprocess.run(['nvidia-smi','--query-compute-apps=pid','--format=csv,noheader,nounits'],capture_output=True,text=True,check=True,timeout=5).stdout.strip()
if apps:raise RuntimeError('comparison GPU must have no active model')
config=Path(stable['root'])/'server-config.json'
if config.is_symlink() or config.stat().st_uid!=10250 or config.stat().st_size>16384:raise RuntimeError('qualified stable config differs')
print(json.dumps({'config':json.loads(config.read_text()),'built':json.loads((Path(build['root'])/'result.json').read_text()),'gpu_gate_passed':True}))
'''.replace('@GATE@',repr(gate)).replace('@STABLE@',repr(stable)).replace('@BUILD@',repr(build))
q = owned(code)
root = lease()['remote']+'/serve-resident-compare-v37-'+profile
plan = dict(stable['plan'])
plan.update(schema='aster-xolotl-resident-mmq-comparison-v37',lease_ends_unix=o['resource-result.json']['leaseEndsUnix'],optimization_gpu_uuid=stable['gpu_uuid'])
plan['engine_env'] = dict(plan['engine_env'],MIMO_CHUNK_PROF='1',STRATA_DENSE_PROF='1')
if profile!='v15_control':
    plan.update(binary=q['built']['binary']['path'],binary_sha256=q['built']['binary']['sha256'])
    plan['engine_env'].update(STRATA_SERVE_KEEP_CACHE='1',STRATA_SERVE_KEEP_LRU='1',ASTER_XQ_KEEP_MIXED='1',ASTER_XQ_RESIDENT_MMQ='1' if profile=='v35_packed' else '0')
config = q['config']
config.update(exe=plan['binary'],cwd=root,tokenizer=root+'/tokenizer',log=root+'/engine.log',parallel=2)
worker = (BASE/'xolotl_context_1m_worker_v8.frozen.py').read_text()
anchor = " env.update(PLAN['engine_env']);env['STRATA_API_KEY']=key"
if worker.count(anchor)!=1:raise RuntimeError('exact service environment handoff differs')
worker = worker.replace(anchor," env.pop('DS41_CHECK_ARENA',None)  # omit the presence-only expensive diagnostic after numerical gates\n"+anchor)
ast.parse(worker)
payload = {'service-worker.py':worker.encode(),'service-plan.json':(json.dumps(plan,indent=2)+'\n').encode(),'server-config.json':(json.dumps(config,indent=2)+'\n').encode(),'prompt-id-controls.json':(BASE/'xolotl-trial-prompt-ids.json').read_bytes()}
r = dict(unix=time.time(),phase='prepared',profile=profile,root=root,source_public_root=stable['root'],allocation_job=o['job'],allocation_uid=o['job_uid'],replica=o['replicas'][0]['name'],replica_uid=o['replicas'][0]['uid'],gpu_uuid=o['resource-result.json']['gpus'][0]['uuid'],pod_ip=o['replicas'][0]['status']['podIP'],host_ip=o['replicas'][0]['status']['hostIP'],plan=plan,credential_file=stable['credential_file'],formal_service_unchanged=True,purpose='Same-card comparison of exact V15, checked retention, and opt-in resident D2D packing; no memo and no expensive diagnostic byte reads',files=[dict(file=n,bytes=len(b),sha256=hashlib.sha256(b).hexdigest()) for n,b in payload.items()])
tree = ast.parse((BASE/'launch_xolotl_longctx_shadow_v1.py').read_text())
bodies = [node.value for node in ast.walk(tree) if isinstance(node,ast.Constant) and isinstance(node.value,str) and "root=Path(r['root']);src=Path(r['source_public_root'])" in node.value]
if len(bodies)!=1:raise RuntimeError('exact public source bootstrap differs')
code = bodies[0].replace('@R@',repr(r)).replace('@DATA@',repr({n:base64.b64encode(b).decode() for n,b in payload.items()}))
compile(code,'resident-compare-service-v37','exec')
with ip.open('x') as f:f.write(json.dumps(r,indent=2)+'\n')
r['phase']='launch_requested'
ip.write_text(json.dumps(r,indent=2)+'\n')
try:
    r['process']=owned(code)
    r['phase']='launched'
except BaseException:
    r['phase']='launch_outcome_requires_inspection'
    ip.write_text(json.dumps(r,indent=2)+'\n')
    raise
ip.write_text(json.dumps(r,indent=2)+'\n')
print(json.dumps({'phase':r['phase'],'profile':profile,'gpu_uuid':r['gpu_uuid'],'context_tokens':1048576,'formal_service_unchanged':True}))
