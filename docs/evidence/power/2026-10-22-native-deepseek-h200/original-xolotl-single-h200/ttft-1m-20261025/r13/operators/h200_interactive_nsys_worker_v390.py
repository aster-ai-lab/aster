"""Finite real-GPU start/stop/export proof before another expensive model load."""
import hashlib,json,os,queue,signal,sqlite3,subprocess,sys,threading,time
from pathlib import Path
from aster_nsys_lifecycle_v302 import require_no_active_sessions
ROOT=Path(__file__).resolve().parent;P=json.loads((ROOT/'plan.json').read_text())
STATE=dict(unix=time.time(),phase='preflight',model_executed=False,raw_reports_published=False,GPU_kernel_replay_used=False)
NSYS='/usr/local/cuda/bin/nsys';SESSION='aster-h200-nsys-start-v390';child=None;pidfd=None;capturing=False
def save():
 p=ROOT/'result.tmp';p.write_text(json.dumps(STATE,indent=2)+'\n');p.replace(ROOT/'result.json')
def stop(signum,frame):raise InterruptedError('bounded exact interactive preflight deadline')
def run(args,name,timeout,env):
 with (ROOT/(name+'.log')).open('x') as out:r=subprocess.run(args,cwd=ROOT,env=env,stdout=out,stderr=out,timeout=timeout)
 return r.returncode
def exact(h):
 p=Path('/proc/'+str(h['pid']))
 try:
  f=(p/'stat').read_text().split(') ',1)[1].split()
  return p.stat().st_uid==10250 and int(f[19])==h['start_ticks'] and f[0]!='Z'
 except FileNotFoundError:return False
def finish_target():
 p=ROOT/'target-process.json'
 if p.exists():
  h=json.loads(p.read_text())
  if exact(h):
   fd=os.pidfd_open(h['pid'])
   try:
    if exact(h):signal.pidfd_send_signal(fd,signal.SIGTERM)
   finally:os.close(fd)
try:
 os.umask(0o077);signal.signal(signal.SIGTERM,stop);signal.signal(signal.SIGALRM,stop);signal.alarm(170);save()
 if os.getuid()!=10250 or ROOT.resolve()!=ROOT or ROOT.stat().st_uid!=10250:raise RuntimeError('exact interactive smoke owner differs')
 proof=json.loads((ROOT.parent/'resource-result.json').read_text())
 if proof['phase']!='passed' or proof['gpus'][0]['uuid']!=P['gpu_uuid']:raise RuntimeError('exact physical H200 proof differs')
 info=subprocess.run(['nvidia-smi','--query-gpu=index,uuid,pci.bus_id','--format=csv,noheader,nounits'],capture_output=True,text=True,check=True,timeout=5).stdout.strip()
 if info!=P['NVML_inventory_line']:raise RuntimeError('explicit Nsight index physical mapping changed')
 apps=subprocess.run(['nvidia-smi','--query-compute-apps=pid','--format=csv,noheader,nounits'],capture_output=True,text=True,check=True,timeout=5).stdout.strip()
 if apps:raise RuntimeError('tiny interactive preflight requires idle optimization GPU')
 for row in P['files']:
  p=ROOT/row['file'];b=p.read_bytes()
  if p.is_symlink() or len(b)!=row['bytes'] or hashlib.sha256(b).hexdigest()!=row['sha256']:raise RuntimeError('frozen interactive proof file changed')
 env={k:os.environ[k] for k in ['PATH','LD_LIBRARY_PATH'] if k in os.environ};env.update(CUDA_VISIBLE_DEVICES='',LC_ALL='C',OMP_NUM_THREADS='1')
 if run(['/usr/local/cuda/bin/nvcc','-std=c++17','-O2','--generate-code=arch=compute_90,code=sm_90',str(ROOT/'h200_interactive_nsys_smoke_v314.cu'),'-o',str(ROOT/'smoke')],'compile',35,env):raise RuntimeError('tiny interactive CUDA compile failed')
 inventory=subprocess.run([NSYS,'sessions','list','--show-header=false'],capture_output=True,text=True,timeout=8);require_no_active_sessions(inventory)
 target=ROOT/'target.py';target.write_text("import json,os,sys\nfrom pathlib import Path\nr=Path(__file__).resolve().parent\nh=dict(pid=os.getpid(),start_ticks=int(Path('/proc/self/stat').read_text().split(') ',1)[1].split()[19]),uid=os.getuid())\n(r/'target-process.json').write_text(json.dumps(h))\nos.execv(str(r/'smoke'),[str(r/'smoke'),sys.argv[1]])\n")
 env['CUDA_VISIBLE_DEVICES']='0';events=queue.Queue(maxsize=128)
 def reader(pipe):
  for line in iter(pipe.readline,''):
   if len(line)>4096:break
   events.put(line.rstrip('\n'))
  events.put(None)
 def until(prefix):
  end=time.monotonic()+20
  while time.monotonic()<end:
   line=events.get(timeout=max(.1,end-time.monotonic()))
   if line is None:raise RuntimeError('exact interactive target exited before '+prefix)
   if line.startswith(prefix):return line
  raise RuntimeError('bounded interactive target response timed out')
 with (ROOT/'target.stderr.log').open('x') as err:
  child=subprocess.Popen([NSYS,'launch','--session-new='+SESSION,'--trace=cuda','--cuda-graph-trace=node','--wait=primary','--show-output=true',sys.executable,'-u',str(target),P['gpu_uuid'].removeprefix('GPU-').replace('-','')],cwd=ROOT,env=env,stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=err,text=True,bufsize=1,start_new_session=True)
  pidfd=os.pidfd_open(child.pid);threading.Thread(target=reader,args=(child.stdout,),daemon=True).start();until('READY_SMOKE')
  STATE.update(phase='target_ready',target=json.loads((ROOT/'target-process.json').read_text()));save()
  command=[NSYS,'start','--session='+SESSION,'--sample=none','--cpuctxsw=none','--capture-range=none','--gpu-metrics-devices='+str(P['nsys_gpu_index']),'--gpu-metrics-set=gh100','--gpu-metrics-frequency=100','--output='+str(ROOT/'private-smoke')]
  STATE['start_command']=command;STATE['phase']='starting_capture';save()
  if run(command,'start',20,env):raise RuntimeError('explicit-index interactive start failed')
  capturing=True;child.stdin.write('GO\n');child.stdin.flush();line=until('DONE_SMOKE ');parts=line.split()
  if len(parts)!=3 or int(parts[1])!=65536 or int(parts[2])<=0:raise RuntimeError('real CUDA target output validation failed')
  STATE.update(checked_output_words=65536,kernel_launches=int(parts[2]),phase='stopping_capture');save()
  if run([NSYS,'stop','--session='+SESSION],'stop',35,env):raise RuntimeError('explicit interactive stop failed')
  capturing=False;child.stdin.write('QUIT\n');child.stdin.flush();child.stdin.close();child.wait(timeout=20)
  if child.returncode:raise RuntimeError('real target did not finish successfully')
 report=ROOT/'private-smoke.nsys-rep'
 if report.is_symlink() or not 0<report.stat().st_size<64*1024**2:raise RuntimeError('bounded tiny private report absent')
 database=ROOT/'private-smoke.sqlite';env['CUDA_VISIBLE_DEVICES']=''
 if run([NSYS,'export','--type=sqlite','--output='+str(database),str(report)],'export',35,env):raise RuntimeError('private interactive trace export failed')
 if database.is_symlink() or database.stat().st_size>128*1024**2:raise RuntimeError('bounded interactive database exceeded')
 db=sqlite3.connect(database.as_uri()+'?mode=ro&immutable=1',uri=True);db.execute('PRAGMA query_only=ON')
 tables=[r[0] for r in db.execute("SELECT name FROM sqlite_master WHERE type='table'")]
 if len(tables)>128:raise RuntimeError('bounded trace tables exceeded')
 schema={t:[dict(name=row[1],type=row[2]) for row in db.execute('PRAGMA table_info("'+t.replace('"','""')+'")')] for t in tables}
 if 'GPU_METRICS' not in tables or 'CUPTI_ACTIVITY_KIND_KERNEL' not in tables:raise RuntimeError('real CUDA kernel and GPU metric data required')
 counts={t:db.execute('SELECT COUNT(*) FROM "'+t+'"').fetchone()[0] for t in ['GPU_METRICS','CUPTI_ACTIVITY_KIND_KERNEL']}
 if any(n<=0 for n in counts.values()):raise RuntimeError('interactive CUDA or GPU metric trace empty')
 db.close();STATE.update(phase='passed',start_stop_export_passed=True,nsys_gpu_index=P['nsys_gpu_index'],gpu_uuid=P['gpu_uuid'],counts=counts,schema=schema,
  report=dict(bytes=report.stat().st_size,sha256=hashlib.sha256(report.read_bytes()).hexdigest()),database_bytes=database.stat().st_size,finished_unix=time.time());save()
