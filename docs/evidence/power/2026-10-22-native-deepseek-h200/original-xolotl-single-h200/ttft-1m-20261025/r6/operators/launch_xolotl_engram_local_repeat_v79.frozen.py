"""Same-native-binary cumulative draft comparison, with one common validated API adapter."""
import argparse
import ast
import base64
import hashlib
import json
import time
from xolotl_trial_common import BASE, jump, lease, observe, owned

parser = argparse.ArgumentParser()
parser.add_argument('profile', choices=['local_repeat'])
profile = parser.parse_args().profile
modes = {'local_repeat':'0'}
intent = BASE / ('xolotl-engram-local-repeat-v79-' + profile + '-intent.json')
if intent.exists():
    raise RuntimeError('V50 service has an exact handle; observe it')
baseline = json.loads((BASE / 'xolotl-engram-disk-v68-local-intent.json').read_text())
build = json.loads((BASE / 'xolotl-draft-cumulative-v49-native-link-intent.json').read_text())
adapter = json.loads((BASE / 'xolotl-api-hybrid-v52-intent.json').read_text())
cpu = json.loads((BASE / 'xolotl-api-hybrid-v52-latest.json').read_text())
if cpu['cpu-preflight-result.json']['phase'] != 'passed' or cpu['exact_cpu_process_alive']:
    raise RuntimeError('common adapter CPU qualification must pass and finish')
observation = observe()
if observation['job_uid'] != baseline['allocation_uid'] or observation['job_uid'] != build['allocation_uid']:
    raise RuntimeError('exact independent cumulative comparison allocation differs')
code = '''import hashlib,json,subprocess
from pathlib import Path
baseline=@BASELINE@;build=@BUILD@;adapter=@ADAPTER@
root=Path(build['root']);q=json.loads((root/'native-link-result.json').read_text());p=Path('/proc/'+str(build['process']['pid'])+'/stat')
alive=p.exists() and p.stat().st_uid==10250 and int(p.read_text().split(') ',1)[1].split()[19])==build['process']['start_ticks'] and p.read_text().split(') ',1)[1].split()[0]!='Z'
if alive or q['phase']!='linked' or not q['control_binary_byte_equal'] or q['invalid_startup_modes_rejected']!=8:raise RuntimeError('exact controlled native link must pass and finish')
active=subprocess.run(['nvidia-smi','--query-compute-apps=pid','--format=csv,noheader,nounits'],capture_output=True,text=True,check=True,timeout=5).stdout.strip()
if active:raise RuntimeError('cumulative comparison GPU must be idle')
p=Path(adapter['root'])/'source/tools/v41/serve/serve_ds41.py'
if p.is_symlink() or hashlib.sha256(p.read_bytes()).hexdigest()!='f062d9d39131959da9c981b6704e2ec27f89d8e2806730062dcabc3eb27682ba':raise RuntimeError('exact common API adapter differs')
p=Path(baseline['root'])/'server-config.json'
if p.is_symlink() or p.stat().st_size>16384:raise RuntimeError('bounded qualified sub-batch config differs')
print(json.dumps({'built':q,'config':json.loads(p.read_text())}))
'''.replace('@BASELINE@', repr(baseline)).replace('@BUILD@', repr(build)).replace('@ADAPTER@', repr(adapter))
result = owned(code)
root = lease()['remote'] + '/serve-engram-local-repeat-v79-' + profile
plan = dict(baseline['plan'])
plan.update(schema='aster-xolotl-engram-disk-plan-v68', binary=result['built']['binary']['path'],
            binary_sha256=result['built']['binary']['sha256'], prefill_chunk_tokens=512,
            selected_qualified_baseline='Same native34f/API f062/S512/memo1; V56 actual1M-plus1024 single and V66 replay qualified')
