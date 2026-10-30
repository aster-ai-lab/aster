"""Qualify exact seed capture, graph-aware retained LRU cache and bounded first/replay prefill scheduling."""
import ast, base64, hashlib, json, time
from xolotl_trial_common import BASE, observe, lease, owned

ip=BASE/'xolotl-ttft-shadow-v15-intent.json'
if ip.exists():raise RuntimeError('V15 intent exists; observe exact process instead')
stable=json.loads((BASE/'xolotl-stable-service-v8-intent.json').read_text())
o=observe();gpu=o['resource-result.json']['gpus'][0]['uuid']
if o['job_uid']==stable['allocation_uid'] or gpu==stable['gpu_uuid']:raise RuntimeError('separate optimization GPU required')
code='''import json,subprocess
from pathlib import Path
root=Path(@ROOT@)
old=json.loads((root/'serve-ttft-v13/service-longctx-v1-result.json').read_text())
if old['phase']!='stopped' or not old.get('api_reaped'):raise RuntimeError('prior exact API must be stopped and reaped')
p=subprocess.run(['nvidia-smi','--query-compute-apps=pid','--format=csv,noheader,nounits'],capture_output=True,text=True,timeout=8,check=True)
if p.stdout.strip():raise RuntimeError('optimization GPU busy')
print(json.dumps(json.loads(Path(@CONFIG@).read_text())))
'''.replace('@ROOT@',repr(lease()['remote'])).replace('@CONFIG@',repr(stable['root']+'/server-config.json'))
config=owned(code);root=lease()['remote']+'/serve-ttft-v15'
plan=dict(stable['plan']);plan.update(schema='aster-xolotl-ttft-shadow-plan-v15',lease_ends_unix=o['resource-result.json']['leaseEndsUnix'],optimization_gpu_uuid=stable['gpu_uuid'])
build=json.loads((BASE/'xolotl-ttft-build-v15-latest.json').read_text())['result.json']
if build['phase']!='built' or not build['policy_test_passed'] or build['control_binary_sha256']!=stable['plan']['binary_sha256']:raise RuntimeError('controlled build gate differs')
plan.update(binary=build['binary']['path'],binary_sha256=build['binary']['sha256'],prefill_batch_tokens=8192)
plan['engine_env']=dict(plan['engine_env'],STRATA_SERVE_BATCH_TAIL='1',STRATA_SERVE_KEEP_CACHE='0',STRATA_SERVE_KEEP_MAX='1048576',STRATA_SERVE_KEEP_LRU='0',ASTER_XQ_PREFILL_QOS='1')
config['exe']=plan['binary'];config['args'][config['args'].index('--prompt-batch')+1]='8192'
config.update(cwd=root,tokenizer=root+'/tokenizer',log=root+'/engine.log',parallel=2)
worker=(BASE/'xolotl_context_1m_worker_v8.frozen.py').read_bytes()
payload={'service-worker.py':worker,'service-plan.json':(json.dumps(plan,indent=2)+'\n').encode(),'server-config.json':(json.dumps(config,indent=2)+'\n').encode(),'prompt-id-controls.json':(BASE/'xolotl-trial-prompt-ids.json').read_bytes()}
r=dict(unix=time.time(),phase='prepared',root=root,source_public_root=stable['root'],allocation_job=o['job'],allocation_uid=o['job_uid'],replica=o['replicas'][0]['name'],replica_uid=o['replicas'][0]['uid'],gpu_uuid=gpu,pod_ip=o['replicas'][0]['status']['podIP'],host_ip=o['replicas'][0]['status']['hostIP'],plan=plan,credential_file=stable['credential_file'],purpose='Uncontended1M baseline and TTFT optimization; no VIP promotion before qualification',files=[dict(file=n,bytes=len(b),sha256=hashlib.sha256(b).hexdigest()) for n,b in payload.items()])
ip.write_text(json.dumps(r,indent=2)+'\n')
tree=ast.parse((BASE/'launch_xolotl_longctx_shadow_v1.py').read_text())
body=[x.value for x in ast.walk(tree) if isinstance(x,ast.Constant) and isinstance(x.value,str) and "root=Path(r['root']);src=Path(r['source_public_root'])" in x.value]
if len(body)!=1:raise RuntimeError('public staging bootstrap structure differs')
code=body[0].replace('@R@',repr(r)).replace('@DATA@',repr({n:base64.b64encode(b).decode() for n,b in payload.items()}))
compile(code,'v15-shadow-bootstrap','exec');r['phase']='launch_requested';ip.write_text(json.dumps(r,indent=2)+'\n')
r['process']=owned(code);r['phase']='launched';ip.write_text(json.dumps(r,indent=2)+'\n')
print(json.dumps(dict(phase='launched',gpu_uuid=gpu,context=plan['context_tokens'],formal_service_unchanged=True)))
