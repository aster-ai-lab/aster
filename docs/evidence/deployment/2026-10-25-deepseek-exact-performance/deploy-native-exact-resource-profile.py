import hashlib,json,os,pathlib,shutil,subprocess,sys,time,urllib.request
B=pathlib.Path("/data/deepseek-v41-xq-1017")
CFG=pathlib.Path("/etc/a3s/power/deepseek-gpu6.acl")
UNIT="a3s-power-deepseek-gpu6.service"
C=json.loads(pathlib.Path(sys.argv[1]).read_text())
OUT=B/C["receipt"]
PRESERVED=["a3s-gateway.service",*[f"prism-27b@{i}.service" for i in range(6)],*[f"a3s-power@{i}.service" for i in range(6)],"qwen3-asr.service","qwen3-asr-b.service"]
proof={"at_epoch":time.time(),"profile":C["profile"],"phase":"preflight","activated":False,"success":False}
def sha(p):
 with pathlib.Path(p).open("rb") as f:return hashlib.file_digest(f,"sha256").hexdigest()
def props(u):return dict(l.split("=",1) for l in subprocess.check_output(["systemctl","show",u,"-p","MainPID","-p","ActiveState"],text=True).splitlines())
def get(u):
 with urllib.request.urlopen(u,timeout=5) as r:return json.load(r)
def save(p):
 proof.update(phase=p,updated_at_epoch=time.time());OUT.write_text(json.dumps(proof,indent=2));OUT.chmod(0o644);print(json.dumps({"phase":p,"profile":C["profile"]}),flush=True)
def atomic(src,dst):
 t=dst.with_name("."+dst.name+"-resource-"+str(os.getpid()));s=src.stat();shutil.copy2(src,t);os.chown(t,s.st_uid,s.st_gid);os.replace(t,dst)
def restart():subprocess.run(["systemctl","restart",UNIT],check=True,capture_output=True,timeout=55)
def ready():
 deadline=time.monotonic()+900
 while time.monotonic()<deadline:
  try:
   h=get("http://127.0.0.1:18206/health");n=get("http://127.0.0.1:18106/health")
   if h.get("status")=="ok" and h.get("loaded_models")==1 and n.get("loaded") is True and n.get("max_context")==1048576:return h,n
  except Exception:pass
  if props(UNIT)["ActiveState"]=="failed":raise RuntimeError("Native resource profile failed during startup")
  time.sleep(1)
 raise RuntimeError("Native profile readiness deadline exceeded")
