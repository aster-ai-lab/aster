"""Recover unchanged stable engine and exact repaired tool protocol; no experimental CUDA source is deployed."""
import ast,base64,hashlib,json,time
from xolotl_trial_common import BASE,jump,lease,observe,owned
from xolotl_sealed_bootstrap_v373 import sealed_owned
import argparse
parser=argparse.ArgumentParser();parser.add_argument('profile',choices=['control_a','guard','control_b']);profile=parser.parse_args().profile
ip=BASE/('xolotl-graph-native-v373-'+profile+'-intent.json')
if ip.exists():raise RuntimeError('exact graph profile handle exists; observe it')
old=json.loads((BASE/'xolotl-stable-service-v368-intent.json').read_text())
verified=json.loads((BASE/'xolotl-C14-serving-v368-verification.json').read_text())
link=json.loads((BASE/'xolotl-graph-native-link-v371-latest.json').read_text())
if verified['phase']!='verified_serving' or link['exact_worker_alive'] or link['result.json']['phase']!='linked' or not link['result.json']['control_byte_equal']:raise RuntimeError('verified repaired source and exact controlled native link required')
binary=link['result.json']['candidate']
if binary['sha256']!='b7124c78d711d3f48b928a292bd6423af4137816533e0acfbeb5114644508185':raise RuntimeError('exact guarded core native differs')
o=observe()
if o['phase']!='Running' or o['job_uid']!='5bc306b2-e9e1-4e25-bd05-d78eca2b4833' or o.get('resource-result.json',{}).get('phase')!='passed':raise RuntimeError('exact C14 resource must pass first')
gpu=o['resource-result.json']['gpus'][0]['uuid']
if gpu==old['gpu_uuid']:raise RuntimeError('formal C14 must remain separate')
root=lease()['remote']+'/serve-graph-native-v373-'+profile
actual=jump('''import base64,hashlib,json
from pathlib import Path
r=@OLD@;root=Path(r['root']);files={}
if root.resolve()!=root or root.stat().st_uid!=10250:raise RuntimeError('exact repaired predecessor root differs')
for row in r['files']:
 p=root/row['file']
 if p.is_symlink() or p.stat().st_size!=row['bytes'] or hashlib.sha256(p.read_bytes()).hexdigest()!=row['sha256']:raise RuntimeError('qualified repaired predecessor file differs')
 if row['file'] not in ['service-plan.json','server-config.json'] and not row['file'].startswith('source/'):files[row['file']]=base64.b64encode(p.read_bytes()).decode()
p=root/'server-config.json'
if p.is_symlink() or p.stat().st_uid!=10250 or p.stat().st_size>16384:raise RuntimeError('bounded actual deployed config required')
required={'source/serve/frontend.py':'11215fa883d935150d5d1eb34d40a25d823880a0b7540befd6e86cac3fc5c557','source/tools/v41/serve/serve_ds41.py':'5834c9c5b1501eb4dcd1b757bbc493ee6326e8220a193de70c43ff8b1044b049','source/tools/v41/serve/aster_tool_roundtrip.py':'fda429021909a14df85d3752ceb226199e4ba5c016b19b4450fc1eadca7404af'}
for name,want in required.items():
 p=root/name
 if p.is_symlink() or hashlib.sha256(p.read_bytes()).hexdigest()!=want:raise RuntimeError('deployed repaired source differs')
print(json.dumps(dict(files=files,plan=json.loads((root/'service-plan.json').read_text()),config=json.loads((root/'server-config.json').read_text()),repaired_sources=required)))
'''.replace('@OLD@',repr(old)))
plan=actual['plan']
if plan['binary_sha256']!='fa0195a9b64f5e18136eec6852997cd0637f3197e9b6b97e4f512c5f50d5f373':raise RuntimeError('unchanged qualified stable native required')
plan.update(schema='aster-graph-native-v373',binary=binary['path'],binary_sha256=binary['sha256'],lease_ends_unix=o['resource-result.json']['leaseEndsUnix'],
 expected_gpu_uuid=gpu,independent_formal_gpu_uuid=old['gpu_uuid'],prefill_chunk_tokens=512,
 selected_qualified_baseline='C15 same native b7124 graph-type guard0/1/0; repaired API5834/frontend11215; TopK1, S512, 1M, DSpark5/.45, GPFS2M, admission1/pool1 constant; no formal promotion')
