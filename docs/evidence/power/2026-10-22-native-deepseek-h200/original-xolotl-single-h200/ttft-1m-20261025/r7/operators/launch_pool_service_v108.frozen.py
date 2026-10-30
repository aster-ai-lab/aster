"""Fresh-process same-binary isolation matrix on the exact replacement optimization allocation."""
import argparse
import ast
import base64
import hashlib
import json
import time
from xolotl_trial_common import BASE, lease, observe, owned

parser=argparse.ArgumentParser()
parser.add_argument('profile',choices=['pool_off','pool_on','pool_off_repeat'])
profile=parser.parse_args().profile
intent=BASE/('xolotl-pool-service-v108-'+profile+'-intent.json')
if intent.exists():raise RuntimeError('exact isolation service handle exists; observe it')
baseline=json.loads((BASE/'xolotl-engram-disk-v68-gpfs_a-intent.json').read_text())
build=json.loads((BASE/'xolotl-pool-seed-native-v106-intent.json').read_text())
adapter=json.loads((BASE/'xolotl-api-hybrid-v52-intent.json').read_text())
resource=observe()
if resource['phase']!='Running' or resource['job_uid']!='7455d989-8255-4f7c-ae8f-3cc0aa5928e4' or resource.get('resource-result.json',{}).get('phase')!='passed':
    raise RuntimeError('exact independent C10 allocation must pass its hardware probe')
proof=resource['resource-result.json'];status=resource['replicas'][0]['status']
if len(proof['gpus'])!=1 or proof['gpus'][0]['name']!='NVIDIA H200':raise RuntimeError('exact single H200 hardware differs')
code='''import hashlib,json,subprocess
from pathlib import Path
baseline=@BASE@;build=@BUILD@;adapter=@API@
root=Path(build['root']);q=json.loads((root/'native-link-result.json').read_text())
if q['phase']!='linked' or not q['control_binary_byte_equal'] or q['invalid_startup_modes_rejected']!=8:raise RuntimeError('qualified native34f link proof differs')
p=Path(q['binary']['path'])
if p.is_symlink() or p.stat().st_uid!=10250 or hashlib.sha256(p.read_bytes()).hexdigest()!='9a684b2442d4b648fff6a73024de5b7bab096657fe45897bf036c7e216895bbf':raise RuntimeError('exact unchanged native34f binary differs')
active=subprocess.run(['nvidia-smi','--query-compute-apps=pid','--format=csv,noheader,nounits'],capture_output=True,text=True,check=True,timeout=5).stdout.strip()
if active:raise RuntimeError('exact isolated GPU must be idle before a fresh service')
p=Path(adapter['root'])/'source/tools/v41/serve/serve_ds41.py'
if p.is_symlink() or hashlib.sha256(p.read_bytes()).hexdigest()!='f062d9d39131959da9c981b6704e2ec27f89d8e2806730062dcabc3eb27682ba':raise RuntimeError('exact qualified API f062 differs')
p=Path(baseline['root'])/'server-config.json'
if p.is_symlink() or p.stat().st_size>16384:raise RuntimeError('bounded qualified original configuration differs')
print(json.dumps({'binary':q['binary'],'config':json.loads(p.read_text())}))
'''.replace('@BASE@',repr(baseline)).replace('@BUILD@',repr(build)).replace('@API@',repr(adapter))
result=owned(code)
root=lease()['remote']+'/serve-pool-v108-'+profile
plan=dict(baseline['plan'])
plan.update(schema='aster-xolotl-isolation-plan-v89',binary=result['binary']['path'],binary_sha256=result['binary']['sha256'],
            lease_ends_unix=proof['leaseEndsUnix'],expected_gpu_uuid=proof['gpus'][0]['uuid'],independent_formal_gpu_uuid=json.loads((BASE/'xolotl-stable-service-v97-intent.json').read_text())['gpu_uuid'],prefill_chunk_tokens=512,
            selected_qualified_baseline='Controlled pool-seed9a684/API f062/S512/GPFS/cache2M; admissionseed1, pool0/1/0repeat; no adoption')
