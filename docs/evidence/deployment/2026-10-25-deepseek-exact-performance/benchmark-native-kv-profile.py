from pathlib import Path
import hashlib,json,statistics,sys,time,urllib.request,uuid
cfg=json.load(sys.stdin)
out=Path(cfg["out"]);out.mkdir(exist_ok=False)
model="deepseek-v41-flash-1017-xq-1m"
quality_cases=[
 ("arithmetic","Return a JSON object with one key answer, whose integer value is (19*23)-(17*11). No Markdown.",{"answer":250}),
 ("python-slices","For Python 3, let a=[2,5,8,11,14]. Evaluate sum(a[1::2]). Return only JSON with key answer and the integer result.",{"answer":16}),
 ("rust-overflow","In Rust, what integer does u8::MAX.wrapping_add(2) return? Return only JSON with key answer and that integer.",{"answer":1}),
 ("json-unicode",'Return only this exact JSON object, preserving the Chinese characters: {"city":"杭州","ok":true}',{"city":"杭州","ok":True}),
 ("sql-three-valued-logic","In SQL, WHERE keeps only TRUE. What row count results from SELECT COUNT(*) FROM (VALUES (1),(NULL),(3)) AS t(x) WHERE x <> 1? Return only JSON with key answer and the integer count.",{"answer":1})]
def state(stage,**facts):
 v={"at_epoch":time.time(),"stage":stage,"profile":cfg["profile"],**facts}
 (out/"progress.json").write_text(json.dumps(v,indent=2));print(json.dumps(v),flush=True)
def receive_stream(req,started):
 content="";reasoning="";finish=None;usage=None;timings=None;first=None;first_content=None;identity={}
 wire_path=out/("stream-"+uuid.uuid4().hex+".sse")
 with urllib.request.urlopen(req,timeout=600) as r,wire_path.open("wb") as retained:
  status=r.status
  assert status==200 and r.headers.get("Content-Type","").startswith("text/event-stream")
  frame=[];frame_size=0;done=False
  for line in r:
   retained.write(line)
   frame_size+=len(line)
   assert frame_size<=8*1024*1024,"Native SSE frame exceeded bound"
   if line.strip():frame.append(line);continue
   fields=[v[5:].lstrip().rstrip(b"\r\n") for v in frame if v.startswith(b"data:")]
   frame=[];frame_size=0
   if not fields:continue
   data=b"\n".join(fields)
   if data==b"[DONE]":done=True;break
   event=json.loads(data)
   assert not event.get("error"),"Native stream returned error"
   for k in ("id","created","model"):
    if k in event:identity[k]=event[k]
   if event.get("usage"):usage=event["usage"]
   if event.get("timings"):timings=event["timings"]
   for choice in event.get("choices",[]):
    assert choice.get("index",0)==0
    delta=choice.get("delta",{})
    assert not delta.get("tool_calls"),"Benchmark supplied no tools"
    text=delta.get("content") or "";thought=delta.get("reasoning_content") or ""
    if first is None and (text or thought):first=time.monotonic()-started
    if first_content is None and text:first_content=time.monotonic()-started
    content+=text;reasoning+=thought
    if choice.get("finish_reason"):finish=choice["finish_reason"]
  assert done and finish in ("stop","length") and usage and timings and first is not None
 response={**identity,"object":"chat.completion","choices":[{"index":0,"message":{"role":"assistant","content":content or None,"reasoning_content":reasoning},"finish_reason":finish}],"usage":usage,"timings":timings}
 return status,response,{"ttft_seconds":first,"first_content_seconds":first_content,"sse_file":wire_path.name}

