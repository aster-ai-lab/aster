"""Prepare same native graph guard0/1/0 on C15 using the verified repaired C14 API."""
import ast,hashlib,json
from xolotl_trial_common import BASE
def read(n):return json.loads((BASE/n).read_text())
link=read('xolotl-graph-native-link-v371-latest.json')
if link['exact_worker_alive'] or link['result.json']['phase']!='linked' or not link['result.json']['control_byte_equal']:raise RuntimeError('completed exact unchanged native control relink required')
source=(BASE/'launch_c14_repaired_service_v357.py').read_text()
def change(before,after):
 global source
 if source.count(before)!=1:raise RuntimeError('closed graph profile anchor differs: '+before[:100])
 source=source.replace(before,after)
change('from xolotl_recovery_common_v354 import BASE,jump,lease,observe,owned','from xolotl_trial_common import BASE,jump,lease,observe,owned')
change('from xolotl_sealed_bootstrap_v354 import sealed_owned','from xolotl_sealed_bootstrap_v373 import sealed_owned')
start=source.index("ip=BASE/'xolotl-stable-recovery-v354-stable-intent.json'")
end=source.index('o=observe()',start)
source=source[:start]+'''import argparse
parser=argparse.ArgumentParser();parser.add_argument('profile',choices=['control_a','guard','control_b']);profile=parser.parse_args().profile
ip=BASE/('xolotl-graph-native-v373-'+profile+'-intent.json')
if ip.exists():raise RuntimeError('exact graph profile handle exists; observe it')
old=json.loads((BASE/'xolotl-stable-service-v368-intent.json').read_text())
verified=json.loads((BASE/'xolotl-C14-serving-v368-verification.json').read_text())
link=json.loads((BASE/'xolotl-graph-native-link-v371-latest.json').read_text())
if verified['phase']!='verified_serving' or link['exact_worker_alive'] or link['result.json']['phase']!='linked' or not link['result.json']['control_byte_equal']:raise RuntimeError('verified repaired source and exact controlled native link required')
binary=link['result.json']['candidate']
if binary['sha256']!='b7124c78d711d3f48b928a292bd6423af4137816533e0acfbeb5114644508185':raise RuntimeError('exact guarded core native differs')
'''+source[end:]
change("o['job_uid']!='bd638591-0ba6-4c60-9eb6-129a531714ee'","o['job_uid']!='5bc306b2-e9e1-4e25-bd05-d78eca2b4833'")
change("if gpu=='GPU-099e8da4-8df0-c431-9e67-eb721e59beee':raise RuntimeError('independent live C12 GPU must remain separate')","if gpu==old['gpu_uuid']:raise RuntimeError('formal C14 must remain separate')")
change("root=lease()['remote']+'/serve-stable-recovery-v354'","root=lease()['remote']+'/serve-graph-native-v373-'+profile")
start=source.index('plan.update(lease_ends_unix=')
end=source.index("for row in plan['inputs']:",start)
source=source[:start]+'''plan.update(schema='aster-graph-native-v373',binary=binary['path'],binary_sha256=binary['sha256'],lease_ends_unix=o['resource-result.json']['leaseEndsUnix'],
 expected_gpu_uuid=gpu,independent_formal_gpu_uuid=old['gpu_uuid'],prefill_chunk_tokens=512,
 selected_qualified_baseline='C15 same native b7124 graph-type guard0/1/0; repaired API5834/frontend11215; TopK1, S512, 1M, DSpark5/.45, GPFS2M, admission1/pool1 constant; no formal promotion')
plan.pop('optimization_gpu_uuid',None)
plan['engine_env'].update(STRATA_ROWS_REF='0',STRATA_BATCH_SERIAL='0',STRATA_ENGRAM_CACHE_ROWS='2000000',ASTER_XQ_ADMIT_DRAFT_SEED='1',ASTER_XQ_POOL_DRAFT_SEED='1',
 ASTER_XQ_PREFILL_PHASE_TRACE='1',ASTER_XQ_TOPK35='1',ASTER_XQ_DSPARK_CUM='0',ASTER_XQ_GRAPH_SOURCE_TYPES=('1' if profile=='guard' else '0'),ASTER_XQ_INDEXER_MXFP4_HEADER='0')
'''+source[end:]
change("config=actual['config'];config.update(exe=plan['binary'],cwd=root,tokenizer=root+'/tokenizer',log=root+'/engine.log')","config=actual['config'];config['args'][config['args'].index('--prefill-chunk')+1]='512';config.update(exe=plan['binary'],cwd=root,tokenizer=root+'/tokenizer',log=root+'/engine.log')")
change("config['args'][config['args'].index('--prefill-chunk')+1]!='128'","config['args'][config['args'].index('--prefill-chunk')+1]!='512'")
before="files={n:base64.b64decode(b) for n,b in actual['files'].items()}"
after=before+'''
worker=files['service-worker.py'].decode()
before="proof['gpus'][0]['uuid']==PLAN['optimization_gpu_uuid']"
if worker.count(before)!=1:raise RuntimeError('exact owned role guard differs')
worker=worker.replace(before,"(proof['gpus'][0]['uuid']!=PLAN['expected_gpu_uuid'] or proof['gpus'][0]['uuid']==PLAN['independent_formal_gpu_uuid'])")
before=" env.pop('DS41_CHECK_ARENA',None)"
if worker.count(before)!=1:raise RuntimeError('exact clean native graph environment anchor differs')
worker=worker.replace(before,before+"\\n env.pop('GGML_CUDA_DISABLE_GRAPHS',None)")
compile(worker,'graph-native-service-v373-worker','exec');files['service-worker.py']=worker.encode()
'''
change(before,after)
change("profile='stable'","profile=profile")
change("sealed_owned(code,'stable-recovery-v354')","sealed_owned(code,'graph-native-v373-'+profile)")
source=source.replace('same_native=True','same_binary_across_profiles=True')
files={'launch_graph_native_service_v373.frozen.py':source}
sealed=(BASE/'xolotl_sealed_bootstrap_v212.py').read_text().replace("LABELS={'topk-native-v212-control_a','topk-native-v212-topk','topk-native-v212-control_b'}","LABELS={'graph-native-v373-control_a','graph-native-v373-guard','graph-native-v373-control_b'}")
files['xolotl_sealed_bootstrap_v373.py']=sealed
for old,new in [('manage_topk_native_basic_v212.frozen.py','manage_graph_native_basic_v373.frozen.py'),('manage_topk_native_extended_v212.frozen.py','manage_graph_native_extended_v373.frozen.py')]:
 raw=(BASE/old).read_text().replace("choices=['control_a','topk','control_b']","choices=['control_a','guard','control_b']").replace('topk-native','graph-native').replace('v212','v373')
 files[new]=raw
