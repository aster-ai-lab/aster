"""Actual capacity plus three-position retrieval and isolated KV restore, public data only."""
import hashlib,json,os,socket,sys,time,urllib.error,urllib.request
from pathlib import Path
ROOT=Path(__file__).resolve().parent;P=json.loads((ROOT/'request-plan.json').read_text());service=Path(P['service_root'])
STATE=dict(schema='aster-xolotl-full1m-qualification-v17',unix=time.time(),phase='preflight',tests=[],client=socket.gethostname(),destination=P['base_url'],full_1m_length_qualified=False)
def save():
    p=ROOT/'request-result.tmp';p.write_text(json.dumps(STATE,indent=2)+'\n');p.replace(ROOT/'request-result.json')
def ask(name,text,max_tokens):
    key=(service/'credentials/api-key.txt').read_text().strip()
    payload=dict(model='deepseek-v4.1-flash-1017-xq',messages=[dict(role='user',content=text)],max_tokens=max_tokens,temperature=0,seed=1,reasoning_effort='none',stream=True,stream_options=dict(include_usage=True))
    raw=json.dumps(payload).encode()
    if len(raw)>8*1024**2:raise RuntimeError('public full1M payload bound exceeded')
    req=urllib.request.Request(P['base_url']+'/chat/completions',data=raw,headers={'Authorization':'Bearer '+key,'Content-Type':'application/json'})
    start=time.monotonic();first=None;usage=None;finish=None;done=False;content='';total=0
    STATE.update(phase='requesting',current_test=name,request_started_unix=time.time(),payload_bytes=len(raw));save()
    try:
        response=urllib.request.build_opener(urllib.request.ProxyHandler({})).open(req,timeout=3000)
    except urllib.error.HTTPError as e:
        error_body=e.read(4097)
        if len(error_body)>4096:raise RuntimeError('bounded public API error body exceeded') from e
        STATE.update(http_status=e.code,http_error_body=error_body.decode('utf-8',errors='replace').replace(key,'[REDACTED]'));save();raise
    with response:
        status=response.status;STATE.update(phase='prefilling',http_status=status);save()
        for line in response:
            total+=len(line)
            if total>1024**2 or time.monotonic()-start>3000:raise RuntimeError('full1M SSE response budget exceeded')
            if not line.startswith(b'data:'):continue
            value=line[5:].strip()
            if value==b'[DONE]':done=True;break
            obj=json.loads(value)
            for c in obj.get('choices',[]):
                part=c.get('delta',{}).get('content') or '';content+=part
                if part and first is None:first=time.monotonic()
                finish=c.get('finish_reason') or finish
            usage=obj.get('usage') or usage
            if len(content)>32768:raise RuntimeError('full1M completion text bound exceeded')
    row=dict(name=name,status=status,done=done,finish_reason=finish,content=content,usage=usage,ttft_seconds=first-start if first else None,seconds=time.monotonic()-start)
    STATE['tests'].append(row);save()
    if status!=200 or not done or not finish or usage is None or first is None:raise RuntimeError(name+' incomplete generation')
    return row
try:
    os.umask(0o077);save()
    # Same exact tokenizer/template used by the service, no token estimates from character counts.
    sys.path[:0]=[str(service/'python-deps-v2'),str(service/'source'),str(service/'source/tools/v41/serve')]
    import serve_ds41 as D
    context=P['context_tokens'];slack=P['context_safety_tokens'];max_output=64
    if context!=1048576 or slack!=8 or D.S.CTX_SLACK!=slack:raise RuntimeError('declared1M context safety margin differs from actual API source')
    key=(service/'credentials/api-key.txt').read_text().strip()
    health_req=urllib.request.Request(P['base_url'].removesuffix('/v1')+'/health',headers={'Authorization':'Bearer '+key})
    with urllib.request.build_opener(urllib.request.ProxyHandler({})).open(health_req,timeout=10) as response:health=json.load(response)
    if not health.get('loaded') or health.get('max_context')!=context:raise RuntimeError('actual isolated1M API not ready')
    STATE.update(health=dict(loaded=True,max_context=context),context_safety_tokens=slack);save()
    vocab=json.loads((service/'tokenizer/vocab.json').read_text());tokens=[None]*len(vocab)
    for word,i in vocab.items():tokens[i]=word
    tk=D.Ds41Tokenizer(tokens,(service/'tokenizer/merges.txt').read_text().split('\n'),json.loads((service/'tokenizer/token_type.json').read_text()));template=D.Ds41Template()
    def render(s):return template.render([dict(role='user',content=s)],enable_thinking=False)
    def count(s):return len(tk.encode(render(s),parse_special=True))
    expected=dict(begin=731927,middle=408563,end=962814)
    prefix='Public retrieval test. Memorize the three record codes below. Ignore filler words.\nBEGIN_RECORD_CODE: 731927\nFiller:'
    middle='\nMIDDLE_RECORD_CODE: 408563\nFiller:'
    suffix='\nEND_RECORD_CODE: 962814\nReturn the three record codes in a plain JSON object with keys begin, middle, end and integer values. No markdown or commentary.'
    target=context-slack-max_output;n=target-count(prefix+middle+suffix)
    def fixture(n):return prefix+(' cat'*(n//2))+middle+(' cat'*(n-n//2))+suffix
    text=fixture(n);actual=count(text)
    for _ in range(8):
        if actual==target:break
        n+=target-actual;text=fixture(n);actual=count(text)
    if actual!=target or actual+max_output+slack!=context:raise RuntimeError('exact full1M input/output capacity preflight failed')
    raw=text.encode();(ROOT/'public-fixture.txt').write_bytes(raw)
    rendered=render(text);positions={k:len(tk.encode(rendered[:rendered.index(marker)],parse_special=True)) for k,marker in [('begin','BEGIN_RECORD_CODE:'),('middle','MIDDLE_RECORD_CODE:'),('end','END_RECORD_CODE:')]}
    STATE.update(phase='fixture_verified',actual_prompt_tokens=actual,max_new_tokens=max_output,prompt_plus_output_tokens=actual+max_output,total_reserved_tokens=context,prompt_sha256=hashlib.sha256(raw).hexdigest(),prompt_bytes=len(raw),record_token_positions=positions,expected=expected);save()
    cold=ask('cold-full1m-three-records',text,64)
    if cold['usage']['prompt_tokens']!=target:raise RuntimeError('actual API did not account for exact full1M input')
    if json.loads(cold['content'])!=expected:raise RuntimeError('full1M begin/middle/end retrieval output mismatch')
    STATE['full_1m_length_qualified']=True;STATE['three_position_retrieval_passed']=True;save()
    short=ask('interleaved-isolation-53','Calculate 19 + 34. Reply with only the integer answer.',16)
    if short['content'].strip()!='53':raise RuntimeError('interleaved short request isolation failed')
    warm=ask('restored-full1m-three-records',text,64)
    if warm['usage']['prompt_tokens']!=target or warm['usage'].get('prompt_tokens_details',{}).get('cached_tokens',0)<target-8 or json.loads(warm['content'])!=expected:raise RuntimeError('full1M KV restore/accounting/identity failed')
    STATE.update(phase='passed',full1m_cache_restore_passed=True,finished_unix=time.time());save()
except BaseException as e:
    STATE.update(phase='failed',error=type(e).__name__+': '+str(e),finished_unix=time.time());save();raise
