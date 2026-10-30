"""Qualify checked mixed-cache retention plus exact tokenizer memo on a separate H200."""
import ast, base64, hashlib, json, time
from xolotl_trial_common import BASE, lease, observe, owned, jump

ip = BASE / 'xolotl-cache-memo-v23-intent.json'
if ip.exists():
    raise RuntimeError('V23 service intent exists; observe its exact supervisor')
stable = json.loads((BASE / 'xolotl-stable-service-v15-intent.json').read_text())
observation = observe()
proof = observation.get('resource-result.json', {})
if (observation['phase'] != 'Running' or proof.get('phase') != 'passed' or proof['expectedGpus'] != 1 or
        observation['job_uid'] == stable['allocation_uid'] or proof['gpus'][0]['uuid'] == stable['gpu_uuid']):
    raise RuntimeError('independent verified single-H200 optimization allocation required')
gpu = proof['gpus'][0]['uuid']
gate = json.loads((BASE / 'xolotl-mixed-cache-new-device-v24-result.json').read_text())
cpu = json.loads((BASE / 'xolotl-exact-tokenizer-v21-latest.json').read_text())
stage = json.loads((BASE / 'xolotl-exact-tokenizer-v21-intent.json').read_text())
build = json.loads((BASE / 'xolotl-mixed-keep-build-v19-latest.json').read_text())['result.json']
if gate['phase'] != 'passed' or gate['gpu_uuid'] != gpu:
    raise RuntimeError('fresh physical GPU cache byte gate must pass')
if cpu['cpu-preflight-result.json']['phase'] != 'passed' or cpu['real-tokenizer-benchmark.json']['phase'] != 'passed' or cpu['exact_cpu_client_alive']:
    raise RuntimeError('exact full-window tokenizer CPU gates must pass and finish')
if build['phase'] != 'built' or not build['cache_policy_tests_passed'] or build['control_binary_sha256'] != stable['plan']['binary_sha256']:
    raise RuntimeError('controlled V19 cache build differs from qualified V15 baseline')
code = '''import json,subprocess
from pathlib import Path
active=subprocess.run(['nvidia-smi','--query-compute-apps=pid','--format=csv,noheader,nounits'],capture_output=True,text=True,check=True,timeout=8).stdout.strip()
if active:raise RuntimeError('new isolated optimization GPU already has a workload')
p=Path(@CONFIG@)
if p.is_symlink() or p.stat().st_uid!=10250 or p.stat().st_size>16384:raise RuntimeError('qualified public server config differs')
print(json.dumps(json.loads(p.read_text())))
'''.replace('@CONFIG@', repr(stable['root'] + '/server-config.json'))
config = owned(code)
root = lease()['remote'] + '/serve-cache-memo-v23'
plan = dict(stable['plan'])
plan.update(schema='aster-xolotl-checked-cache-exact-tokenizer-v23', lease_ends_unix=proof['leaseEndsUnix'],
            optimization_gpu_uuid=stable['gpu_uuid'], binary=build['binary']['path'], binary_sha256=build['binary']['sha256'],
            context_tokens=1048576, prefill_batch_tokens=8192, source_overlay=stage['source_overlay'])
plan['engine_env'] = dict(plan['engine_env'], STRATA_SERVE_KEEP_CACHE='1', STRATA_SERVE_KEEP_MAX='1048576',
                          STRATA_SERVE_KEEP_LRU='1', ASTER_XQ_KEEP_MIXED='1', ASTER_XQ_PREFILL_QOS='1',
                          STRATA_SERVE_BATCH_TAIL='1', DS41_CHECK_ARENA='1', STRATA_DENSE_PROF='1', STRATA_TEXT_LOG='0')
config['exe'] = plan['binary']
config.update(cwd=root, tokenizer=root + '/tokenizer', log=root + '/engine.log', parallel=2)
worker = (BASE / 'xolotl_context_1m_worker_v8.frozen.py').read_text()
anchor = " STATE.update(context_tokens=PLAN['context_tokens'],phase='starting_api',"
if worker.count(anchor) != 1:
    raise RuntimeError('exact V23 CPU memo regression gate insertion differs')
worker = worker.replace(anchor, " run([sys.executable,str(ROOT/'source/tools/v41/serve/test_exact_token_cache.py')],'exact-token-cache-tests-v23',30,env)\n" + anchor)
ast.parse(worker)
tests = (BASE / 'tokenizer-v21/test_exact_token_cache.py').read_bytes()
plan['inputs'] = list(plan['inputs']) + [dict(path=root + '/source/tools/v41/serve/test_exact_token_cache.py', bytes=len(tests),
                                            sha256=hashlib.sha256(tests).hexdigest(), verify_small_sha256=True)]
payload = {'service-worker.py': worker.encode(), 'service-plan.json': (json.dumps(plan, indent=2) + '\n').encode(),
           'server-config.json': (json.dumps(config, indent=2) + '\n').encode(),
           'prompt-id-controls.json': (BASE / 'xolotl-trial-prompt-ids.json').read_bytes(),
           'source/tools/v41/serve/test_exact_token_cache.py': tests}
r = dict(unix=time.time(), phase='prepared', root=root, source_public_root=stage['root'],
         allocation_job=observation['job'], allocation_uid=observation['job_uid'],
         replica=observation['replicas'][0]['name'], replica_uid=observation['replicas'][0]['uid'],
         gpu_uuid=gpu, pod_ip=observation['replicas'][0]['status']['podIP'], host_ip=observation['replicas'][0]['status']['hostIP'],
         plan=plan, credential_file=stable['credential_file'], credential_source_root=stable['root'],
         purpose='Validate real checked-cache retention and exact tokenizer memo; public V15 VIP unchanged until qualification',
         files=[dict(file=name, bytes=len(raw), sha256=hashlib.sha256(raw).hexdigest()) for name, raw in payload.items()])
tree = ast.parse((BASE / 'launch_xolotl_longctx_shadow_v1.py').read_text())
bodies = [node.value for node in ast.walk(tree) if isinstance(node, ast.Constant) and isinstance(node.value, str)
          and "root=Path(r['root']);src=Path(r['source_public_root'])" in node.value]
if len(bodies) != 1:
    raise RuntimeError('bounded public source staging bootstrap differs')
template = bodies[0]
credential_anchor = "p=src/'credentials/api-key.txt'"
if template.count(credential_anchor) != 1:
    raise RuntimeError('exact private credential transfer anchor differs')
template = template.replace(credential_anchor, "p=Path(r['credential_source_root'])/'credentials/api-key.txt'")
code = template.replace('@R@', repr(r)).replace('@DATA@', repr({name: base64.b64encode(raw).decode() for name, raw in payload.items()}))
compile(code, 'checked-cache-exact-tokenizer-V23-bootstrap', 'exec')
r['phase'] = 'launch_requested'
with ip.open('x') as stream:
    stream.write(json.dumps(r, indent=2) + '\n')
try:
    r['process'] = owned(code)
    r['phase'] = 'launched'
except BaseException:
    r['phase'] = 'launch_outcome_requires_inspection'
    ip.write_text(json.dumps(r, indent=2) + '\n')
    raise
ip.write_text(json.dumps(r, indent=2) + '\n')
print(json.dumps(dict(phase=r['phase'], gpu_uuid=gpu, context_tokens=1048576,
                      checked_cache_mode=True, exact_tokenizer_memo=True, formal_v15_unchanged=True)))
