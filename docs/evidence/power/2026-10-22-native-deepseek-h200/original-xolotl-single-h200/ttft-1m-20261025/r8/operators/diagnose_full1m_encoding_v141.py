"""CPU-only replay of the exact API prompt encoder for the public1M fixture.

No GPU, model request, API change, private credential read or live service
mutation is performed. Store numeric timing/IDs digests, never prompt text.
"""
import json
import time
from xolotl_trial_common import BASE, jump, lease

intent_path = BASE / 'xolotl-full1m-api-encoding-v141-intent.json'
if intent_path.exists():
    raise RuntimeError('exact finite encoder probe has a handle; inspect its remote receipt')
fixture = json.loads((BASE / 'xolotl-code-fullwindow-fixture-v114-intent.json').read_text())
record = dict(unix=time.time(), phase='prepared', root=lease()['remote'] + '/cpu-api-encoding-v141',
              fixture_root=fixture['root'], source_public_root=fixture['source_public_root'],
              gpu_executed=False, model_requests_sent=0, live_service_changed=False)
code = '''import hashlib,json,os,signal,sys,time
from array import array
from pathlib import Path
r=@R@;root=Path(r['root']);src=Path(r['source_public_root']);fixture=Path(r['fixture_root'])
if os.getuid()!=10250 or root.parent.resolve()!=root.parent or root.parent.stat().st_uid!=10250 or root.exists():raise RuntimeError('exact exclusive encoder probe root differs')
os.umask(0o077);root.mkdir(mode=0o700)
p=Path('/proc/'+str(os.getpid())+'/stat');process=dict(pid=os.getpid(),start_ticks=int(p.read_text().split(') ',1)[1].split()[19]),uid=os.getuid(),namespace='jump host')
state=dict(unix=time.time(),phase='preflight',process=process,gpu_executed=False,model_requests_sent=0,live_service_changed=False,runs=[])
def save():
 p=root/'encoding-result.tmp';p.write_text(json.dumps(state,indent=2)+'\\n');p.replace(root/'encoding-result.json')
def deadline(signum,frame):raise TimeoutError('finite70-second CPU encoder probe exceeded')
signal.signal(signal.SIGALRM,deadline);signal.alarm(70);save()
try:
 os.environ['CUDA_VISIBLE_DEVICES']='';os.environ['ASTER_XQ_EXACT_TOKEN_CACHE']='1';os.nice(19)
 api=src/'source/tools/v41/serve/serve_ds41.py'
 if api.is_symlink() or hashlib.sha256(api.read_bytes()).hexdigest()!='f062d9d39131959da9c981b6704e2ec27f89d8e2806730062dcabc3eb27682ba':raise RuntimeError('exact API f062 differs')
 sys.path[:0]=[str(src/'python-deps-preflight'),str(src/'source'),str(src/'source/tools/v41/serve')]
 import serve_ds41 as D
 vocab=json.loads((src/'tokenizer/vocab.json').read_text());tokens=[None]*len(vocab)
 for word,index in vocab.items():tokens[index]=word
 tk=D.Ds41Tokenizer(tokens,(src/'tokenizer/merges.txt').read_text().split('\\n'),json.loads((src/'tokenizer/token_type.json').read_text()))
 meta=json.loads((fixture/'fixture-metadata.json').read_text());p=fixture/'public-fixture.txt'
 if p.is_symlink() or p.stat().st_size!=meta['prompt_bytes'] or p.stat().st_size>8*1024**2:raise RuntimeError('bounded exact public1M fixture differs')
 raw=p.read_bytes()
 if hashlib.sha256(raw).hexdigest()!=meta['prompt_sha256']:raise RuntimeError('public1M fixture SHA differs')
 text=raw.decode('utf-8','strict');messages=[dict(role='user',content=text)];kwargs=dict(enable_thinking=False)
 svc=object.__new__(D.S.Service);svc.tok=tk;svc.template=D.Ds41Template();svc.effort_end=False;svc.literals=D.F.literal_tags(tk.control_tokens)
 mark_begin=time.monotonic();marked,tools,changed=D.F.mark_think_literals(messages,None,svc.literals)
 rendered=svc.render_prompt(marked,tools,kwargs)
 if changed:rendered,plain=D.F.unmark_think_literals(rendered,svc.literals)
 else:plain=()
 state.update(phase='encoding',literal_marking_changed=changed,literal_spans_type=type(plain).__name__,literal_spans_count=len(plain),mark_render_seconds=time.monotonic()-mark_begin,
  tokenizer_source_sha256=hashlib.sha256(Path(D.ST.__file__).read_bytes()).hexdigest(),server_source_sha256=hashlib.sha256(Path(D.S.__file__).read_bytes()).hexdigest(),
  expected_prompt_tokens=meta['actual_prompt_tokens'],expected_prompt_id_sha256=meta['prompt_id_sha256'])
 save()
 for index in range(3):
  begin=time.monotonic();ids=svc.encode_prompt(messages,None,kwargs);elapsed=time.monotonic()-begin
  packed=array('I',ids)
  if packed.itemsize!=4:raise RuntimeError('exact32-bit token digest requires4-byte unsigned ints')
  if sys.byteorder!='little':packed.byteswap()
  digest=hashlib.sha256(packed.tobytes()).hexdigest()
  state['runs'].append(dict(index=index,seconds=elapsed,prompt_tokens=len(ids),prompt_id_sha256=digest,memo=tk._aster_encode_memo.stats()))
  save()
  if len(ids)!=meta['actual_prompt_tokens'] or digest!=meta['prompt_id_sha256']:raise RuntimeError('actual API encoder IDs differ from direct fixture preparation')
 begin=time.monotonic();serialized=','.join(map(str,ids));serial_seconds=time.monotonic()-begin
 state.update(phase='passed',protocol_csv_bytes=len(serialized),protocol_csv_seconds=serial_seconds,finished_unix=time.time());save()
except BaseException as error:
 state.update(phase='failed',error=type(error).__name__+': '+str(error)[:1500],finished_unix=time.time());save()
finally:signal.alarm(0)
print(json.dumps(state))
'''.replace('@R@', repr(record))
compile(code, 'actual-api-prompt-encoder-v141', 'exec')
with intent_path.open('x') as output:
    output.write(json.dumps(record, indent=2) + '\n')
record['phase'] = 'execution_requested'
intent_path.write_text(json.dumps(record, indent=2) + '\n')
try:
    result = jump(code, 90)
except BaseException:
    record['phase'] = 'execution_outcome_requires_inspection'
    intent_path.write_text(json.dumps(record, indent=2) + '\n')
    raise
record.update(phase=result['phase'], process=result['process'])
intent_path.write_text(json.dumps(record, indent=2) + '\n')
(BASE / 'xolotl-full1m-api-encoding-v141-latest.json').write_text(json.dumps(result, indent=2) + '\n')
print(json.dumps(result, indent=2))
