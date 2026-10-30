"""Reproduce the actual HTTP failure on CPU, apply the exact grammar delta, then run all CPU gates."""
import ast
import base64
import hashlib
import json
import time
from xolotl_trial_common import BASE, jump, lease

intent = BASE / 'xolotl-api-hybrid-v52-intent.json'
if intent.exists():
    raise RuntimeError('V52 CPU stage has an exact handle; observe it')
parent = json.loads((BASE / 'xolotl-api-adapter-v46-intent.json').read_text())
local = BASE / 'api-adapter-v52'
overlay = json.loads((local / 'overlay-plan.json').read_text())
root = lease()['remote'] + '/api-hybrid-preflight-v52'
full = json.loads((BASE / 'xolotl-resident-v38-fullwindow-intent.json').read_text())
benchmark = dict(public_fixture=full['root'] + '/public-fixture.txt',
    fixture_sha256='0149634cc9fbae367f049cd432cc316a98454a0f814725e56fce2563b0b77d5f',
    expected_id_sha256='b9b006a173dc68f8da156b6bd01e0d90c3abdef62f24561475e8b4fa17e5374c')
worker = (BASE / 'api-adapter-v46/cpu_preflight_worker.py').read_text()
anchor = " tests=[('upstream-security'"
if worker.count(anchor) != 1:
    raise RuntimeError('exact original CPU test suite anchor differs')
intro = ''' http_args=[sys.executable,str(ROOT/'test_native_http.py'),'--source-root',str(ROOT/'source'),'--tokenizer',str(ROOT/'tokenizer'),'--fixture',str(ROOT/'actual-tool-output.json')]
 with (ROOT/'cpu-red-native-http.stdout.log').open('xb') as a,(ROOT/'cpu-red-native-http.stderr.log').open('xb') as b:q=subprocess.run(http_args+['--out',str(ROOT/'red-http-result.json')],cwd=ROOT/'source',env=env,stdout=a,stderr=b,timeout=50)
 red=json.loads((ROOT/'red-http-result.json').read_text())
 if q.returncode!=1 or red['phase']!='failed' or 'Extra data:' not in red.get('error',''):raise RuntimeError('actual native HTTP fixture did not reproduce the original failure')
 STATE['red_actual_http_bug_reproduced']=True;save()
 overlay=json.loads((ROOT/'overlay-plan.json').read_text());target=ROOT/'source/tools/v41/serve/serve_ds41.py';patched=(ROOT/'overlay/serve_ds41.py').read_bytes()
 import hashlib
 if hashlib.sha256(target.read_bytes()).hexdigest()!=overlay['upstream_sha256'] or hashlib.sha256(patched).hexdigest()!=overlay['patched_sha256']:raise RuntimeError('exact observed grammar patch input differs')
 helper=ROOT/'source/tools/v41/serve/aster_token_cache.py'
 if hashlib.sha256(helper.read_bytes()).hexdigest()!=overlay['helper_sha256']:raise RuntimeError('unchanged bounded token helper differs')
 target.write_bytes(patched)
 rows=[row for row in manifest['files'] if row['file']=='source/tools/v41/serve/serve_ds41.py']
 if len(rows)!=1:raise RuntimeError('exact API manifest row differs')
 rows[0].update(bytes=len(patched),sha256=overlay['patched_sha256'])
 manifest['previous_overlay']=manifest.get('overlay');manifest['overlay']=overlay;manifest['schema']='aster-public-effective-native-hybrid-v52'
 (ROOT/'public-manifest.json').write_text(json.dumps(manifest,indent=2)+'\\n')
 STATE['exact_grammar_patch_applied']=True;save()
'''
worker = worker.replace(anchor, intro + anchor)
anchor = "('actual-fullwindow-tokenizer',[sys.executable,str(ROOT/'benchmark_real_tokenizer.py')])"
if worker.count(anchor) != 1:
    raise RuntimeError('exact CPU actual-tokenizer test anchor differs')
