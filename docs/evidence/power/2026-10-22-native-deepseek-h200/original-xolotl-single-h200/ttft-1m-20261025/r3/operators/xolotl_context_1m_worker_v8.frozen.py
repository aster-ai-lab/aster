"""Supervise one qualified engine and upstream authenticated API on its own H200."""
import hashlib,importlib.metadata,json,os,signal,socket,subprocess,sys,time,urllib.error,urllib.request,zipfile
from pathlib import Path,PurePosixPath
ROOT=Path(__file__).resolve().parent
PLAN=json.loads((ROOT/'service-plan.json').read_text())
STATE=dict(schema='aster-xolotl-stable-service-v1',phase='preflight',unix=time.time(),model=PLAN['model_name'],performance_300_claimed=False)
server=None;stopping=False
def save():
 p=ROOT/'service-longctx-v1-result.tmp';p.write_text(json.dumps(STATE,indent=2)+'\n');p.replace(ROOT/'service-longctx-v1-result.json')
def digest(p):return hashlib.sha256(p.read_bytes()).hexdigest()
def stop_signal(signum,frame):
 global stopping
 stopping=True
def run(args,name,limit,env):
 with (ROOT/(name+'.stdout.log')).open('xb') as a,(ROOT/(name+'.stderr.log')).open('xb') as b:q=subprocess.run(args,cwd=ROOT/'source',env=env,stdout=a,stderr=b,timeout=limit)
 if q.returncode:raise RuntimeError(name+' exit '+str(q.returncode))
def stop_server():
 if server is None or server.poll() is not None:return
 server.terminate()
 try:server.wait(timeout=20)
 except subprocess.TimeoutExpired:
  os.killpg(server.pid,signal.SIGKILL);server.wait(timeout=10)
def active_gpu():
 q=subprocess.run(['nvidia-smi','--query-compute-apps=pid,gpu_uuid','--format=csv,noheader,nounits'],capture_output=True,text=True,check=True,timeout=8)
 return q.stdout.strip()
