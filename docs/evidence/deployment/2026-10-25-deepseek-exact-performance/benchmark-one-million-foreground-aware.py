import hashlib,http.client,json,pathlib,socket,subprocess,sys,threading,time,urllib.request
cfg=json.load(sys.stdin)
out=pathlib.Path(cfg["out"]);out.mkdir(parents=True,exist_ok=False)
fixture=pathlib.Path("/data/deepseek-v41-xq-1017/one-million-context-fixture")
assert (fixture/"fixture.json").stat().st_size<=65536 and (fixture/"prompt.txt").stat().st_size<=8*1024*1024
meta=json.loads((fixture/"fixture.json").read_text());prompt=(fixture/"prompt.txt").read_text()
assert meta["prompt_sha256"]=="539aadd60c25d9abfbfc4defab2a3c658b9abc9e78cd45355b3b0930bdd12489"
assert hashlib.sha256(prompt.encode()).hexdigest()==meta["prompt_sha256"]
def get(u):
 with urllib.request.urlopen(u,timeout=5) as r:raw=r.read(8*1024*1024+1)
 assert len(raw)<=8*1024*1024,"Observation exceeded byte bound"
 return json.loads(raw)
lock=threading.Lock()
def state(stage,**facts):
 with lock:
  v={"at_epoch":time.time(),"stage":stage,**facts}
  (out/"progress.json").write_text(json.dumps(v,indent=2));print(json.dumps(v),flush=True)
def verify_identity():
 expected=cfg["expected_engine"]
 pid=expected["pid"]
 actual=pathlib.Path("/proc/"+str(pid)+"/cmdline").read_bytes().decode().rstrip("\0").split("\0")
 assert actual==expected["argv"],"Engine argv changed"
 power_pid=int(subprocess.check_output(["systemctl","show","a3s-power-deepseek-gpu6.service","-p","MainPID","--value"],text=True))
 assert power_pid==cfg["expected_power_pid"],"Power owner changed"
 with pathlib.Path("/proc/"+str(power_pid)+"/exe").open("rb") as f:power_sha=hashlib.file_digest(f,"sha256").hexdigest()
 assert power_sha==cfg["expected_power_sha256"],"Power binary changed"
 rows=subprocess.check_output(["nvidia-smi","--query-compute-apps=gpu_uuid,pid","--format=csv,noheader"],text=True).splitlines()
 placements=[parts[0] for row in rows if len(parts:=[v.strip() for v in row.split(",")])==2 and parts[1]==str(pid)]
 assert placements==["GPU-ab93a347-c678-1d71-0e85-0f520463debe"],"Engine is not exclusively on GPU6"
 for flag,value in [("--ctx","1048576"),("--route-bias","0"),("--route-bias-file","0"),("--route-bias-remote","0")]:
  assert actual.count(flag)==1 and actual[actual.index(flag)+1]==value
 return {"power_pid":power_pid,"power_sha256":power_sha,"engine_pid":pid,"gpu_uuid":"GPU-ab93a347-c678-1d71-0e85-0f520463debe","engine_argv":actual}
def resource_observation():
 group=pathlib.Path("/sys/fs/cgroup/system.slice/a3s-power-deepseek-gpu6.service")
 mem=dict(l.split(":",1) for l in pathlib.Path("/proc/meminfo").read_text().splitlines() if ":" in l)
 return {"at_epoch":time.time(),"unit_memory_gib":int((group/"memory.current").read_text())/2**30,
  "cpu_stat":dict(l.split() for l in (group/"cpu.stat").read_text().splitlines()),
  "memory_events":dict(l.split() for l in (group/"memory.events").read_text().splitlines()),
  "host_available_gib":int(mem["MemAvailable"].split()[0])/2**20,
  "gpu6_util_memory_total_power":subprocess.check_output(["nvidia-smi","-i","6","--query-gpu=utilization.gpu,memory.used,memory.total,power.draw","--format=csv,noheader,nounits"],text=True,timeout=5).strip()}

def safe_status():
 s=get("http://127.0.0.1:18106/status")
 result={k:s.get(k) for k in ("busy","phase","elapsed_s","generated","queued","prompt_tokens","tokens_per_s_mean")}
 if s.get("busy") and s.get("prompt_tokens")==meta["rendered_prompt_tokens"]:
  live=get("http://127.0.0.1:18106/metrics")["live"]
  for key in ("prompt_read","prompt_total","prefill_tok_s_mean"):
   value=live.get(key);result[key]=value if type(value) in (int,float) else None
 return result
identity_before=verify_identity()
(out/"identity-before.json").write_text(json.dumps(identity_before,indent=2))
(out/"profile.json").write_text(json.dumps(cfg,indent=2))
(out/"fixture.json").write_text(json.dumps(meta,indent=2))
state("waiting_for_foreground_idle")
idle_since=None
while True:
 try:
  a=get("http://127.0.0.1:18206/health")["worker"]["admission"];s=safe_status()
  idle=a["active"]==0 and a["waiting"]==0 and s["busy"] is False and not s["queued"]
  if idle:
   if idle_since is None:idle_since=time.monotonic()
   elif time.monotonic()-idle_since>=cfg.get("idle_seconds",60):break
  else:idle_since=None
 except Exception:idle_since=None
 time.sleep(1)
