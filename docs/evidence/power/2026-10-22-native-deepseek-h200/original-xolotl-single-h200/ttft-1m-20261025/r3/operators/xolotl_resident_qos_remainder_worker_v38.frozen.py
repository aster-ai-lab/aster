"""Use public synthetic context only. Never persist auth headers or private Workbuddy text."""
import hashlib,json,os,socket,sys,time,urllib.error,urllib.request
from pathlib import Path
ROOT=Path(__file__).resolve().parent
R=json.loads((ROOT/'request-plan.json').read_text())
STATE=dict(schema='aster-xolotl-qos-one-token-remainder-v15',phase='preflight',unix=time.time(),source_hostname=socket.gethostname(),destination=R['base_url'],performance_300_claimed=False)
def save():
 p=ROOT/'request-result.tmp';p.write_text(json.dumps(STATE,ensure_ascii=False,indent=2)+'\n');p.replace(ROOT/'request-result.json')
try:
 os.umask(0o077);save()
 key=(Path(R['service_root'])/'credentials/api-key.txt').read_text().strip()
 request=urllib.request.Request(R['base_url'].removesuffix('/v1')+'/health',headers={'Authorization':'Bearer '+key})
 with urllib.request.urlopen(request,timeout=10) as response:health=json.load(response)
 if not health.get('loaded') or health.get('max_context')!=1048576:raise RuntimeError('actual1M API health not ready')
 STATE['health']=dict(loaded=True,max_context=health['max_context'])
 payload=dict(model='deepseek-v4.1-flash-1017-xq',messages=[dict(role='user',content='Calculate 17 + 25. Reply with only the integer answer.')],max_tokens=16,temperature=0,reasoning_effort='none',stream=False)
 req=urllib.request.Request(R['base_url']+'/chat/completions',data=json.dumps(payload).encode(),headers={'Authorization':'Bearer '+key,'Content-Type':'application/json'})
 with urllib.request.urlopen(req,timeout=30) as response:short=json.load(response)
 if short['choices'][0]['message']['content'].strip()!='42':raise RuntimeError('1M short generation control failed')
 STATE['short_control']=dict(passed=True,content='42',usage=short['usage']);save()
 service=Path(R['service_root'])
 sys.path[:0]=[str(service/'python-deps-v2'),str(service/'source'),str(service/'source/tools/v41/serve')]
 import serve_ds41 as D
 v=json.loads((service/'tokenizer/vocab.json').read_text());tokens=[None]*len(v)
 for word,i in v.items():tokens[i]=word
 tk=D.Ds41Tokenizer(tokens,(service/'tokenizer/merges.txt').read_text().split('\n'),json.loads((service/'tokenizer/token_type.json').read_text()))
 def count(text):return len(tk.encode(D.Ds41Template().render([dict(role='user',content=text)],enable_thinking=False),parse_special=True))
 prefix='This is a public synthetic long-context test. Ignore the filler and answer the final arithmetic question.\nFiller:'
 suffix='\nEnd of filler. Calculate 17 + 25. Reply with only the integer answer.'
 n=132097-count(prefix+suffix)
 text=prefix+(' cat'*n)+suffix
 actual=count(text)
 for _ in range(8):
  if actual==132097:break
  n+=132097-actual;text=prefix+(' cat'*n)+suffix;actual=count(text)
 if actual<132097 or actual>132097:raise RuntimeError('synthetic actual prompt token count is outside qualification bounds')
 (ROOT/'public-fixture.txt').write_text(text)
 STATE.update(phase='requesting',prompt_tokens_verified=actual,prompt_sha256=hashlib.sha256(text.encode()).hexdigest(),prompt_characters=len(text),max_new_tokens=64,context_tokens=R['context_tokens'],request_started_unix=time.time());save()
 key=(service/'credentials/api-key.txt').read_text().strip()
 payload=dict(model='deepseek-v4.1-flash-1017-xq',messages=[dict(role='user',content=text)],max_tokens=64,temperature=0,seed=1,reasoning_effort='none',stream=True,stream_options=dict(include_usage=True))
 req=urllib.request.Request(R['base_url']+'/chat/completions',data=json.dumps(payload).encode(),headers={'Content-Type':'application/json','Authorization':'Bearer '+key})
 content='';reasoning='';usage=None;finish=None;events=0;first_content=None;done=False
 with urllib.request.urlopen(req,timeout=900) as response:
  STATE.update(http_status=response.status,phase='prefilling',headers_received_unix=time.time());save()
  for line in response:
   if len(line)>1048576:raise RuntimeError('SSE event exceeds bound')
   if not line.startswith(b'data:'):continue
   raw=line[5:].strip()
   if raw==b'[DONE]':done=True;break
   data=json.loads(raw);events+=1
   for choice in data.get('choices',[]):
    delta=choice.get('delta',{});part=delta.get('content') or '';content+=part;reasoning+=delta.get('reasoning_content') or ''
    if part and first_content is None:first_content=time.time()
    finish=choice.get('finish_reason') or finish
   if data.get('usage'):usage=data['usage']
   if len(content)+len(reasoning)>32768:raise RuntimeError('long-context response text exceeds bound')
   STATE.update(phase='streaming' if first_content else 'prefilling',events=events,first_content_unix=first_content,last_event_unix=time.time());save()
 elapsed=time.time()-STATE['request_started_unix']
 STATE.update(phase='passed' if done and finish and content.strip()=='42' and usage is not None and usage.get('prompt_tokens')==132097 else 'failed',done=done,finish_reason=finish,content=content,reasoning_characters=len(reasoning),usage=usage,elapsed_seconds=elapsed,time_to_first_content_seconds=(first_content-STATE['request_started_unix']) if first_content else None,finished_unix=time.time());save()
 if STATE['phase']!='passed':raise RuntimeError('long-context request did not finish with the expected arithmetic answer')
except BaseException as e:
 STATE.update(phase='failed',error=type(e).__name__+': '+str(e),finished_unix=time.time());save();raise
