"""Copy only public inputs and run CPU/token-ID regression without a credential or GPU."""
import ast, base64, hashlib, json, time
from xolotl_trial_common import BASE, jump, lease

ip = BASE / 'xolotl-api-adapter-v46-intent.json'
if ip.exists():
    raise RuntimeError('exact tokenizer CPU intent exists; observe its exact worker')
stable = json.loads((BASE / 'xolotl-indexer-service-v44-intent.json').read_text())
full = json.loads((BASE / 'xolotl-resident-v38-fullwindow-latest.json').read_text())['result']
if full['phase'] != 'passed' or not full.get('full1m_cache_restore_passed'):
    raise RuntimeError('actual full-window fixture must be qualified before tokenizer optimization')
local = BASE / 'api-adapter-v46'
overlay = json.loads((local / 'overlay-plan.json').read_text())
root = lease()['remote'] + '/api-preflight-v46'
benchmark = dict(public_fixture=json.loads((BASE / 'xolotl-resident-v38-fullwindow-intent.json').read_text())['root'] + '/public-fixture.txt',
                 fixture_sha256=full['prompt_sha256'], expected_id_sha256='b9b006a173dc68f8da156b6bd01e0d90c3abdef62f24561475e8b4fa17e5374c')
payload = {
    'overlay/serve_ds41.py': (local / 'serve_ds41.py').read_bytes(),
    'overlay/aster_token_cache.py': (local / 'aster_token_cache.py').read_bytes(),
    'test_exact_token_cache.py': (local / 'test_exact_token_cache.py').read_bytes(),
    'benchmark_real_tokenizer.py': (local / 'benchmark_real_tokenizer.py').read_bytes(),
    'benchmark-plan.json': (json.dumps(benchmark, indent=2) + '\n').encode(),
    'test_strict_native_json.py': (local / 'test_strict_native_json.py').read_bytes(),
    'test_combined_adapter.py': (local / 'test_combined_adapter.py').read_bytes(),
    'cpu_preflight_worker.py': (local / 'cpu_preflight_worker.py').read_bytes(),
}
r = dict(unix=time.time(), phase='prepared', root=root, source_public_root=stable['root'], source_overlay=overlay,
         public_fixture_only=True, gpu_executed=False, credentials_copied=False,
         files=[dict(file=name, bytes=len(raw), sha256=hashlib.sha256(raw).hexdigest()) for name, raw in payload.items()])
tree = ast.parse((BASE / 'launch_xolotl_longctx_shadow_v1.py').read_text())
bodies = [node.value for node in ast.walk(tree) if isinstance(node, ast.Constant) and isinstance(node.value, str)
          and "root=Path(r['root']);src=Path(r['source_public_root'])" in node.value]
if len(bodies) != 1:
    raise RuntimeError('bounded original public staging bootstrap differs')
template = bodies[0]
template = template[:template.index('# Carry the existing authorized service credential')]
anchor = " with (root/row['file']).open('xb') as f:f.write(raw)"
if template.count(anchor) != 1:
    raise RuntimeError('exact public overlay staging anchor differs')
template = template.replace(anchor, " (root/row['file']).parent.mkdir(mode=0o700,parents=True,exist_ok=True)\n" + anchor)
template += '''
target=root/'source/tools/v41/serve/serve_ds41.py';patched=(root/'overlay/serve_ds41.py').read_bytes();helper=(root/'overlay/aster_token_cache.py').read_bytes()
if hashlib.sha256(target.read_bytes()).hexdigest()!=r['source_overlay']['upstream_sha256'] or hashlib.sha256(patched).hexdigest()!=r['source_overlay']['patched_sha256'] or hashlib.sha256(helper).hexdigest()!=r['source_overlay']['helper_sha256']:raise RuntimeError('exact tokenizer overlay base or payload differs')
target.write_bytes(patched)
helper_target=root/'source/tools/v41/serve/aster_token_cache.py'
with helper_target.open('xb') as f:f.write(helper)
rows=[row for row in manifest['files'] if row['file']=='source/tools/v41/serve/serve_ds41.py']
if len(rows)!=1:raise RuntimeError('exact original adapter manifest entry differs')
rows[0].update(bytes=len(patched),sha256=hashlib.sha256(patched).hexdigest())
manifest['files'].append(dict(file='source/tools/v41/serve/aster_token_cache.py',bytes=len(helper),sha256=hashlib.sha256(helper).hexdigest()))
manifest['base_archive_sha256']=manifest.pop('archive_sha256');manifest['base_archive_bytes']=manifest.pop('archive_bytes');manifest['schema']='aster-public-effective-tokenizer-overlay-v21';manifest['overlay']=r['source_overlay']
(root/'public-manifest.json').write_text(json.dumps(manifest,indent=2)+'\\n')
with (root/'cpu-preflight-worker.log').open('xb') as log:
 p=subprocess.Popen(['timeout','--signal=TERM','--kill-after=15','240','python3','-u',str(root/'cpu_preflight_worker.py')],cwd=root,stdin=subprocess.DEVNULL,stdout=log,stderr=log,start_new_session=True)
process=dict(pid=p.pid,start_ticks=int(Path('/proc/'+str(p.pid)+'/stat').read_text().split(') ',1)[1].split()[19]),uid=os.getuid(),unix=time.time(),budget_seconds=240)
(root/'cpu-preflight-process.json').write_text(json.dumps(process,indent=2)+'\\n');print(json.dumps(process))
'''
code = template.replace('@R@', repr(r)).replace('@DATA@', repr({name: base64.b64encode(raw).decode() for name, raw in payload.items()}))
compile(code, 'exact-tokenizer-public-CPU-staging', 'exec')
r['phase'] = 'stage_requested'
with ip.open('x') as stream:
    stream.write(json.dumps(r, indent=2) + '\n')
r['process'] = jump(code)
r['phase'] = 'cpu_preflight_running'
ip.write_text(json.dumps(r, indent=2) + '\n')
print(json.dumps(dict(phase=r['phase'], gpu_executed=False, credentials_copied=False, root=root)))