def call(prompt,route,tokens=512):
 while True:
  try:
   with urllib.request.urlopen("http://127.0.0.1:18206/health",timeout=4) as health_response:admission=json.load(health_response)["worker"]["admission"]
   with urllib.request.urlopen("http://127.0.0.1:18106/status",timeout=4) as status_response:status_now=json.load(status_response)
   if admission["active"]==0 and admission["waiting"]==0 and status_now.get("busy") is False and not status_now.get("queued",0):break
   state("waiting_for_foreground_idle",active=admission["active"],waiting=admission["waiting"],native_busy=status_now.get("busy"),native_phase=status_now.get("phase"))
  except Exception as error:state("waiting_for_foreground_idle",health_error=type(error).__name__)
  time.sleep(2)
 body={"model":model,"messages":[{"role":"user","content":prompt}],"max_tokens":tokens,"temperature":0,"seed":42,"stream":cfg.get("stream_io",False)}
 if body["stream"]:body["stream_options"]={"include_usage":True}
 if route=="exact":body["strata_tune"]={"route_bias":0.0,"route_bias_file":0.0,"route_bias_remote":0.0}
 raw=json.dumps(body,ensure_ascii=False).encode()
 req=urllib.request.Request("http://127.0.0.1:18106/v1/chat/completions",data=raw,headers={"Content-Type":"application/json"})
 started=time.monotonic()
 observations={}
 if body["stream"]:status,response,observations=receive_stream(req,started)
 else:
  with urllib.request.urlopen(req,timeout=600) as r:response=json.load(r);status=r.status
 usage=response["usage"];timings=response["timings"]
 assert status==200 and usage["prompt_tokens"]+usage["completion_tokens"]==usage["total_tokens"]
 with (out/"raw-responses.jsonl").open("a") as log:log.write(json.dumps({"at_epoch":time.time(),"request_sha256":hashlib.sha256(raw).hexdigest(),"response":response},ensure_ascii=False)+"\n")
 assert timings["prompt_n"]+timings.get("cache_n",0)==usage["prompt_tokens"] and timings["predicted_n"]>0
 return {**observations,"wall_seconds":time.monotonic()-started,"request_sha256":hashlib.sha256(raw).hexdigest(),"request_utf8_bytes":len(raw),"route":route,"response":response}
try:
 (out/"profile.json").write_text(json.dumps(cfg,indent=2))
 qualities=[];decodes=[];prefills=[]
 for route in cfg.get("routes",["default"]):
  for ident,prompt,expected in quality_cases:
   state("quality_running",route=route,case=ident)
   result=call(prompt,route);content=result["response"]["choices"][0]["message"].get("content") or ""
   try:value=json.loads(content.strip())
   except ValueError:value=None
   result.update(case=ident,correct=value==expected,expected=expected);qualities.append(result)
   (out/"quality.json").write_text(json.dumps(qualities,ensure_ascii=False,indent=2))
   assert result["correct"],"quality mismatch: "+ident
  prompt="Write a detailed numbered explanation of how a CPU, RAM, NVMe storage, PCIe, GPU VRAM and CUDA kernels cooperate to run a mixture-of-experts language model. Use at least 600 words. Be concrete and continue until all components and bottlenecks are covered."
  for trial in range(cfg.get("decode_trials",4)):
   state("decode_running",route=route,trial=trial)
   result=call(prompt,route,128);result.update(trial=trial,warmup=trial==0);decodes.append(result)
   (out/"decode-trials.json").write_text(json.dumps(decodes,ensure_ascii=False,indent=2))
   state("decode_trial_complete",route=route,trial=trial,timings=result["response"]["timings"])
 if cfg.get("prefill",True):
  archive=Path("/data/deepseek-v41-xq-1017/one-million-context-fixture/prompt.txt").read_text()[:90000]
  for trial in range(3):
   nonce=uuid.uuid4().hex
   prompt="Timing trial "+nonce+".\n"+archive+"\nThis is a truncated prefill timing exercise. Reply with exactly OK."
   state("prefill_running",trial=trial,prompt_utf8_bytes=len(prompt.encode()))
   result=call(prompt,"default",512);content=(result["response"]["choices"][0]["message"].get("content") or "").strip()
   result.update(trial=trial,correct=content=="OK");prefills.append(result)
   (out/"prefill-trials.json").write_text(json.dumps(prefills,ensure_ascii=False,indent=2))
   assert result["correct"] and result["response"]["usage"]["prompt_tokens_details"]["cached_tokens"]==0
   state("prefill_trial_complete",trial=trial,timings=result["response"]["timings"])
 summary={"profile":cfg["profile"],"quality_passed":len(qualities),"decode_medians":{route:statistics.median(r["response"]["timings"]["predicted_per_second"] for r in decodes if r["route"]==route and not r["warmup"]) for route in cfg.get("routes",["default"])},"prefill_median":statistics.median(r["response"]["timings"]["prompt_per_second"] for r in prefills) if prefills else None,"full_one_million_completed":False}
 (out/"summary.json").write_text(json.dumps(summary,indent=2));state("complete",**summary)
except BaseException as error:
 state("failed",error_type=type(error).__name__,reason=str(error));raise