verify_identity()
body={"model":"deepseek-v41-flash-1017-xq-1m","messages":[{"role":"user","content":prompt}],"max_tokens":cfg.get("max_tokens",1024),"temperature":0,"seed":42,"stream":True,"stream_options":{"include_usage":True}}
wire=json.dumps(body,ensure_ascii=False).encode()
conn=http.client.HTTPConnection("127.0.0.1",18106,timeout=7200)
started=time.monotonic();stop=threading.Event();yielded=threading.Event();reason={}
state("submitting_one_million",request_utf8_bytes=len(wire),request_sha256=hashlib.sha256(wire).hexdigest())
conn.request("POST","/v1/chat/completions",body=wire,headers={"Content-Type":"application/json"})
owned_socket=conn.sock
def monitor():
 last_resource=0.0
 while not stop.wait(1):
  try:
   a=get("http://127.0.0.1:18206/health")["worker"]["admission"];s=safe_status()
   if stop.is_set():return
   if a["active"]>0 or a["waiting"]>0 or s["queued"]>0:
    reason.update(reason="foreground_request_arrived",power_active=a["active"],power_waiting=a["waiting"],native_queued=s["queued"])
    yielded.set()
    try:owned_socket.shutdown(socket.SHUT_RDWR)
    except OSError:pass
    state("yielding_to_foreground",**reason);return
   if stop.is_set():return
   if time.monotonic()-last_resource>=10:
    observation=resource_observation()
    with (out/"resource-observations.jsonl").open("a") as f:f.write(json.dumps(observation)+"\n")
    last_resource=time.monotonic()
   if stop.is_set():return
   state("one_million_running",wall_seconds=round(time.monotonic()-started,2),native=s)
  except Exception as e:state("monitor_observation_error",error_type=type(e).__name__)
watcher=threading.Thread(target=monitor,daemon=True);watcher.start()
content="";reasoning="";usage=None;timings=None;done=False;finish=None;events=0;ttft=None;first_content=None
try:
 response=conn.getresponse()
 assert response.status==200,"native HTTP "+str(response.status)
 assert response.getheader("Content-Type","").startswith("text/event-stream")
 frame=[]
 with (out/"response.sse").open("wb") as raw:
  while True:
   line=response.readline(8*1024*1024+1)
   if not line:break
   if len(line)>8*1024*1024:raise RuntimeError("SSE line exceeded bound")
   raw.write(line);raw.flush()
   if line.strip():frame.append(line);continue
   fields=[v[5:].lstrip().rstrip(b"\r\n") for v in frame if v.startswith(b"data:")];frame=[]
   if not fields:continue
   data=b"\n".join(fields)
   if data==b"[DONE]":done=True;stop.set();break
   event=json.loads(data);events+=1
   if event.get("error"):raise RuntimeError("Native SSE error")
   if event.get("usage"):usage=event["usage"]
   if event.get("timings"):timings=event["timings"]
   for choice in event.get("choices",[]):
    delta=choice.get("delta",{});text=delta.get("content") or "";thought=delta.get("reasoning_content") or ""
    if ttft is None and (text or thought):ttft=time.monotonic()-started
    if first_content is None and text:first_content=time.monotonic()-started
    content+=text;reasoning+=thought
    if choice.get("finish_reason"):finish=choice["finish_reason"]
 if yielded.is_set():raise InterruptedError("Yielded owned benchmark socket for foreground request")
 assert done and finish=="stop","One-million generation did not finish normally"
 assert usage and usage["prompt_tokens"]>=1000000 and usage["completion_tokens"]>0
 assert usage["total_tokens"]==usage["prompt_tokens"]+usage["completion_tokens"]
 assert content.strip()==meta["expected_exact"],"One-million needle retrieval mismatch"
 assert timings and timings["prompt_n"]+timings.get("cache_n",0)==usage["prompt_tokens"]
 stop.set();watcher.join(timeout=12)
 identity_after=verify_identity();(out/"identity-after.json").write_text(json.dumps(identity_after,indent=2))
 if cfg.get("require_cold",True):assert timings.get("cache_n",0)==0,"Full cold prefill was not measured"
 result={"ttft_seconds":ttft,"first_content_seconds":first_content,"success":True,"full_one_million_completed":True,"retrieval_passed":True,"content":content,"reasoning_content":reasoning,"usage":usage,"timings":timings,"wall_seconds":time.monotonic()-started,"events":events,"finish_reason":finish,"fixture_sha256":meta["prompt_sha256"]}
 (out/"result.json").write_text(json.dumps(result,ensure_ascii=False,indent=2));state("complete",**{k:v for k,v in result.items() if k!="reasoning_content"})
except BaseException as e:
 stop.set()
 result={"content":content,"reasoning_content":reasoning,"ttft_seconds":ttft,"first_content_seconds":first_content,"success":False,"full_one_million_completed":False,"yielded_to_foreground":yielded.is_set(),"error_type":type(e).__name__,"reason":str(e),"foreground":reason,"wall_seconds":time.monotonic()-started,"received_events":events,"usage":usage,"timings":timings}
 (out/"result.json").write_text(json.dumps(result,indent=2));state("interrupted_for_foreground" if yielded.is_set() else "failed",**result)
 raise SystemExit(75 if yielded.is_set() else 1)
finally:
 stop.set()
 try:owned_socket.shutdown(socket.SHUT_RDWR)
 except OSError:pass
 conn.close();watcher.join(timeout=6)
