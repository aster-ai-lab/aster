"""Public synthetic, isolated TTFT / KV affinity / head-of-line blocking proof."""
import concurrent.futures,hashlib,json,os,socket,statistics,time,urllib.request
from pathlib import Path
ROOT=Path(__file__).resolve().parent;P=json.loads((ROOT/'request-plan.json').read_text())
STATE=dict(schema='aster-xolotl-ttft-qualification-v9',unix=time.time(),phase='starting',client=socket.gethostname(),destination=P['base_url'],tests=[])
key=(Path(P['service_root'])/'credentials/api-key.txt').read_text().strip()
def save():
    p=ROOT/'request-result.tmp';p.write_text(json.dumps(STATE,indent=2)+'\n');p.replace(ROOT/'request-result.json')
def ask(name,text,max_tokens):
    payload=dict(model='deepseek-v4.1-flash-1017-xq',messages=[dict(role='user',content=text)],max_tokens=max_tokens,temperature=0,seed=1,reasoning_effort='none',stream=True,stream_options=dict(include_usage=True))
    req=urllib.request.Request(P['base_url']+'/chat/completions',data=json.dumps(payload).encode(),headers={'Authorization':'Bearer '+key,'Content-Type':'application/json'})
    t=time.monotonic();first=None;last=None;usage=None;finish=None;done=False;content='';total=0
    with urllib.request.build_opener(urllib.request.ProxyHandler({})).open(req,timeout=600) as response:
        status=response.status
        for raw in response:
            total+=len(raw)
            if total>2*1024**2:raise RuntimeError('bounded SSE response exceeded')
            if not raw.startswith(b'data:'):continue
            raw=raw[5:].strip()
            if raw==b'[DONE]':done=True;break
            obj=json.loads(raw)
            for c in obj.get('choices',[]):
                part=c.get('delta',{}).get('content') or ''
                if part:
                    now=time.monotonic();first=now if first is None else first;last=now;content+=part
                finish=c.get('finish_reason') or finish
            usage=obj.get('usage') or usage
    if status!=200 or not done or not finish or usage is None or not content.strip() or first is None:raise RuntimeError(name+' incomplete generation')
    return dict(name=name,ttft_seconds=first-t,seconds=time.monotonic()-t,first_to_last_text_seconds=last-first,finish_reason=finish,content=content,output_sha256=hashlib.sha256(content.encode()).hexdigest(),usage=usage)
def record(row):STATE['tests'].append(row);save();return row
def quantile(v,p):
    a=sorted(v);x=(len(a)-1)*p;lo=int(x);hi=min(lo+1,len(a)-1);return a[lo]+(a[hi]-a[lo])*(x-lo)
try:
    os.umask(0o077);save()
    req=urllib.request.Request(P['base_url'].removesuffix('/v1')+'/health',headers={'Authorization':'Bearer '+key})
    with urllib.request.urlopen(req,timeout=10) as q:health=json.load(q)
    if not health.get('loaded') or health.get('max_context')!=1048576:raise RuntimeError('isolated1M service not ready')
    STATE['health']=dict(loaded=True,max_context=1048576,total_slots=health.get('total_slots'));save()
    cases=json.loads((ROOT/'prompt-id-controls.json').read_text())['cases'];cases=[c for c in cases if c['name']!='arithmetic']
    STATE['phase']='short_requests';save()
    for i in range(20):
        c=cases[i%len(cases)];record(ask(('warmup-' if i<4 else 'measured-')+str(i)+'-'+c['name'],c['prompt'],c['max_new_tokens']))
    rows=STATE['tests'][4:20];ts=[x['ttft_seconds'] for x in rows];n=sum(x['usage']['completion_tokens'] for x in rows)
    STATE['short_summary']=dict(measured_requests=16,ttft_p50_seconds=quantile(ts,.5),ttft_p95_seconds=quantile(ts,.95),api_wall_completion_tok_s=n/sum(x['seconds'] for x in rows),stream_interval_tok_s_estimate=(n-len(rows))/sum(x['first_to_last_text_seconds'] for x in rows));save()
    raw=Path(P['public_fixture']).read_bytes()
    if hashlib.sha256(raw).hexdigest()!=P['fixture_sha256']:raise RuntimeError('public35700 fixture differs')
    text=raw.decode();STATE['phase']='cold_long';save();cold=record(ask('cold35700',text,64))
    if cold['usage']['prompt_tokens']!=35700 or cold['content'].strip()!='42':raise RuntimeError('cold35700 token/output control failed')
    record(ask('interleaved-arithmetic','Calculate 17 + 25. Reply with only the integer answer.',16))
    warm=record(ask('restored35700',text,64))
    if warm['content'].strip()!='42' or warm['usage'].get('prompt_tokens_details',{}).get('cached_tokens',0)<35000:raise RuntimeError('long KV cache restore failed')
    # Changing every filler token defeats prefix reuse while keeping public prompt size bounded.
    mixed=text.replace(' cat',' dog')
    STATE['phase']='mixed_long_short';save()
    with concurrent.futures.ThreadPoolExecutor(max_workers=2) as pool:
        f=pool.submit(ask,'mixed-cold-long',mixed,64);time.sleep(.5)
        short=pool.submit(ask,'mixed-short','Calculate 19 + 23. Reply with only the integer answer.',16)
        record(short.result());record(f.result())
    if any(x['content'].strip()!='42' for x in STATE['tests'][-2:]):raise RuntimeError('mixed request isolation/output failed')
    STATE.update(phase='passed',finished_unix=time.time());save()
except BaseException as e:
    STATE.update(phase='failed',error=type(e).__name__+': '+str(e),finished_unix=time.time());save();raise