worker = worker.replace(anchor, "('hybrid-native-syntax',[sys.executable,str(ROOT/'test_hybrid_native_json.py'),str(ROOT/'source')]),('actual-native-http',http_args+['--out',str(ROOT/'green-http-result.json')])," + anchor)
compile(worker, 'native-hybrid-cpu-preflight-v52', 'exec')
(local / 'cpu_preflight_worker.py').write_text(worker, encoding='utf-8', newline='\n')
files = {
    'overlay/serve_ds41.py': (local / 'serve_ds41.py').read_bytes(),
    'overlay-plan.json': (local / 'overlay-plan.json').read_bytes(),
    'actual-tool-output.json': (local / 'actual-tool-output.json').read_bytes(),
    'benchmark-plan.json': (json.dumps(benchmark, indent=2) + '\n').encode(),
    'cpu_preflight_worker.py': worker.encode(),
    'test_native_http.py': (BASE / 'test_xolotl_native_http_v52.frozen.py').read_bytes(),
}
for name in ['test_exact_token_cache.py', 'test_strict_native_json.py', 'test_combined_adapter.py',
             'test_hybrid_native_json.py', 'benchmark_real_tokenizer.py']:
    files[name] = (local / name).read_bytes()
record = dict(unix=time.time(), phase='prepared', root=root, source_public_root=parent['root'],
    source_overlay=overlay, public_fixture_only=True, gpu_executed=False, credentials_copied=False,
    files=[dict(file=n, bytes=len(v), sha256=hashlib.sha256(v).hexdigest()) for n, v in files.items()])
tree = ast.parse((BASE / 'launch_xolotl_longctx_shadow_v1.py').read_text())
bodies = [node.value for node in ast.walk(tree) if isinstance(node, ast.Constant) and isinstance(node.value, str)
          and "root=Path(r['root']);src=Path(r['source_public_root'])" in node.value]
if len(bodies) != 1:
    raise RuntimeError('exact public CPU bootstrap differs')
template = bodies[0].split('# Carry the existing authorized service credential', 1)[0]
anchor = " with (root/row['file']).open('xb') as f:f.write(raw)"
if template.count(anchor) != 1:
    raise RuntimeError('exact staged CPU payload writer differs')
template = template.replace(anchor, " (root/row['file']).parent.mkdir(mode=0o700,parents=True,exist_ok=True)\n" + anchor)
template += '''
with (root/'cpu-preflight-worker.log').open('xb') as log:
 p=subprocess.Popen(['timeout','--signal=TERM','--kill-after=15','300','python3','-u',str(root/'cpu_preflight_worker.py')],cwd=root,stdin=subprocess.DEVNULL,stdout=log,stderr=log,start_new_session=True)
process=dict(pid=p.pid,start_ticks=int(Path('/proc/'+str(p.pid)+'/stat').read_text().split(') ',1)[1].split()[19]),uid=os.getuid(),unix=time.time(),budget_seconds=300)
(root/'cpu-preflight-process.json').write_text(json.dumps(process,indent=2)+'\\n');print(json.dumps(process))
'''
code = template.replace('@R@', repr(record)).replace('@DATA@', repr({n: base64.b64encode(v).decode() for n, v in files.items()}))
compile(code, 'native-hybrid-cpu-staging-v52', 'exec')
with intent.open('x') as output:
    output.write(json.dumps(record, indent=2) + '\n')
record['phase'] = 'launch_requested'
intent.write_text(json.dumps(record, indent=2) + '\n')
try:
    record['process'] = jump(code)
    record['phase'] = 'cpu_preflight_running'
except BaseException:
    record['phase'] = 'launch_outcome_requires_inspection'
    intent.write_text(json.dumps(record, indent=2) + '\n')
    raise
intent.write_text(json.dumps(record, indent=2) + '\n')
print(json.dumps(dict(phase=record['phase'], process=record['process'], gpu_executed=False, credentials_copied=False)))
