"""Recover the same VIP/key on one replacement H200 using the actual1M-qualified V44 profile."""
import ast
import base64
import hashlib
import json
import time
from xolotl_cont8_common_v58 import BASE, lease, observe, owned

intent = BASE / 'xolotl-stable-recovery-v58-stable-intent.json'
if intent.exists():
    raise RuntimeError('replacement stable service has an exact handle; observe it')
baseline = json.loads((BASE / 'xolotl-indexer-service-v44-intent.json').read_text())
old = json.loads((BASE / 'xolotl-stable-service-v31-intent.json').read_text())
adapter = json.loads((BASE / 'xolotl-api-hybrid-v52-intent.json').read_text())
proof = json.loads((BASE / 'xolotl-indexer-fullwindow-suite-v45-latest.json').read_text())
parent = json.loads((BASE / 'xolotl-stable-v58-public-parent.json').read_text())
if (proof['suite-result.json']['phase'] != 'passed' or proof['exact_client_alive']
        or not proof['fullwindow/request-result.json']['full1m_cache_restore_passed']):
    raise RuntimeError('exact native V44 actual1M profile must be qualified and finished')
resource = observe()
if resource['phase'] != 'Running' or resource.get('resource-result.json', {}).get('phase') != 'passed':
    raise RuntimeError('exact replacement resource must pass the H200/CUDA probe first')
gpu = resource['resource-result.json']['gpus'][0]['uuid']
optimization = json.loads((BASE / 'xolotl-draft-service-v53-off_repeat-intent.json').read_text())
if gpu == optimization['gpu_uuid'] or resource['job_uid'] == optimization['allocation_uid']:
    raise RuntimeError('independent optimization GPU must remain separate')
if parent['config']['args'][parent['config']['args'].index('--prefill-chunk') + 1] != '128':
    raise RuntimeError('actual1M-qualified attention sub-batch must stay128')
code = '''import json,subprocess
p=subprocess.run(['nvidia-smi','--query-compute-apps=pid','--format=csv,noheader,nounits'],capture_output=True,text=True,timeout=5,check=True)
if p.stdout.strip():raise RuntimeError('replacement stable GPU must be idle')
print(json.dumps({'idle':True}))
'''
owned(code)
root = lease()['remote'] + '/serve-stable-recovery-v58'
plan = dict(baseline['plan'])
plan.update(schema='aster-xolotl-stable-recovery-plan-v58',
            lease_ends_unix=resource['resource-result.json']['leaseEndsUnix'],
            optimization_gpu_uuid=optimization['gpu_uuid'], prefill_chunk_tokens=128,
            selected_qualified_baseline='V44/V45 actual1M exact mask-skip profile; corrected strict API f062 and exact token memo1')
plan['engine_env'] = dict(baseline['plan']['engine_env'], ASTER_XQ_EXACT_TOKEN_CACHE='1')
plan['engine_env'].pop('DS41_CHECK_ARENA', None)
config = parent['config']
config.update(exe=plan['binary'], cwd=root, tokenizer=root + '/tokenizer', log=root + '/engine.log')
files = {'service-worker.py': (BASE / 'xolotl_qualified_api_worker_v53.frozen.py').read_bytes(),
         'server-config.json': (json.dumps(config, indent=2) + '\n').encode(),
         'prompt-id-controls.json': (BASE / 'xolotl-trial-prompt-ids.json').read_bytes()}
if hashlib.sha256(files['service-worker.py']).hexdigest() != parent['worker_sha256']:
    raise RuntimeError('exact GPU-qualified common service worker differs')
for name in ['test_exact_token_cache.py', 'test_strict_native_json.py', 'test_combined_adapter.py',
             'test_hybrid_native_json.py', 'actual-tool-output.json']:
    raw = (BASE / 'api-adapter-v52' / name).read_bytes()
    files[name] = raw
    plan['inputs'] = list(plan['inputs']) + [dict(path=root + '/' + name, bytes=len(raw),
        sha256=hashlib.sha256(raw).hexdigest(), verify_small_sha256=True)]
for name in ['serve_ds41.py', 'aster_token_cache.py']:
    raw = (BASE / 'api-adapter-v52' / name).read_bytes()
    plan['inputs'].append(dict(path=root + '/source/tools/v41/serve/' + name, bytes=len(raw),
        sha256=hashlib.sha256(raw).hexdigest(), verify_small_sha256=True))
files['service-plan.json'] = (json.dumps(plan, indent=2) + '\n').encode()
record = dict(unix=time.time(), phase='prepared', profile='stable', root=root,
    source_public_root=adapter['root'], credential_source_root=old['root'],
    allocation_job=resource['job'], allocation_uid=resource['job_uid'],
    replica=resource['replicas'][0]['name'], replica_uid=resource['replicas'][0]['uid'],
    gpu_uuid=gpu, pod_ip=resource['replicas'][0]['status']['podIP'],
    host_ip=resource['replicas'][0]['status']['hostIP'], plan=plan,
    credential_file=old['credential_file'], same_existing_key=True,
    engine_profile_full1m_evidence='xolotl-indexer-fullwindow-suite-v45-latest.json',
    instance_full_1m_execution_tested=False,
    files=[dict(file=n, bytes=len(v), sha256=hashlib.sha256(v).hexdigest()) for n, v in files.items()])
tree = ast.parse((BASE / 'launch_xolotl_longctx_shadow_v1.py').read_text())
bodies = [node.value for node in ast.walk(tree) if isinstance(node, ast.Constant) and isinstance(node.value, str)
          and "root=Path(r['root']);src=Path(r['source_public_root'])" in node.value]
if len(bodies) != 1 or bodies[0].count("p=src/'credentials/api-key.txt'") != 1:
    raise RuntimeError('exact public/credential bootstrap differs')
template = bodies[0].replace("p=src/'credentials/api-key.txt'",
                            "p=Path(r['credential_source_root'])/'credentials/api-key.txt'")
code = template.replace('@R@', repr(record)).replace('@DATA@', repr({n: base64.b64encode(v).decode() for n, v in files.items()}))
compile(code, 'stable-recovery-v58-bootstrap', 'exec')
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
print(json.dumps(dict(phase=record['phase'], gpu_uuid=gpu, same_existing_key=True,
                      native_sha256=plan['binary_sha256'], prefill_chunk=128)))
