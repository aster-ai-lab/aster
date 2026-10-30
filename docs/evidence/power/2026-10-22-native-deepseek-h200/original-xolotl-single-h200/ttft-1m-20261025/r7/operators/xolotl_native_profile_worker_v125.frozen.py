"""One bounded original-engine trial; real token IDs and monotonic arrivals."""
import json
import math
import os
from pathlib import Path
import queue
import signal
import subprocess
import threading
import time

ROOT=Path(__file__).resolve().parent
PLAN=json.loads((ROOT/'plan.json').read_text())
RESULT=dict(schema='aster-original-xolotl-protocol-trial-v1',phase='preflight',unix=time.time(),plan=PLAN,requests=[])
def save():
    p=ROOT/'result.tmp'
    p.write_text(json.dumps(RESULT,ensure_ascii=False,indent=2)+'\n')
    p.replace(ROOT/'result.json')
def checked_file(path,size):
    p=Path(path)
    if not p.is_file() or p.stat().st_size!=size:raise RuntimeError('trial model input size differs: '+str(p))
    return p
def reader(pipe,events):
    try:
        for line in iter(pipe.readline,''):
            if len(line)>65536:raise RuntimeError('engine protocol line exceeds bound')
            events.put((time.monotonic(),line.rstrip('\n')),timeout=2)
    finally:events.put((time.monotonic(),None),timeout=2)

child=None;pidfd=None;stop=threading.Event();telemetry=[]
capture_active=False
NSYS='/usr/local/cuda/bin/nsys'
SESSION=ROOT.name
def profile_tool(action):
    command=[NSYS,action,'--session='+SESSION]
    if action=='start':command+=['--sample=none','--cpuctxsw=none','--output='+str(ROOT/'warm-prose')]
    with (ROOT/('nsys-'+action+'.log')).open('x') as out:
        p=subprocess.run(command,stdout=out,stderr=out,timeout=90)
    if p.returncode:raise RuntimeError('diagnostic profiler '+action+' failed')

def sample():
    while not stop.is_set() and len(telemetry)<1800:
        p=subprocess.run(['nvidia-smi','--query-gpu=index,uuid,utilization.gpu,memory.used,power.draw','--format=csv,noheader,nounits'],capture_output=True,text=True,timeout=8)
        telemetry.append(dict(unix=time.time(),exit=p.returncode,stdout=p.stdout[:4096]))
        stop.wait(1)