plan['engine_env'] = dict(plan['engine_env'], ASTER_XQ_DSPARK_CUM=modes[profile], ASTER_XQ_EXACT_TOKEN_CACHE='1')
config = result['config']
plan['engine_env']['STRATA_ENGRAM_CACHE_ROWS']='2000000'
if profile=='local_repeat':
    staging=json.loads((BASE/'xolotl-engram-local-copy-v63-intent.json').read_text())
    check_code='''import hashlib,json
from pathlib import Path
root=Path(@LOCAL@)
if root.parent!=Path('/var/tmp') or root.resolve()!=root or root.stat().st_uid!=10250:raise RuntimeError('exact local Engram directory owner differs')
q=json.loads((root/'complete.json').read_text())
if q['phase']!='passed' or not q['all_full_sha256_readbacks_passed'] or not q['exact_original_weight_bytes']:raise RuntimeError('both local Engram full SHA gates must pass')
expected=@ROWS@
for row in expected:
 p=root/(row['expected_sha256']+'.safetensors')
 proofs=[x for x in q['files'] if x['file']==p.name]
 if p.is_symlink() or p.stat().st_uid!=10250 or p.stat().st_size!=row['bytes'] or len(proofs)!=1 or proofs[0]['local_sha256']!=row['expected_sha256'] or p.stat().st_mtime>q['finished_unix']:raise RuntimeError('exact verified local Engram blob differs')
p=root/'model.safetensors.index.json'
if p.is_symlink() or p.stat().st_size>4096 or hashlib.sha256(p.read_bytes()).hexdigest()!=q['source_index_sha256']:raise RuntimeError('exact original local Engram index differs')
print(json.dumps({'directory':str(root),'full_sha256_qualified':True}))
'''.replace('@LOCAL@',repr(staging['plan']['local_root'])).replace('@ROWS@',repr(staging['plan']['blobs']))
    local=owned(check_code)
    config['args'][config['args'].index('--engram')+1]=local['directory']

if config['args'][config['args'].index('--prefill-chunk') + 1] != '512':
    raise RuntimeError('same512 attention sub-batch control differs')
config.update(exe=plan['binary'], cwd=root, tokenizer=root + '/tokenizer', log=root + '/engine.log')
worker = (BASE / 'xolotl_context_1m_worker_v8.frozen.py').read_text()
anchor = "PYTHONPATH=str(deps)+os.pathsep+str(ROOT/'source'),"
if worker.count(anchor) != 1:
    raise RuntimeError('exact API preflight import environment differs')
worker = worker.replace(anchor, "PYTHONPATH=str(deps)+os.pathsep+str(ROOT/'source')+os.pathsep+str(ROOT/'source/tools/v41/serve'),")
anchor = " STATE.update(context_tokens=PLAN['context_tokens'],phase='starting_api',"
if worker.count(anchor) != 1:
    raise RuntimeError('exact API qualification insertion differs')
worker = worker.replace(anchor, " run([sys.executable,str(ROOT/'test_exact_token_cache.py')],'exact-token-cache-tests-v50',30,env)\n run([sys.executable,str(ROOT/'test_strict_native_json.py'),str(ROOT/'source')],'strict-native-json-tests-v50',30,env)\n run([sys.executable,str(ROOT/'test_combined_adapter.py')],'combined-adapter-tests-v50',30,env)\n run([sys.executable,str(ROOT/'test_hybrid_native_json.py'),str(ROOT/'source')],'hybrid-native-json-tests-v53',30,env)\n" + anchor)
anchor = " env.update(PLAN['engine_env']);env['STRATA_API_KEY']=key"
if worker.count(anchor) != 1:
    raise RuntimeError('exact engine environment handoff differs')
worker = worker.replace(anchor, " env.pop('DS41_CHECK_ARENA',None)\n" + anchor)
compile(worker, 'cumulative-v50-service-worker', 'exec')
files = {'service-worker.py': worker.encode(), 'server-config.json': (json.dumps(config, indent=2) + '\n').encode(),
         'prompt-id-controls.json': (BASE / 'xolotl-trial-prompt-ids.json').read_bytes()}