plan.pop('optimization_gpu_uuid',None)
plan['engine_env'].update(STRATA_ROWS_REF='0',STRATA_BATCH_SERIAL='0',STRATA_ENGRAM_CACHE_ROWS='2000000',ASTER_XQ_ADMIT_DRAFT_SEED='1',ASTER_XQ_POOL_DRAFT_SEED='1',
 ASTER_XQ_PREFILL_PHASE_TRACE='1',ASTER_XQ_TOPK35='1',ASTER_XQ_DSPARK_CUM='0',ASTER_XQ_GRAPH_SOURCE_TYPES=('1' if profile=='guard' else '0'),ASTER_XQ_INDEXER_MXFP4_HEADER='0')
for row in plan['inputs']:
 if row['path'].startswith(old['root']+'/'):row['path']=root+row['path'][len(old['root']):]
config=actual['config'];config['args'][config['args'].index('--prefill-chunk')+1]='512';config.update(exe=plan['binary'],cwd=root,tokenizer=root+'/tokenizer',log=root+'/engine.log')
if config['args'][config['args'].index('--ctx')+1]!='1048576' or config['args'][config['args'].index('--prefill-chunk')+1]!='512':raise RuntimeError('stable1M/S128 arguments differ')
files={n:base64.b64decode(b) for n,b in actual['files'].items()}
worker=files['service-worker.py'].decode()
before="proof['gpus'][0]['uuid']==PLAN['optimization_gpu_uuid']"
if worker.count(before)!=1:raise RuntimeError('exact owned role guard differs')
worker=worker.replace(before,"(proof['gpus'][0]['uuid']!=PLAN['expected_gpu_uuid'] or proof['gpus'][0]['uuid']==PLAN['independent_formal_gpu_uuid'])")
before=" env.pop('DS41_CHECK_ARENA',None)"
if worker.count(before)!=1:raise RuntimeError('exact clean native graph environment anchor differs')
worker=worker.replace(before,before+"\n env.pop('GGML_CUDA_DISABLE_GRAPHS',None)")
compile(worker,'graph-native-service-v373-worker','exec');files['service-worker.py']=worker.encode()

files.update({'service-plan.json':(json.dumps(plan,indent=2)+'\n').encode(),'server-config.json':(json.dumps(config,indent=2)+'\n').encode()})
record=dict(unix=time.time(),phase='prepared',profile=profile,root=root,source_public_root=old['root'],credential_source_root=old['root'],
 allocation_job=o['job'],allocation_uid=o['job_uid'],replica=o['replicas'][0]['name'],replica_uid=o['replicas'][0]['uid'],gpu_uuid=gpu,
 pod_ip=o['replicas'][0]['status']['podIP'],host_ip=o['replicas'][0]['status']['hostIP'],plan=plan,credential_file=old['credential_file'],same_existing_key=True,
 engine_profile_full1m_evidence='xolotl-indexer-fullwindow-suite-v45-latest.json',instance_full_1m_execution_tested=False,
 repaired_sources=actual['repaired_sources'],performance_candidate_promoted=False,
 files=[dict(file=n,bytes=len(b),sha256=hashlib.sha256(b).hexdigest()) for n,b in files.items()])
tree=ast.parse((BASE/'launch_xolotl_longctx_shadow_v1.py').read_text())
templates=[n.value for n in ast.walk(tree) if isinstance(n,ast.Constant) and isinstance(n.value,str) and "root=Path(r['root']);src=Path(r['source_public_root'])" in n.value]
if len(templates)!=1:raise RuntimeError('exact qualified public bootstrap required')
code=templates[0].replace('@R@',repr(record)).replace('@DATA@',repr({n:base64.b64encode(b).decode() for n,b in files.items()}))
compile(code,'repaired-stable-C14-v355-bootstrap','exec')
with ip.open('x') as f:f.write(json.dumps(record,indent=2)+'\n')
record['phase']='launch_requested';ip.write_text(json.dumps(record,indent=2)+'\n')
try:record.update(process=sealed_owned(code,'graph-native-v373-'+profile),phase='launched')
except BaseException:record['phase']='launch_outcome_requires_inspection';ip.write_text(json.dumps(record,indent=2)+'\n');raise
ip.write_text(json.dumps(record,indent=2)+'\n')
print(json.dumps(dict(phase=record['phase'],GPU=gpu,process=record['process'],same_binary_across_profiles=True,repaired_tool_sources_preserved=True,same_key=True),indent=2))
