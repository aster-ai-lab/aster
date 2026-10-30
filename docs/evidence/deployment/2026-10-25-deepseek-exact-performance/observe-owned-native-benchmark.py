import json,pathlib,subprocess,sys,time,urllib.request
cfg=json.load(sys.stdin);out=pathlib.Path(cfg["out"]);pid=int(cfg["pid"])
group=pathlib.Path("/sys/fs/cgroup/system.slice/a3s-power-deepseek-gpu6.service")
def alive():
 try:return pathlib.Path("/proc/"+str(pid)+"/stat").read_text().rsplit(")",1)[1].split()[0]!="Z"
 except (FileNotFoundError,ProcessLookupError):return False
def counters():return dict(l.split() for l in (group/"cpu.stat").read_text().splitlines())
def progress():
 try:return json.loads((out/"progress.json").read_text()).get("stage")
 except (OSError,ValueError):return "unavailable"
deadline=time.monotonic()+10
while not out.exists() and alive() and time.monotonic()<deadline:time.sleep(.1)
if not out.is_dir():raise SystemExit("Benchmark output did not appear")
previous=counters();at=time.monotonic()
while alive():
 phase=progress()
 if phase in ("complete","failed"):break
 time.sleep(5)
 try:
  now=time.monotonic();current=counters()
  mem=dict(l.split(":",1) for l in pathlib.Path("/proc/meminfo").read_text().splitlines() if ":" in l)
  gpu=subprocess.check_output(["nvidia-smi","-i","6","--query-gpu=utilization.gpu,memory.used,memory.total,power.draw","--format=csv,noheader,nounits"],text=True,timeout=5).strip()
  with urllib.request.urlopen("http://127.0.0.1:18206/health",timeout=3) as r:admission=json.load(r)["worker"]["admission"]
  row={"at_epoch":time.time(),"profile":cfg["profile"],"phase_start":phase,"phase_end":progress(),"sample_seconds":now-at,
   "unit_cpu_percent":(int(current["usage_usec"])-int(previous["usage_usec"]))/((now-at)*10000),
   "unit_memory_gib":int((group/"memory.current").read_text())/2**30,
   "host_available_gib":int(mem["MemAvailable"].split()[0])/2**20,
   "host_swap_used_gib":(int(mem["SwapTotal"].split()[0])-int(mem["SwapFree"].split()[0]))/2**20,
   "memory_events":dict(l.split() for l in (group/"memory.events").read_text().splitlines()),
   "gpu6_util_memory_total_power":gpu,"power_admission":admission}
  with (out/"resource-observations.jsonl").open("a") as f:f.write(json.dumps(row)+"\n")
  previous=current;at=now
 except Exception as e:
  with (out/"resource-observations.jsonl").open("a") as f:f.write(json.dumps({"at_epoch":time.time(),"observation_error":type(e).__name__})+"\n")
print("observer finished",flush=True)