except BaseException as e:STATE.update(phase='failed',error=type(e).__name__+': '+str(e),finished_unix=time.time());save();raise
finally:
 signal.alarm(0)
 if capturing:
  try:subprocess.run([NSYS,'stop','--session='+SESSION],capture_output=True,text=True,timeout=20)
  except Exception:pass
 finish_target()
 if child is not None and child.poll() is None:
  if pidfd is not None:signal.pidfd_send_signal(pidfd,signal.SIGTERM)
  try:child.wait(timeout=10)
  except subprocess.TimeoutExpired:signal.pidfd_send_signal(pidfd,signal.SIGKILL);child.wait(timeout=10)
 if pidfd is not None:os.close(pidfd)
 sessions=subprocess.run([NSYS,'sessions','list','--show-header=false'],capture_output=True,text=True,timeout=8)
 if sessions.returncode==0 and sessions.stdout.strip():
  subprocess.run([NSYS,'shutdown','--session='+SESSION,'--kill=none'],capture_output=True,text=True,timeout=15)
  sessions=subprocess.run([NSYS,'sessions','list','--show-header=false'],capture_output=True,text=True,timeout=8)
 try:require_no_active_sessions(sessions);STATE['sessions_closed']=True
 except ValueError as e:STATE.update(phase='failed',sessions_closed=False,error=str(e))
 STATE['target_reaped']=not (ROOT/'target-process.json').exists() or not exact(json.loads((ROOT/'target-process.json').read_text()))
 STATE['launcher_reaped']=child is None or child.poll() is not None
 STATE['GPU_idle_after']=not subprocess.run(['nvidia-smi','--query-compute-apps=pid','--format=csv,noheader,nounits'],capture_output=True,text=True,check=True,timeout=5).stdout.strip()
 if not STATE['target_reaped'] or not STATE['launcher_reaped'] or not STATE['GPU_idle_after']:STATE.update(phase='failed',error='exact tiny preflight cleanup not proved')
 save()