try:
    if ROOT!=Path(__file__).absolute().parent or ROOT.stat().st_uid!=os.getuid() or os.getuid()!=10250:raise RuntimeError('trial worker ownership differs')
    if time.time()+1800>PLAN['lease_ends_unix']:raise RuntimeError('insufficient immutable lease remaining')
    checked_file(PLAN['model'],162954903552)
    checked_file(PLAN['rank'],122880)
    active=subprocess.run(['nvidia-smi','--query-compute-apps=pid,gpu_uuid','--format=csv,noheader,nounits'],capture_output=True,text=True,check=True,timeout=10)
    if active.stdout.strip():raise RuntimeError('trial GPUs already have a workload')
    env={k:v for k,v in os.environ.items() if not k.startswith(('STRATA_','ASTER_XQ_','DS41_CHECK_ARENA'))}
    env.update(CUDA_VISIBLE_DEVICES='0',GGML_NO_BACKTRACE='1',OMP_NUM_THREADS='16',OPENBLAS_NUM_THREADS='1')
    env.update(PLAN['engine_env'])
    env.pop('STRATA_API_KEY',None)
    if PLAN['native_args'][PLAN['native_args'].index('--ctx')+1]!='1048576' or PLAN['native_args'][PLAN['native_args'].index('--prefill-chunk')+1]!='512':raise RuntimeError('exact current 1M/S512 native profile differs')
    import hashlib
    if hashlib.sha256(Path(PLAN['binary']).read_bytes()).hexdigest()!=PLAN['binary_sha256']:raise RuntimeError('exact current native binary SHA differs')
    command=[PLAN['binary'],'--serve']+PLAN['native_args']


    target_script=ROOT/'profile_target.py'
    target_script.write_text("import os,sys,json\nfrom pathlib import Path\nr=Path(__file__).resolve().parent\nb=sys.argv[1]\np=dict(pid=os.getpid(),start_ticks=int(Path('/proc/self/stat').read_text().split(') ',1)[1].split()[19]),uid=os.getuid(),binary=b)\n(r/'profile-target-process.json').write_text(json.dumps(p))\nos.execv(b,sys.argv[1:])\n")
    command=[NSYS,'launch','--session-new='+SESSION,'--trace=cuda,osrt','--cuda-graph-trace=node','--wait=primary','--show-output=true','python3','-u',str(target_script)]+command
    RESULT.update(command=command,phase='loading',cuda_visible_devices=env['CUDA_VISIBLE_DEVICES'],dspark=PLAN['dspark']);save()
    events=queue.Queue(maxsize=4096)
    with (ROOT/'engine.stderr.log').open('x',buffering=1) as stderr:
        child=subprocess.Popen(command,cwd=ROOT,env=env,stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=stderr,text=True,bufsize=1,start_new_session=True)
        pidfd=os.pidfd_open(child.pid)
        process=dict(pid=child.pid,start_ticks=int(Path('/proc/'+str(child.pid)+'/stat').read_text().split(') ',1)[1].split()[19]),uid=os.getuid(),binary=PLAN['binary'])
        (ROOT/'nsys-launch-process.json').write_text(json.dumps(dict(process,role='nsys-launch'),indent=2)+'\n')
        threading.Thread(target=reader,args=(child.stdout,events),daemon=True).start()
        sampler=threading.Thread(target=lambda:None,daemon=True);sampler.start()
        deadline=time.monotonic()+600
        while True:
            ts,line=events.get(timeout=max(.01,deadline-time.monotonic()))
            if line is None:raise RuntimeError('engine exited before READY')
            if line.startswith('INFO '):
                parts=line.split()[1:]
                if any('=' not in part for part in parts):raise RuntimeError('exact native INFO syntax differs')
                info=dict(part.split('=',1) for part in parts)
                if len(info)!=len(parts) or info.get('ctx')!='1048576' or info.get('batch_slots')!='2' or info.get('slot_cache')!='1':raise RuntimeError('actual native1M batch2 slots not established')
                RESULT['native_info']=info
            if line.startswith('READY '):
                if not RESULT.get('native_info'):raise RuntimeError('actual native batch2 INFO proof required before READY')
                RESULT['ready_line']=line;break
            if line.startswith('ERR '):raise RuntimeError(line)
        if PLAN['visible_devices']=='0,1':
            stderr.flush()
            startup=(ROOT/'engine.stderr.log').read_text()
            expected='ds41: 1-way layer split at 20 across GPUs 0 1'
            if expected not in startup.splitlines():raise RuntimeError('actual Xolotl stage devices differ from explicit pair placement')
        RESULT.update(phase='running',ready_unix=time.time());save()
        cases=PLAN['cases']
        case=next(c for c in cases if c['name']=='prose-128')
        runs=[dict(case,role='warmup',repeat=i) for i in range(3)]+[dict(case,role='profiled')]
        with (ROOT/'protocol.jsonl').open('x',buffering=1) as journal:
            for case in runs:
                if case['role']=='profiled':profile_tool('start');capture_active=True
                start=time.monotonic();tokens=[];lines=[]
                child.stdin.write('GEN '+str(case['max_new_tokens'])+' temperature=0 seed=1 '+','.join(map(str,case['ids']))+'\n');child.stdin.flush()
                deadline=start+(120 if case['role']=='profiled' else 180)
                while True:
                    ts,line=events.get(timeout=max(.01,deadline-time.monotonic()))
                    if line is None:raise RuntimeError('engine exited during request')
                    journal.write(json.dumps(dict(case=case['name'],role=case['role'],seconds=ts-start,line=line))+'\n')
                    lines.append(line)
                    if line.startswith('T '):
                        fields=line.split()
                        if len(fields)!=2:raise RuntimeError('unexpected token record')
                        tokens.append(dict(id=int(fields[1]),seconds=ts-start))
                    elif line.startswith('ERR '):raise RuntimeError(line)
                    elif line.startswith('DONE '):break
                    if len(lines)>4096 or len(tokens)>case['max_new_tokens']:raise RuntimeError('request output exceeds bound')
                if capture_active:profile_tool('stop');capture_active=False
                if len(tokens)!=128:raise RuntimeError('exact profiler/control128 token count differs')
                done=lines[-1].split()
                if int(done[1])!=len(tokens):raise RuntimeError('engine completion count disagrees with emitted tokens')
                elapsed=time.monotonic()-start
                span=tokens[-1]['seconds']-tokens[0]['seconds']
                rate=(len(tokens)-1)/span if len(tokens)>1 and span>0 else None
                if rate is not None and not math.isfinite(rate):raise RuntimeError('invalid decode rate')
                RESULT['requests'].append(dict(case=case['name'],role=case['role'],prompt_tokens=len(case['ids']),tokens=tokens,completion_tokens=len(tokens),ttft_seconds=tokens[0]['seconds'],request_seconds=elapsed,selected_id_decode_tok_s=rate,done=line,protocol_metadata=[s for s in lines if not s.startswith('T ')]))
                save()
        trace=ROOT/'warm-prose.nsys-rep'
        if not trace.is_file() or trace.stat().st_uid!=os.getuid() or not 0<trace.stat().st_size<=512*1024**2:raise RuntimeError('diagnostic trace missing, foreign or exceeds bound')
        import hashlib
        RESULT['trace']=dict(file=str(trace),bytes=trace.stat().st_size,sha256=hashlib.sha256(trace.read_bytes()).hexdigest(),diagnostic_only=True)
        child.stdin.write('QUIT\n');child.stdin.flush();child.wait(timeout=30)
        if child.returncode:raise RuntimeError('engine failed during orderly shutdown')
        RESULT.update(phase='complete',finished_unix=time.time(),engine_exit=child.returncode,
            all_output_id_sequences_equal=all([t['id'] for t in r['tokens']]==[t['id'] for t in RESULT['requests'][0]['tokens']] for r in RESULT['requests']),
            diagnostic_only=True,performance_300_claimed=False,
            scope='Current9a684/1M/S512 and actual API --batch2; one direct GEN request at a time. Three pre-trace controls then one trace. Not API throughput or actual1M-input validation.')
