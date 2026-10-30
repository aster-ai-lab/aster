"""CPU-only upstream regression and strict native-argument tests before replacing any GPU runtime."""
import json,os,subprocess,sys,time,zipfile
from pathlib import Path,PurePosixPath
ROOT=Path(__file__).resolve().parent
STATE=dict(phase='starting',unix=time.time(),gpu_execution=False)
def save():
 p=ROOT/'cpu-preflight-result.tmp';p.write_text(json.dumps(STATE,indent=2)+'\n');p.replace(ROOT/'cpu-preflight-result.json')
try:
 save();manifest=json.loads((ROOT/'public-manifest.json').read_text());deps=ROOT/'python-deps-preflight';deps.mkdir(mode=0o700);total=0
 for row in manifest['wheels']:
  with zipfile.ZipFile(ROOT/row['file']) as archive:
   for m in archive.infolist():
    name=PurePosixPath(m.filename);total+=m.file_size
    if name.is_absolute() or '..' in name.parts or m.file_size>8*1024**2 or total>24*1024**2 or (m.external_attr>>16)&0o170000==0o120000:raise RuntimeError('unsafe pinned wheel member')
    p=deps.joinpath(*name.parts)
    if m.is_dir():p.mkdir(mode=0o700,parents=True,exist_ok=True);continue
    p.parent.mkdir(mode=0o700,parents=True,exist_ok=True)
    with p.open('xb') as f:f.write(archive.read(m))
 env=dict(os.environ);env.update(PYTHONPATH=str(deps)+os.pathsep+str(ROOT/'source')+os.pathsep+str(ROOT/'source/tools/v41/serve'),PYTHONDONTWRITEBYTECODE='1',STRATA_TEXT_LOG='0',CUDA_VISIBLE_DEVICES='',ASTER_XQ_EXACT_TOKEN_CACHE='1')
 http_args=[sys.executable,str(ROOT/'test_native_http.py'),'--source-root',str(ROOT/'source'),'--tokenizer',str(ROOT/'tokenizer'),'--fixture',str(ROOT/'actual-tool-output.json')]
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
 (ROOT/'public-manifest.json').write_text(json.dumps(manifest,indent=2)+'\n')
 STATE['exact_grammar_patch_applied']=True;save()
 tests=[('upstream-security',[sys.executable,'-m','unittest','serve.test_security']),('upstream-ds41',[sys.executable,str(ROOT/'source/tools/v41/serve/test_serve_ds41.py'),str(ROOT/'tokenizer')]),('exact-memo-hermetic',[sys.executable,str(ROOT/'test_exact_token_cache.py')]),('strict-native-json',[sys.executable,str(ROOT/'test_strict_native_json.py'),str(ROOT/'source')]),('combined-adapter',[sys.executable,str(ROOT/'test_combined_adapter.py')]),('hybrid-native-syntax',[sys.executable,str(ROOT/'test_hybrid_native_json.py'),str(ROOT/'source')]),('actual-native-http',http_args+['--out',str(ROOT/'green-http-result.json')]),('actual-fullwindow-tokenizer',[sys.executable,str(ROOT/'benchmark_real_tokenizer.py')])]
 STATE['tests']=[]
 for name,args in tests:
  with (ROOT/('cpu-'+name+'.stdout.log')).open('xb') as a,(ROOT/('cpu-'+name+'.stderr.log')).open('xb') as b:q=subprocess.run(args,cwd=ROOT/'source',env=env,stdout=a,stderr=b,timeout=65)
  STATE['tests'].append(dict(name=name,exit_code=q.returncode));save()
  if q.returncode:raise RuntimeError(name+' failed')
 STATE.update(phase='passed',finished_unix=time.time());save()
except BaseException as e:
 STATE.update(phase='failed',error=type(e).__name__+': '+str(e),finished_unix=time.time());save();raise
