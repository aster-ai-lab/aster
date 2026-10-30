"""Change only attention sub-batch width on the exact idle optimization H200."""
import argparse,ast,base64,hashlib,json,time
from xolotl_trial_common import BASE,lease,observe,owned,jump
parser=argparse.ArgumentParser();parser.add_argument('profile',choices=['dense256','dense512','dense128_repeat']);profile=parser.parse_args().profile
ip=BASE/('xolotl-dense-subbatch-v47-'+profile+'-intent.json')
if ip.exists():raise RuntimeError('V47 has an exact handle; observe it')
baseline=json.loads((BASE/'xolotl-indexer-service-v44-intent.json').read_text())
suite=json.loads((BASE/'xolotl-indexer-fullwindow-suite-v45-intent.json').read_text())
o=observe();reservation=json.loads((BASE/'xolotl-active-serving-reservation.json').read_text())
if o['phase']!='Running' or o['job_uid']!=baseline['allocation_uid'] or o['job_uid']==reservation['allocation_uid'] or len(o['replicas'])!=1 or o['replicas'][0]['uid']!=baseline['replica_uid']:raise RuntimeError('exact independent sub-batch allocation differs')
code='''import json,time
from pathlib import Path
suite=@SUITE@;root=Path(suite['root']);q=json.loads((root/'suite-result.json').read_text())
p=Path('/proc/'+str(suite['process']['pid'])+'/stat')
alive=p.exists() and p.stat().st_uid==10250 and int(p.read_text().split(') ',1)[1].split()[19])==suite['process']['start_ticks'] and p.read_text().split(') ',1)[1].split()[0]!='Z'
if q['phase']!='passed' or alive or [x['name'] for x in q['stages']]!=['remainder','fullwindow','cache-load']:raise RuntimeError('exact V45 full suite must pass and finish before sub-batch screen')
print(json.dumps({'suite_passed':True}))
'''.replace('@SUITE@',repr(suite));jump(code)
code='''import hashlib,json,subprocess
from pathlib import Path
r=@BASELINE@;root=Path(r['root'])
if root.resolve()!=root or root.stat().st_uid!=10250:raise RuntimeError('exact reference source owner differs')
state=json.loads((root/'service-longctx-v1-result.json').read_text())
if state['phase']!='stopped' or not state.get('api_reaped'):raise RuntimeError('V44 must be stopped and reaped first')
apps=subprocess.run(['nvidia-smi','--query-compute-apps=pid','--format=csv,noheader,nounits'],capture_output=True,text=True,check=True,timeout=5).stdout.strip()
if apps:raise RuntimeError('sub-batch comparison GPU must be idle')
binary=Path(r['plan']['binary'])
if hashlib.sha256(binary.read_bytes()).hexdigest()!=r['plan']['binary_sha256']:raise RuntimeError('same qualified native binary differs')
p=root/'server-config.json'
if p.is_symlink() or p.stat().st_size>16384:raise RuntimeError('bounded exact public config differs')
print(json.dumps(json.loads(p.read_text())))
'''.replace('@BASELINE@',repr(baseline));config=owned(code)
width={'dense256':256,'dense512':512,'dense128_repeat':128}[profile]
root=lease()['remote']+'/serve-dense-subbatch-v47-'+profile
plan=dict(baseline['plan']);plan.update(schema='aster-xolotl-attention-subbatch-v47',prefill_chunk_tokens=width)
plan['engine_env']=dict(plan['engine_env'])
if plan['engine_env'].get('ASTER_XQ_INDEXER_MASK_SKIP')!='1' or plan['binary_sha256']!='fa0195a9b64f5e18136eec6852997cd0637f3197e9b6b97e4f512c5f50d5f373':raise RuntimeError('same V44 indexer controls differ')
args=config['args']
if args.count('--prefill-chunk')!=1 or args[args.index('--prefill-chunk')+1]!='128':raise RuntimeError('exact reference attention sub-batch differs')
args[args.index('--prefill-chunk')+1]=str(width)
config.update(cwd=root,tokenizer=root+'/tokenizer',log=root+'/engine.log')
worker=(BASE/'xolotl_context_1m_worker_v8.frozen.py').read_text()
anchor=" env.update(PLAN['engine_env']);env['STRATA_API_KEY']=key"
if worker.count(anchor)!=1:raise RuntimeError('exact service environment handoff differs')
worker=worker.replace(anchor," env.pop('DS41_CHECK_ARENA',None)\n"+anchor)
files={'service-worker.py':worker.encode(),'service-plan.json':(json.dumps(plan,indent=2)+'\n').encode(),'server-config.json':(json.dumps(config,indent=2)+'\n').encode(),'prompt-id-controls.json':(BASE/'xolotl-trial-prompt-ids.json').read_bytes()}
r=dict(unix=time.time(),phase='prepared',profile=profile,root=root,source_public_root=baseline['root'],allocation_job=o['job'],allocation_uid=o['job_uid'],replica=o['replicas'][0]['name'],replica_uid=o['replicas'][0]['uid'],gpu_uuid=baseline['gpu_uuid'],pod_ip=baseline['pod_ip'],host_ip=baseline['host_ip'],plan=plan,credential_file=baseline['credential_file'],formal_service_unchanged=True,changed_control='attention sub-batch width only; layer-major batch8192, exact routing, original Python API, native binary and all other flags preserved',files=[dict(file=n,bytes=len(raw),sha256=hashlib.sha256(raw).hexdigest()) for n,raw in files.items()])
tree=ast.parse((BASE/'launch_xolotl_longctx_shadow_v1.py').read_text());bodies=[n.value for n in ast.walk(tree) if isinstance(n,ast.Constant) and isinstance(n.value,str) and "root=Path(r['root']);src=Path(r['source_public_root'])" in n.value]
if len(bodies)!=1:raise RuntimeError('exact source staging bootstrap differs')
code=bodies[0].replace('@R@',repr(r)).replace('@DATA@',repr({n:base64.b64encode(v).decode() for n,v in files.items()}));compile(code,'subbatch-v47-bootstrap','exec')
with ip.open('x') as f:f.write(json.dumps(r,indent=2)+'\n')
r['phase']='launch_requested';ip.write_text(json.dumps(r,indent=2)+'\n')
try:
 r['process']=owned(code);r['phase']='launched'
except BaseException:
 r['phase']='launch_outcome_requires_inspection';ip.write_text(json.dumps(r,indent=2)+'\n');raise
ip.write_text(json.dumps(r,indent=2)+'\n');print(json.dumps({'phase':r['phase'],'profile':profile,'attention_sub_batch_tokens':width,'formal_service_unchanged':True}))
