"""Materialize the exact protected API's real1M input IDs for direct native profiling, CPU only."""
import json
import time
from xolotl_trial_common import BASE, jump, lease

ip=BASE/'xolotl-native-actual1m-input-v169-intent.json'
if ip.exists():raise RuntimeError('exact canonical native-input preparation has a handle; inspect it')
fixture=json.loads((BASE/'xolotl-canonical-code-fixture-v147-intent.json').read_text())
api=json.loads((BASE/'xolotl-api-literal-v148-intent.json').read_text())
record=dict(unix=time.time(),phase='prepared',root=lease()['remote']+'/native-actual1m-input-v169-cpu',
    fixture_root=fixture['root'],api_root=api['root'],gpu_executed=False,model_requests_sent=0,credentials_copied=False)
code='''from array import array
import hashlib,json,os,signal,sys,time
from pathlib import Path
r=@R@;root=Path(r['root']);src=Path(r['api_root']);fixture=Path(r['fixture_root'])
if os.getuid()!=10250 or root.exists() or root.parent.resolve()!=root.parent or root.parent.stat().st_uid!=10250:raise RuntimeError('exclusive exact public ID stage owner differs')
os.umask(0o077);root.mkdir(mode=0o700)
signal.signal(signal.SIGALRM,lambda signum,frame:(_ for _ in ()).throw(TimeoutError('finite90s native input encoder exceeded')));signal.alarm(90)
state=dict(unix=time.time(),phase='preflight',gpu_executed=False,model_requests_sent=0,credentials_copied=False)
def save():
 p=root/'input-result.tmp';p.write_text(json.dumps(state,indent=2)+'\\n');p.replace(root/'input-result.json')
try:
 os.environ.update(CUDA_VISIBLE_DEVICES='',ASTER_XQ_EXACT_TOKEN_CACHE='1',ASTER_XQ_LITERAL_SCAN='1');os.nice(19)
 for p in [src,fixture]:
  if p.resolve()!=p or p.stat().st_uid!=10250:raise RuntimeError('exact public input source root differs')
 api=src/'source/tools/v41/serve/serve_ds41.py'
 if api.is_symlink() or hashlib.sha256(api.read_bytes()).hexdigest()!='6fd2a8b6135fc5e676933f8c729f33dc2bba5849d721cab6d28b6a68a9d43954':raise RuntimeError('exact protected API overlay differs')
 sys.path[:0]=[str(src/'python-deps-preflight'),str(src/'source'),str(api.parent)]
 import serve_ds41 as D
 vocab=json.loads((src/'tokenizer/vocab.json').read_text());tokens=[None]*len(vocab)
 for word,index in vocab.items():tokens[index]=word
 tk=D.Ds41Tokenizer(tokens,(src/'tokenizer/merges.txt').read_text().split('\\n'),json.loads((src/'tokenizer/token_type.json').read_text()))
 meta=json.loads((fixture/'fixture-metadata.json').read_text());p=fixture/'public-fixture.txt'
 if p.is_symlink() or p.stat().st_size!=meta['prompt_bytes'] or p.stat().st_size>8*1024**2:raise RuntimeError('bounded exact real1M body differs')
 raw=p.read_bytes()
 if hashlib.sha256(raw).hexdigest()!=meta['prompt_sha256']:raise RuntimeError('exact real1M public body SHA differs')
 svc=object.__new__(D.S.Service);svc.tok=tk;svc.template=D.Ds41Template();svc.effort_end=False;svc.literals=D.F.literal_tags(tk.control_tokens)
 start=time.monotonic();ids=svc.encode_prompt([dict(role='user',content=raw.decode())],None,dict(enable_thinking=False));elapsed=time.monotonic()-start
 packed=array('I',ids)
 if packed.itemsize!=4:raise RuntimeError('exact 32-bit ID serialization differs')
 if sys.byteorder!='little':packed.byteswap()
 binary=packed.tobytes();digest=hashlib.sha256(binary).hexdigest()
 if len(ids)!=1047544 or digest!='b442a593a4066e3e99b663b670db4d7d624c471f1c5f6ee906e315c99d1969c4' or digest!=meta['prompt_id_sha256']:raise RuntimeError('canonical actual protected API IDs differ')
 csv=','.join(map(str,ids)).encode('ascii')
 if len(csv)>8*1024**2 or len(binary)!=4*1047544:raise RuntimeError('bounded real1M native protocol IDs exceeded')
 files=[]
 for name,data in [('prompt.u32le',binary),('prompt.csv',csv)]:
  with (root/name).open('xb') as output:output.write(data)
  files.append(dict(file=name,bytes=len(data),sha256=hashlib.sha256(data).hexdigest()))
 state.update(phase='passed',prompt_tokens=len(ids),prompt_id_sha256=digest,prompt_body_sha256=meta['prompt_sha256'],api_source_sha256=hashlib.sha256(api.read_bytes()).hexdigest(),files=files,encoder_seconds=elapsed,
  expected_record_codes=meta['expected'],context_tokens=1048576,profile_completion_tokens=128,full_body_unchanged=True,native_engine_executed=False,finished_unix=time.time());save()
except BaseException as error:
 state.update(phase='failed',error=type(error).__name__+': '+str(error)[:1500],finished_unix=time.time());save();raise
finally:signal.alarm(0)
print(json.dumps(state))
'''.replace('@R@',repr(record))
compile(code,'prepare-canonical-real1m-native-IDs-v169','exec')
with ip.open('x') as output:output.write(json.dumps(record,indent=2)+'\n')
record['phase']='execution_requested';ip.write_text(json.dumps(record,indent=2)+'\n')
try:result=jump(code,105)
except BaseException:
    record['phase']='outcome_requires_inspection';ip.write_text(json.dumps(record,indent=2)+'\n');raise
record['phase']=result['phase'];ip.write_text(json.dumps(record,indent=2)+'\n')
(BASE/'xolotl-native-actual1m-input-v169-latest.json').write_text(json.dumps(result,indent=2)+'\n')
print(json.dumps(result,indent=2))