old=None
try:
 if os.geteuid()!=0:raise RuntimeError("Root required")
 if sha(CFG)!=C["old_cfg_sha"] or props(UNIT)!={"MainPID":str(C["old_pid"]),"ActiveState":"active"}:raise RuntimeError("Active native identity changed")
 if sha("/proc/"+str(C["old_pid"])+"/exe")!=C["power_sha"]:raise RuntimeError("Active native binary changed")
 if sha(C["power"])!=C["power_sha"]:raise RuntimeError("Candidate binary changed")
 staged=pathlib.Path(C["staged"])
 if staged.parent!=CFG.parent or not staged.name.startswith(".deepseek-gpu6-exact-resource-") or sha(staged)!=C["staged_sha"]:raise RuntimeError("Staged resource profile identity mismatch")
 checked=subprocess.run([C["power"],"serve","--config",str(staged),"--check-config"],capture_output=True,timeout=15)
 if checked.returncode:raise RuntimeError("Typed candidate refused profile")
 proof["preserved_services_before"]={u:props(u) for u in PRESERVED}
 deadline=time.monotonic()+900;idle_since=None;save("waiting_for_foreground_idle")
 while time.monotonic()<deadline:
  a=get("http://127.0.0.1:18206/health")["worker"]["admission"];s=get("http://127.0.0.1:18106/status")
  idle=a["active"]==0 and a["waiting"]==0 and s.get("busy") is False and not s.get("queued",0)
  if idle:
   if idle_since is None:idle_since=time.monotonic()
   elif time.monotonic()-idle_since>=2:break
  else:idle_since=None
  time.sleep(.2)
 else:raise RuntimeError("Foreground requests did not leave an idle cutover window")
 if sha(CFG)!=C["old_cfg_sha"] or props(UNIT)["MainPID"]!=str(C["old_pid"]):raise RuntimeError("Native identity changed while draining")
 backup=CFG.parent/"backups";backup.mkdir(mode=0o700,exist_ok=True)
 old=backup/("before-resource-"+C["old_cfg_sha"][:16]+".acl")
 if old.exists():
  if sha(old)!=C["old_cfg_sha"]:raise RuntimeError("Backup collision")
 else:
  s=CFG.stat();shutil.copy2(CFG,old);os.chown(old,s.st_uid,s.st_gid)
 atomic(staged,CFG);proof["activated"]=True;proof.update(old_acl_sha256=C["old_cfg_sha"],new_acl_sha256=C["staged_sha"],power_binary_sha256=C["power_sha"])
 save("restarting_gpu6_resource_profile");restart();save("loading_weights");h,n=ready()
 body={"model":"deepseek-v41-flash-1017-xq-1m","messages":[{"role":"user","content":"Return a JSON object with one key answer, whose integer value is (19*23)-(17*11). No Markdown."}],"max_tokens":512,"temperature":0,"seed":42,"stream":False}
 req=urllib.request.Request("http://127.0.0.1:18106/v1/chat/completions",data=json.dumps(body).encode(),headers={"Content-Type":"application/json"})
 save("validating_arithmetic")
 with urllib.request.urlopen(req,timeout=180) as r:reply=json.load(r)
 (B/(C["receipt"]+".smoke.json")).write_text(json.dumps(reply,indent=2))
 if reply["choices"][0]["finish_reason"]!="stop" or json.loads(reply["choices"][0]["message"]["content"])!={"answer":250}:raise RuntimeError("Exact profile arithmetic mismatch")
 engines=[]
 rows=subprocess.check_output(["nvidia-smi","--query-compute-apps=gpu_uuid,pid","--format=csv,noheader"],text=True).splitlines()
 for row in rows:
  gpu,pid=(v.strip() for v in row.split(","))
  if gpu!="GPU-ab93a347-c678-1d71-0e85-0f520463debe":continue
  argv=pathlib.Path("/proc/"+pid+"/cmdline").read_bytes().decode().rstrip("\0").split("\0")
  if "--serve" not in argv:continue
  for f,v in [("--ctx","1048576"),("--threads",str(C["threads"])),("--arena-gib",str(C["arena_gib"])),("--pcie",f'{C["pcie_bp"]/10000:.4f}'),("--route-bias","0"),("--route-bias-file","0"),("--route-bias-remote","0")]:
   if argv.count(f)!=1 or argv[argv.index(f)+1]!=v:raise RuntimeError("Engine resource argv mismatch")
  if argv.count("--kv-fp4")!=(1 if C.get("kv_fp4",True) else 0):raise RuntimeError("Selected KV precision was not preserved")
  engines.append({"pid":int(pid),"argv":argv})
 if len(engines)!=1:raise RuntimeError("Expected one GPU6 engine")
 proof["preserved_services_after"]={u:props(u) for u in PRESERVED}
 if proof["preserved_services_before"]!=proof["preserved_services_after"]:raise RuntimeError("Other model service identity changed")
 if sha(CFG)!=C["staged_sha"]:raise RuntimeError("Active profile changed")
 proof.update(success=True,new_pid=int(props(UNIT)["MainPID"]),engine=engines[0],context_tokens=n["max_context"],arithmetic_smoke_passed=True,other_service_pids_unchanged=True)
 save("ready_for_controlled_measurement")
except BaseException as e:
 proof.update(error_type=type(e).__name__,error=str(e))
 if proof["activated"] and old is not None:
  try:
   if sha(CFG)!=C["staged_sha"]:raise RuntimeError("Concurrent config change prevents fallback")
   atomic(old,CFG);restart();ready();proof["previous_exact_fp4_restored"]=True
  except BaseException as rollback:proof["rollback_error_type"]=type(rollback).__name__
 save("failed");raise SystemExit(1)