env=dict(plan['engine_env'],STRATA_ROWS_REF='0',STRATA_BATCH_SERIAL='0',STRATA_ENGRAM_CACHE_ROWS='2000000',ASTER_XQ_ADMIT_DRAFT_SEED='1',ASTER_XQ_POOL_DRAFT_SEED='1' if profile=='pool_on' else '0')
plan['engine_env']=env
config=result['config']
if config['args'][config['args'].index('--prefill-chunk')+1]!='512' or config['args'][config['args'].index('--ctx')+1]!='1048576':raise RuntimeError('common S512/1M native profile differs')
config.update(exe=plan['binary'],cwd=root,tokenizer=root+'/tokenizer',log=root+'/engine.log')
worker=(BASE/'xolotl_context_1m_worker_v8.frozen.py').read_text()
before="proof['gpus'][0]['uuid']==PLAN['optimization_gpu_uuid']"
if worker.count(before)!=1:raise RuntimeError('exact independent-role hardware guard differs')
worker=worker.replace(before,"(proof['gpus'][0]['uuid']!=PLAN['expected_gpu_uuid'] or proof['gpus'][0]['uuid']==PLAN['independent_formal_gpu_uuid'])")
plan.pop('optimization_gpu_uuid',None)
before="PYTHONPATH=str(deps)+os.pathsep+str(ROOT/'source'),"
if worker.count(before)!=1:raise RuntimeError('exact API import environment anchor differs')
worker=worker.replace(before,"PYTHONPATH=str(deps)+os.pathsep+str(ROOT/'source')+os.pathsep+str(ROOT/'source/tools/v41/serve'),")
before=" STATE.update(context_tokens=PLAN['context_tokens'],phase='starting_api',"
if worker.count(before)!=1:raise RuntimeError('exact API preflight anchor differs')
worker=worker.replace(before," run([sys.executable,str(ROOT/'test_exact_token_cache.py')],'exact-token-cache-tests-v89',30,env)\n run([sys.executable,str(ROOT/'test_strict_native_json.py'),str(ROOT/'source')],'strict-native-json-tests-v89',30,env)\n run([sys.executable,str(ROOT/'test_combined_adapter.py')],'combined-adapter-tests-v89',30,env)\n run([sys.executable,str(ROOT/'test_hybrid_native_json.py'),str(ROOT/'source')],'hybrid-native-json-tests-v89',30,env)\n"+before)
before=" env.update(PLAN['engine_env']);env['STRATA_API_KEY']=key"
if worker.count(before)!=1:raise RuntimeError('exact isolated child environment anchor differs')
worker=worker.replace(before," env.pop('DS41_CHECK_ARENA',None)\n for name in ['STRATA_SERVE_DSPARK','STRATA_DSPARK_DEPTH','STRATA_DSPARK_CONF']:env.pop(name,None)\n"+before)
compile(worker,'bounded-isolation-service-v89','exec')
files={'service-worker.py':worker.encode(),'server-config.json':(json.dumps(config,indent=2)+'\n').encode(),
       'prompt-id-controls.json':(BASE/'xolotl-trial-prompt-ids.json').read_bytes()}
for name in ['test_exact_token_cache.py','test_strict_native_json.py','test_combined_adapter.py','test_hybrid_native_json.py','actual-tool-output.json']:
    raw=(BASE/'api-adapter-v52'/name).read_bytes();files[name]=raw
    plan['inputs']=list(plan['inputs'])+[dict(path=root+'/'+name,bytes=len(raw),sha256=hashlib.sha256(raw).hexdigest(),verify_small_sha256=True)]
for name in ['serve_ds41.py','aster_token_cache.py']:
    raw=(BASE/'api-adapter-v52'/name).read_bytes()
    plan['inputs'].append(dict(path=root+'/source/tools/v41/serve/'+name,bytes=len(raw),sha256=hashlib.sha256(raw).hexdigest(),verify_small_sha256=True))
files['service-plan.json']=(json.dumps(plan,indent=2)+'\n').encode()
record=dict(unix=time.time(),phase='prepared',profile=profile,root=root,source_public_root=adapter['root'],credential_source_root=baseline['root'],
    allocation_job=resource['job'],allocation_uid=resource['job_uid'],replica=resource['replicas'][0]['name'],replica_uid=resource['replicas'][0]['uid'],
    gpu_uuid=proof['gpus'][0]['uuid'],pod_ip=status['podIP'],host_ip=status['hostIP'],plan=plan,credential_file=baseline['credential_file'],
    formal_service_unchanged=True,diagnostic_only=True,actual1m_input_tested=False,performance_qualified=False,
    copy_may_overlap_diagnostics=False,files=[dict(file=name,bytes=len(raw),sha256=hashlib.sha256(raw).hexdigest()) for name,raw in files.items()])
tree=ast.parse((BASE/'launch_xolotl_longctx_shadow_v1.py').read_text())
bodies=[node.value for node in ast.walk(tree) if isinstance(node,ast.Constant) and isinstance(node.value,str) and "root=Path(r['root']);src=Path(r['source_public_root'])" in node.value]
if len(bodies)!=1:raise RuntimeError('exact public source bootstrap differs')
template=bodies[0];before="p=src/'credentials/api-key.txt'"
if template.count(before)!=1:raise RuntimeError('exact authorized private credential handoff differs')
template=template.replace(before,"p=Path(r['credential_source_root'])/'credentials/api-key.txt'")
code=template.replace('@R@',repr(record)).replace('@DATA@',repr({name:base64.b64encode(raw).decode() for name,raw in files.items()}))
compile(code,'exact-independent-isolation-bootstrap-v89','exec')
with intent.open('x') as output:output.write(json.dumps(record,indent=2)+'\n')
record['phase']='launch_requested';intent.write_text(json.dumps(record,indent=2)+'\n')
try:record.update(process=owned(code),phase='launched')
except BaseException:
    record['phase']='launch_outcome_requires_inspection';intent.write_text(json.dumps(record,indent=2)+'\n');raise
intent.write_text(json.dumps(record,indent=2)+'\n')
print(json.dumps(dict(phase=record['phase'],profile=profile,gpu_uuid=record['gpu_uuid'],process=record['process'],draft_enabled='STRATA_SERVE_DSPARK' in env,rows_reference=env['STRATA_ROWS_REF'],slots_serial=env['STRATA_BATCH_SERIAL'],admission_seed=env['ASTER_XQ_ADMIT_DRAFT_SEED']),indent=2))