try:
 os.umask(0o077);save()
 if os.getuid()!=10250 or ROOT.resolve()!=ROOT or ROOT.stat().st_uid!=10250 or sys.version_info[:2]!=(3,10):raise RuntimeError('serving owner/runtime identity differs')
 proof=json.loads((ROOT.parent/'resource-result.json').read_text())
 if proof['phase']!='passed' or proof['expectedGpus']!=1 or proof['leaseEndsUnix']!=PLAN['lease_ends_unix'] or proof['gpus'][0]['uuid']==PLAN['optimization_gpu_uuid'] or time.time()+600>proof['leaseEndsUnix']:raise RuntimeError('independent serving resource identity/deadline differs')
 if active_gpu():raise RuntimeError('stable serving GPU already has a workload')
 manifest=json.loads((ROOT/'public-manifest.json').read_text())
 for r in manifest['files']:
  p=ROOT/r['file']
  if p.is_symlink() or p.stat().st_uid!=10250 or p.stat().st_size!=r['bytes'] or digest(p)!=r['sha256']:raise RuntimeError('stable public input differs: '+r['file'])
 for r in [{'file': 'source/serve/chat_template.jinja', 'bytes': 9993, 'sha256': '12827f24b742ea4e80cdc12dbcf9622227056b9f797252a3149263d4f9aaadce'}, {'file': 'source/serve/chat_golden.json', 'bytes': 12284, 'sha256': 'b9006c519218fcb7e5e38ba603905029f79c5c53431b3c9049466a4b0f3bd96f'}]:
  p=ROOT/r['file']
  if p.is_symlink() or p.stat().st_uid!=10250 or p.stat().st_size!=r['bytes'] or digest(p)!=r['sha256']:raise RuntimeError('supplemental upstream test template differs')
 for m in PLAN['inputs']:
  p=Path(m['path'])
  if not p.is_file() or p.stat().st_size!=m['bytes']:raise RuntimeError('stable model/rank/sidecar input size differs')
  if m.get('verify_small_sha256') and digest(p)!=m['sha256']:raise RuntimeError('stable small input SHA differs')
 if digest(Path(PLAN['binary']))!=PLAN['binary_sha256']:raise RuntimeError('qualified stable executable SHA differs')
 deps=ROOT/'python-deps-v2';deps.mkdir(mode=0o700)
 total=0
 for r in manifest['wheels']:
  with zipfile.ZipFile(ROOT/r['file']) as archive:
   for m in archive.infolist():
    name=PurePosixPath(m.filename);total+=m.file_size
    if name.is_absolute() or '..' in name.parts or m.file_size>8*1024**2 or total>24*1024**2 or (m.external_attr>>16)&0o170000==0o120000:raise RuntimeError('unsafe pinned wheel member')
    p=deps.joinpath(*name.parts)
    if m.is_dir():p.mkdir(mode=0o700,parents=True,exist_ok=True);continue
    p.parent.mkdir(mode=0o700,parents=True,exist_ok=True)
    with p.open('xb') as f:f.write(archive.read(m))
 env={k:v for k,v in os.environ.items() if not k.startswith(('STRATA_','ASTER_XQ_','PYTHONPATH'))}
 env.update(PYTHONPATH=str(deps)+os.pathsep+str(ROOT/'source'),PYTHONDONTWRITEBYTECODE='1',CUDA_VISIBLE_DEVICES='0',GGML_NO_BACKTRACE='1',OMP_NUM_THREADS='16',OPENBLAS_NUM_THREADS='1')
 run([sys.executable,'-m','unittest','serve.test_security'],'security-tests-v2',60,env)
 run([sys.executable,str(ROOT/'source/tools/v41/serve/test_serve_ds41.py'),str(ROOT/'tokenizer')],'ds41-api-tests-v2',60,env)
 # Exact prompt-ID identity with the tokenizer already used for GPU qualification.
 check='''import sys,json
from pathlib import Path
root=Path(sys.argv[1]);sys.path.insert(0,str(root/'source/tools/v41/serve'))
import serve_ds41 as D
v=json.loads((root/'tokenizer/vocab.json').read_text());t=[None]*len(v)
for word,i in v.items():t[i]=word
tk=D.Ds41Tokenizer(t,(root/'tokenizer/merges.txt').read_text().split('\\n'),json.loads((root/'tokenizer/token_type.json').read_text()))
cases=json.loads((root/'prompt-id-controls.json').read_text())['cases']
for c in cases:
 rendered=D.Ds41Template().render([dict(role='user',content=c['prompt'])],enable_thinking=False)
 if rendered!=c['rendered'] or tk.encode(rendered,parse_special=True)!=c['ids']:raise RuntimeError('official template/tokenizer prompt IDs differ')
print('PASS: all official template/tokenizer prompt IDs match prior GPU qualification')
'''
 run([sys.executable,'-c',check,str(ROOT)],'prompt-id-tests-v2',15,env)
 STATE.update(context_tokens=PLAN['context_tokens'],phase='starting_api',cpu_api_tests_passed=True,gpu_uuid=proof['gpus'][0]['uuid'],lease_ends_unix=proof['leaseEndsUnix'],binary_sha256=PLAN['binary_sha256']);save()
 key=(ROOT/'credentials/api-key.txt').read_text().strip()
 if len(key)<40 or digest(ROOT/'credentials/api-key.txt')!=PLAN['credential_file_sha256']:raise RuntimeError('private serving credential differs')
 env.update(PLAN['engine_env']);env['STRATA_API_KEY']=key
 signal.signal(signal.SIGTERM,stop_signal);signal.signal(signal.SIGINT,stop_signal)
 command=[sys.executable,'-u',str(ROOT/'source/tools/v41/serve/serve_ds41.py'),'--engine','strata','--config',str(ROOT/'server-config.json'),'--host','0.0.0.0','--port','30003']
 attempts=0
 while not stopping and time.time()<proof['leaseEndsUnix']-45:
  if server is None or server.poll() is not None:
   if server is not None:
    STATE['last_api_exit']=server.returncode
    # Descendants share this exact supervised process group, never another job.
    try:os.killpg(server.pid,signal.SIGTERM)
    except ProcessLookupError:pass
    if attempts>=3:raise RuntimeError('bounded API restart budget exhausted')
   if active_gpu():raise RuntimeError('previous serving engine must be gone before API start')
   attempts+=1
   with (ROOT/('api-longctx-v1-attempt'+str(attempts)+'.stdout.log')).open('xb') as a,(ROOT/('api-longctx-v1-attempt'+str(attempts)+'.stderr.log')).open('xb') as b:
    server=subprocess.Popen(command,cwd=ROOT,env=env,stdout=a,stderr=b,start_new_session=True)
   ticks=int(Path('/proc/'+str(server.pid)+'/stat').read_text().split(') ',1)[1].split()[19])
   STATE.update(phase='loading',api_attempt=attempts,api_process=dict(pid=server.pid,start_ticks=ticks,uid=os.getuid()),startup_unix=time.time());save()
  try:
   request=urllib.request.Request('http://127.0.0.1:30003/health',headers={'Authorization':'Bearer '+key})
   with urllib.request.urlopen(request,timeout=2) as q:
    if q.status==200:STATE.update(phase='serving',last_health_status=200,last_health_unix=time.time())
  except (OSError,urllib.error.URLError):
   if STATE['phase']=='serving':STATE['phase']='health_unavailable'
  if STATE['phase']=='loading' and time.time()-STATE['startup_unix']>600:raise RuntimeError('stable API startup exceeded600seconds')
  save();time.sleep(5)
 STATE['phase']='draining';save();stop_server();STATE.update(phase='stopped',api_reaped=True,api_exit=server.returncode if server else None,finished_unix=time.time());save()
except BaseException as e:
 stop_server();STATE.update(phase='failed',error=type(e).__name__+': '+str(e),api_reaped=server is None or server.poll() is not None,finished_unix=time.time());save();raise
