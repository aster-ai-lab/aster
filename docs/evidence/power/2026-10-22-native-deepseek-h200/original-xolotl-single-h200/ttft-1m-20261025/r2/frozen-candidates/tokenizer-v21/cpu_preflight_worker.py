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
 env=dict(os.environ);env.update(PYTHONPATH=str(deps)+os.pathsep+str(ROOT/'source')+os.pathsep+str(ROOT/'source/tools/v41/serve'),PYTHONDONTWRITEBYTECODE='1',STRATA_TEXT_LOG='0',CUDA_VISIBLE_DEVICES='')
 tests=[('upstream-security',[sys.executable,'-m','unittest','serve.test_security']),('upstream-ds41',[sys.executable,str(ROOT/'source/tools/v41/serve/test_serve_ds41.py'),str(ROOT/'tokenizer')]),('exact-memo-hermetic',[sys.executable,str(ROOT/'test_exact_token_cache.py')]),('actual-fullwindow-tokenizer',[sys.executable,str(ROOT/'benchmark_real_tokenizer.py')])]
 STATE['tests']=[]
 for name,args in tests:
  with (ROOT/('cpu-'+name+'.stdout.log')).open('xb') as a,(ROOT/('cpu-'+name+'.stderr.log')).open('xb') as b:q=subprocess.run(args,cwd=ROOT/'source',env=env,stdout=a,stderr=b,timeout=65)
  STATE['tests'].append(dict(name=name,exit_code=q.returncode));save()
  if q.returncode:raise RuntimeError(name+' failed')
 STATE.update(phase='passed',finished_unix=time.time());save()
except BaseException as e:
 STATE.update(phase='failed',error=type(e).__name__+': '+str(e),finished_unix=time.time());save();raise
