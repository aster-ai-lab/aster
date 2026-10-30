"""Same-pod fault recovery only. Never allocates GPUs or changes model settings."""
import json,os,signal,subprocess,time,urllib.request
from pathlib import Path
from aster_stall_window_v415 import StallWindow

ROOT=Path(__file__).resolve().parent
PLAN=json.loads((ROOT/'guard-plan.json').read_text())
SERVICE=Path(PLAN['service_root']);STATE=dict(unix=time.time(),phase='starting',recoveries=[],optimization=False)
window=StallWindow();stopping=False
def save():
    p=ROOT/'guard-result.tmp';p.write_text(json.dumps(STATE,indent=2)+'\n');p.replace(ROOT/'guard-result.json')
def stop(signum,frame):
    global stopping
    stopping=True
def proc(pid):
    p=Path('/proc/'+str(pid));s=(p/'stat').read_text().split(') ',1)[1].split()
    if p.stat().st_uid!=10250 or s[0]=='Z':raise RuntimeError('owned live process required')
    io={k:int(v) for k,v in (line.split(':',1) for line in (p/'io').read_text().splitlines())}
    return dict(pid=pid,start_ticks=int(s[19]),parent=int(s[1]),cpu_seconds=(int(s[11])+int(s[12]))/os.sysconf('SC_CLK_TCK'),io_bytes=io['rchar']+io['wchar'],exe=os.readlink(p/'exe'))
def observe():
    state=json.loads((SERVICE/'service-longctx-v1-result.json').read_text());a=state['api_process'];api=proc(a['pid'])
    if api['start_ticks']!=a['start_ticks']:raise RuntimeError('API identity changed')
    native=[]
    for index,p in enumerate(Path('/proc').iterdir()):
        if index>=512:raise RuntimeError('bounded pod process inventory exceeded')
        if not p.name.isdecimal():continue
        try:
            if p.stat().st_uid!=10250 or os.readlink(p/'exe')!=PLAN['binary']:continue
            row=proc(int(p.name))
            if row['parent']==api['pid']:native.append(row)
        except (FileNotFoundError,PermissionError,ProcessLookupError):continue
    if len(native)!=1:raise RuntimeError('unique formal native child required')
    native=native[0]
    if os.getpgid(api['pid'])!=api['pid'] or os.getpgid(native['pid'])!=api['pid']:raise RuntimeError('exact supervised process group required')
    req=urllib.request.Request('http://127.0.0.1:30003/metrics',headers={'Authorization':'Bearer '+key,'Accept':'application/json'})
    with opener.open(req,timeout=5) as response:
        raw=response.read(1048577)
        if len(raw)>1048576:raise RuntimeError('bounded observation exceeded')
        live=json.loads(raw)['live']
    q=subprocess.run(['nvidia-smi','--query-gpu=uuid,utilization.gpu,utilization.memory','--format=csv,noheader,nounits'],capture_output=True,text=True,check=True,timeout=5)
    fields=q.stdout.strip().split(', ')
    if len(fields)!=3 or fields[0]!=PLAN['gpu_uuid']:raise RuntimeError('physical GPU identity differs')
    s=dict(now=time.monotonic(),identity=(api['pid'],api['start_ticks'],native['pid'],native['start_ticks']),
        active=live.get('state') in ('reading','generating') and (live.get('running') or 0)>0,
        gpu_idle=int(fields[1])==0 and int(fields[2])==0,cpu_seconds=api['cpu_seconds']+native['cpu_seconds'],
        io_bytes=api['io_bytes']+native['io_bytes'],progress=(live.get('prompt_tokens'),live.get('generated'),live.get('prompt_read'),live.get('running')))
    brief={k:live.get(k) for k in ['state','phase','prompt_tokens','prompt_read','prompt_total','generated','elapsed_s','running','waiting']}
    return s,api,native,brief,state

if os.getuid()!=10250 or ROOT.stat().st_uid!=10250 or ROOT.resolve()!=ROOT:raise RuntimeError('owned guard root required')
proof=json.loads((SERVICE.parent/'resource-result.json').read_text())
if proof['gpus'][0]['uuid']!=PLAN['gpu_uuid'] or proof['leaseEndsUnix']!=PLAN['lease_ends_unix']:raise RuntimeError('exact formal resource/lease required')
key=Path(PLAN['credential_file']).read_text().strip()
opener=urllib.request.build_opener(urllib.request.ProxyHandler({}))
signal.signal(signal.SIGTERM,stop);signal.signal(signal.SIGINT,stop)
STATE['phase']='watching';save()
while not stopping and time.time()<PLAN['lease_ends_unix']-90:
    try:
        s,api,native,live,state=observe();frozen=window.observe(s)
        STATE.update(unix=time.time(),phase='watching',last_sample=s,live=live,observation_errors=0)
        if frozen:
            if len(STATE['recoveries'])>=2 or state['api_attempt']>=3:
                STATE.update(phase='recovery_budget_exhausted',finished_unix=time.time());save();break
            # Revalidate immediately; no signal from stale or failed telemetry.
            fresh,aa,nn,ll,ss=observe()
            if fresh['identity']!=s['identity'] or fresh['progress']!=s['progress'] or not fresh['active'] or not fresh['gpu_idle'] or fresh['cpu_seconds']-s['cpu_seconds']>0.5 or fresh['io_bytes']-s['io_bytes']>1048576:raise RuntimeError('recovery precondition changed')
            STATE['recoveries'].append(dict(unix=time.time(),api=aa,native=nn,reason='90s active request with no CPU, IO, GPU or prompt/output progress'))
            STATE['phase']='recovering_exact_API_group';save()
            os.killpg(aa['pid'],signal.SIGTERM)
            window.reset();STATE.update(phase='waiting_for_existing_supervisor_restart',unix=time.time());save()
    except Exception as e:
        window.reset();STATE.update(unix=time.time(),observation_errors=STATE.get('observation_errors',0)+1,last_error_type=type(e).__name__)
    save()
    for _ in range(10):
        if stopping:break
        time.sleep(1)
else:STATE.update(phase='stopped',finished_unix=time.time());save()