raw=(BASE/'manage_topk_fullwindow_suite_v214.py').read_text().replace("choices=['control_a','topk','control_b']","choices=['control_a','guard','control_b']").replace('topk-native','graph-native').replace('v212','v373').replace('topk-fullwindow','graph-fullwindow').replace('canonical-topk','canonical-graph').replace('v214','v374')
raw=raw.replace('f062d9d39131959da9c981b6704e2ec27f89d8e2806730062dcabc3eb27682ba','5834c9c5b1501eb4dcd1b757bbc493ee6326e8220a193de70c43ff8b1044b049')
files['manage_graph_fullwindow_suite_v374.py']=raw
rows=[]
for name,text in files.items():
 ast.parse(text)
 with (BASE/name).open('x') as f:f.write(text)
 raw=(BASE/name).read_bytes();rows.append(dict(file=name,bytes=len(raw),sha256=hashlib.sha256(raw).hexdigest()))
with (BASE/'xolotl-graph-model-profiles-v373-prepared.json').open('x') as f:f.write(json.dumps(dict(files=rows,profiles=['control_a','guard','control_b'],flag_modes=['0','1','0'],binary=link['result.json']['candidate'],ordinary_acceptance_requests=40,fullwindow_calls_per_profile=20,actual1M_calls_per_profile=18,full_output_tokens=1024,model_acceptance_changed=False,formal_changed=False),indent=2)+'\n')
print(json.dumps(dict(prepared=True,files=len(files),same_binary_guard_modes=['0','1','0'],formal_changed=False)))