for name in ['test_exact_token_cache.py', 'test_strict_native_json.py', 'test_combined_adapter.py', 'test_hybrid_native_json.py', 'actual-tool-output.json']:
    raw = (BASE / 'api-adapter-v52' / name).read_bytes()
    files[name] = raw
    plan['inputs'] = list(plan['inputs']) + [dict(path=root + '/' + name, bytes=len(raw),
        sha256=hashlib.sha256(raw).hexdigest(), verify_small_sha256=True)]
for name in ['serve_ds41.py', 'aster_token_cache.py']:
    raw = (BASE / 'api-adapter-v52' / name).read_bytes()
    plan['inputs'].append(dict(path=root + '/source/tools/v41/serve/' + name, bytes=len(raw),
        sha256=hashlib.sha256(raw).hexdigest(), verify_small_sha256=True))
files['service-plan.json'] = (json.dumps(plan, indent=2) + '\n').encode()
record = dict(unix=time.time(), phase='prepared', profile=profile, root=root, source_public_root=adapter['root'],
    credential_source_root=baseline['root'], allocation_job=observation['job'], allocation_uid=observation['job_uid'],
    replica=observation['replicas'][0]['name'], replica_uid=observation['replicas'][0]['uid'],
    gpu_uuid=baseline['gpu_uuid'], pod_ip=baseline['pod_ip'], host_ip=baseline['host_ip'], plan=plan,
    credential_file=baseline['credential_file'], formal_service_unchanged=True, full1m_qualified=False,
    changed_comparison_control='Repeat native34f local-table control after LUT; fixed seven natural requests, same S512/API f062/memo1/cache_rows2000000',
    files=[dict(file=n, bytes=len(v), sha256=hashlib.sha256(v).hexdigest()) for n, v in files.items()])
tree = ast.parse((BASE / 'launch_xolotl_longctx_shadow_v1.py').read_text())
bodies = [node.value for node in ast.walk(tree) if isinstance(node, ast.Constant) and isinstance(node.value, str)
          and "root=Path(r['root']);src=Path(r['source_public_root'])" in node.value]
if len(bodies) != 1:
    raise RuntimeError('bounded public source bootstrap differs')
template = bodies[0]
anchor = "p=src/'credentials/api-key.txt'"
if template.count(anchor) != 1:
    raise RuntimeError('exact existing credential transfer anchor differs')
template = template.replace(anchor, "p=Path(r['credential_source_root'])/'credentials/api-key.txt'")
anchor = "if deadline<1800:raise RuntimeError('shadow context qualification deadline insufficient')"
if template.count(anchor)!=1:raise RuntimeError("exact finite repeat startup guard differs")
template=template.replace(anchor,"if deadline<720:raise RuntimeError('finite7-request repeat deadline insufficient')")
code = template.replace('@R@', repr(record)).replace('@DATA@', repr({n: base64.b64encode(v).decode() for n, v in files.items()}))
compile(code, 'cumulative-v50-bootstrap', 'exec')
with intent.open('x') as output:
    output.write(json.dumps(record, indent=2) + '\n')
record['phase'] = 'launch_requested'
intent.write_text(json.dumps(record, indent=2) + '\n')
try:
    record['process'] = owned(code)
    record['phase'] = 'launched'
except BaseException:
    record['phase'] = 'launch_outcome_requires_inspection'
    intent.write_text(json.dumps(record, indent=2) + '\n')
    raise
intent.write_text(json.dumps(record, indent=2) + '\n')
print(json.dumps(dict(phase=record['phase'], profile=profile, native_sha256=plan['binary_sha256'],
    common_api_sha256='f062d9d39131959da9c981b6704e2ec27f89d8e2806730062dcabc3eb27682ba',
    cumulative_cutoff=modes[profile], formal_service_unchanged=True)))