except Exception as e:
    RESULT.update(phase='failed',error=repr(e),finished_unix=time.time())
    raise
finally:
    if capture_active:
        try:profile_tool('stop')
        except Exception as e:RESULT['capture_stop_error']=repr(e)
    if child is not None:
        target=ROOT/'profile-target-process.json'
        if target.exists():
            rec=json.loads(target.read_text());proc=Path('/proc/'+str(rec['pid']))
            if proc.exists() and rec['uid']==os.getuid() and rec['binary']==PLAN['binary']:
                descriptor=os.pidfd_open(rec['pid'])
                try:
                    if proc.stat().st_uid!=rec['uid'] or int((proc/'stat').read_text().split(') ',1)[1].split()[19])!=rec['start_ticks']:raise RuntimeError('profile target PID identity changed')
                    signal.pidfd_send_signal(descriptor,signal.SIGKILL)
                except ProcessLookupError:pass
                finally:os.close(descriptor)
    if child is not None and child.poll() is None:
        if pidfd is not None:signal.pidfd_send_signal(pidfd,signal.SIGTERM)
        try:child.wait(timeout=15)
        except subprocess.TimeoutExpired:
            signal.pidfd_send_signal(pidfd,signal.SIGKILL);child.wait(timeout=15)
    if pidfd is not None:os.close(pidfd)
    stop.set()
    if 'sampler' in globals():sampler.join(timeout=10)
    RESULT['telemetry']=telemetry
    target=ROOT/'profile-target-process.json';native_gone=True
    if target.exists():
        rec=json.loads(target.read_text());p=Path('/proc/'+str(rec['pid'])+'/stat')
        native_gone=not (p.exists() and p.stat().st_uid==rec['uid'] and int(p.read_text().split(') ',1)[1].split()[19])==rec['start_ticks'] and p.read_text().split(') ',1)[1].split()[0]!='Z')
    RESULT['native_target_gone']=native_gone
    RESULT['engine_reaped']=(child is None or child.poll() is not None) and native_gone
    if not native_gone:RESULT.update(phase='failed',error='Exact native profiler target still alive after bounded cleanup')
    closed=subprocess.run([NSYS,'shutdown','--session='+SESSION,'--kill=none'],capture_output=True,text=True,timeout=20)
    RESULT['named_session_shutdown_exit']=closed.returncode
    RESULT['engine_exit']=None if child is None else child.returncode
    save()
